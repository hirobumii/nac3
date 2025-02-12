use inkwell::values::{BasicValueEnum, IntValue};

use super::get_usize_dependent_function_name;
use crate::codegen::{expr::infer_and_call_function, values::StringValue, CodeGenContext};

/// Generates a call to string equality comparison. Returns an `i1` representing whether the strings are equal.
pub fn call_string_eq<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    str1: StringValue<'ctx>,
    str2: StringValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();

    let func_name = get_usize_dependent_function_name(ctx, "nac3_str_eq");

    infer_and_call_function(
        ctx,
        &func_name,
        Some(llvm_i1.into()),
        &[
            str1.extract_ptr(ctx).into(),
            str1.extract_len(ctx).into(),
            str2.extract_ptr(ctx).into(),
            str2.extract_len(ctx).into(),
        ],
        Some("str_eq_call"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}
