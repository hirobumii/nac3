use inkwell::{
    types::BasicType,
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
    IntPredicate, OptimizationLevel,
};

use nac3parser::ast::{Operator, StrRef};

use super::{
    expr::gen_binop_expr_with_values,
    irrt::{
        calculate_len_for_slice_range,
        ndarray::{
            call_ndarray_calc_broadcast, call_ndarray_calc_broadcast_index,
            call_ndarray_calc_nd_indices, call_ndarray_calc_size,
        },
    },
    llvm_intrinsics::{self, call_memcpy_generic},
    macros::codegen_unreachable,
    stmt::{gen_for_callback_incrementing, gen_for_range_callback, gen_if_else_expr_callback},
    types::ndarray::{factory::ndarray_zero_value, NDArrayType},
    values::{
        ndarray::{shape::parse_numpy_int_sequence, NDArrayValue},
        ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue, ListValue, ProxyValue,
        TypedArrayLikeAccessor, TypedArrayLikeAdapter, TypedArrayLikeMutator,
        UntypedArrayLikeAccessor, UntypedArrayLikeMutator,
    },
    CodeGenContext, CodeGenerator,
};
use crate::{
    symbol_resolver::ValueEnum,
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys, DefinitionId},
    typecheck::{
        magic_methods::Binop,
        typedef::{FunSignature, Type},
    },
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
    DataFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        &V,
        IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, String>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_elem_ty = ctx.get_llvm_type(generator, elem_ty);

    // Assert that all dimensions are non-negative
    let shape_len = shape_len_fn(generator, ctx, shape)?;
    gen_for_callback_incrementing(
        generator,
        ctx,
        None,
        llvm_usize.const_zero(),
        (shape_len, false),
        |generator, ctx, _, i| {
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

            // TODO: Disallow shape > u32_MAX

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )?;

    let num_dims = shape_len_fn(generator, ctx, shape)?;

    let ndarray = NDArrayType::new(generator, ctx.ctx, llvm_elem_ty, None)
        .construct_dyn_ndims(generator, ctx, num_dims, None);

    // Copy the dimension sizes from shape to ndarray.dims
    let shape_len = shape_len_fn(generator, ctx, shape)?;
    gen_for_callback_incrementing(
        generator,
        ctx,
        None,
        llvm_usize.const_zero(),
        (shape_len, false),
        |generator, ctx, _, i| {
            let shape_dim = shape_data_fn(generator, ctx, shape, i)?;
            debug_assert!(shape_dim.get_type().get_bit_width() <= llvm_usize.get_bit_width());
            let shape_dim = ctx.builder.build_int_z_extend(shape_dim, llvm_usize, "").unwrap();

            let ndarray_pdim =
                unsafe { ndarray.shape().ptr_offset_unchecked(ctx, generator, &i, None) };

            ctx.builder.build_store(ndarray_pdim, shape_dim).unwrap();

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )?;

    unsafe { ndarray.create_data(generator, ctx) };

    Ok(ndarray)
}

/// Creates an `NDArray` instance from a constant shape.
///
/// * `elem_ty` - The element type of the `NDArray`.
/// * `shape` - The shape of the `NDArray`, represented am array of [`IntValue`]s.
pub fn create_ndarray_const_shape<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    shape: &[IntValue<'ctx>],
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_usize = generator.get_size_type(ctx.ctx);

    for &shape_dim in shape {
        let shape_dim = ctx.builder.build_int_z_extend(shape_dim, llvm_usize, "").unwrap();
        let shape_dim_gez = ctx
            .builder
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

        // TODO: Disallow shape > u32_MAX
    }

    let llvm_dtype = ctx.get_llvm_type(generator, elem_ty);

    let ndarray = NDArrayType::new(generator, ctx.ctx, llvm_dtype, Some(shape.len() as u64))
        .construct_dyn_shape(generator, ctx, shape, None);
    unsafe { ndarray.create_data(generator, ctx) };

    Ok(ndarray)
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

    let ndarray_num_elems = ndarray.size(generator, ctx);

    gen_for_callback_incrementing(
        generator,
        ctx,
        None,
        llvm_usize.const_zero(),
        (ndarray_num_elems, false),
        |generator, ctx, _, i| {
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
        &TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>>,
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
    (lhs_ty, lhs_val, lhs_scalar): (Type, BasicValueEnum<'ctx>, bool),
    (rhs_ty, rhs_val, rhs_scalar): (Type, BasicValueEnum<'ctx>, bool),
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
    assert!(
        !(lhs_scalar && rhs_scalar),
        "One of the operands must be a ndarray instance: `{}`, `{}`",
        lhs_val.get_type(),
        rhs_val.get_type()
    );

    // Returns the element of an ndarray indexed by the given indices, performing int-promotion on
    // `indices` where necessary.
    //
    // Required for compatibility with `NDArrayType::get_unchecked`.
    let get_data_by_indices_compat =
        |generator: &mut G,
         ctx: &mut CodeGenContext<'ctx, '_>,
         ndarray: NDArrayValue<'ctx>,
         indices: TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>>| {
            let llvm_usize = generator.get_size_type(ctx.ctx);

            // Workaround: Promote lhs_idx to usize* to make the array compatible with new IRRT
            let stackptr = llvm_intrinsics::call_stacksave(ctx, None);
            let indices = if llvm_usize == ctx.ctx.i32_type() {
                indices
            } else {
                let indices_usize = TypedArrayLikeAdapter::<G, IntValue<'ctx>>::from(
                    ArraySliceValue::from_ptr_val(
                        ctx.builder
                            .build_array_alloca(llvm_usize, indices.size(ctx, generator), "")
                            .unwrap(),
                        indices.size(ctx, generator),
                        None,
                    ),
                    |_, _, val| val.into_int_value(),
                    |_, _, val| val.into(),
                );

                gen_for_callback_incrementing(
                    generator,
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (indices.size(ctx, generator), false),
                    |generator, ctx, _, i| {
                        let idx = unsafe { indices.get_typed_unchecked(ctx, generator, &i, None) };
                        let idx = ctx
                            .builder
                            .build_int_z_extend_or_bit_cast(idx, llvm_usize, "")
                            .unwrap();
                        unsafe {
                            indices_usize.set_typed_unchecked(ctx, generator, &i, idx);
                        }

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                )
                .unwrap();

                indices_usize
            };

            let elem = unsafe { ndarray.data().get_unchecked(ctx, generator, &indices, None) };

            llvm_intrinsics::call_stackrestore(ctx, stackptr);

            elem
        };

    // Assert that all ndarray operands are broadcastable to the target size
    if !lhs_scalar {
        let lhs_val = NDArrayType::from_unifier_type(generator, ctx, lhs_ty)
            .map_value(lhs_val.into_pointer_value(), None);
        ndarray_assert_is_broadcastable(generator, ctx, res, lhs_val);
    }

    if !rhs_scalar {
        let rhs_val = NDArrayType::from_unifier_type(generator, ctx, rhs_ty)
            .map_value(rhs_val.into_pointer_value(), None);
        ndarray_assert_is_broadcastable(generator, ctx, res, rhs_val);
    }

    ndarray_fill_indexed(generator, ctx, res, |generator, ctx, idx| {
        let lhs_elem = if lhs_scalar {
            lhs_val
        } else {
            let lhs = NDArrayType::from_unifier_type(generator, ctx, lhs_ty)
                .map_value(lhs_val.into_pointer_value(), None);
            let lhs_idx = call_ndarray_calc_broadcast_index(generator, ctx, lhs, idx);

            get_data_by_indices_compat(generator, ctx, lhs, lhs_idx)
        };

        let rhs_elem = if rhs_scalar {
            rhs_val
        } else {
            let rhs = NDArrayType::from_unifier_type(generator, ctx, rhs_ty)
                .map_value(rhs_val.into_pointer_value(), None);
            let rhs_idx = call_ndarray_calc_broadcast_index(generator, ctx, rhs, idx);

            get_data_by_indices_compat(generator, ctx, rhs, rhs_idx)
        };

        value_fn(generator, ctx, (lhs_elem, rhs_elem))
    })?;

    Ok(res)
}

/// Copies a slice of an [`NDArrayValue`] to another.
///
/// - `dst_arr`: The [`NDArrayValue`] instance of the destination array. The `ndims` and `shape`
///   fields should be populated before calling this function.
/// - `dst_slice_ptr`: The [`PointerValue`] to the first element of the currently processing
///   dimensional slice in the destination array.
/// - `src_arr`: The [`NDArrayValue`] instance of the source array.
/// - `src_slice_ptr`: The [`PointerValue`] to the first element of the currently processing
///   dimensional slice in the source array.
/// - `dim`: The index of the currently processing dimension.
/// - `slices`: List of all slices, with the first element corresponding to the slice applicable to
///   this dimension. The `start`/`stop` values of each slice must be non-negative indices.
fn ndarray_sliced_copyto_impl<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (dst_arr, dst_slice_ptr): (NDArrayValue<'ctx>, PointerValue<'ctx>),
    (src_arr, src_slice_ptr): (NDArrayValue<'ctx>, PointerValue<'ctx>),
    dim: u64,
    slices: &[(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)],
) -> Result<(), String> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    assert_eq!(dst_arr.get_type().element_type(), src_arr.get_type().element_type());

    let sizeof_elem = dst_arr.get_type().element_type().size_of().unwrap();

    // If there are no (remaining) slice expressions, memcpy the entire dimension
    if slices.is_empty() {
        let stride = call_ndarray_calc_size(
            generator,
            ctx,
            &src_arr.shape(),
            (Some(llvm_usize.const_int(dim, false)), None),
        );
        let stride =
            ctx.builder.build_int_z_extend_or_bit_cast(stride, sizeof_elem.get_type(), "").unwrap();

        let cpy_len = ctx.builder.build_int_mul(stride, sizeof_elem, "").unwrap();

        call_memcpy_generic(ctx, dst_slice_ptr, src_slice_ptr, cpy_len, llvm_i1.const_zero());

        return Ok(());
    }

    // The stride of elements in this dimension, i.e. the number of elements between arr[i] and
    // arr[i + 1] in this dimension
    let src_stride = call_ndarray_calc_size(
        generator,
        ctx,
        &src_arr.shape(),
        (Some(llvm_usize.const_int(dim + 1, false)), None),
    );
    let dst_stride = call_ndarray_calc_size(
        generator,
        ctx,
        &dst_arr.shape(),
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
        None,
        false,
        |_, _| Ok(start),
        (|_, _| Ok(stop), true),
        |_, _| Ok(step),
        |generator, ctx, _, src_i| {
            // Calculate the offset of the active slice
            let src_data_offset = ctx.builder.build_int_mul(src_stride, src_i, "").unwrap();
            let src_data_offset = ctx
                .builder
                .build_int_mul(
                    src_data_offset,
                    ctx.builder
                        .build_int_z_extend_or_bit_cast(sizeof_elem, src_data_offset.get_type(), "")
                        .unwrap(),
                    "",
                )
                .unwrap();
            let dst_i =
                ctx.builder.build_load(dst_i_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let dst_data_offset = ctx.builder.build_int_mul(dst_stride, dst_i, "").unwrap();
            let dst_data_offset = ctx
                .builder
                .build_int_mul(
                    dst_data_offset,
                    ctx.builder
                        .build_int_z_extend_or_bit_cast(sizeof_elem, dst_data_offset.get_type(), "")
                        .unwrap(),
                    "",
                )
                .unwrap();

            let (src_ptr, dst_ptr) = unsafe {
                (
                    ctx.builder.build_gep(src_slice_ptr, &[src_data_offset], "").unwrap(),
                    ctx.builder.build_gep(dst_slice_ptr, &[dst_data_offset], "").unwrap(),
                )
            };

            ndarray_sliced_copyto_impl(
                generator,
                ctx,
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
///   this dimension. The `start`/`stop` values of each slice must be positive indices.
pub fn ndarray_sliced_copy<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    this: NDArrayValue<'ctx>,
    slices: &[(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)],
) -> Result<NDArrayValue<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_elem_ty = ctx.get_llvm_type(generator, elem_ty);

    let ndarray =
        if slices.is_empty() {
            create_ndarray_dyn_shape(
                generator,
                ctx,
                elem_ty,
                &this,
                |_, ctx, shape| Ok(shape.load_ndims(ctx)),
                |generator, ctx, shape, idx| unsafe {
                    Ok(shape.shape().get_typed_unchecked(ctx, generator, &idx, None))
                },
            )?
        } else {
            let ndarray = NDArrayType::new(generator, ctx.ctx, llvm_elem_ty, None)
                .construct_dyn_ndims(generator, ctx, this.load_ndims(ctx), None);

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
                    ndarray.shape().set_typed_unchecked(
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
                None,
                llvm_usize.const_int(slices.len() as u64, false),
                (this.load_ndims(ctx), false),
                |generator, ctx, _, idx| {
                    unsafe {
                        let shape = this.shape().get_typed_unchecked(ctx, generator, &idx, None);
                        ndarray.shape().set_typed_unchecked(ctx, generator, &idx, shape);
                    }

                    Ok(())
                },
                llvm_usize.const_int(1, false),
            )
            .unwrap();

            unsafe { ndarray.create_data(generator, ctx) };

            ndarray
        };

    ndarray_sliced_copyto_impl(
        generator,
        ctx,
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
                Ok(v.shape().get_typed_unchecked(ctx, generator, &idx, None))
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
///   written to a new `ndarray`.
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
    lhs: (Type, BasicValueEnum<'ctx>, bool),
    rhs: (Type, BasicValueEnum<'ctx>, bool),
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
    let (lhs_ty, lhs_val, lhs_scalar) = lhs;
    let (rhs_ty, rhs_val, rhs_scalar) = rhs;

    assert!(
        !(lhs_scalar && rhs_scalar),
        "One of the operands must be a ndarray instance: `{}`, `{}`",
        lhs_val.get_type(),
        rhs_val.get_type()
    );

    let ndarray = res.unwrap_or_else(|| {
        if lhs_scalar && rhs_scalar {
            let lhs_val = NDArrayType::from_unifier_type(generator, ctx, lhs_ty)
                .map_value(lhs_val.into_pointer_value(), None);
            let rhs_val = NDArrayType::from_unifier_type(generator, ctx, rhs_ty)
                .map_value(rhs_val.into_pointer_value(), None);

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
            let ndarray = NDArrayType::from_unifier_type(
                generator,
                ctx,
                if lhs_scalar { rhs_ty } else { lhs_ty },
            )
            .map_value(if lhs_scalar { rhs_val } else { lhs_val }.into_pointer_value(), None);

            create_ndarray_dyn_shape(
                generator,
                ctx,
                elem_ty,
                &ndarray,
                |_, ctx, v| Ok(v.load_ndims(ctx)),
                |generator, ctx, v, idx| unsafe {
                    Ok(v.shape().get_typed_unchecked(ctx, generator, &idx, None))
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
///   written to a new `ndarray`.
pub fn ndarray_matmul_2d<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    elem_ty: Type,
    res: Option<NDArrayValue<'ctx>>,
    lhs: NDArrayValue<'ctx>,
    rhs: NDArrayValue<'ctx>,
) -> Result<NDArrayValue<'ctx>, String> {
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
                res.shape().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
            };
            let res_dim1 = unsafe {
                res.shape().get_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(1, false),
                    None,
                )
            };
            let lhs_dim0 = unsafe {
                lhs.shape().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
            };
            let rhs_dim1 = unsafe {
                rhs.shape().get_typed_unchecked(
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
            lhs.shape().get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None)
        };
        let rhs_dim0 = unsafe {
            rhs.shape().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
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
                            lhs.shape().get_typed_unchecked(
                                ctx,
                                generator,
                                &llvm_usize.const_zero(),
                                None,
                            )
                        }))
                    },
                    |generator, ctx| {
                        Ok(Some(unsafe {
                            rhs.shape().get_typed_unchecked(
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
                lhs.shape().get_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(1, false),
                    None,
                )
            };
            let rhs_idx0 = unsafe {
                rhs.shape().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
            };

            let idx = llvm_intrinsics::call_expect(ctx, rhs_idx0, lhs_idx1, None);

            ctx.builder.build_int_z_extend_or_bit_cast(idx, llvm_usize, "").unwrap()
        };

        let idx0 = unsafe {
            let idx0 = idx.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None);

            ctx.builder.build_int_z_extend_or_bit_cast(idx0, llvm_usize, "").unwrap()
        };
        let idx1 = unsafe {
            let idx1 =
                idx.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None);

            ctx.builder.build_int_z_extend_or_bit_cast(idx1, llvm_usize, "").unwrap()
        };

        let result_addr = generator.gen_var_alloc(ctx, llvm_ndarray_ty, None)?;
        let result_identity = ndarray_zero_value(generator, ctx, elem_ty);
        ctx.builder.build_store(result_addr, result_identity).unwrap();

        gen_for_callback_incrementing(
            generator,
            ctx,
            None,
            llvm_usize.const_zero(),
            (common_dim, false),
            |generator, ctx, _, i| {
                let ab_idx = generator.gen_array_var_alloc(
                    ctx,
                    llvm_usize.into(),
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
                    Binop::normal(Operator::Mult),
                    (&Some(elem_ty), b),
                    ctx.current_loc,
                )?
                .unwrap()
                .to_basic_value_enum(ctx, generator, elem_ty)?;

                let result = ctx.builder.build_load(result_addr, "").unwrap();
                let result = gen_binop_expr_with_values(
                    generator,
                    ctx,
                    (&Some(elem_ty), result),
                    Binop::normal(Operator::Add),
                    (&Some(elem_ty), a_mul_b),
                    ctx.current_loc,
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

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_empty(generator, context, &shape, None);
    Ok(ndarray.as_base_value())
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

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_zeros(generator, context, dtype, &shape, None);
    Ok(ndarray.as_base_value())
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

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_ones(generator, context, dtype, &shape, None);
    Ok(ndarray.as_base_value())
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

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;
    let fill_value_ty = fun.0.args[1].ty;
    let fill_value_arg =
        args[1].1.clone().to_basic_value_enum(context, generator, fill_value_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_full(generator, context, &shape, fill_value_arg, None);
    Ok(ndarray.as_base_value())
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

    // The ndmin argument is ignored. We can simply force the ndarray's number of dimensions to be
    // the `ndims` of the function return type.
    let (_, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let ndims = extract_ndims(&context.unifier, ndims);

    let copy = generator.bool_to_i1(context, copy_arg.into_int_value());
    let ndarray = NDArrayType::from_unifier_type(generator, context, fun.0.ret)
        .construct_numpy_array(generator, context, (obj_ty, obj_arg), copy, None)
        .atleast_nd(generator, context, ndims);

    Ok(ndarray.as_base_value())
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

    let (dtype, _) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);

    let llvm_usize = generator.get_size_type(context.ctx);
    let llvm_dtype = context.get_llvm_type(generator, dtype);

    let nrows = context
        .builder
        .build_int_s_extend_or_bit_cast(nrows_arg.into_int_value(), llvm_usize, "")
        .unwrap();
    let ncols = context
        .builder
        .build_int_s_extend_or_bit_cast(ncols_arg.into_int_value(), llvm_usize, "")
        .unwrap();
    let offset = context
        .builder
        .build_int_s_extend_or_bit_cast(offset_arg.into_int_value(), llvm_usize, "")
        .unwrap();

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(2))
        .construct_numpy_eye(generator, context, dtype, nrows, ncols, offset, None);
    Ok(ndarray.as_base_value())
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

    let n_ty = fun.0.args[0].ty;
    let n_arg = args[0].1.clone().to_basic_value_enum(context, generator, n_ty)?;

    let (dtype, _) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);

    let llvm_usize = generator.get_size_type(context.ctx);
    let llvm_dtype = context.get_llvm_type(generator, dtype);

    let n = context
        .builder
        .build_int_s_extend_or_bit_cast(n_arg.into_int_value(), llvm_usize, "")
        .unwrap();
    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(2))
        .construct_numpy_identity(generator, context, dtype, n, None);
    Ok(ndarray.as_base_value())
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

    let this_ty = obj.as_ref().unwrap().0;
    let (this_elem_ty, _) = unpack_ndarray_var_tys(&mut context.unifier, this_ty);
    let this_arg =
        obj.as_ref().unwrap().1.clone().to_basic_value_enum(context, generator, this_ty)?;

    let llvm_this_ty = NDArrayType::from_unifier_type(generator, context, this_ty);

    ndarray_copy_impl(
        generator,
        context,
        this_elem_ty,
        llvm_this_ty.map_value(this_arg.into_pointer_value(), None),
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

    let llvm_this_ty = NDArrayType::from_unifier_type(generator, context, this_ty);

    ndarray_fill_flattened(
        generator,
        context,
        llvm_this_ty.map_value(this_arg, None),
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
                codegen_unreachable!(ctx)
            };

            Ok(value)
        },
    )?;

    Ok(())
}

/// Generates LLVM IR for `ndarray.transpose`.
pub fn ndarray_transpose<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "ndarray_transpose";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    if let BasicValueEnum::PointerValue(n1) = x1 {
        let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, x1_ty);
        let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
        let n1 = llvm_ndarray_ty.map_value(n1, None);
        let n_sz = n1.size(generator, ctx);

        // Dimensions are reversed in the transposed array
        let out = create_ndarray_dyn_shape(
            generator,
            ctx,
            elem_ty,
            &n1,
            |_, ctx, n| Ok(n.load_ndims(ctx)),
            |generator, ctx, n, idx| {
                let new_idx = ctx.builder.build_int_sub(n.load_ndims(ctx), idx, "").unwrap();
                let new_idx = ctx
                    .builder
                    .build_int_sub(new_idx, new_idx.get_type().const_int(1, false), "")
                    .unwrap();
                unsafe { Ok(n.shape().get_typed_unchecked(ctx, generator, &new_idx, None)) }
            },
        )
        .unwrap();

        gen_for_callback_incrementing(
            generator,
            ctx,
            None,
            llvm_usize.const_zero(),
            (n_sz, false),
            |generator, ctx, _, idx| {
                let elem = unsafe { n1.data().get_unchecked(ctx, generator, &idx, None) };

                let new_idx = generator.gen_var_alloc(ctx, llvm_usize.into(), None)?;
                let rem_idx = generator.gen_var_alloc(ctx, llvm_usize.into(), None)?;
                ctx.builder.build_store(new_idx, llvm_usize.const_zero()).unwrap();
                ctx.builder.build_store(rem_idx, idx).unwrap();

                // Incrementally calculate the new index in the transposed array
                // For each index, we first decompose it into the n-dims and use those to reconstruct the new index
                // The formula used for indexing is:
                // idx = dim_n * ( ... (dim2 * (dim0 * dim1) + dim1) + dim2 ... ) + dim_n
                gen_for_callback_incrementing(
                    generator,
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (n1.load_ndims(ctx), false),
                    |generator, ctx, _, ndim| {
                        let ndim_rev =
                            ctx.builder.build_int_sub(n1.load_ndims(ctx), ndim, "").unwrap();
                        let ndim_rev = ctx
                            .builder
                            .build_int_sub(ndim_rev, llvm_usize.const_int(1, false), "")
                            .unwrap();
                        let dim = unsafe {
                            n1.shape().get_typed_unchecked(ctx, generator, &ndim_rev, None)
                        };

                        let rem_idx_val =
                            ctx.builder.build_load(rem_idx, "").unwrap().into_int_value();
                        let new_idx_val =
                            ctx.builder.build_load(new_idx, "").unwrap().into_int_value();

                        let add_component =
                            ctx.builder.build_int_unsigned_rem(rem_idx_val, dim, "").unwrap();
                        let rem_idx_val =
                            ctx.builder.build_int_unsigned_div(rem_idx_val, dim, "").unwrap();

                        let new_idx_val = ctx.builder.build_int_mul(new_idx_val, dim, "").unwrap();
                        let new_idx_val =
                            ctx.builder.build_int_add(new_idx_val, add_component, "").unwrap();

                        ctx.builder.build_store(rem_idx, rem_idx_val).unwrap();
                        ctx.builder.build_store(new_idx, new_idx_val).unwrap();

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                )?;

                let new_idx_val = ctx.builder.build_load(new_idx, "").unwrap().into_int_value();
                unsafe { out.data().set_unchecked(ctx, generator, &new_idx_val, elem) };
                Ok(())
            },
            llvm_usize.const_int(1, false),
        )?;

        Ok(out.as_base_value().into())
    } else {
        codegen_unreachable!(
            ctx,
            "{FN_NAME}() not supported for '{}'",
            format!("'{}'", ctx.unifier.stringify(x1_ty))
        )
    }
}

/// LLVM-typed implementation for generating the implementation for `ndarray.reshape`.
///
/// * `x1` - `NDArray` to reshape.
/// * `shape` - The `shape` parameter used to construct the new `NDArray`.
///   Just like numpy, the `shape` argument can be:
///   1. A list of `int32`;   e.g., `np.reshape(arr, [600, -1, 3])`
///   2. A tuple of `int32`;  e.g., `np.reshape(arr, (-1, 800, 3))`
///   3. A scalar `int32`;     e.g., `np.reshape(arr, 3)`
///
/// Note that unlike other generating functions, one of the dimensions in the shape can be negative.
pub fn ndarray_reshape<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    shape: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "ndarray_reshape";
    let (x1_ty, x1) = x1;
    let (_, shape) = shape;

    let llvm_usize = generator.get_size_type(ctx.ctx);

    if let BasicValueEnum::PointerValue(n1) = x1 {
        let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
        let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, x1_ty);
        let n1 = llvm_ndarray_ty.map_value(n1, None);
        let n_sz = n1.size(generator, ctx);

        let acc = generator.gen_var_alloc(ctx, llvm_usize.into(), None)?;
        let num_neg = generator.gen_var_alloc(ctx, llvm_usize.into(), None)?;
        ctx.builder.build_store(acc, llvm_usize.const_int(1, false)).unwrap();
        ctx.builder.build_store(num_neg, llvm_usize.const_zero()).unwrap();

        let out = match shape {
            BasicValueEnum::PointerValue(shape_list_ptr)
                if ListValue::is_representable(shape_list_ptr, llvm_usize).is_ok() =>
            {
                // 1. A list of ints; e.g., `np.reshape(arr, [int64(600), int64(800, -1])`

                let shape_list = ListValue::from_pointer_value(shape_list_ptr, llvm_usize, None);
                // Check for -1 in dimensions
                gen_for_callback_incrementing(
                    generator,
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (shape_list.load_size(ctx, None), false),
                    |generator, ctx, _, idx| {
                        let ele =
                            shape_list.data().get(ctx, generator, &idx, None).into_int_value();
                        let ele = ctx.builder.build_int_s_extend(ele, llvm_usize, "").unwrap();

                        gen_if_else_expr_callback(
                            generator,
                            ctx,
                            |_, ctx| {
                                Ok(ctx
                                    .builder
                                    .build_int_compare(
                                        IntPredicate::SLT,
                                        ele,
                                        llvm_usize.const_zero(),
                                        "",
                                    )
                                    .unwrap())
                            },
                            |_, ctx| -> Result<Option<IntValue>, String> {
                                let num_neg_value =
                                    ctx.builder.build_load(num_neg, "").unwrap().into_int_value();
                                let num_neg_value = ctx
                                    .builder
                                    .build_int_add(
                                        num_neg_value,
                                        llvm_usize.const_int(1, false),
                                        "",
                                    )
                                    .unwrap();
                                ctx.builder.build_store(num_neg, num_neg_value).unwrap();
                                Ok(None)
                            },
                            |_, ctx| {
                                let acc_value =
                                    ctx.builder.build_load(acc, "").unwrap().into_int_value();
                                let acc_value =
                                    ctx.builder.build_int_mul(acc_value, ele, "").unwrap();
                                ctx.builder.build_store(acc, acc_value).unwrap();
                                Ok(None)
                            },
                        )?;
                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                )?;
                let acc_val = ctx.builder.build_load(acc, "").unwrap().into_int_value();
                let rem = ctx.builder.build_int_unsigned_div(n_sz, acc_val, "").unwrap();
                // Generate the output shape by filling -1 with `rem`
                create_ndarray_dyn_shape(
                    generator,
                    ctx,
                    elem_ty,
                    &shape_list,
                    |_, ctx, _| Ok(shape_list.load_size(ctx, None)),
                    |generator, ctx, shape_list, idx| {
                        let dim =
                            shape_list.data().get(ctx, generator, &idx, None).into_int_value();
                        let dim = ctx.builder.build_int_s_extend(dim, llvm_usize, "").unwrap();

                        Ok(gen_if_else_expr_callback(
                            generator,
                            ctx,
                            |_, ctx| {
                                Ok(ctx
                                    .builder
                                    .build_int_compare(
                                        IntPredicate::SLT,
                                        dim,
                                        llvm_usize.const_zero(),
                                        "",
                                    )
                                    .unwrap())
                            },
                            |_, _| Ok(Some(rem)),
                            |_, _| Ok(Some(dim)),
                        )?
                        .unwrap()
                        .into_int_value())
                    },
                )
            }
            BasicValueEnum::StructValue(shape_tuple) => {
                // 2. A tuple of `int32`;  e.g., `np.reshape(arr, (-1, 800, 3))`

                let ndims = shape_tuple.get_type().count_fields();
                // Check for -1 in dims
                for dim_i in 0..ndims {
                    let dim = ctx
                        .builder
                        .build_extract_value(shape_tuple, dim_i, "")
                        .unwrap()
                        .into_int_value();
                    let dim = ctx.builder.build_int_s_extend(dim, llvm_usize, "").unwrap();

                    gen_if_else_expr_callback(
                        generator,
                        ctx,
                        |_, ctx| {
                            Ok(ctx
                                .builder
                                .build_int_compare(
                                    IntPredicate::SLT,
                                    dim,
                                    llvm_usize.const_zero(),
                                    "",
                                )
                                .unwrap())
                        },
                        |_, ctx| -> Result<Option<IntValue>, String> {
                            let num_negs =
                                ctx.builder.build_load(num_neg, "").unwrap().into_int_value();
                            let num_negs = ctx
                                .builder
                                .build_int_add(num_negs, llvm_usize.const_int(1, false), "")
                                .unwrap();
                            ctx.builder.build_store(num_neg, num_negs).unwrap();
                            Ok(None)
                        },
                        |_, ctx| {
                            let acc_val = ctx.builder.build_load(acc, "").unwrap().into_int_value();
                            let acc_val = ctx.builder.build_int_mul(acc_val, dim, "").unwrap();
                            ctx.builder.build_store(acc, acc_val).unwrap();
                            Ok(None)
                        },
                    )?;
                }

                let acc_val = ctx.builder.build_load(acc, "").unwrap().into_int_value();
                let rem = ctx.builder.build_int_unsigned_div(n_sz, acc_val, "").unwrap();
                let mut shape = Vec::with_capacity(ndims as usize);

                // Reconstruct shape filling negatives with rem
                for dim_i in 0..ndims {
                    let dim = ctx
                        .builder
                        .build_extract_value(shape_tuple, dim_i, "")
                        .unwrap()
                        .into_int_value();
                    let dim = ctx.builder.build_int_s_extend(dim, llvm_usize, "").unwrap();

                    let dim = gen_if_else_expr_callback(
                        generator,
                        ctx,
                        |_, ctx| {
                            Ok(ctx
                                .builder
                                .build_int_compare(
                                    IntPredicate::SLT,
                                    dim,
                                    llvm_usize.const_zero(),
                                    "",
                                )
                                .unwrap())
                        },
                        |_, _| Ok(Some(rem)),
                        |_, _| Ok(Some(dim)),
                    )?
                    .unwrap()
                    .into_int_value();
                    shape.push(dim);
                }
                create_ndarray_const_shape(generator, ctx, elem_ty, shape.as_slice())
            }
            BasicValueEnum::IntValue(shape_int) => {
                // 3. A scalar `int32`;     e.g., `np.reshape(arr, 3)`
                let shape_int = gen_if_else_expr_callback(
                    generator,
                    ctx,
                    |_, ctx| {
                        Ok(ctx
                            .builder
                            .build_int_compare(
                                IntPredicate::SLT,
                                shape_int,
                                llvm_usize.const_zero(),
                                "",
                            )
                            .unwrap())
                    },
                    |_, _| Ok(Some(n_sz)),
                    |_, ctx| {
                        Ok(Some(ctx.builder.build_int_s_extend(shape_int, llvm_usize, "").unwrap()))
                    },
                )?
                .unwrap()
                .into_int_value();
                create_ndarray_const_shape(generator, ctx, elem_ty, &[shape_int])
            }
            _ => codegen_unreachable!(ctx),
        }
        .unwrap();

        // Only allow one dimension to be negative
        let num_negs = ctx.builder.build_load(num_neg, "").unwrap().into_int_value();
        ctx.make_assert(
            generator,
            ctx.builder
                .build_int_compare(IntPredicate::ULT, num_negs, llvm_usize.const_int(2, false), "")
                .unwrap(),
            "0:ValueError",
            "can only specify one unknown dimension",
            [None, None, None],
            ctx.current_loc,
        );

        // The new shape must be compatible with the old shape
        let out_sz = out.size(generator, ctx);
        ctx.make_assert(
            generator,
            ctx.builder.build_int_compare(IntPredicate::EQ, out_sz, n_sz, "").unwrap(),
            "0:ValueError",
            "cannot reshape array of size {0} into provided shape of size {1}",
            [Some(n_sz), Some(out_sz), None],
            ctx.current_loc,
        );

        gen_for_callback_incrementing(
            generator,
            ctx,
            None,
            llvm_usize.const_zero(),
            (n_sz, false),
            |generator, ctx, _, idx| {
                let elem = unsafe { n1.data().get_unchecked(ctx, generator, &idx, None) };
                unsafe { out.data().set_unchecked(ctx, generator, &idx, elem) };
                Ok(())
            },
            llvm_usize.const_int(1, false),
        )?;

        Ok(out.as_base_value().into())
    } else {
        codegen_unreachable!(
            ctx,
            "{FN_NAME}() not supported for '{}'",
            format!("'{}'", ctx.unifier.stringify(x1_ty))
        )
    }
}

/// Generates LLVM IR for `ndarray.dot`.
/// Calculate inner product of two vectors or literals
/// For matrix multiplication use `np_matmul`
///
/// The input `NDArray` are flattened and treated as 1D
/// The operation is equivalent to `np.dot(arr1.ravel(), arr2.ravel())`
pub fn ndarray_dot<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "ndarray_dot";
    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    let llvm_usize = generator.get_size_type(ctx.ctx);

    match (x1, x2) {
        (BasicValueEnum::PointerValue(n1), BasicValueEnum::PointerValue(n2)) => {
            let n1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(n1, None);
            let n2 = NDArrayType::from_unifier_type(generator, ctx, x2_ty).map_value(n2, None);

            let n1_sz = n1.size(generator, ctx);
            let n2_sz = n2.size(generator, ctx);

            ctx.make_assert(
                generator,
                ctx.builder.build_int_compare(IntPredicate::EQ, n1_sz, n2_sz, "").unwrap(),
                "0:ValueError",
                "shapes ({0}), ({1}) not aligned",
                [Some(n1_sz), Some(n2_sz), None],
                ctx.current_loc,
            );

            let identity =
                unsafe { n1.data().get_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
            let acc = ctx.builder.build_alloca(identity.get_type(), "").unwrap();
            ctx.builder.build_store(acc, identity.get_type().const_zero()).unwrap();

            gen_for_callback_incrementing(
                generator,
                ctx,
                None,
                llvm_usize.const_zero(),
                (n1_sz, false),
                |generator, ctx, _, idx| {
                    let elem1 = unsafe { n1.data().get_unchecked(ctx, generator, &idx, None) };
                    let elem2 = unsafe { n2.data().get_unchecked(ctx, generator, &idx, None) };

                    let product = match elem1 {
                        BasicValueEnum::IntValue(e1) => ctx
                            .builder
                            .build_int_mul(e1, elem2.into_int_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        BasicValueEnum::FloatValue(e1) => ctx
                            .builder
                            .build_float_mul(e1, elem2.into_float_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        _ => codegen_unreachable!(ctx, "product: {}", elem1.get_type()),
                    };
                    let acc_val = ctx.builder.build_load(acc, "").unwrap();
                    let acc_val = match acc_val {
                        BasicValueEnum::IntValue(e1) => ctx
                            .builder
                            .build_int_add(e1, product.into_int_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        BasicValueEnum::FloatValue(e1) => ctx
                            .builder
                            .build_float_add(e1, product.into_float_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        _ => codegen_unreachable!(ctx, "acc_val: {}", acc_val.get_type()),
                    };
                    ctx.builder.build_store(acc, acc_val).unwrap();

                    Ok(())
                },
                llvm_usize.const_int(1, false),
            )?;
            let acc_val = ctx.builder.build_load(acc, "").unwrap();
            Ok(acc_val)
        }
        (BasicValueEnum::IntValue(e1), BasicValueEnum::IntValue(e2)) => {
            Ok(ctx.builder.build_int_mul(e1, e2, "").unwrap().as_basic_value_enum())
        }
        (BasicValueEnum::FloatValue(e1), BasicValueEnum::FloatValue(e2)) => {
            Ok(ctx.builder.build_float_mul(e1, e2, "").unwrap().as_basic_value_enum())
        }
        _ => codegen_unreachable!(
            ctx,
            "{FN_NAME}() not supported for '{}'",
            format!("'{}'", ctx.unifier.stringify(x1_ty))
        ),
    }
}
