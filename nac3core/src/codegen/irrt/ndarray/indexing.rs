use crate::codegen::{
    expr::infer_and_call_function,
    irrt::get_usize_dependent_function_name,
    values::{ndarray::NDArrayValue, ArrayLikeValue, ArraySliceValue, ProxyValue},
    CodeGenContext, CodeGenerator,
};

/// Generates a call to `__nac3_ndarray_index`.
///
/// Performs [basic indexing](https://numpy.org/doc/stable/user/basics.indexing.html#basic-indexing)
/// on `src_ndarray` using `indices`, writing the result to `dst_ndarray`, corresponding to the
/// operation `dst_ndarray = src_ndarray[indices]`.
pub fn call_nac3_ndarray_index<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    indices: ArraySliceValue<'ctx>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_index");
    infer_and_call_function(
        ctx,
        &name,
        None,
        &[
            indices.size(ctx, generator).into(),
            indices.base_ptr(ctx, generator).into(),
            src_ndarray.as_base_value().into(),
            dst_ndarray.as_base_value().into(),
        ],
        None,
        None,
    );
}
