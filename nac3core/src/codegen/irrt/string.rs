use inkwell::values::IntValue;

use crate::codegen::{CodeGenContext, expr::call_extern, types::StringValue};

/// Generates a call to string equality comparison. Returns an `i1` representing whether the strings are equal.
pub fn call_string_eq<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    str1: StringValue<'ctx>,
    str2: StringValue<'ctx>,
) -> anyhow::Result<IntValue<'ctx>> {
    let llvm_i1 = ctx.i1;

    call_extern!(ctx: llvm_i1 "str_eq_call" = "nac3_str_eq"(
        str1.ptr(ctx)?,
        str1.len(ctx)?,
        str2.ptr(ctx)?,
        str2.len(ctx)?,
    ))
}
