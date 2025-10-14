use inkwell::values::IntValue;

use crate::codegen::{
    CodeGenContext,
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
pub fn call_nac3_nditer_initialize<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
    indices: &impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.size_t;
    assert_eq!(indices.element_type(ctx), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_initialize");
    call_extern!(ctx: void _ = name(
        iter.as_abi_value(ctx),
        ndarray.as_abi_value(ctx),
        indices.base_ptr(ctx),
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
    call_extern!(ctx: (ctx.i1) _ = name(iter.as_abi_value(ctx)))
}

/// Generates a call to `__nac3_nditer_next`.
///
/// Moves `iter` to point to the next element.
pub fn call_nac3_nditer_next<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, iter: NDIterValue<'ctx>) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_next");
    call_extern!(ctx: void _ = name(iter.as_abi_value(ctx)));
}
