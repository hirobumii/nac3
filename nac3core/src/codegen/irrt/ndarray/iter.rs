use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue},
    AddressSpace,
};

use crate::codegen::{
    expr::{create_and_call_function, infer_and_call_function},
    irrt::get_usize_dependent_function_name,
    types::ProxyType,
    values::{
        ndarray::{NDArrayValue, NDIterValue},
        ProxyValue, TypedArrayLikeAccessor,
    },
    CodeGenContext, CodeGenerator,
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
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    assert_eq!(
        BasicTypeEnum::try_from(indices.element_type(ctx, generator)).unwrap(),
        llvm_usize.into()
    );

    let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_initialize");

    create_and_call_function(
        ctx,
        &name,
        None,
        &[
            (iter.get_type().as_base_type().into(), iter.as_base_value().into()),
            (ndarray.get_type().as_base_type().into(), ndarray.as_base_value().into()),
            (llvm_pusize.into(), indices.base_ptr(ctx, generator).into()),
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
        &[iter.as_base_value().into()],
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

    infer_and_call_function(ctx, &name, None, &[iter.as_base_value().into()], None, None);
}
