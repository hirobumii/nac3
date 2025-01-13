use inkwell::values::IntValue;

use crate::codegen::{
    expr::infer_and_call_function,
    irrt::get_usize_dependent_function_name,
    types::{ndarray::ShapeEntryType, ProxyType},
    values::{
        ndarray::NDArrayValue, ArrayLikeValue, ArraySliceValue, ProxyValue, TypedArrayLikeAccessor,
        TypedArrayLikeMutator,
    },
    CodeGenContext, CodeGenerator,
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
    ctx: &CodeGenContext<'ctx, '_>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_broadcast_to");
    infer_and_call_function(
        ctx,
        &name,
        None,
        &[src_ndarray.as_base_value().into(), dst_ndarray.as_base_value().into()],
        None,
        None,
    );
}

/// Generates a call to `__nac3_ndarray_broadcast_shapes`.
///
/// Attempts to calculate the resultant shape from broadcasting all shapes in `shape_entries`,
/// writing the result to `dst_shape`.
pub fn call_nac3_ndarray_broadcast_shapes<'ctx, G, Shape>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    num_shape_entries: IntValue<'ctx>,
    shape_entries: ArraySliceValue<'ctx>,
    dst_ndims: IntValue<'ctx>,
    dst_shape: &Shape,
) where
    G: CodeGenerator + ?Sized,
    Shape: TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>
        + TypedArrayLikeMutator<'ctx, G, IntValue<'ctx>>,
{
    let llvm_usize = ctx.get_size_type();

    assert_eq!(num_shape_entries.get_type(), llvm_usize);
    assert!(ShapeEntryType::is_type(
        generator,
        ctx.ctx,
        shape_entries.base_ptr(ctx, generator).get_type()
    )
    .is_ok());
    assert_eq!(dst_ndims.get_type(), llvm_usize);
    assert_eq!(dst_shape.element_type(ctx, generator), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_broadcast_shapes");
    infer_and_call_function(
        ctx,
        &name,
        None,
        &[
            num_shape_entries.into(),
            shape_entries.base_ptr(ctx, generator).into(),
            dst_ndims.into(),
            dst_shape.base_ptr(ctx, generator).into(),
        ],
        None,
        None,
    );
}
