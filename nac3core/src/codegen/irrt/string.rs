use inkwell::values::IntValue;

use super::get_usize_dependent_function_name;
use crate::codegen::{CodeGenContext, expr::call_extern, values::StringValue};

/// Generates a call to string equality comparison. Returns an `i1` representing whether the strings are equal.
pub fn call_string_eq<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    str1: StringValue<'ctx>,
    str2: StringValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.i1;

    let func_name = get_usize_dependent_function_name(ctx, "nac3_str_eq");

    call_extern!(ctx: llvm_i1 "str_eq_call" = func_name(
        str1.extract_ptr(ctx),
        str1.extract_len(ctx),
        str2.extract_ptr(ctx),
        str2.extract_len(ctx),
    ))
}
