use inkwell::{
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace,
};

use super::get_usize_dependent_function_name;
use crate::codegen::{expr::infer_and_call_function, CodeGenContext};

/// Generates a call to string equality comparison. Returns an `i1` representing whether the strings are equal.
pub fn call_string_eq<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    str1_ptr: PointerValue<'ctx>,
    str1_len: IntValue<'ctx>,
    str2_ptr: PointerValue<'ctx>,
    str2_len: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_pi8 = ctx.ctx.i8_type().ptr_type(AddressSpace::default());
    let llvm_usize = ctx.get_size_type();
    assert_eq!(str1_ptr.get_type(), llvm_pi8);
    assert_eq!(str1_len.get_type(), llvm_usize);
    assert_eq!(str2_ptr.get_type(), llvm_pi8);
    assert_eq!(str2_len.get_type(), llvm_usize);

    let func_name = get_usize_dependent_function_name(ctx, "nac3_str_eq");

    infer_and_call_function(
        ctx,
        &func_name,
        Some(llvm_i1.into()),
        &[str1_ptr.into(), str1_len.into(), str2_ptr.into(), str2_len.into()],
        Some("str_eq_call"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}
