use crate::{
    codegen::{
        classes::{
            ArrayLikeIndexer, ArrayLikeValue, ListType, ListValue, NDArrayType, NDArrayValue,
            ProxyType, ProxyValue, TypedArrayLikeAccessor, TypedArrayLikeAdapter,
            TypedArrayLikeMutator, UntypedArrayLikeAccessor, UntypedArrayLikeMutator,
        },
        expr::gen_binop_expr_with_values,
        irrt::{
            calculate_len_for_slice_range, call_ndarray_calc_broadcast,
            call_ndarray_calc_broadcast_index, call_ndarray_calc_nd_indices,
            call_ndarray_calc_size,
        },
        llvm_intrinsics,
        llvm_intrinsics::call_memcpy_generic,
        stmt::{gen_for_callback_incrementing, gen_for_range_callback, gen_if_else_expr_callback},
        CodeGenContext, CodeGenerator,
    },
    symbol_resolver::ValueEnum,
    toplevel::{
        helper::PRIMITIVE_DEF_IDS,
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
        DefinitionId,
    },
    typecheck::typedef::{FunSignature, Type, TypeEnum},
};
use inkwell::types::{AnyTypeEnum, BasicTypeEnum, PointerType};
use inkwell::{
    types::BasicType,
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use nac3parser::ast::{Operator, StrRef};

/// Creates an uninitialized `NDArray` instance.
fn create_ndarray_uninitialized<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray_ty = make_ndarray_ty(&mut ctx.unifier, &ctx.primitives, Some(elem_ty), None);

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_ndarray_t = ctx
        .get_llvm_type(generator, ndarray_ty)
        .into_pointer_type()
        .get_element_type()
        .into_struct_type();

    let ndarray = generator.gen_var_alloc(ctx, llvm_ndarray_t.into(), None)?;

    Ok(NDArrayValue::from_ptr_val(ndarray, llvm_usize, None))
}

/// Creates an `NDArray` instance from a dynamic shape.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The shape of the `NDArray`.
/// * `shape_len_fn` - A function that retrieves the number of dimensions from `shape`.
/// * `shape_data_fn` - A function that retrieves the size of a dimension from `shape`.
fn create_ndarray_dyn_shape<'ctx, 'a, G, V, LenFn, DataFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    shape: &V,
    shape_len_fn: LenFn,
    shape_data_fn: DataFn,
) -> Result<NDArrayValue<'ctx>, String>
where
    G: CodeGenerator + ?Sized,
    LenFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>, &V) -> Result<IntValue<'ctx>, String>,
    DataFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        &V,
        IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    // Assert that all dimensions are non-negative
    let shape_len = shape_len_fn(generator, ctx, shape)?;
    gen_for_callback_incrementing(
        generator,
        ctx,
        llvm_usize.const_zero(),
        (shape_len, false),
        |generator, ctx, i| {
            let shape_dim = shape_data_fn(generator, ctx, shape, i)?;
            debug_assert!(shape_dim.get_type().get_bit_width() <= llvm_usize.get_bit_width());

            let shape_dim_gez = ctx
                .builder
                .build_int_compare(
                    IntPredicate::SGE,
                    shape_dim,
                    shape_dim.get_type().const_zero(),
                    "",
                )
                .unwrap();

            ctx.make_assert(
                generator,
                shape_dim_gez,
                "0:ValueError",
                "negative dimensions not supported",
                [None, None, None],
                ctx.current_loc,
            );

            // TODO: Disallow dim_sz > u32_MAX

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )?;

    let ndarray = create_ndarray_uninitialized(generator, ctx, elem_ty)?;

    let num_dims = shape_len_fn(generator, ctx, shape)?;
    ndarray.store_ndims(ctx, generator, num_dims);

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    ndarray.create_dim_sizes(ctx, llvm_usize, ndarray_num_dims);

    // Copy the dimension sizes from shape to ndarray.dims
    let shape_len = shape_len_fn(generator, ctx, shape)?;
    gen_for_callback_incrementing(
        generator,
        ctx,
        llvm_usize.const_zero(),
        (shape_len, false),
        |generator, ctx, i| {
            let shape_dim = shape_data_fn(generator, ctx, shape, i)?;
            debug_assert!(shape_dim.get_type().get_bit_width() <= llvm_usize.get_bit_width());
            let shape_dim = ctx.builder.build_int_z_extend(shape_dim, llvm_usize, "").unwrap();

            let ndarray_pdim =
                unsafe { ndarray.dim_sizes().ptr_offset_unchecked(ctx, generator, &i, None) };

            ctx.builder.build_store(ndarray_pdim, shape_dim).unwrap();

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )?;

    let ndarray = ndarray_init_data(generator, ctx, elem_ty, ndarray);

    Ok(ndarray)
}

/// Creates an `NDArray` instance from a constant shape.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The shape of the `NDArray`, represented am array of [`IntValue`]s.
fn create_ndarray_const_shape<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    shape: &[IntValue<'ctx>],
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_usize = generator.get_size_type(ctx.ctx);

    for shape_dim in shape {
        let shape_dim_gez = ctx
            .builder
            .build_int_compare(IntPredicate::SGE, *shape_dim, llvm_usize.const_zero(), "")
            .unwrap();

        ctx.make_assert(
            generator,
            shape_dim_gez,
            "0:ValueError",
            "negative dimensions not supported",
            [None, None, None],
            ctx.current_loc,
        );

        // TODO: Disallow dim_sz > u32_MAX
    }

    let ndarray = create_ndarray_uninitialized(generator, ctx, elem_ty)?;

    let num_dims = llvm_usize.const_int(shape.len() as u64, false);
    ndarray.store_ndims(ctx, generator, num_dims);

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    ndarray.create_dim_sizes(ctx, llvm_usize, ndarray_num_dims);

    for (i, shape_dim) in shape.iter().enumerate() {
        let ndarray_dim = unsafe {
            ndarray.dim_sizes().ptr_offset_unchecked(
                ctx,
                generator,
                &llvm_usize.const_int(i as u64, true),
                None,
            )
        };

        ctx.builder.build_store(ndarray_dim, *shape_dim).unwrap();
    }

    let ndarray = ndarray_init_data(generator, ctx, elem_ty, ndarray);

    Ok(ndarray)
}

/// Initializes the `data` field of [`NDArrayValue`] based on the `ndims` and `dim_sz` fields.
fn ndarray_init_data<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    ndarray: NDArrayValue<'ctx>,
) -> NDArrayValue<'ctx> {
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        &ndarray.dim_sizes().as_slice_value(ctx, generator),
        (None, None),
    );
    ndarray.create_data(ctx, llvm_ndarray_data_t, ndarray_num_elems);

    ndarray
}

fn ndarray_zero_value<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32]
        .iter()
        .any(|ty| ctx.unifier.unioned(elem_ty, *ty))
    {
        ctx.ctx.i32_type().const_zero().into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(elem_ty, *ty))
    {
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

fn ndarray_one_value<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32]
        .iter()
        .any(|ty| ctx.unifier.unioned(elem_ty, *ty))
    {
        let is_signed = ctx.unifier.unioned(elem_ty, ctx.primitives.int32);
        ctx.ctx.i32_type().const_int(1, is_signed).into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(elem_ty, *ty))
    {
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
fn call_ndarray_empty_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    create_ndarray_dyn_shape(
        generator,
        ctx,
        elem_ty,
        &shape,
        |_, ctx, shape| Ok(shape.load_size(ctx, None)),
        |generator, ctx, shape, idx| {
            Ok(shape.data().get(ctx, generator, &idx, None).into_int_value())
        },
    )
}

/// Generates LLVM IR for populating the entire `NDArray` using a lambda with its flattened index as
/// its input.
fn ndarray_fill_flattened<'ctx, 'a, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    ndarray: NDArrayValue<'ctx>,
    value_fn: ValueFn,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    ValueFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        &ndarray.dim_sizes().as_slice_value(ctx, generator),
        (None, None),
    );

    gen_for_callback_incrementing(
        generator,
        ctx,
        llvm_usize.const_zero(),
        (ndarray_num_elems, false),
        |generator, ctx, i| {
            let elem = unsafe { ndarray.data().ptr_offset_unchecked(ctx, generator, &i, None) };

            let value = value_fn(generator, ctx, i)?;
            ctx.builder.build_store(elem, value).unwrap();

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )
}

/// Generates LLVM IR for populating the entire `NDArray` using a lambda with the dimension-indices
/// as its input.
fn ndarray_fill_indexed<'ctx, 'a, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    ndarray: NDArrayValue<'ctx>,
    value_fn: ValueFn,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    ValueFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        &TypedArrayLikeAdapter<'ctx, IntValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, String>,
{
    ndarray_fill_flattened(generator, ctx, ndarray, |generator, ctx, idx| {
        let indices = call_ndarray_calc_nd_indices(generator, ctx, idx, ndarray);

        value_fn(generator, ctx, &indices)
    })
}

fn ndarray_fill_mapping<'ctx, 'a, G, MapFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    src: NDArrayValue<'ctx>,
    dest: NDArrayValue<'ctx>,
    map_fn: MapFn,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    MapFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String>,
{
    ndarray_fill_flattened(generator, ctx, dest, |generator, ctx, i| {
        let elem = unsafe { src.data().get_unchecked(ctx, generator, &i, None) };

        map_fn(generator, ctx, elem)
    })
}

/// Generates the LLVM IR for checking whether the source `ndarray` can be broadcast to the shape of
/// the target `ndarray`.
fn ndarray_assert_is_broadcastable<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    target: NDArrayValue<'ctx>,
    source: NDArrayValue<'ctx>,
) {
    let array_ndims = source.load_ndims(ctx);
    let broadcast_size = target.load_ndims(ctx);

    ctx.make_assert(
        generator,
        ctx.builder.build_int_compare(IntPredicate::ULE, array_ndims, broadcast_size, "").unwrap(),
        "0:ValueError",
        "operands cannot be broadcast together",
        [None, None, None],
        ctx.current_loc,
    );
}

/// Generates the LLVM IR for populating the entire `NDArray` from two `ndarray` or scalar value
/// with broadcast-compatible shapes.
fn ndarray_broadcast_fill<'ctx, 'a, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    res: NDArrayValue<'ctx>,
    lhs: (BasicValueEnum<'ctx>, bool),
    rhs: (BasicValueEnum<'ctx>, bool),
    value_fn: ValueFn,
) -> Result<NDArrayValue<'ctx>, String>
where
    G: CodeGenerator + ?Sized,
    ValueFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>),
    ) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (lhs_val, lhs_scalar) = lhs;
    let (rhs_val, rhs_scalar) = rhs;

    assert!(
        !(lhs_scalar && rhs_scalar),
        "One of the operands must be a ndarray instance: `{}`, `{}`",
        lhs_val.get_type(),
        rhs_val.get_type()
    );

    // Assert that all ndarray operands are broadcastable to the target size
    if !lhs_scalar {
        let lhs_val = NDArrayValue::from_ptr_val(lhs_val.into_pointer_value(), llvm_usize, None);
        ndarray_assert_is_broadcastable(generator, ctx, res, lhs_val);
    }

    if !rhs_scalar {
        let rhs_val = NDArrayValue::from_ptr_val(rhs_val.into_pointer_value(), llvm_usize, None);
        ndarray_assert_is_broadcastable(generator, ctx, res, rhs_val);
    }

    ndarray_fill_indexed(generator, ctx, res, |generator, ctx, idx| {
        let lhs_elem = if lhs_scalar {
            lhs_val
        } else {
            let lhs = NDArrayValue::from_ptr_val(lhs_val.into_pointer_value(), llvm_usize, None);
            let lhs_idx = call_ndarray_calc_broadcast_index(generator, ctx, lhs, idx);

            unsafe { lhs.data().get_unchecked(ctx, generator, &lhs_idx, None) }
        };

        let rhs_elem = if rhs_scalar {
            rhs_val
        } else {
            let rhs = NDArrayValue::from_ptr_val(rhs_val.into_pointer_value(), llvm_usize, None);
            let rhs_idx = call_ndarray_calc_broadcast_index(generator, ctx, rhs, idx);

            unsafe { rhs.data().get_unchecked(ctx, generator, &rhs_idx, None) }
        };

        value_fn(generator, ctx, (lhs_elem, rhs_elem))
    })?;

    Ok(res)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.zeros`.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_zeros_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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
    ndarray_fill_flattened(generator, ctx, ndarray, |generator, ctx, _| {
        let value = ndarray_zero_value(generator, ctx, elem_ty);

        Ok(value)
    })?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.ones`.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_ones_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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
    ndarray_fill_flattened(generator, ctx, ndarray, |generator, ctx, _| {
        let value = ndarray_one_value(generator, ctx, elem_ty);

        Ok(value)
    })?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.full`.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The `shape` parameter used to construct the `NDArray`.
fn call_ndarray_full_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    shape: ListValue<'ctx>,
    fill_value: BasicValueEnum<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let ndarray = call_ndarray_empty_impl(generator, ctx, elem_ty, shape)?;
    ndarray_fill_flattened(generator, ctx, ndarray, |generator, ctx, _| {
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
    })?;

    Ok(ndarray)
}

/// Returns the number of dimensions for a multidimensional list as an [`IntValue`].
fn llvm_ndlist_get_ndims<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ty: PointerType<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let list_ty = ListType::from_type(ty, llvm_usize);
    let list_elem_ty = list_ty.element_type();

    let ndims = llvm_usize.const_int(1, false);
    match list_elem_ty {
        AnyTypeEnum::PointerType(ptr_ty) if ListType::is_type(ptr_ty, llvm_usize).is_ok() => {
            ndims.const_add(llvm_ndlist_get_ndims(generator, ctx, ptr_ty))
        }

        AnyTypeEnum::PointerType(ptr_ty) if NDArrayType::is_type(ptr_ty, llvm_usize).is_ok() => {
            todo!("Getting ndims for list[ndarray] not supported")
        }

        _ => ndims,
    }
}

/// Returns the number of dimensions for an array-like object as an [`IntValue`].
fn llvm_arraylike_get_ndims<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    value: BasicValueEnum<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = generator.get_size_type(ctx.ctx);

    match value {
        BasicValueEnum::PointerValue(v) if NDArrayValue::is_instance(v, llvm_usize).is_ok() => {
            NDArrayValue::from_ptr_val(v, llvm_usize, None).load_ndims(ctx)
        }

        BasicValueEnum::PointerValue(v) if ListValue::is_instance(v, llvm_usize).is_ok() => {
            llvm_ndlist_get_ndims(generator, ctx, v.get_type())
        }

        _ => llvm_usize.const_zero(),
    }
}

/// Flattens and copies the values from a multidimensional list into an [`NDArrayValue`].
fn ndarray_from_ndlist_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    (dst_arr, dst_slice_ptr): (NDArrayValue<'ctx>, PointerValue<'ctx>),
    src_lst: ListValue<'ctx>,
    dim: u64,
) -> Result<(), String> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let list_elem_ty = src_lst.get_type().element_type();

    match list_elem_ty {
        AnyTypeEnum::PointerType(ptr_ty) if ListType::is_type(ptr_ty, llvm_usize).is_ok() => {
            // The stride of elements in this dimension, i.e. the number of elements between arr[i]
            // and arr[i + 1] in this dimension
            let stride = call_ndarray_calc_size(
                generator,
                ctx,
                &dst_arr.dim_sizes(),
                (Some(llvm_usize.const_int(dim + 1, false)), None),
            );

            gen_for_range_callback(
                generator,
                ctx,
                true,
                |_, _| Ok(llvm_usize.const_zero()),
                (|_, ctx| Ok(src_lst.load_size(ctx, None)), false),
                |_, _| Ok(llvm_usize.const_int(1, false)),
                |generator, ctx, i| {
                    let offset = ctx.builder.build_int_mul(stride, i, "").unwrap();

                    let dst_ptr =
                        unsafe { ctx.builder.build_gep(dst_slice_ptr, &[offset], "").unwrap() };

                    let nested_lst_elem = ListValue::from_ptr_val(
                        unsafe { src_lst.data().get_unchecked(ctx, generator, &i, None) }
                            .into_pointer_value(),
                        llvm_usize,
                        None,
                    );

                    ndarray_from_ndlist_impl(
                        generator,
                        ctx,
                        elem_ty,
                        (dst_arr, dst_ptr),
                        nested_lst_elem,
                        dim + 1,
                    )?;

                    Ok(())
                },
            )?;
        }

        AnyTypeEnum::PointerType(ptr_ty) if NDArrayType::is_type(ptr_ty, llvm_usize).is_ok() => {
            todo!("Not implemented for list[ndarray]")
        }

        _ => {
            let lst_len = src_lst.load_size(ctx, None);
            let sizeof_elem = ctx.get_llvm_type(generator, elem_ty).size_of().unwrap();
            let cpy_len = ctx
                .builder
                .build_int_mul(
                    ctx.builder.build_int_z_extend_or_bit_cast(lst_len, llvm_usize, "").unwrap(),
                    sizeof_elem,
                    "",
                )
                .unwrap();

            call_memcpy_generic(
                ctx,
                dst_slice_ptr,
                src_lst.data().base_ptr(ctx, generator),
                cpy_len,
                llvm_i1.const_zero(),
            );
        }
    }

    Ok(())
}

/// LLVM-typed implementation for `ndarray.array`.
fn call_ndarray_array_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    object: BasicValueEnum<'ctx>,
    copy: IntValue<'ctx>,
    ndmin: IntValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let ndmin = ctx.builder.build_int_z_extend_or_bit_cast(ndmin, llvm_usize, "").unwrap();

    // TODO(Derppening): Add assertions for sizes of different dimensions

    // object is not a pointer - 0-dim NDArray
    if !object.is_pointer_value() {
        let ndarray = create_ndarray_const_shape(generator, ctx, elem_ty, &[])?;

        unsafe {
            ndarray.data().set_unchecked(ctx, generator, &llvm_usize.const_zero(), object);
        }

        return Ok(ndarray);
    }

    let object = object.into_pointer_value();

    // object is an NDArray instance - copy object unless copy=0 && ndmin < object.ndims
    if NDArrayValue::is_instance(object, llvm_usize).is_ok() {
        let object = NDArrayValue::from_ptr_val(object, llvm_usize, None);

        let ndarray = gen_if_else_expr_callback(
            generator,
            ctx,
            |_, ctx| {
                let copy_nez = ctx
                    .builder
                    .build_int_compare(IntPredicate::NE, copy, llvm_i1.const_zero(), "")
                    .unwrap();
                let ndmin_gt_ndims = ctx
                    .builder
                    .build_int_compare(IntPredicate::UGT, ndmin, object.load_ndims(ctx), "")
                    .unwrap();

                Ok(ctx.builder.build_and(copy_nez, ndmin_gt_ndims, "").unwrap())
            },
            |generator, ctx| {
                let ndarray = create_ndarray_dyn_shape(
                    generator,
                    ctx,
                    elem_ty,
                    &object,
                    |_, ctx, object| {
                        let ndims = object.load_ndims(ctx);
                        let ndmin_gt_ndims = ctx
                            .builder
                            .build_int_compare(IntPredicate::UGT, ndmin, object.load_ndims(ctx), "")
                            .unwrap();

                        Ok(ctx
                            .builder
                            .build_select(ndmin_gt_ndims, ndmin, ndims, "")
                            .map(BasicValueEnum::into_int_value)
                            .unwrap())
                    },
                    |generator, ctx, object, idx| {
                        let ndims = object.load_ndims(ctx);
                        let ndmin = llvm_intrinsics::call_int_umax(ctx, ndims, ndmin, None);
                        // The number of dimensions to prepend 1's to
                        let offset = ctx.builder.build_int_sub(ndmin, ndims, "").unwrap();

                        Ok(gen_if_else_expr_callback(
                            generator,
                            ctx,
                            |_, ctx| {
                                Ok(ctx
                                    .builder
                                    .build_int_compare(IntPredicate::UGE, idx, offset, "")
                                    .unwrap())
                            },
                            |_, _| Ok(Some(llvm_usize.const_int(1, false))),
                            |_, ctx| Ok(Some(ctx.builder.build_int_sub(idx, offset, "").unwrap())),
                        )?
                        .map(BasicValueEnum::into_int_value)
                        .unwrap())
                    },
                )?;

                ndarray_sliced_copyto_impl(
                    generator,
                    ctx,
                    elem_ty,
                    (ndarray, ndarray.data().base_ptr(ctx, generator)),
                    (object, object.data().base_ptr(ctx, generator)),
                    0,
                    &[],
                )?;

                Ok(Some(ndarray.as_base_value()))
            },
            |_, _| Ok(Some(object.as_base_value())),
        )?;

        return Ok(NDArrayValue::from_ptr_val(
            ndarray.map(BasicValueEnum::into_pointer_value).unwrap(),
            llvm_usize,
            None,
        ));
    }

    // Remaining case: TList
    assert!(ListValue::is_instance(object, llvm_usize).is_ok());
    let object = ListValue::from_ptr_val(object, llvm_usize, None);

    // The number of dimensions to prepend 1's to
    let ndims = llvm_ndlist_get_ndims(generator, ctx, object.as_base_value().get_type());
    let ndmin = llvm_intrinsics::call_int_umax(ctx, ndims, ndmin, None);
    let offset = ctx.builder.build_int_sub(ndmin, ndims, "").unwrap();

    let ndarray = create_ndarray_dyn_shape(
        generator,
        ctx,
        elem_ty,
        &object,
        |generator, ctx, object| {
            let ndims = llvm_ndlist_get_ndims(generator, ctx, object.as_base_value().get_type());
            let ndmin_gt_ndims =
                ctx.builder.build_int_compare(IntPredicate::UGT, ndmin, ndims, "").unwrap();

            Ok(ctx
                .builder
                .build_select(ndmin_gt_ndims, ndmin, ndims, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap())
        },
        |generator, ctx, object, idx| {
            Ok(gen_if_else_expr_callback(
                generator,
                ctx,
                |_, ctx| {
                    Ok(ctx.builder.build_int_compare(IntPredicate::ULT, idx, offset, "").unwrap())
                },
                |_, _| Ok(Some(llvm_usize.const_int(1, false))),
                |generator, ctx| {
                    let make_llvm_list = |elem_ty: BasicTypeEnum<'ctx>| {
                        ctx.ctx.struct_type(
                            &[elem_ty.ptr_type(AddressSpace::default()).into(), llvm_usize.into()],
                            false,
                        )
                    };

                    let llvm_i8 = ctx.ctx.i8_type();
                    let llvm_list_i8 = make_llvm_list(llvm_i8.into());
                    let llvm_plist_i8 = llvm_list_i8.ptr_type(AddressSpace::default());

                    // Cast list to { i8*, usize } since we only care about the size
                    let lst = generator
                        .gen_var_alloc(
                            ctx,
                            ListType::new(generator, ctx.ctx, llvm_i8.into()).as_base_type().into(),
                            None,
                        )
                        .unwrap();
                    ctx.builder
                        .build_store(
                            lst,
                            ctx.builder
                                .build_bitcast(object.as_base_value(), llvm_plist_i8, "")
                                .unwrap(),
                        )
                        .unwrap();

                    let stop = ctx.builder.build_int_sub(idx, offset, "").unwrap();
                    gen_for_range_callback(
                        generator,
                        ctx,
                        true,
                        |_, _| Ok(llvm_usize.const_zero()),
                        (|_, _| Ok(stop), false),
                        |_, _| Ok(llvm_usize.const_int(1, false)),
                        |generator, ctx, _| {
                            let plist_plist_i8 = make_llvm_list(llvm_plist_i8.into())
                                .ptr_type(AddressSpace::default());

                            let this_dim = ctx
                                .builder
                                .build_load(lst, "")
                                .map(BasicValueEnum::into_pointer_value)
                                .map(|v| ctx.builder.build_bitcast(v, plist_plist_i8, "").unwrap())
                                .map(BasicValueEnum::into_pointer_value)
                                .unwrap();
                            let this_dim = ListValue::from_ptr_val(this_dim, llvm_usize, None);

                            // TODO: Assert this_dim.sz != 0

                            let next_dim = unsafe {
                                this_dim.data().get_unchecked(
                                    ctx,
                                    generator,
                                    &llvm_usize.const_zero(),
                                    None,
                                )
                            }
                            .into_pointer_value();
                            ctx.builder
                                .build_store(
                                    lst,
                                    ctx.builder.build_bitcast(next_dim, llvm_plist_i8, "").unwrap(),
                                )
                                .unwrap();

                            Ok(())
                        },
                    )?;

                    let lst = ListValue::from_ptr_val(
                        ctx.builder
                            .build_load(lst, "")
                            .map(BasicValueEnum::into_pointer_value)
                            .unwrap(),
                        llvm_usize,
                        None,
                    );

                    Ok(Some(lst.load_size(ctx, None)))
                },
            )?
            .map(BasicValueEnum::into_int_value)
            .unwrap())
        },
    )?;

    ndarray_from_ndlist_impl(
        generator,
        ctx,
        elem_ty,
        (ndarray, ndarray.data().base_ptr(ctx, generator)),
        object,
        0,
    )?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.eye`.
///
/// * `elem_ty` - The element type of the `NDArray`.
fn call_ndarray_eye_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    nrows: IntValue<'ctx>,
    ncols: IntValue<'ctx>,
    offset: IntValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let nrows = ctx.builder.build_int_z_extend_or_bit_cast(nrows, llvm_usize, "").unwrap();
    let ncols = ctx.builder.build_int_z_extend_or_bit_cast(ncols, llvm_usize, "").unwrap();

    let ndarray = create_ndarray_const_shape(generator, ctx, elem_ty, &[nrows, ncols])?;

    ndarray_fill_indexed(generator, ctx, ndarray, |generator, ctx, indices| {
        let (row, col) = unsafe {
            (
                indices.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None),
                indices.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None),
            )
        };

        let col_with_offset = ctx
            .builder
            .build_int_add(
                col,
                ctx.builder.build_int_s_extend_or_bit_cast(offset, llvm_i32, "").unwrap(),
                "",
            )
            .unwrap();
        let is_on_diag =
            ctx.builder.build_int_compare(IntPredicate::EQ, row, col_with_offset, "").unwrap();

        let zero = ndarray_zero_value(generator, ctx, elem_ty);
        let one = ndarray_one_value(generator, ctx, elem_ty);

        let value = ctx.builder.build_select(is_on_diag, one, zero, "").unwrap();

        Ok(value)
    })?;

    Ok(ndarray)
}

/// Copies a slice of an [`NDArrayValue`] to another.
///
/// - `dst_arr`: The [`NDArrayValue`] instance of the destination array. The `ndims` and `dim_sz`
/// fields should be populated before calling this function.
/// - `dst_slice_ptr`: The [`PointerValue`] to the first element of the currently processing
/// dimensional slice in the destination array.
/// - `src_arr`: The [`NDArrayValue`] instance of the source array.
/// - `src_slice_ptr`: The [`PointerValue`] to the first element of the currently processing
/// dimensional slice in the source array.
/// - `dim`: The index of the currently processing dimension.
/// - `slices`: List of all slices, with the first element corresponding to the slice applicable to
/// this dimension. The `start`/`stop` values of each slice must be non-negative indices.
fn ndarray_sliced_copyto_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    (dst_arr, dst_slice_ptr): (NDArrayValue<'ctx>, PointerValue<'ctx>),
    (src_arr, src_slice_ptr): (NDArrayValue<'ctx>, PointerValue<'ctx>),
    dim: u64,
    slices: &[(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)],
) -> Result<(), String> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    // If there are no (remaining) slice expressions, memcpy the entire dimension
    if slices.is_empty() {
        let stride = call_ndarray_calc_size(
            generator,
            ctx,
            &src_arr.dim_sizes(),
            (Some(llvm_usize.const_int(dim, false)), None),
        );
        let sizeof_elem = ctx.get_llvm_type(generator, elem_ty).size_of().unwrap();
        let cpy_len = ctx.builder.build_int_mul(stride, sizeof_elem, "").unwrap();

        call_memcpy_generic(ctx, dst_slice_ptr, src_slice_ptr, cpy_len, llvm_i1.const_zero());

        return Ok(());
    }

    // The stride of elements in this dimension, i.e. the number of elements between arr[i] and
    // arr[i + 1] in this dimension
    let src_stride = call_ndarray_calc_size(
        generator,
        ctx,
        &src_arr.dim_sizes(),
        (Some(llvm_usize.const_int(dim + 1, false)), None),
    );
    let dst_stride = call_ndarray_calc_size(
        generator,
        ctx,
        &dst_arr.dim_sizes(),
        (Some(llvm_usize.const_int(dim + 1, false)), None),
    );

    let (start, stop, step) = slices[0];
    let start = ctx.builder.build_int_s_extend_or_bit_cast(start, llvm_usize, "").unwrap();
    let stop = ctx.builder.build_int_s_extend_or_bit_cast(stop, llvm_usize, "").unwrap();
    let step = ctx.builder.build_int_s_extend_or_bit_cast(step, llvm_usize, "").unwrap();

    let dst_i_addr = generator.gen_var_alloc(ctx, start.get_type().into(), None).unwrap();
    ctx.builder.build_store(dst_i_addr, start.get_type().const_zero()).unwrap();

    gen_for_range_callback(
        generator,
        ctx,
        false,
        |_, _| Ok(start),
        (|_, _| Ok(stop), true),
        |_, _| Ok(step),
        |generator, ctx, src_i| {
            // Calculate the offset of the active slice
            let src_data_offset = ctx.builder.build_int_mul(src_stride, src_i, "").unwrap();
            let dst_i =
                ctx.builder.build_load(dst_i_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let dst_data_offset = ctx.builder.build_int_mul(dst_stride, dst_i, "").unwrap();

            let (src_ptr, dst_ptr) = unsafe {
                (
                    ctx.builder.build_gep(src_slice_ptr, &[src_data_offset], "").unwrap(),
                    ctx.builder.build_gep(dst_slice_ptr, &[dst_data_offset], "").unwrap(),
                )
            };

            ndarray_sliced_copyto_impl(
                generator,
                ctx,
                elem_ty,
                (dst_arr, dst_ptr),
                (src_arr, src_ptr),
                dim + 1,
                &slices[1..],
            )?;

            let dst_i =
                ctx.builder.build_load(dst_i_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let dst_i_add1 =
                ctx.builder.build_int_add(dst_i, llvm_usize.const_int(1, false), "").unwrap();
            ctx.builder.build_store(dst_i_addr, dst_i_add1).unwrap();

            Ok(())
        },
    )?;

    Ok(())
}

/// Copies a [`NDArrayValue`] using slices.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// - `slices`: List of all slices, with the first element corresponding to the slice applicable to
/// this dimension. The `start`/`stop` values of each slice must be positive indices.
pub fn ndarray_sliced_copy<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    this: NDArrayValue<'ctx>,
    slices: &[(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)],
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let ndarray = if slices.is_empty() {
        create_ndarray_dyn_shape(
            generator,
            ctx,
            elem_ty,
            &this,
            |_, ctx, shape| Ok(shape.load_ndims(ctx)),
            |generator, ctx, shape, idx| unsafe {
                Ok(shape.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None))
            },
        )?
    } else {
        let ndarray = create_ndarray_uninitialized(generator, ctx, elem_ty)?;
        ndarray.store_ndims(ctx, generator, this.load_ndims(ctx));

        let ndims = this.load_ndims(ctx);
        ndarray.create_dim_sizes(ctx, llvm_usize, ndims);

        // Populate the first slices.len() dimensions by computing the size of each dim slice
        for (i, (start, stop, step)) in slices.iter().enumerate() {
            // HACK: workaround calculate_len_for_slice_range requiring exclusive stop
            let stop = ctx
                .builder
                .build_select(
                    ctx.builder
                        .build_int_compare(
                            IntPredicate::SLT,
                            *step,
                            llvm_i32.const_zero(),
                            "is_neg",
                        )
                        .unwrap(),
                    ctx.builder
                        .build_int_sub(*stop, llvm_i32.const_int(1, true), "e_min_one")
                        .unwrap(),
                    ctx.builder
                        .build_int_add(*stop, llvm_i32.const_int(1, true), "e_add_one")
                        .unwrap(),
                    "final_e",
                )
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            let slice_len = calculate_len_for_slice_range(generator, ctx, *start, stop, *step);
            let slice_len =
                ctx.builder.build_int_z_extend_or_bit_cast(slice_len, llvm_usize, "").unwrap();

            unsafe {
                ndarray.dim_sizes().set_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(i as u64, false),
                    slice_len,
                );
            }
        }

        // Populate the rest by directly copying the dim size from the source array
        gen_for_callback_incrementing(
            generator,
            ctx,
            llvm_usize.const_int(slices.len() as u64, false),
            (this.load_ndims(ctx), false),
            |generator, ctx, idx| {
                unsafe {
                    let dim_sz = this.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None);
                    ndarray.dim_sizes().set_typed_unchecked(ctx, generator, &idx, dim_sz);
                }

                Ok(())
            },
            llvm_usize.const_int(1, false),
        )
        .unwrap();

        ndarray_init_data(generator, ctx, elem_ty, ndarray)
    };

    ndarray_sliced_copyto_impl(
        generator,
        ctx,
        elem_ty,
        (ndarray, ndarray.data().base_ptr(ctx, generator)),
        (this, this.data().base_ptr(ctx, generator)),
        0,
        slices,
    )?;

    Ok(ndarray)
}

/// LLVM-typed implementation for generating the implementation for `ndarray.copy`.
///
/// * `elem_ty` - The element type of the `NDArray`.
fn ndarray_copy_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    this: NDArrayValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    ndarray_sliced_copy(generator, ctx, elem_ty, this, &[])
}

pub fn ndarray_elementwise_unaryop_impl<'ctx, 'a, G, MapFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    res: Option<NDArrayValue<'ctx>>,
    operand: NDArrayValue<'ctx>,
    map_fn: MapFn,
) -> Result<NDArrayValue<'ctx>, String>
where
    G: CodeGenerator + ?Sized,
    MapFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String>,
{
    let res = res.unwrap_or_else(|| {
        create_ndarray_dyn_shape(
            generator,
            ctx,
            elem_ty,
            &operand,
            |_, ctx, v| Ok(v.load_ndims(ctx)),
            |generator, ctx, v, idx| unsafe {
                Ok(v.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None))
            },
        )
        .unwrap()
    });

    ndarray_fill_mapping(generator, ctx, operand, res, |generator, ctx, elem| {
        map_fn(generator, ctx, elem)
    })?;

    Ok(res)
}

/// LLVM-typed implementation for computing elementwise binary operations on two input operands.
///
/// If the operand is a `ndarray`, the broadcast index corresponding to each element in the output
/// is computed, the element accessed and used as an operand of the `value_fn` arguments tuple.
/// Otherwise, the operand is treated as a scalar value, and is used as an operand of the
/// `value_fn` arguments tuple for all output elements.
///
/// The second element of the tuple indicates whether to treat the operand value as a `ndarray`
/// (which would be accessed by its broadcast index) or as a scalar value (which would be
/// broadcast to all elements).
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `res` - The `ndarray` instance to write results into, or [`None`] if the result should be
/// written to a new `ndarray`.
/// * `value_fn` - Function mapping the two input elements into the result.
///
/// # Panic
///
/// This function will panic if neither input operands (`lhs` or `rhs`) is a `ndarray`.
pub fn ndarray_elementwise_binop_impl<'ctx, 'a, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    res: Option<NDArrayValue<'ctx>>,
    lhs: (BasicValueEnum<'ctx>, bool),
    rhs: (BasicValueEnum<'ctx>, bool),
    value_fn: ValueFn,
) -> Result<NDArrayValue<'ctx>, String>
where
    G: CodeGenerator + ?Sized,
    ValueFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>),
    ) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (lhs_val, lhs_scalar) = lhs;
    let (rhs_val, rhs_scalar) = rhs;

    assert!(
        !(lhs_scalar && rhs_scalar),
        "One of the operands must be a ndarray instance: `{}`, `{}`",
        lhs_val.get_type(),
        rhs_val.get_type()
    );

    let ndarray = res.unwrap_or_else(|| {
        if lhs_scalar && rhs_scalar {
            let lhs_val =
                NDArrayValue::from_ptr_val(lhs_val.into_pointer_value(), llvm_usize, None);
            let rhs_val =
                NDArrayValue::from_ptr_val(rhs_val.into_pointer_value(), llvm_usize, None);

            let ndarray_dims = call_ndarray_calc_broadcast(generator, ctx, lhs_val, rhs_val);

            create_ndarray_dyn_shape(
                generator,
                ctx,
                elem_ty,
                &ndarray_dims,
                |generator, ctx, v| Ok(v.size(ctx, generator)),
                |generator, ctx, v, idx| unsafe {
                    Ok(v.get_typed_unchecked(ctx, generator, &idx, None))
                },
            )
            .unwrap()
        } else {
            let ndarray = NDArrayValue::from_ptr_val(
                if lhs_scalar { rhs_val } else { lhs_val }.into_pointer_value(),
                llvm_usize,
                None,
            );

            create_ndarray_dyn_shape(
                generator,
                ctx,
                elem_ty,
                &ndarray,
                |_, ctx, v| Ok(v.load_ndims(ctx)),
                |generator, ctx, v, idx| unsafe {
                    Ok(v.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None))
                },
            )
            .unwrap()
        }
    });

    ndarray_broadcast_fill(generator, ctx, ndarray, lhs, rhs, |generator, ctx, elems| {
        value_fn(generator, ctx, elems)
    })?;

    Ok(ndarray)
}

/// LLVM-typed implementation for computing matrix multiplication between two 2D `ndarray`s.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `res` - The `ndarray` instance to write results into, or [`None`] if the result should be
/// written to a new `ndarray`.
pub fn ndarray_matmul_2d<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    res: Option<NDArrayValue<'ctx>>,
    lhs: NDArrayValue<'ctx>,
    rhs: NDArrayValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    if cfg!(debug_assertions) {
        let lhs_ndims = lhs.load_ndims(ctx);
        let rhs_ndims = rhs.load_ndims(ctx);

        // lhs.ndims == 2
        ctx.make_assert(
            generator,
            ctx.builder
                .build_int_compare(IntPredicate::EQ, lhs_ndims, llvm_usize.const_int(2, false), "")
                .unwrap(),
            "0:ValueError",
            "",
            [None, None, None],
            ctx.current_loc,
        );

        // rhs.ndims == 2
        ctx.make_assert(
            generator,
            ctx.builder
                .build_int_compare(IntPredicate::EQ, rhs_ndims, llvm_usize.const_int(2, false), "")
                .unwrap(),
            "0:ValueError",
            "",
            [None, None, None],
            ctx.current_loc,
        );

        if let Some(res) = res {
            let res_ndims = res.load_ndims(ctx);
            let res_dim0 = unsafe {
                res.dim_sizes().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
            };
            let res_dim1 = unsafe {
                res.dim_sizes().get_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(1, false),
                    None,
                )
            };
            let lhs_dim0 = unsafe {
                lhs.dim_sizes().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
            };
            let rhs_dim1 = unsafe {
                rhs.dim_sizes().get_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(1, false),
                    None,
                )
            };

            // res.ndims == 2
            ctx.make_assert(
                generator,
                ctx.builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        res_ndims,
                        llvm_usize.const_int(2, false),
                        "",
                    )
                    .unwrap(),
                "0:ValueError",
                "",
                [None, None, None],
                ctx.current_loc,
            );

            // res.dims[0] == lhs.dims[0]
            ctx.make_assert(
                generator,
                ctx.builder.build_int_compare(IntPredicate::EQ, lhs_dim0, res_dim0, "").unwrap(),
                "0:ValueError",
                "",
                [None, None, None],
                ctx.current_loc,
            );

            // res.dims[1] == rhs.dims[0]
            ctx.make_assert(
                generator,
                ctx.builder.build_int_compare(IntPredicate::EQ, rhs_dim1, res_dim1, "").unwrap(),
                "0:ValueError",
                "",
                [None, None, None],
                ctx.current_loc,
            );
        }
    }

    if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
        let lhs_dim1 = unsafe {
            lhs.dim_sizes().get_typed_unchecked(
                ctx,
                generator,
                &llvm_usize.const_int(1, false),
                None,
            )
        };
        let rhs_dim0 = unsafe {
            rhs.dim_sizes().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
        };

        // lhs.dims[1] == rhs.dims[0]
        ctx.make_assert(
            generator,
            ctx.builder.build_int_compare(IntPredicate::EQ, lhs_dim1, rhs_dim0, "").unwrap(),
            "0:ValueError",
            "",
            [None, None, None],
            ctx.current_loc,
        );
    }

    let lhs = if res.is_some_and(|res| res.as_base_value() == lhs.as_base_value()) {
        ndarray_copy_impl(generator, ctx, elem_ty, lhs)?
    } else {
        lhs
    };

    let ndarray = res.unwrap_or_else(|| {
        create_ndarray_dyn_shape(
            generator,
            ctx,
            elem_ty,
            &(lhs, rhs),
            |_, _, _| Ok(llvm_usize.const_int(2, false)),
            |generator, ctx, (lhs, rhs), idx| {
                gen_if_else_expr_callback(
                    generator,
                    ctx,
                    |_, ctx| {
                        Ok(ctx
                            .builder
                            .build_int_compare(IntPredicate::EQ, idx, llvm_usize.const_zero(), "")
                            .unwrap())
                    },
                    |generator, ctx| {
                        Ok(Some(unsafe {
                            lhs.dim_sizes().get_typed_unchecked(
                                ctx,
                                generator,
                                &llvm_usize.const_zero(),
                                None,
                            )
                        }))
                    },
                    |generator, ctx| {
                        Ok(Some(unsafe {
                            rhs.dim_sizes().get_typed_unchecked(
                                ctx,
                                generator,
                                &llvm_usize.const_int(1, false),
                                None,
                            )
                        }))
                    },
                )
                .map(|v| v.map(BasicValueEnum::into_int_value).unwrap())
            },
        )
        .unwrap()
    });

    let llvm_ndarray_ty = ctx.get_llvm_type(generator, elem_ty);

    ndarray_fill_indexed(generator, ctx, ndarray, |generator, ctx, idx| {
        llvm_intrinsics::call_expect(
            ctx,
            idx.size(ctx, generator).get_type().const_int(2, false),
            idx.size(ctx, generator),
            None,
        );

        let common_dim = {
            let lhs_idx1 = unsafe {
                lhs.dim_sizes().get_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(1, false),
                    None,
                )
            };
            let rhs_idx0 = unsafe {
                rhs.dim_sizes().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
            };

            let idx = llvm_intrinsics::call_expect(ctx, rhs_idx0, lhs_idx1, None);

            ctx.builder.build_int_truncate(idx, llvm_i32, "").unwrap()
        };

        let idx0 = unsafe {
            let idx0 = idx.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None);

            ctx.builder.build_int_truncate(idx0, llvm_i32, "").unwrap()
        };
        let idx1 = unsafe {
            let idx1 =
                idx.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None);

            ctx.builder.build_int_truncate(idx1, llvm_i32, "").unwrap()
        };

        let result_addr = generator.gen_var_alloc(ctx, llvm_ndarray_ty, None)?;
        let result_identity = ndarray_zero_value(generator, ctx, elem_ty);
        ctx.builder.build_store(result_addr, result_identity).unwrap();

        gen_for_callback_incrementing(
            generator,
            ctx,
            llvm_i32.const_zero(),
            (common_dim, false),
            |generator, ctx, i| {
                let i = ctx.builder.build_int_truncate(i, llvm_i32, "").unwrap();

                let ab_idx = generator.gen_array_var_alloc(
                    ctx,
                    llvm_i32.into(),
                    llvm_usize.const_int(2, false),
                    None,
                )?;

                let a = unsafe {
                    ab_idx.set_unchecked(ctx, generator, &llvm_usize.const_zero(), idx0.into());
                    ab_idx.set_unchecked(ctx, generator, &llvm_usize.const_int(1, false), i.into());

                    lhs.data().get_unchecked(ctx, generator, &ab_idx, None)
                };
                let b = unsafe {
                    ab_idx.set_unchecked(ctx, generator, &llvm_usize.const_zero(), i.into());
                    ab_idx.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(1, false),
                        idx1.into(),
                    );

                    rhs.data().get_unchecked(ctx, generator, &ab_idx, None)
                };

                let a_mul_b = gen_binop_expr_with_values(
                    generator,
                    ctx,
                    (&Some(elem_ty), a),
                    &Operator::Mult,
                    (&Some(elem_ty), b),
                    ctx.current_loc,
                    false,
                )?
                .unwrap()
                .to_basic_value_enum(ctx, generator, elem_ty)?;

                let result = ctx.builder.build_load(result_addr, "").unwrap();
                let result = gen_binop_expr_with_values(
                    generator,
                    ctx,
                    (&Some(elem_ty), result),
                    &Operator::Add,
                    (&Some(elem_ty), a_mul_b),
                    ctx.current_loc,
                    false,
                )?
                .unwrap()
                .to_basic_value_enum(ctx, generator, elem_ty)?;
                ctx.builder.build_store(result_addr, result).unwrap();

                Ok(())
            },
            llvm_usize.const_int(1, false),
        )?;

        let result = ctx.builder.build_load(result_addr, "").unwrap();
        Ok(result)
    })?;

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
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_empty_impl(
        generator,
        context,
        context.primitives.float,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
    )
    .map(NDArrayValue::into)
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
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_zeros_impl(
        generator,
        context,
        context.primitives.float,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
    )
    .map(NDArrayValue::into)
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
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_ones_impl(
        generator,
        context,
        context.primitives.float,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
    )
    .map(NDArrayValue::into)
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
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;
    let fill_value_ty = fun.0.args[1].ty;
    let fill_value_arg =
        args[1].1.clone().to_basic_value_enum(context, generator, fill_value_ty)?;

    call_ndarray_full_impl(
        generator,
        context,
        fill_value_ty,
        ListValue::from_ptr_val(shape_arg.into_pointer_value(), llvm_usize, None),
        fill_value_arg,
    )
    .map(NDArrayValue::into)
}

pub fn gen_ndarray_array<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert!(matches!(args.len(), 1..=3));

    let obj_ty = fun.0.args[0].ty;
    let obj_elem_ty = match &*context.unifier.get_ty(obj_ty) {
        TypeEnum::TObj { obj_id, .. } if *obj_id == PRIMITIVE_DEF_IDS.ndarray => {
            unpack_ndarray_var_tys(&mut context.unifier, obj_ty).0
        }

        TypeEnum::TList { ty } => {
            let mut ty = *ty;
            while let TypeEnum::TList { ty: elem_ty } = &*context.unifier.get_ty_immutable(ty) {
                ty = *elem_ty;
            }
            ty
        }

        _ => obj_ty,
    };
    let obj_arg = args[0].1.clone().to_basic_value_enum(context, generator, obj_ty)?;

    let copy_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[1].name))
    {
        let copy_ty = fun.0.args[1].ty;
        arg.1.clone().to_basic_value_enum(context, generator, copy_ty)?
    } else {
        context.gen_symbol_val(
            generator,
            fun.0.args[1].default_value.as_ref().unwrap(),
            fun.0.args[1].ty,
        )
    };

    let ndmin_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[2].name))
    {
        let ndmin_ty = fun.0.args[2].ty;
        arg.1.clone().to_basic_value_enum(context, generator, ndmin_ty)?
    } else {
        context.gen_symbol_val(
            generator,
            fun.0.args[2].default_value.as_ref().unwrap(),
            fun.0.args[2].ty,
        )
    };

    call_ndarray_array_impl(
        generator,
        context,
        obj_elem_ty,
        obj_arg,
        copy_arg.into_int_value(),
        ndmin_arg.into_int_value(),
    )
    .map(NDArrayValue::into)
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
    let nrows_arg = args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)?;

    let ncols_ty = fun.0.args[1].ty;
    let ncols_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[1].name))
    {
        arg.1.clone().to_basic_value_enum(context, generator, ncols_ty)
    } else {
        args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)
    }?;

    let offset_ty = fun.0.args[2].ty;
    let offset_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[2].name))
    {
        arg.1.clone().to_basic_value_enum(context, generator, offset_ty)
    } else {
        Ok(context.gen_symbol_val(
            generator,
            fun.0.args[2].default_value.as_ref().unwrap(),
            offset_ty,
        ))
    }?;

    call_ndarray_eye_impl(
        generator,
        context,
        context.primitives.float,
        nrows_arg.into_int_value(),
        ncols_arg.into_int_value(),
        offset_arg.into_int_value(),
    )
    .map(NDArrayValue::into)
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
    let n_arg = args[0].1.clone().to_basic_value_enum(context, generator, n_ty)?;

    call_ndarray_eye_impl(
        generator,
        context,
        context.primitives.float,
        n_arg.into_int_value(),
        n_arg.into_int_value(),
        llvm_usize.const_zero(),
    )
    .map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.copy`.
pub fn gen_ndarray_copy<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    _fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_some());
    assert!(args.is_empty());

    let llvm_usize = generator.get_size_type(context.ctx);

    let this_ty = obj.as_ref().unwrap().0;
    let (this_elem_ty, _) = unpack_ndarray_var_tys(&mut context.unifier, this_ty);
    let this_arg =
        obj.as_ref().unwrap().1.clone().to_basic_value_enum(context, generator, this_ty)?;

    ndarray_copy_impl(
        generator,
        context,
        this_elem_ty,
        NDArrayValue::from_ptr_val(this_arg.into_pointer_value(), llvm_usize, None),
    )
    .map(NDArrayValue::into)
}

/// Generates LLVM IR for `ndarray.fill`.
pub fn gen_ndarray_fill<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<(), String> {
    assert!(obj.is_some());
    assert_eq!(args.len(), 1);

    let llvm_usize = generator.get_size_type(context.ctx);

    let this_ty = obj.as_ref().unwrap().0;
    let this_arg = obj
        .as_ref()
        .unwrap()
        .1
        .clone()
        .to_basic_value_enum(context, generator, this_ty)?
        .into_pointer_value();
    let value_ty = fun.0.args[0].ty;
    let value_arg = args[0].1.clone().to_basic_value_enum(context, generator, value_ty)?;

    ndarray_fill_flattened(
        generator,
        context,
        NDArrayValue::from_ptr_val(this_arg, llvm_usize, None),
        |generator, ctx, _| {
            let value = if value_arg.is_pointer_value() {
                let llvm_i1 = ctx.ctx.bool_type();

                let copy = generator.gen_var_alloc(ctx, value_arg.get_type(), None)?;

                call_memcpy_generic(
                    ctx,
                    copy,
                    value_arg.into_pointer_value(),
                    value_arg.get_type().size_of().map(Into::into).unwrap(),
                    llvm_i1.const_zero(),
                );

                copy.into()
            } else if value_arg.is_int_value() || value_arg.is_float_value() {
                value_arg
            } else {
                unreachable!()
            };

            Ok(value)
        },
    )?;

    Ok(())
}
