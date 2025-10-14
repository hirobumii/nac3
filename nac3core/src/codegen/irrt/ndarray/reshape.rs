use inkwell::values::IntValue;

use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    values::{ArrayLikeValue, ArraySliceValue},
};

/// Generates a call to `__nac3_ndarray_reshape_resolve_and_check_new_shape`.
///
/// Resolves unknown dimensions in `new_shape` for `numpy.reshape(<ndarray>, new_shape)`, raising an
/// assertion if multiple dimensions are unknown (`-1`).
pub fn call_nac3_ndarray_reshape_resolve_and_check_new_shape<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    size: IntValue<'ctx>,
    new_ndims: IntValue<'ctx>,
    new_shape: ArraySliceValue<'ctx>,
) {
    let llvm_usize = ctx.size_t;

    assert_eq!(size.get_type(), llvm_usize);
    assert_eq!(new_ndims.get_type(), llvm_usize);
    assert_eq!(new_shape.element_type(ctx), llvm_usize.into());

    let name = get_usize_dependent_function_name(
        ctx,
        "__nac3_ndarray_reshape_resolve_and_check_new_shape",
    );
    call_extern!(ctx: void _ = name(size, new_ndims, new_shape.base_ptr(ctx)));
}
