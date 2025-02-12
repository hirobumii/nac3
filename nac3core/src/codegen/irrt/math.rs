use inkwell::{
    values::{BasicValueEnum, FloatValue, IntValue},
    IntPredicate,
};

use crate::codegen::{
    expr::infer_and_call_function,
    macros::codegen_unreachable,
    {CodeGenContext, CodeGenerator},
};

// repeated squaring method adapted from GNU Scientific Library:
// https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
pub fn integer_power<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    base: IntValue<'ctx>,
    exp: IntValue<'ctx>,
    signed: bool,
) -> IntValue<'ctx> {
    let base_type = base.get_type();

    let symbol = match (base_type.get_bit_width(), exp.get_type().get_bit_width(), signed) {
        (32, 32, true) => "__nac3_int_exp_int32_t",
        (64, 64, true) => "__nac3_int_exp_int64_t",
        (32, 32, false) => "__nac3_int_exp_uint32_t",
        (64, 64, false) => "__nac3_int_exp_uint64_t",
        _ => codegen_unreachable!(ctx),
    };

    // throw exception when exp < 0
    let ge_zero = ctx
        .builder
        .build_int_compare(
            IntPredicate::SGE,
            exp,
            exp.get_type().const_zero(),
            "assert_int_pow_ge_0",
        )
        .unwrap();
    ctx.make_assert(
        generator,
        ge_zero,
        "0:ValueError",
        "integer power must be positive or zero",
        [None, None, None],
        ctx.current_loc,
    );

    infer_and_call_function(
        ctx,
        symbol,
        Some(base_type.into()),
        &[base.into(), exp.into()],
        Some("call_int_pow"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

/// Generates a call to `isinf` in IR. Returns an `i1` representing the result.
pub fn call_isinf<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(
        ctx,
        "__nac3_isinf",
        Some(llvm_i32.into()),
        &[v.into()],
        Some("isinf"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .map(|ret| generator.bool_to_i1(ctx, ret))
    .unwrap()
}

/// Generates a call to `isnan` in IR. Returns an `i1` representing the result.
pub fn call_isnan<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(
        ctx,
        "__nac3_isnan",
        Some(llvm_i32.into()),
        &[v.into()],
        Some("isnan"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .map(|ret| generator.bool_to_i1(ctx, ret))
    .unwrap()
}

/// Generates a call to `gamma` in IR. Returns an `f64` representing the result.
pub fn call_gamma<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(
        ctx,
        "__nac3_gamma",
        Some(llvm_f64.into()),
        &[v.into()],
        Some("gamma"),
        None,
    )
    .map(BasicValueEnum::into_float_value)
    .unwrap()
}

/// Generates a call to `gammaln` in IR. Returns an `f64` representing the result.
pub fn call_gammaln<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(
        ctx,
        "__nac3_gammaln",
        Some(llvm_f64.into()),
        &[v.into()],
        Some("gammaln"),
        None,
    )
    .map(BasicValueEnum::into_float_value)
    .unwrap()
}

/// Generates a call to `j0` in IR. Returns an `f64` representing the result.
pub fn call_j0<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(ctx, "__nac3_j0", Some(llvm_f64.into()), &[v.into()], Some("j0"), None)
        .map(BasicValueEnum::into_float_value)
        .unwrap()
}
