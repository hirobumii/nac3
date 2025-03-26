use inkwell::values::{BasicValueEnum, IntValue};

use crate::codegen::{
    CodeGenContext, CodeGenerator,
    expr::infer_and_call_function,
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
    ctx: &CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
    indices: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.get_size_type();

    assert_eq!(indices.element_type(ctx, generator), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_initialize");

    infer_and_call_function(
        ctx,
        &name,
        None,
        &[
            iter.as_abi_value(ctx).into(),
            ndarray.as_abi_value(ctx).into(),
            indices.base_ptr(ctx, generator).into(),
        ],
        None,
        None,
    );
}

/// Generates a call to `__nac3_nditer_initialize_has_element`.
///
/// Returns an `i1` value indicating whether there are elements left to traverse for the `iter`
/// object.
pub fn call_nac3_nditer_has_element<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
) -> IntValue<'ctx> {
    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_has_element");

    infer_and_call_function(
        ctx,
        &name,
        Some(ctx.ctx.bool_type().into()),
        &[iter.as_abi_value(ctx).into()],
        None,
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

/// Generates a call to `__nac3_nditer_next`.
///
/// Moves `iter` to point to the next element.
pub fn call_nac3_nditer_next<'ctx>(ctx: &CodeGenContext<'ctx, '_>, iter: NDIterValue<'ctx>) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_next");

    infer_and_call_function(ctx, &name, None, &[iter.as_abi_value(ctx).into()], None, None);
}
