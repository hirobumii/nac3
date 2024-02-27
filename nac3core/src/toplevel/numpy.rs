use inkwell::{IntPredicate, types::BasicType, values::{BasicValueEnum, PointerValue}};
use inkwell::values::{AggregateValueEnum, ArrayValue, IntValue};
use itertools::Itertools;
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
        llvm_intrinsics::call_memcpy_generic,
        stmt::gen_for_callback
    },
    symbol_resolver::ValueEnum,
    toplevel::{DefinitionId, helper::PRIMITIVE_DEF_IDS},
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{FunSignature, Mapping, Type, TypeEnum, Unifier},
    },
};

/// Creates a `ndarray` [`Type`] with the given type arguments.
/// 
/// * `dtype` - The element type of the `ndarray`, or [`None`] if the type variable is not
/// specialized.
/// * `ndims` - The number of dimensions of the `ndarray`, or [`None`] if the type variable is not
/// specialized.
pub fn make_ndarray_ty(
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    dtype: Option<Type>,
    ndims: Option<Type>,
) -> Type {
    let ndarray = primitives.ndarray;

    let TypeEnum::TObj { obj_id, params, .. } = &*unifier.get_ty_immutable(ndarray) else {
        panic!("Expected `ndarray` to be TObj, but got {}", unifier.stringify(ndarray))
    };
    debug_assert_eq!(*obj_id, PRIMITIVE_DEF_IDS.ndarray);

    let tvar_ids = params.iter()
        .map(|(obj_id, _)| *obj_id)
        .sorted()
        .collect_vec();
    debug_assert_eq!(tvar_ids.len(), 2);

    let mut tvar_subst = Mapping::new();
    if let Some(dtype) = dtype {
        tvar_subst.insert(tvar_ids[0], dtype);
    }
    if let Some(ndims) = ndims {
        tvar_subst.insert(tvar_ids[1], ndims);
    }

    unifier.subst(ndarray, &tvar_subst).unwrap_or(ndarray)
}

/// Unpacks the type variables of `ndarray` into a tuple. The elements of the tuple corresponds to
/// `dtype` (the element type) and `ndims` (the number of dimensions) of the `ndarray` respectively.
pub fn unpack_ndarray_tvars(
    unifier: &mut Unifier,
    ndarray: Type,
) -> (Type, Type) {
    let TypeEnum::TObj { obj_id, params, .. } = &*unifier.get_ty_immutable(ndarray) else {
        panic!("Expected `ndarray` to be TObj, but got {}", unifier.stringify(ndarray))
    };
    debug_assert_eq!(*obj_id, PRIMITIVE_DEF_IDS.ndarray);
    debug_assert_eq!(params.len(), 2);

    params.iter()
        .sorted_by_key(|(obj_id, _)| *obj_id)
        .map(|(_, ty)| *ty)
        .collect_tuple()
        .unwrap()
}

/// Creates an `NDArray` instance from a constant shape.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The shape of the `NDArray`, represented as an LLVM [`ArrayValue`].
fn create_ndarray_const_shape<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    shape: ArrayValue<'ctx>
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray_ty = make_ndarray_ty(&mut ctx.unifier, &ctx.primitives, Some(elem_ty), None);

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

    for i in 0..shape.get_type().len() {
        let shape_dim = ctx.builder
            .build_extract_value(shape, i, "")
            .map(BasicValueEnum::into_int_value)
            .unwrap();

        let shape_dim_gez = ctx.builder
            .build_int_compare(IntPredicate::SGE, shape_dim, llvm_usize.const_zero(), "")
            .unwrap();

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
            .map(BasicValueEnum::into_int_value)
            .unwrap();

        ctx.builder.build_store(ndarray_dim, shape_dim).unwrap();
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

fn ndarray_zero_value<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
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
        ctx.gen_string(generator, "")
    } else {
        unreachable!()
    }
}

fn ndarray_one_value<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
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
        ctx.gen_string(generator, "1")
    } else {
        unreachable!()
    }
}

/// LLVM-typed implementation for generating the implementation for constructing an `NDArray`.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_empty_impl<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray_ty = make_ndarray_ty(
        &mut ctx.unifier,
        &ctx.primitives,
        Some(elem_ty),
        None,
    );

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
            ctx.builder.build_store(i, llvm_usize.const_zero()).unwrap();

            Ok(i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let shape_len = shape.load_size(ctx, None);

            Ok(ctx.builder.build_int_compare(IntPredicate::ULT, i, shape_len, "").unwrap())
        },
        |generator, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let shape_dim = shape.get_data().get(ctx, generator, i, None).into_int_value();

            let shape_dim_gez = ctx.builder
                .build_int_compare(IntPredicate::SGE, shape_dim, llvm_i32.const_zero(), "")
                .unwrap();

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
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let i = ctx.builder.build_int_add(i, llvm_usize.const_int(1, true), "").unwrap();
            ctx.builder.build_store(i_addr, i).unwrap();

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
            ctx.builder.build_store(i, llvm_usize.const_zero()).unwrap();

            Ok(i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            Ok(ctx.builder.build_int_compare(IntPredicate::ULT, i, ndarray_num_elems, "").unwrap())
        },
        |generator, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let elem = unsafe {
                ndarray.get_data().ptr_to_data_flattened_unchecked(ctx, i, None)
            };

            let value = value_fn(generator, ctx, i)?;
            ctx.builder.build_store(elem, value).unwrap();

            Ok(())
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let i = ctx.builder.build_int_add(i, llvm_usize.const_int(1, true), "").unwrap();
            ctx.builder.build_store(i_addr, i).unwrap();

            Ok(())
        },
    )
}

/// Generates LLVM IR for populating the entire `NDArray` using a lambda with the dimension-indices
/// as its input
///
/// Note that this differs from `ndarray.fill`, which instead replaces all first-dimension elements
/// with the given value (as opposed to all elements within the array).
fn ndarray_fill_indexed<'ctx, ValueFn>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    value_fn: ValueFn,
) -> Result<(), String>
    where
        ValueFn: Fn(&mut dyn CodeGenerator, &mut CodeGenContext<'ctx, '_>, PointerValue<'ctx>) -> Result<BasicValueEnum<'ctx>, String>,
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
            );

            value_fn(generator, ctx, indices)
        }
    )
}

/// LLVM-typed implementation for generating the implementation for `ndarray.zeros`.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_zeros_impl<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
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
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_ones_impl<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
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

/// LLVM-typed implementation for generating the implementation for `ndarray.full`.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_full_impl<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
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
                let llvm_i1 = ctx.ctx.bool_type();

                let copy = generator.gen_var_alloc(ctx, fill_value.get_type(), None)?;

                call_memcpy_generic(
                    ctx,
                    copy,
                    fill_value.into_pointer_value(),
                    fill_value.get_type().size_of().map(Into::into).unwrap(),
                    llvm_i1.const_zero(),
                );

                copy.into()
            } else if fill_value.is_int_value() || fill_value.is_float_value() {
                fill_value
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
/// * `elem_ty` - The element type of the `NDArray`.
fn call_ndarray_eye_impl<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
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
        .map(BasicValueEnum::into_array_value)
        .unwrap();

    let nrows = ctx.builder.build_int_z_extend_or_bit_cast(nrows, llvm_usize, "").unwrap();
    let shape = ctx.builder
        .build_insert_value(shape, nrows, 0, "")
        .map(AggregateValueEnum::into_array_value)
        .unwrap();

    let ncols = ctx.builder.build_int_z_extend_or_bit_cast(ncols, llvm_usize, "").unwrap();
    let shape = ctx.builder
        .build_insert_value(shape, ncols, 1, "")
        .map(AggregateValueEnum::into_array_value)
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

            let col_with_offset = ctx.builder
                .build_int_add(
                    col,
                    ctx.builder.build_int_z_extend_or_bit_cast(offset, llvm_usize, "").unwrap(),
                    "",
                )
                .unwrap();
            let is_on_diag = ctx.builder
                .build_int_compare(IntPredicate::EQ, row, col_with_offset, "")
                .unwrap();

            let zero = ndarray_zero_value(generator, ctx, elem_ty);
            let one = ndarray_one_value(generator, ctx, elem_ty);

            let value = ctx.builder.build_select(is_on_diag, one, zero, "").unwrap();

            Ok(value)
        },
    )?;

    Ok(ndarray)
}

/// Generates LLVM IR for `ndarray.empty`.
pub fn gen_ndarray_empty<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
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
pub fn gen_ndarray_zeros<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
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
pub fn gen_ndarray_ones<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
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
pub fn gen_ndarray_full<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
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
pub fn gen_ndarray_eye<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert!(matches!(args.len(), 1..=3));

    let nrows_ty = fun.0.args[0].ty;
    let nrows_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, nrows_ty)?;

    let ncols_ty = fun.0.args[1].ty;
    let ncols_arg = args.iter()
        .find(|arg| arg.0.is_some_and(|name| name == fun.0.args[1].name))
        .map(|arg| arg.1.clone().to_basic_value_enum(context, generator, ncols_ty))
        .unwrap_or_else(|| {
            args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)
        })?;

    let offset_ty = fun.0.args[2].ty;
    let offset_arg = args.iter()
        .find(|arg| arg.0.is_some_and(|name| name == fun.0.args[2].name))
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
pub fn gen_ndarray_identity<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
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