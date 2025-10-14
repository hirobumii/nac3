use inkwell::{AddressSpace, values::IntValue};

use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    values::{ProxyValue, TypedArrayLikeAccessor, ndarray::NDArrayValue},
};

/// Generates a call to `__nac3_ndarray_transpose`.
///
/// Creates a transpose view of `src_ndarray` and writes the result to `dst_ndarray`.
///
/// `dst_ndarray` must fulfill the following preconditions:
///
/// - `dst_ndarray.ndims` must be initialized and must be equal to `src_ndarray.ndims`.
/// - `dst_ndarray.shape` must be allocated and may contain uninitialized values.
/// - `dst_ndarray.strides` must be allocated and may contain uninitialized values.
pub fn call_nac3_ndarray_transpose<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
    axes: Option<&impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>>,
) {
    let llvm_usize = ctx.size_t;

    assert!(axes.is_none_or(|axes| axes.size(ctx).get_type() == llvm_usize));
    assert!(axes.is_none_or(|axes| axes.element_type(ctx) == llvm_usize.into()));

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_transpose");
    call_extern!(ctx: void _ = name(
        src_ndarray.as_abi_value(ctx),
        dst_ndarray.as_abi_value(ctx),
        axes.map_or(llvm_usize.const_zero(), |axes| axes.size(ctx)),
        axes.map_or(llvm_usize.ptr_type(AddressSpace::default()).const_null(), |axes| {
            axes.base_ptr(ctx)
        }),
    ));
}
