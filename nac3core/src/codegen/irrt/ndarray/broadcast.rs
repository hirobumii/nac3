use inkwell::values::IntValue;

use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    types::{ProxyType, ndarray::ShapeEntryType},
    values::{
        ArrayLikeValue, ArraySliceValue, ProxyValue, TypedArrayLikeAccessor, TypedArrayLikeMutator,
        ndarray::NDArrayValue,
    },
};

/// Generates a call to `__nac3_ndarray_broadcast_to`.
///
/// Attempts to broadcast `src_ndarray` to the new shape defined by `dst_ndarray`.
///
/// `dst_ndarray` must meet the following preconditions:
///
/// - `dst_ndarray.ndims` must be initialized and matching the length of `dst_ndarray.shape`.
/// - `dst_ndarray.shape` must be initialized and contains the target broadcast shape.
/// - `dst_ndarray.strides` must be allocated and may contain uninitialized values.
pub fn call_nac3_ndarray_broadcast_to<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_broadcast_to");
    call_extern!(ctx: void _ = name(src_ndarray.as_abi_value(ctx), dst_ndarray.as_abi_value(ctx)));
}

/// Generates a call to `__nac3_ndarray_broadcast_shapes`.
///
/// Attempts to calculate the resultant shape from broadcasting all shapes in `shape_entries`,
/// writing the result to `dst_shape`.
pub fn call_nac3_ndarray_broadcast_shapes<'ctx, Shape>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    num_shape_entries: IntValue<'ctx>,
    shape_entries: ArraySliceValue<'ctx>,
    dst_ndims: IntValue<'ctx>,
    dst_shape: &Shape,
) where
    Shape:
        TypedArrayLikeAccessor<'ctx, IntValue<'ctx>> + TypedArrayLikeMutator<'ctx, IntValue<'ctx>>,
{
    let llvm_usize = ctx.size_t;

    assert_eq!(num_shape_entries.get_type(), llvm_usize);
    assert!(
        ShapeEntryType::is_representable(shape_entries.base_ptr(ctx).get_type(), llvm_usize,)
            .is_ok()
    );
    assert_eq!(dst_ndims.get_type(), llvm_usize);
    assert_eq!(dst_shape.element_type(ctx), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_broadcast_shapes");
    call_extern!(ctx: void _ = name(
        num_shape_entries,
        shape_entries.base_ptr(ctx),
        dst_ndims,
        dst_shape.base_ptr(ctx),
    ));
}
