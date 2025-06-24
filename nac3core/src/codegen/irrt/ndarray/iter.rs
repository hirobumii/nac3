use inkwell::values::IntValue;

use crate::codegen::{
    CodeGenContext, CodeGenerator,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    values::{
        ProxyValue, TypedArrayLikeAccessor,
        ndarray::{NDArrayValue, NDIterValue},
    },
};

/// Generates a call to `__nac3_nditer_initialize`.
///
/// Initializes the `iter` object.
pub fn call_nac3_nditer_initialize<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
    indices: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.get_size_type();
    assert_eq!(indices.element_type(ctx, generator), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_initialize");
    call_extern!(ctx: void _ = name(
        iter.as_abi_value(ctx),
        ndarray.as_abi_value(ctx),
        indices.base_ptr(ctx, generator),
    ));
}

/// Generates a call to `__nac3_nditer_initialize_has_element`.
///
/// Returns an `i1` value indicating whether there are elements left to traverse for the `iter`
/// object.
pub fn call_nac3_nditer_has_element<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
) -> IntValue<'ctx> {
    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_has_element");
    call_extern!(ctx: (ctx.ctx.bool_type()) _ = name(iter.as_abi_value(ctx)))
}

/// Generates a call to `__nac3_nditer_next`.
///
/// Moves `iter` to point to the next element.
pub fn call_nac3_nditer_next<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, iter: NDIterValue<'ctx>) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_next");
    call_extern!(ctx: void _ = name(iter.as_abi_value(ctx)));
}
