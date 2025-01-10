use inkwell::values::IntValue;

use crate::codegen::{
    expr::infer_and_call_function,
    irrt::get_usize_dependent_function_name,
    values::{ArrayLikeValue, ArraySliceValue},
    CodeGenContext, CodeGenerator,
};

/// Generates a call to `__nac3_ndarray_reshape_resolve_and_check_new_shape`.
///
/// Resolves unknown dimensions in `new_shape` for `numpy.reshape(<ndarray>, new_shape)`, raising an
/// assertion if multiple dimensions are unknown (`-1`).
pub fn call_nac3_ndarray_reshape_resolve_and_check_new_shape<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    size: IntValue<'ctx>,
    new_ndims: IntValue<'ctx>,
    new_shape: ArraySliceValue<'ctx>,
) {
    let llvm_usize = generator.get_size_type(ctx.ctx);

    assert_eq!(size.get_type(), llvm_usize);
    assert_eq!(new_ndims.get_type(), llvm_usize);
    assert_eq!(new_shape.element_type(ctx, generator), llvm_usize.into());

    let name = get_usize_dependent_function_name(
        generator,
        ctx,
        "__nac3_ndarray_reshape_resolve_and_check_new_shape",
    );
    infer_and_call_function(
        ctx,
        &name,
        None,
        &[size.into(), new_ndims.into(), new_shape.base_ptr(ctx, generator).into()],
        None,
        None,
    );
}
