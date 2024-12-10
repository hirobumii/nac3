use inkwell::{
    values::{BasicValueEnum, CallSiteValue, IntValue, PointerValue},
    AddressSpace,
};
use itertools::Either;

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
    let string_eq_fn = ctx.module.get_function("nac3_str_eq").unwrap_or_else(|| {
        let i8_ptr_type = ctx.ctx.i8_type().ptr_type(AddressSpace::default());
        let i64_type = ctx.ctx.i64_type();
        let i32_type = ctx.ctx.i32_type();
        let fn_type = i32_type.fn_type(
            &[i8_ptr_type.into(), i64_type.into(), i8_ptr_type.into(), i64_type.into()],
            false,
        );
        ctx.module.add_function("nac3_str_eq", fn_type, None)
    });
    let result = ctx
        .builder
        .build_call(
            string_eq_fn,
            &[
                str1_ptr.into(),
                ctx.builder.build_int_z_extend(str1_len, ctx.ctx.i64_type(), "").unwrap().into(),
                str2_ptr.into(),
                ctx.builder.build_int_z_extend(str2_len, ctx.ctx.i64_type(), "").unwrap().into(),
            ],
            "string_eq",
        )
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();
    generator.bool_to_i1(ctx, result)
}
