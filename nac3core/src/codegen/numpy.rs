use inkwell::{
    IntPredicate, 
    types::BasicType, 
    values::{BasicValueEnum, IntValue, PointerValue}
};
use nac3parser::ast::StrRef;
use crate::{
    codegen::{
        classes::{
            ArrayLikeIndexer,
            ArrayLikeValue,
            ListValue,
            NDArrayValue,
            TypedArrayLikeAccessor,
            TypedArrayLikeAdapter,
            UntypedArrayLikeAccessor,
        },
        CodeGenContext,
        CodeGenerator,
        irrt::{
            call_ndarray_calc_broadcast,
            call_ndarray_calc_broadcast_index,
            call_ndarray_calc_nd_indices,
            call_ndarray_calc_size,
        },
        llvm_intrinsics::call_memcpy_generic,
        stmt::gen_for_callback_incrementing,
    },
    symbol_resolver::ValueEnum,
    toplevel::{
        DefinitionId,
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
    },
    typecheck::typedef::{FunSignature, Type},
};

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
        DataFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>, &V, IntValue<'ctx>) -> Result<IntValue<'ctx>, String>,
{
    let ndarray_ty = make_ndarray_ty(&mut ctx.unifier, &ctx.primitives, Some(elem_ty), None);

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

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

            let shape_dim_gez = ctx.builder
                .build_int_compare(IntPredicate::SGE, shape_dim, shape_dim.get_type().const_zero(), "")
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
        llvm_usize.const_int(1, false),
    )?;

    let ndarray = generator.gen_var_alloc(
        ctx,
        llvm_ndarray_t.into(),
        None,
    )?;
    let ndarray = NDArrayValue::from_ptr_val(ndarray, llvm_usize, None);

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
            let shape_dim = ctx.builder
                .build_int_z_extend(shape_dim, llvm_usize, "")
                .unwrap();

            let ndarray_pdim = unsafe {
                ndarray.dim_sizes().ptr_offset_unchecked(ctx, generator, &i, None)
            };

            ctx.builder.build_store(ndarray_pdim, shape_dim).unwrap();

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )?;

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        &ndarray.dim_sizes().as_slice_value(ctx, generator),
    );
    ndarray.create_data(ctx, llvm_ndarray_data_t, ndarray_num_elems);

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
    let ndarray_ty = make_ndarray_ty(&mut ctx.unifier, &ctx.primitives, Some(elem_ty), None);

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

    for shape_dim in shape {
        let shape_dim_gez = ctx.builder
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
    }

    let ndarray = generator.gen_var_alloc(
        ctx,
        llvm_ndarray_t.into(),
        None,
    )?;
    let ndarray = NDArrayValue::from_ptr_val(ndarray, llvm_usize, None);

    let num_dims = llvm_usize.const_int(shape.len() as u64, false);
    ndarray.store_ndims(ctx, generator, num_dims);

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    ndarray.create_dim_sizes(ctx, llvm_usize, ndarray_num_dims);

    for (i, shape_dim) in shape.iter().enumerate() {
        let ndarray_dim = unsafe {
            ndarray
                .dim_sizes()
                .ptr_offset_unchecked(ctx, generator, &llvm_usize.const_int(i as u64, true), None)
        };

        ctx.builder.build_store(ndarray_dim, *shape_dim).unwrap();
    }

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        &ndarray.dim_sizes().as_slice_value(ctx, generator),
    );
    ndarray.create_data(ctx, llvm_ndarray_data_t, ndarray_num_elems);

    Ok(ndarray)
}

fn ndarray_zero_value<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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

fn ndarray_one_value<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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
        |_, ctx, shape| {
            Ok(shape.load_size(ctx, None))
        },
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
        ValueFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>, IntValue<'ctx>) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let ndarray_num_elems = call_ndarray_calc_size(
        generator,
        ctx,
        &ndarray.dim_sizes().as_slice_value(ctx, generator),
    );

    gen_for_callback_incrementing(
        generator,
        ctx,
        llvm_usize.const_zero(),
        (ndarray_num_elems, false),
        |generator, ctx, i| {
            let elem = unsafe {
                ndarray.data().ptr_offset_unchecked(ctx, generator, &i, None)
            };

            let value = value_fn(generator, ctx, i)?;
            ctx.builder.build_store(elem, value).unwrap();

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )
}

/// Generates LLVM IR for populating the entire `NDArray` using a lambda with the dimension-indices
/// as its input.
fn ndarray_fill_indexed<'ctx, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    value_fn: ValueFn,
) -> Result<(), String>
    where
        G: CodeGenerator + ?Sized,
        ValueFn: Fn(&mut G, &mut CodeGenContext<'ctx, '_>, &TypedArrayLikeAdapter<'ctx, IntValue<'ctx>>) -> Result<BasicValueEnum<'ctx>, String>,
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

            value_fn(generator, ctx, &indices)
        }
    )
}

fn ndarray_fill_mapping<'ctx, G, MapFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    src: NDArrayValue<'ctx>,
    dest: NDArrayValue<'ctx>,
    map_fn: MapFn,
) -> Result<(), String>
    where
        G: CodeGenerator + ?Sized,
        MapFn: Fn(&mut G, &mut CodeGenContext<'ctx, '_>, BasicValueEnum<'ctx>) -> Result<BasicValueEnum<'ctx>, String>,
{
    ndarray_fill_flattened(
        generator,
        ctx,
        dest,
        |generator, ctx, i| {
            let elem = unsafe {
                src.data().get_unchecked(ctx, generator, &i, None)
            };

            map_fn(generator, ctx, elem)
        },
    )
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
fn ndarray_broadcast_fill<'ctx, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    res: NDArrayValue<'ctx>,
    lhs: (BasicValueEnum<'ctx>, bool),
    rhs: (BasicValueEnum<'ctx>, bool),
    value_fn: ValueFn,
) -> Result<NDArrayValue<'ctx>, String>
    where
        G: CodeGenerator + ?Sized,
        ValueFn: Fn(&mut G, &mut CodeGenContext<'ctx, '_>, (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>)) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (lhs_val, lhs_scalar) = lhs;
    let (rhs_val, rhs_scalar) = rhs;

    assert!(!(lhs_scalar && rhs_scalar),
            "One of the operands must be a ndarray instance: `{}`, `{}`",
            lhs_val.get_type(),
            rhs_val.get_type());

    // Assert that all ndarray operands are broadcastable to the target size
    if !lhs_scalar {
        let lhs_val = NDArrayValue::from_ptr_val(lhs_val.into_pointer_value(), llvm_usize, None);
        ndarray_assert_is_broadcastable(generator, ctx, res, lhs_val);
    }

    if !rhs_scalar {
        let rhs_val = NDArrayValue::from_ptr_val(rhs_val.into_pointer_value(), llvm_usize, None);
        ndarray_assert_is_broadcastable(generator, ctx, res, rhs_val);
    }

    ndarray_fill_indexed(
        generator,
        ctx,
        res,
        |generator, ctx, idx| {
            let lhs_elem = if lhs_scalar {
                lhs_val
            } else {
                let lhs = NDArrayValue::from_ptr_val(lhs_val.into_pointer_value(), llvm_usize, None);
                let lhs_idx = call_ndarray_calc_broadcast_index(generator, ctx, lhs, idx);

                unsafe {
                    lhs.data().get_unchecked(ctx, generator, &lhs_idx, None)
                }
            };

            let rhs_elem = if rhs_scalar {
                rhs_val
            } else {
                let rhs = NDArrayValue::from_ptr_val(rhs_val.into_pointer_value(), llvm_usize, None);
                let rhs_idx = call_ndarray_calc_broadcast_index(generator, ctx, rhs, idx);
                
                unsafe {
                    rhs.data().get_unchecked(ctx, generator, &rhs_idx, None)
                }
            };

            debug_assert_eq!(lhs_elem.get_type(), rhs_elem.get_type());

            value_fn(generator, ctx, (lhs_elem, rhs_elem))
        },
    )?;

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
fn call_ndarray_full_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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

    let ndarray = create_ndarray_const_shape(
        generator,
        ctx,
        elem_ty,
        &[nrows, ncols],
    )?;

    ndarray_fill_indexed(
        generator,
        ctx,
        ndarray,
        |generator, ctx, indices| {
            let (row, col) = unsafe {
                (
                    indices.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None),
                    indices.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None),
                )
            };

            let col_with_offset = ctx.builder
                .build_int_add(
                    col,
                    ctx.builder.build_int_s_extend_or_bit_cast(offset, llvm_i32, "").unwrap(),
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

/// LLVM-typed implementation for generating the implementation for `ndarray.copy`.
///
/// * `elem_ty` - The element type of the `NDArray`.
fn ndarray_copy_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    this: NDArrayValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i1 = ctx.ctx.bool_type();

    let ndarray = create_ndarray_dyn_shape(
        generator,
        ctx,
        elem_ty,
        &this,
        |_, ctx, shape| {
            Ok(shape.load_ndims(ctx))
        },
        |generator, ctx, shape, idx| {
            unsafe { Ok(shape.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None)) }
        },
    )?;

    let len = call_ndarray_calc_size(
        generator,
        ctx,
        &ndarray.dim_sizes().as_slice_value(ctx, generator),
    );
    let sizeof_ty = ctx.get_llvm_type(generator, elem_ty);
    let len_bytes = ctx.builder
        .build_int_mul(
            len,
            sizeof_ty.size_of().unwrap(),
            "",
        )
        .unwrap();

    call_memcpy_generic(
        ctx,
        ndarray.data().base_ptr(ctx, generator),
        this.data().base_ptr(ctx, generator),
        len_bytes,
        llvm_i1.const_zero(),
    );

    Ok(ndarray)
}

pub fn ndarray_elementwise_unaryop_impl<'ctx, G, MapFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    res: Option<NDArrayValue<'ctx>>,
    operand: NDArrayValue<'ctx>,
    map_fn: MapFn,
) -> Result<NDArrayValue<'ctx>, String>
    where
        G: CodeGenerator,
        MapFn: Fn(&mut G, &mut CodeGenContext<'ctx, '_>, BasicValueEnum<'ctx>) -> Result<BasicValueEnum<'ctx>, String>,
{
    let res = res.unwrap_or_else(|| {
        create_ndarray_dyn_shape(
            generator,
            ctx,
            elem_ty,
            &operand,
            |_, ctx, v| {
                Ok(v.load_ndims(ctx))
            },
            |generator, ctx, v, idx| {
                unsafe {
                    Ok(v.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None))
                }
            },
        ).unwrap()
    });

    ndarray_fill_mapping(
        generator,
        ctx,
        operand,
        res,
        |generator, ctx, elem| {
            map_fn(generator, ctx, elem)
        }
    )?;

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
pub fn ndarray_elementwise_binop_impl<'ctx, G, ValueFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    res: Option<NDArrayValue<'ctx>>,
    lhs: (BasicValueEnum<'ctx>, bool),
    rhs: (BasicValueEnum<'ctx>, bool),
    value_fn: ValueFn,
) -> Result<NDArrayValue<'ctx>, String>
    where
        G: CodeGenerator,
        ValueFn: Fn(&mut G, &mut CodeGenContext<'ctx, '_>, (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>)) -> Result<BasicValueEnum<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (lhs_val, lhs_scalar) = lhs;
    let (rhs_val, rhs_scalar) = rhs;

    assert!(!(lhs_scalar && rhs_scalar),
            "One of the operands must be a ndarray instance: `{}`, `{}`",
            lhs_val.get_type(),
            rhs_val.get_type());

    let ndarray = res.unwrap_or_else(|| {
        if lhs_scalar && rhs_scalar {
            let lhs_val = NDArrayValue::from_ptr_val(lhs_val.into_pointer_value(), llvm_usize, None);
            let rhs_val = NDArrayValue::from_ptr_val(rhs_val.into_pointer_value(), llvm_usize, None);

            let ndarray_dims = call_ndarray_calc_broadcast(generator, ctx, lhs_val, rhs_val);

            create_ndarray_dyn_shape(
                generator,
                ctx,
                elem_ty,
                &ndarray_dims,
                |generator, ctx, v| {
                    Ok(v.size(ctx, generator))
                },
                |generator, ctx, v, idx| {
                    unsafe {
                        Ok(v.get_typed_unchecked(ctx, generator, &idx, None))
                    }
                },
            ).unwrap()
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
                |_, ctx, v| {
                    Ok(v.load_ndims(ctx))
                },
                |generator, ctx, v, idx| {
                    unsafe {
                        Ok(v.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None))
                    }
                },
            ).unwrap()
        }
    });

    ndarray_broadcast_fill(
        generator,
        ctx,
        ndarray,
        lhs,
        rhs,
        |generator, ctx, elems| {
            value_fn(generator, ctx, elems)
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
    let ncols_arg = if let Some(arg) = 
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[1].name)) {
        arg.1.clone().to_basic_value_enum(context, generator, ncols_ty)
    } else {
        args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)
    }?;

    let offset_ty = fun.0.args[2].ty;
    let offset_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[2].name)) {
        arg.1.clone().to_basic_value_enum(context, generator, offset_ty) 
    } else {
        Ok(context.gen_symbol_val(
            generator,
            fun.0.args[2].default_value.as_ref().unwrap(),
            offset_ty
        )) 
    }?;

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
    let this_arg = obj
        .as_ref()
        .unwrap()
        .1
        .clone()
        .to_basic_value_enum(context, generator, this_ty)?;

    ndarray_copy_impl(
        generator,
        context,
        this_elem_ty,
        NDArrayValue::from_ptr_val(this_arg.into_pointer_value(), llvm_usize, None),
    ).map(NDArrayValue::into)
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
    let this_arg = obj.as_ref().unwrap().1.clone()
        .to_basic_value_enum(context, generator, this_ty)?
        .into_pointer_value();
    let value_ty = fun.0.args[0].ty;
    let value_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, value_ty)?;

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
        }
    )?;

    Ok(())
}