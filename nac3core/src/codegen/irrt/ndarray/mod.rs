use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, CallSiteValue, IntValue},
    AddressSpace, IntPredicate,
};
use itertools::Either;

use super::get_usize_dependent_function_name;
use crate::codegen::{
    llvm_intrinsics,
    macros::codegen_unreachable,
    stmt::gen_for_callback_incrementing,
    values::{
        ndarray::NDArrayValue, ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue,
        TypedArrayLikeAccessor, TypedArrayLikeAdapter, UntypedArrayLikeAccessor,
    },
    CodeGenContext, CodeGenerator,
};
pub use array::*;
pub use basic::*;
pub use broadcast::*;
pub use indexing::*;
pub use iter::*;
pub use reshape::*;

mod array;
mod basic;
mod broadcast;
mod indexing;
mod iter;
mod reshape;

/// Generates a call to `__nac3_ndarray_calc_size`. Returns a
/// [`usize`][CodeGenerator::get_size_type] representing the calculated total size.
///
/// * `dims` - An [`ArrayLikeIndexer`] containing the size of each dimension.
/// * `range` - The dimension index to begin and end (exclusively) calculating the dimensions for,
///   or [`None`] if starting from the first dimension and ending at the last dimension
///   respectively.
pub fn call_ndarray_calc_size<'ctx, G, Dims>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    dims: &Dims,
    (begin, end): (Option<IntValue<'ctx>>, Option<IntValue<'ctx>>),
) -> IntValue<'ctx>
where
    G: CodeGenerator + ?Sized,
    Dims: ArrayLikeIndexer<'ctx>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    assert!(begin.is_none_or(|begin| begin.get_type() == llvm_usize));
    assert!(end.is_none_or(|end| end.get_type() == llvm_usize));
    assert_eq!(
        BasicTypeEnum::try_from(dims.element_type(ctx, generator)).unwrap(),
        llvm_usize.into()
    );

    let ndarray_calc_size_fn_name =
        get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_calc_size");
    let ndarray_calc_size_fn_t = llvm_usize.fn_type(
        &[llvm_pusize.into(), llvm_usize.into(), llvm_usize.into(), llvm_usize.into()],
        false,
    );
    let ndarray_calc_size_fn =
        ctx.module.get_function(&ndarray_calc_size_fn_name).unwrap_or_else(|| {
            ctx.module.add_function(&ndarray_calc_size_fn_name, ndarray_calc_size_fn_t, None)
        });

    let begin = begin.unwrap_or_else(|| llvm_usize.const_zero());
    let end = end.unwrap_or_else(|| dims.size(ctx, generator));
    ctx.builder
        .build_call(
            ndarray_calc_size_fn,
            &[
                dims.base_ptr(ctx, generator).into(),
                dims.size(ctx, generator).into(),
                begin.into(),
                end.into(),
            ],
            "",
        )
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `__nac3_ndarray_calc_nd_indices`. Returns a [`TypedArrayLikeAdapter`]
/// containing `i32` indices of the flattened index.
///
/// * `index` - The `llvm_usize` index to compute the multidimensional index for.
/// * `ndarray` - LLVM pointer to the `NDArray`. This value must be the LLVM representation of an
///   `NDArray`.
pub fn call_ndarray_calc_nd_indices<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    index: IntValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
) -> TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>> {
    let llvm_void = ctx.ctx.void_type();
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pi32 = llvm_i32.ptr_type(AddressSpace::default());
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    assert_eq!(index.get_type(), llvm_usize);

    let ndarray_calc_nd_indices_fn_name =
        get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_calc_nd_indices");
    let ndarray_calc_nd_indices_fn =
        ctx.module.get_function(&ndarray_calc_nd_indices_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(
                &[llvm_usize.into(), llvm_pusize.into(), llvm_usize.into(), llvm_pi32.into()],
                false,
            );

            ctx.module.add_function(&ndarray_calc_nd_indices_fn_name, fn_type, None)
        });

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    let ndarray_dims = ndarray.shape();

    let indices = ctx.builder.build_array_alloca(llvm_i32, ndarray_num_dims, "").unwrap();

    ctx.builder
        .build_call(
            ndarray_calc_nd_indices_fn,
            &[
                index.into(),
                ndarray_dims.base_ptr(ctx, generator).into(),
                ndarray_num_dims.into(),
                indices.into(),
            ],
            "",
        )
        .unwrap();

    TypedArrayLikeAdapter::from(
        ArraySliceValue::from_ptr_val(indices, ndarray_num_dims, None),
        |_, _, v| v.into_int_value(),
        |_, _, v| v.into(),
    )
}

/// Generates a call to `__nac3_ndarray_calc_broadcast`. Returns a tuple containing the number of
/// dimension and size of each dimension of the resultant `ndarray`.
pub fn call_ndarray_calc_broadcast<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    lhs: NDArrayValue<'ctx>,
    rhs: NDArrayValue<'ctx>,
) -> TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>> {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_broadcast_fn_name =
        get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_calc_broadcast");
    let ndarray_calc_broadcast_fn =
        ctx.module.get_function(&ndarray_calc_broadcast_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_usize.fn_type(
                &[
                    llvm_pusize.into(),
                    llvm_usize.into(),
                    llvm_pusize.into(),
                    llvm_usize.into(),
                    llvm_pusize.into(),
                ],
                false,
            );

            ctx.module.add_function(&ndarray_calc_broadcast_fn_name, fn_type, None)
        });

    let lhs_ndims = lhs.load_ndims(ctx);
    let rhs_ndims = rhs.load_ndims(ctx);
    let min_ndims = llvm_intrinsics::call_int_umin(ctx, lhs_ndims, rhs_ndims, None);

    gen_for_callback_incrementing(
        generator,
        ctx,
        None,
        llvm_usize.const_zero(),
        (min_ndims, false),
        |generator, ctx, _, idx| {
            let idx = ctx.builder.build_int_sub(min_ndims, idx, "").unwrap();
            let (lhs_dim_sz, rhs_dim_sz) = unsafe {
                (
                    lhs.shape().get_typed_unchecked(ctx, generator, &idx, None),
                    rhs.shape().get_typed_unchecked(ctx, generator, &idx, None),
                )
            };

            let llvm_usize_const_one = llvm_usize.const_int(1, false);
            let lhs_eqz = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, lhs_dim_sz, llvm_usize_const_one, "")
                .unwrap();
            let rhs_eqz = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, rhs_dim_sz, llvm_usize_const_one, "")
                .unwrap();
            let lhs_or_rhs_eqz = ctx.builder.build_or(lhs_eqz, rhs_eqz, "").unwrap();

            let lhs_eq_rhs = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, lhs_dim_sz, rhs_dim_sz, "")
                .unwrap();

            let is_compatible = ctx.builder.build_or(lhs_or_rhs_eqz, lhs_eq_rhs, "").unwrap();

            ctx.make_assert(
                generator,
                is_compatible,
                "0:ValueError",
                "operands could not be broadcast together",
                [None, None, None],
                ctx.current_loc,
            );

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )
    .unwrap();

    let max_ndims = llvm_intrinsics::call_int_umax(ctx, lhs_ndims, rhs_ndims, None);
    let lhs_dims = lhs.shape().base_ptr(ctx, generator);
    let lhs_ndims = lhs.load_ndims(ctx);
    let rhs_dims = rhs.shape().base_ptr(ctx, generator);
    let rhs_ndims = rhs.load_ndims(ctx);
    let out_dims = ctx.builder.build_array_alloca(llvm_usize, max_ndims, "").unwrap();
    let out_dims = ArraySliceValue::from_ptr_val(out_dims, max_ndims, None);

    ctx.builder
        .build_call(
            ndarray_calc_broadcast_fn,
            &[
                lhs_dims.into(),
                lhs_ndims.into(),
                rhs_dims.into(),
                rhs_ndims.into(),
                out_dims.base_ptr(ctx, generator).into(),
            ],
            "",
        )
        .unwrap();

    TypedArrayLikeAdapter::from(out_dims, |_, _, v| v.into_int_value(), |_, _, v| v.into())
}

/// Generates a call to `__nac3_ndarray_calc_broadcast_idx`. Returns an [`ArrayAllocaValue`]
/// containing the indices used for accessing `array` corresponding to the index of the broadcasted
/// array `broadcast_idx`.
pub fn call_ndarray_calc_broadcast_index<
    'ctx,
    G: CodeGenerator + ?Sized,
    BroadcastIdx: UntypedArrayLikeAccessor<'ctx>,
>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    array: NDArrayValue<'ctx>,
    broadcast_idx: &BroadcastIdx,
) -> TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pi32 = llvm_i32.ptr_type(AddressSpace::default());
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_broadcast_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_broadcast_idx",
        64 => "__nac3_ndarray_calc_broadcast_idx64",
        bw => codegen_unreachable!(ctx, "Unsupported size type bit width: {}", bw),
    };
    let ndarray_calc_broadcast_fn =
        ctx.module.get_function(ndarray_calc_broadcast_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_usize.fn_type(
                &[llvm_pusize.into(), llvm_usize.into(), llvm_pi32.into(), llvm_pi32.into()],
                false,
            );

            ctx.module.add_function(ndarray_calc_broadcast_fn_name, fn_type, None)
        });

    let broadcast_size = broadcast_idx.size(ctx, generator);
    let out_idx = ctx.builder.build_array_alloca(llvm_i32, broadcast_size, "").unwrap();

    let array_dims = array.shape().base_ptr(ctx, generator);
    let array_ndims = array.load_ndims(ctx);
    let broadcast_idx_ptr = unsafe {
        broadcast_idx.ptr_offset_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
    };

    ctx.builder
        .build_call(
            ndarray_calc_broadcast_fn,
            &[array_dims.into(), array_ndims.into(), broadcast_idx_ptr.into(), out_idx.into()],
            "",
        )
        .unwrap();

    TypedArrayLikeAdapter::from(
        ArraySliceValue::from_ptr_val(out_idx, broadcast_size, None),
        |_, _, v| v.into_int_value(),
        |_, _, v| v.into(),
    )
}
