use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    values::{ArrayLikeValue, ArraySliceValue, ProxyValue, ndarray::NDArrayValue},
};

/// Generates a call to `__nac3_ndarray_index`.
///
/// Performs [basic indexing](https://numpy.org/doc/stable/user/basics.indexing.html#basic-indexing)
/// on `src_ndarray` using `indices`, writing the result to `dst_ndarray`, corresponding to the
/// operation `dst_ndarray = src_ndarray[indices]`.
pub fn call_nac3_ndarray_index<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    indices: ArraySliceValue<'ctx>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_index");
    call_extern!(ctx: void _ = name(
        indices.size(ctx),
        indices.base_ptr(ctx),
        src_ndarray.as_abi_value(ctx),
        dst_ndarray.as_abi_value(ctx),
    ));
}
