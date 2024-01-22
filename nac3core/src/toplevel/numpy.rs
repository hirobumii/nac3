use inkwell::{AddressSpace, IntPredicate, types::BasicType, values::{BasicValueEnum, PointerValue}};
use inkwell::values::{ArrayValue, IntValue};
use nac3parser::ast::StrRef;
use crate::{
    codegen::{
        classes::{ListValue, NDArrayValue},
        CodeGenContext,
        CodeGenerator,
        irrt::{
            call_ndarray_calc_nd_indices,
            call_ndarray_calc_size,
            call_ndarray_init_dims,
        },
        stmt::gen_for_callback
    },
    symbol_resolver::ValueEnum,
    toplevel::DefinitionId,
    typecheck::typedef::{FunSignature, Type, TypeEnum},
};

/// Creates an `NDArray` instance from a constant shape.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The shape of the `NDArray`, represented as an LLVM [ArrayValue].
fn create_ndarray_const_shape<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    shape: ArrayValue<'ctx>
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray_ty_enum = TypeEnum::ndarray(&mut ctx.unifier, Some(elem_ty), None, &ctx.primitives);
    let ndarray_ty = ctx.unifier.add_ty(ndarray_ty_enum);

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

    for i in 0..shape.get_type().len() {
        let shape_dim = ctx.builder.build_extract_value(
            shape,
            i,
            "",
        ).unwrap();

        let shape_dim_gez = ctx.builder.build_int_compare(
            IntPredicate::SGE,
            shape_dim.into_int_value(),
            llvm_usize.const_zero(),
            ""
        );

        ctx.make_assert(
            generator,
            shape_dim_gez,
            "0:ValueError",
            "negative dimensions not supported",
            [None, None, None],
            ctx.current_loc,
        );
    }

    let ndarray = generator.gen_var_alloc(
        ctx,
        llvm_ndarray_t.into(),
        None,
    )?;
    let ndarray = NDArrayValue::from_ptr_val(ndarray, llvm_usize, None);

    let num_dims = llvm_usize.const_int(shape.get_type().len() as u64, false);
    ndarray.store_ndims(ctx, generator, num_dims);

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    ndarray.create_dims(ctx, llvm_usize, ndarray_num_dims);

    for i in 0..shape.get_type().len() {
        let ndarray_dim = ndarray
            .get_dims()
            .ptr_offset(ctx, generator, llvm_usize.const_int(i as u64, true), None);
        let shape_dim = ctx.builder.build_extract_value(shape, i, "")
            .map(|val| val.into_int_value())
            .unwrap();

        ctx.builder.build_store(ndarray_dim, shape_dim);
    }

    let ndarray_dims = ndarray.get_dims().get_ptr(ctx);
    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        ndarray.load_ndims(ctx),
        ndarray_dims,
    );
    ndarray.create_data(ctx, llvm_ndarray_data_t, ndarray_num_elems);

    Ok(ndarray)
}

fn ndarray_zero_value<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32].iter().any(|ty| ctx.unifier.unioned(elem_ty, *ty)) {
        ctx.ctx.i32_type().const_zero().into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64].iter().any(|ty| ctx.unifier.unioned(elem_ty, *ty)) {
        ctx.ctx.i64_type().const_zero().into()
    } else if ctx.unifier.unioned(elem_ty, ctx.primitives.float) {
        ctx.ctx.f64_type().const_zero().into()
    } else if ctx.unifier.unioned(elem_ty, ctx.primitives.bool) {
        ctx.ctx.bool_type().const_zero().into()
    } else if ctx.unifier.unioned(elem_ty, ctx.primitives.str) {
        ctx.gen_string(generator, "").into()
    } else {
        unreachable!()
    }
}

fn ndarray_one_value<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32].iter().any(|ty| ctx.unifier.unioned(elem_ty, *ty)) {
        let is_signed = ctx.unifier.unioned(elem_ty, ctx.primitives.int32);
        ctx.ctx.i32_type().const_int(1, is_signed).into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64].iter().any(|ty| ctx.unifier.unioned(elem_ty, *ty)) {
        let is_signed = ctx.unifier.unioned(elem_ty, ctx.primitives.int64);
        ctx.ctx.i64_type().const_int(1, is_signed).into()
    } else if ctx.unifier.unioned(elem_ty, ctx.primitives.float) {
        ctx.ctx.f64_type().const_float(1.0).into()
    } else if ctx.unifier.unioned(elem_ty, ctx.primitives.bool) {
        ctx.ctx.bool_type().const_int(1, false).into()
    } else if ctx.unifier.unioned(elem_ty, ctx.primitives.str) {
        ctx.gen_string(generator, "1").into()
    } else {
        unreachable!()
    }
}

/// LLVM-typed implementation for generating the implementation for constructing an `NDArray`.
///
/// * `elem_ty` - The element type of the NDArray.
/// * `shape` - The `shape` parameter used to construct the NDArray.
fn call_ndarray_empty_impl<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray_ty_enum = TypeEnum::ndarray(&mut ctx.unifier, Some(elem_ty), None, &ctx.primitives);
    let ndarray_ty = ctx.unifier.add_ty(ndarray_ty_enum);

    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

    // Assert that all dimensions are non-negative
    gen_for_callback(
        generator,
        ctx,
        |generator, ctx| {
            let i = generator.gen_var_alloc(ctx, llvm_usize.into(), None)?;
            ctx.builder.build_store(i, llvm_usize.const_zero());

            Ok(i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let shape_len = shape.load_size(ctx, None);

            Ok(ctx.builder.build_int_compare(IntPredicate::ULT, i, shape_len, ""))
        },
        |generator, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let shape_dim = shape.get_data().get(ctx, generator, i, None).into_int_value();

            let shape_dim_gez = ctx.builder.build_int_compare(
                IntPredicate::SGE,
                shape_dim,
                llvm_i32.const_zero(),
                ""
            );

            ctx.make_assert(
                generator,
                shape_dim_gez,
                "0:ValueError",
                "negative dimensions not supported",
                [None, None, None],
                ctx.current_loc,
            );

            Ok(())
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let i = ctx.builder.build_int_add(i, llvm_usize.const_int(1, true), "");
            ctx.builder.build_store(i_addr, i);

            Ok(())
        },
    )?;

    let ndarray = generator.gen_var_alloc(
        ctx,
        llvm_ndarray_t.into(),
        None,
    )?;
    let ndarray = NDArrayValue::from_ptr_val(ndarray, llvm_usize, None);

    let num_dims = shape.load_size(ctx, None);
    ndarray.store_ndims(ctx, generator, num_dims);

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    ndarray.create_dims(ctx, llvm_usize, ndarray_num_dims);

    call_ndarray_init_dims(generator, ctx, ndarray, shape);

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        ndarray.load_ndims(ctx),
        ndarray.get_dims().get_ptr(ctx),
    );
    ndarray.create_data(ctx, llvm_ndarray_data_t, ndarray_num_elems);

    Ok(ndarray)
}

/// Generates LLVM IR for populating the entire `NDArray` using a lambda with its flattened index as
/// its input.
///
/// Note that this differs from `ndarray.fill`, which instead replaces all first-dimension elements
/// with the given value (as opposed to all elements within the array).
fn ndarray_fill_flattened<'ctx, 'a, ValueFn>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    ndarray: NDArrayValue<'ctx>,
    value_fn: ValueFn,
) -> Result<(), String>
    where
        ValueFn: Fn(&mut dyn CodeGenerator, &mut CodeGenContext<'ctx, 'a>, IntValue<'ctx>) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        ndarray.load_ndims(ctx),
        ndarray.get_dims().get_ptr(ctx),
    );

    gen_for_callback(
        generator,
        ctx,
        |generator, ctx| {
            let i = generator.gen_var_alloc(ctx, llvm_usize.into(), None)?;
            ctx.builder.build_store(i, llvm_usize.const_zero());

            Ok(i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();

            Ok(ctx.builder.build_int_compare(IntPredicate::ULT, i, ndarray_num_elems, ""))
        },
        |generator, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let elem = unsafe {
                ndarray.get_data().ptr_to_data_flattened_unchecked(ctx, i, None)
            };

            let value = value_fn(generator, ctx, i)?;
            ctx.builder.build_store(elem, value);

            Ok(())
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let i = ctx.builder.build_int_add(i, llvm_usize.const_int(1, true), "");
            ctx.builder.build_store(i_addr, i);

            Ok(())
        },
    )
}

/// Generates LLVM IR for populating the entire `NDArray` using a lambda with the dimension-indices
/// as its input
///
/// Note that this differs from `ndarray.fill`, which instead replaces all first-dimension elements
/// with the given value (as opposed to all elements within the array).
fn ndarray_fill_indexed<'ctx, 'a, ValueFn>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    ndarray: NDArrayValue<'ctx>,
    value_fn: ValueFn,
) -> Result<(), String>
    where
        ValueFn: Fn(&mut dyn CodeGenerator, &mut CodeGenContext<'ctx, 'a>, PointerValue<'ctx>) -> Result<BasicValueEnum<'ctx>, String>,
{
    ndarray_fill_flattened(
        generator,
        ctx,
        ndarray,
        |generator, ctx, idx| {
            let indices = call_ndarray_calc_nd_indices(
                generator,
                ctx,
                idx,
                ndarray,
            )?;

            value_fn(generator, ctx, indices)
        }
    )
}

/// LLVM-typed implementation for generating the implementation for `ndarray.zeros`.
///
/// * `elem_ty` - The element type of the NDArray.
/// * `shape` - The `shape` parameter used to construct the NDArray.
fn call_ndarray_zeros_impl<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let supported_types = [
        ctx.primitives.int32,
        ctx.primitives.int64,
        ctx.primitives.uint32,
        ctx.primitives.uint64,
        ctx.primitives.float,
        ctx.primitives.bool,
        ctx.primitives.str,
    ];
    assert!(supported_types.iter().any(|supported_ty| ctx.unifier.unioned(*supported_ty, elem_ty)));

    let ndarray = call_ndarray_empty_impl(generator, ctx, elem_ty, shape)?;
    ndarray_fill_flattened(
        generator,
        ctx,
        ndarray,
        |generator, ctx, _| {
            let value = ndarray_zero_value(generator, ctx, elem_ty);

            Ok(value)
        }
    )?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.ones`.
///
/// * `elem_ty` - The element type of the NDArray.
/// * `shape` - The `shape` parameter used to construct the NDArray.
fn call_ndarray_ones_impl<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let supported_types = [
        ctx.primitives.int32,
        ctx.primitives.int64,
        ctx.primitives.uint32,
        ctx.primitives.uint64,
        ctx.primitives.float,
        ctx.primitives.bool,
        ctx.primitives.str,
    ];
    assert!(supported_types.iter().any(|supported_ty| ctx.unifier.unioned(*supported_ty, elem_ty)));

    let ndarray = call_ndarray_empty_impl(generator, ctx, elem_ty, shape)?;
    ndarray_fill_flattened(
        generator,
        ctx,
        ndarray,
        |generator, ctx, _| {
            let value = ndarray_one_value(generator, ctx, elem_ty);

            Ok(value)
        }
    )?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.ones`.
///
/// * `elem_ty` - The element type of the NDArray.
/// * `shape` - The `shape` parameter used to construct the NDArray.
fn call_ndarray_full_impl<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
    fill_value: BasicValueEnum<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray = call_ndarray_empty_impl(generator, ctx, elem_ty, shape)?;
    ndarray_fill_flattened(
        generator,
        ctx,
        ndarray,
        |generator, ctx, _| {
            let value = if fill_value.is_pointer_value() {
                let llvm_void = ctx.ctx.void_type();
                let llvm_i1 = ctx.ctx.bool_type();
                let llvm_i8 = ctx.ctx.i8_type();
                let llvm_usize = generator.get_size_type(ctx.ctx);
                let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());

                let copy = generator.gen_var_alloc(ctx, fill_value.get_type(), None)?;

                let memcpy_fn_name = format!(
                    "llvm.memcpy.p0i8.p0i8.i{}",
                    generator.get_size_type(ctx.ctx).get_bit_width(),
                );
                let memcpy_fn = ctx.module.get_function(memcpy_fn_name.as_str()).unwrap_or_else(|| {
                    let fn_type = llvm_void.fn_type(
                        &[
                            llvm_pi8.into(),
                            llvm_pi8.into(),
                            llvm_usize.into(),
                            llvm_i1.into(),
                        ],
                        false,
                    );

                    ctx.module.add_function(memcpy_fn_name.as_str(), fn_type, None)
                });

                ctx.builder.build_call(
                    memcpy_fn,
                    &[
                        copy.into(),
                        fill_value.into(),
                        fill_value.get_type().size_of().unwrap().into(),
                        llvm_i1.const_zero().into(),
                    ],
                    "",
                );

                copy.into()
            } else if fill_value.is_int_value() || fill_value.is_float_value() {
                fill_value.into()
            } else {
                unreachable!()
            };

            Ok(value)
        }
    )?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.eye`.
///
/// * `elem_ty` - The element type of the NDArray.
fn call_ndarray_eye_impl<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    nrows: IntValue<'ctx>,
    ncols: IntValue<'ctx>,
    offset: IntValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_usize_2 = llvm_usize.array_type(2);

    let shape_addr = generator.gen_var_alloc(ctx, llvm_usize_2.into(), None)?;

    let shape = ctx.builder.build_load(shape_addr, "")
        .into_array_value();

    let nrows = ctx.builder.build_int_z_extend_or_bit_cast(nrows, llvm_usize, "");
    let shape = ctx.builder
        .build_insert_value(shape, nrows, 0, "")
        .map(|val| val.into_array_value())
        .unwrap();

    let ncols = ctx.builder.build_int_z_extend_or_bit_cast(ncols, llvm_usize, "");
    let shape = ctx.builder
        .build_insert_value(shape, ncols, 1, "")
        .map(|val| val.into_array_value())
        .unwrap();

    let ndarray = create_ndarray_const_shape(generator, ctx, elem_ty, shape)?;

    ndarray_fill_indexed(
        generator,
        ctx,
        ndarray,
        |generator, ctx, indices| {
            let row = ctx.build_gep_and_load(
                indices,
                &[llvm_i32.const_zero()],
                None,
            ).into_int_value();
            let col = ctx.build_gep_and_load(
                indices,
                &[llvm_i32.const_int(1, true)],
                None,
            ).into_int_value();

            let col_with_offset = ctx.builder.build_int_add(
                col,
                ctx.builder.build_int_z_extend_or_bit_cast(offset, llvm_usize, ""),
                ""
            );
            let is_on_diag = ctx.builder.build_int_compare(
                IntPredicate::EQ,
                row,
                col_with_offset,
                ""
            );

            let zero = ndarray_zero_value(generator, ctx, elem_ty);
            let one = ndarray_one_value(generator, ctx, elem_ty);

            let value = ctx.builder.build_select(is_on_diag, one, zero, "");

            Ok(value)
        },
    )?;

    Ok(ndarray)
}

/// Generates LLVM IR for `ndarray.empty`.
pub fn gen_ndarray_empty<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let llvm_usize = generator.get_size_type(context.ctx);
    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_empty_impl(
        generator,
        context,
        context.primitives.float,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
    ).map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.zeros`.
pub fn gen_ndarray_zeros<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let llvm_usize = generator.get_size_type(context.ctx);
    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_zeros_impl(
        generator,
        context,
        context.primitives.float,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
    ).map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.ones`.
pub fn gen_ndarray_ones<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let llvm_usize = generator.get_size_type(context.ctx);
    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_ones_impl(
        generator,
        context,
        context.primitives.float,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
    ).map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.full`.
pub fn gen_ndarray_full<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 2);

    let llvm_usize = generator.get_size_type(context.ctx);
    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, shape_ty)?;
    let fill_value_ty = fun.0.args[1].ty;
    let fill_value_arg = args[1].1.clone()
        .to_basic_value_enum(context, generator, fill_value_ty)?;

    call_ndarray_full_impl(
        generator,
        context,
        fill_value_ty,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
        fill_value_arg,
    ).map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.eye`.
pub fn gen_ndarray_eye<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert!(matches!(args.len(), 1..=3));

    let nrows_ty = fun.0.args[0].ty;
    let nrows_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, nrows_ty)?;

    let ncols_ty = fun.0.args[1].ty;
    let ncols_arg = args.iter()
        .find(|arg| arg.0.map(|name| name == fun.0.args[1].name).unwrap_or(false))
        .map(|arg| arg.1.clone().to_basic_value_enum(context, generator, ncols_ty))
        .unwrap_or_else(|| {
            args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)
        })?;

    let offset_ty = fun.0.args[2].ty;
    let offset_arg = args.iter()
        .find(|arg| arg.0.map(|name| name == fun.0.args[2].name).unwrap_or(false))
        .map(|arg| arg.1.clone().to_basic_value_enum(context, generator, offset_ty))
        .unwrap_or_else(|| {
            Ok(context.gen_symbol_val(
                generator,
                fun.0.args[2].default_value.as_ref().unwrap(),
                offset_ty
            ))
        })?;

    call_ndarray_eye_impl(
        generator,
        context,
        context.primitives.float,
        nrows_arg.into_int_value(),
        ncols_arg.into_int_value(),
        offset_arg.into_int_value(),
    ).map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.identity`.
pub fn gen_ndarray_identity<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let llvm_usize = generator.get_size_type(context.ctx);

    let n_ty = fun.0.args[0].ty;
    let n_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, n_ty)?;

    call_ndarray_eye_impl(
        generator,
        context,
        context.primitives.float,
        n_arg.into_int_value(),
        n_arg.into_int_value(),
        llvm_usize.const_zero(),
    ).map(NDArrayValue::into)
}