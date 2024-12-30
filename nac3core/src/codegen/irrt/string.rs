use inkwell::values::{BasicValueEnum, CallSiteValue, IntValue, PointerValue};
use itertools::Either;

use crate::codegen::{macros::codegen_unreachable, CodeGenContext, CodeGenerator};

/// Generates a call to string equality comparison. Returns an `i1` representing whether the strings are equal.
pub fn call_string_eq<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    str1_ptr: PointerValue<'ctx>,
    str1_len: IntValue<'ctx>,
    str2_ptr: PointerValue<'ctx>,
    str2_len: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let (func_name, return_type) = match ctx.ctx.i32_type().get_bit_width() {
        32 => ("nac3_str_eq", ctx.ctx.i32_type()),
        64 => ("nac3_str_eq64", ctx.ctx.i64_type()),
        bw => codegen_unreachable!(ctx, "Unsupported size type bit width: {}", bw),
    };

    let func = ctx.module.get_function(func_name).unwrap_or_else(|| {
        ctx.module.add_function(
            func_name,
            return_type.fn_type(
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
    let result = ctx
        .builder
        .build_call(
            func,
            &[str1_ptr.into(), str1_len.into(), str2_ptr.into(), str2_len.into()],
            "str_eq_call",
        )
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();
    generator.bool_to_i1(ctx, result)
}
