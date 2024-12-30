use inkwell::values::{BasicValueEnum, CallSiteValue, IntValue, PointerValue};
use itertools::Either;

use super::get_usize_dependent_function_name;
use crate::codegen::{CodeGenContext, CodeGenerator};

/// Generates a call to string equality comparison. Returns an `i1` representing whether the strings are equal.
pub fn call_string_eq<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    str1_ptr: PointerValue<'ctx>,
    str1_len: IntValue<'ctx>,
    str2_ptr: PointerValue<'ctx>,
    str2_len: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();

    let func_name = get_usize_dependent_function_name(generator, ctx, "nac3_str_eq");

    let func = ctx.module.get_function(&func_name).unwrap_or_else(|| {
        ctx.module.add_function(
            &func_name,
            llvm_i1.fn_type(
                &[
                    str1_ptr.get_type().into(),
                    str1_len.get_type().into(),
                    str2_ptr.get_type().into(),
                    str2_len.get_type().into(),
                ],
                false,
            ),
            None,
        )
    });

    ctx.builder
        .build_call(
            func,
            &[str1_ptr.into(), str1_len.into(), str2_ptr.into(), str2_len.into()],
            "str_eq_call",
        )
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}
