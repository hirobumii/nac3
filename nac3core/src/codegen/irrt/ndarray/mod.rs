use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, CallSiteValue, IntValue},
    AddressSpace,
};
use itertools::Either;

use super::get_usize_dependent_function_name;
use crate::codegen::{
    values::{
        ndarray::NDArrayValue, ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue,
        TypedArrayLikeAdapter,
    },
    CodeGenContext, CodeGenerator,
};
pub use array::*;
pub use basic::*;
pub use broadcast::*;
pub use indexing::*;
pub use iter::*;
pub use reshape::*;
pub use transpose::*;

mod array;
mod basic;
mod broadcast;
mod indexing;
mod iter;
mod reshape;
mod transpose;

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
