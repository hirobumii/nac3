use inkwell::{
    values::{BasicValueEnum, IntValue},
    AddressSpace,
};

use crate::codegen::{
    expr::{create_and_call_function, infer_and_call_function},
    irrt::get_usize_dependent_function_name,
    types::ProxyType,
    values::{
        ndarray::{NDArrayValue, NDIterValue},
        ArrayLikeValue, ArraySliceValue, ProxyValue,
    },
    CodeGenContext, CodeGenerator,
};

pub fn call_nac3_nditer_initialize<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
    indices: ArraySliceValue<'ctx>,
) {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_nditer_initialize");

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

pub fn call_nac3_nditer_has_element<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
) -> IntValue<'ctx> {
    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_nditer_has_element");

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

pub fn call_nac3_nditer_next<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    iter: NDIterValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_nditer_next");

    infer_and_call_function(ctx, &name, None, &[iter.as_base_value().into()], None, None);
}
