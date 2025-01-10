use inkwell::{
    values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue},
    IntPredicate,
};
use itertools::Either;

use crate::codegen::{
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
    let symbol = match (base.get_type().get_bit_width(), exp.get_type().get_bit_width(), signed) {
        (32, 32, true) => "__nac3_int_exp_int32_t",
        (64, 64, true) => "__nac3_int_exp_int64_t",
        (32, 32, false) => "__nac3_int_exp_uint32_t",
        (64, 64, false) => "__nac3_int_exp_uint64_t",
        _ => codegen_unreachable!(ctx),
    };
    let base_type = base.get_type();
    let pow_fun = ctx.module.get_function(symbol).unwrap_or_else(|| {
        let fn_type = base_type.fn_type(&[base_type.into(), base_type.into()], false);
        ctx.module.add_function(symbol, fn_type, None)
    });
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
    ctx.builder
        .build_call(pow_fun, &[base.into(), exp.into()], "call_int_pow")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
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

    let intrinsic_fn = ctx.module.get_function("__nac3_isinf").unwrap_or_else(|| {
        let fn_type = llvm_i32.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_isinf", fn_type, None)
    });

    let ret = ctx
        .builder
        .build_call(intrinsic_fn, &[v.into()], "isinf")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();

    generator.bool_to_i1(ctx, ret)
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

    let intrinsic_fn = ctx.module.get_function("__nac3_isnan").unwrap_or_else(|| {
        let fn_type = llvm_i32.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_isnan", fn_type, None)
    });

    let ret = ctx
        .builder
        .build_call(intrinsic_fn, &[v.into()], "isnan")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();

    generator.bool_to_i1(ctx, ret)
}

/// Generates a call to `gamma` in IR. Returns an `f64` representing the result.
pub fn call_gamma<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    let intrinsic_fn = ctx.module.get_function("__nac3_gamma").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_gamma", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "gamma")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `gammaln` in IR. Returns an `f64` representing the result.
pub fn call_gammaln<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    let intrinsic_fn = ctx.module.get_function("__nac3_gammaln").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_gammaln", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "gammaln")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `j0` in IR. Returns an `f64` representing the result.
pub fn call_j0<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    let intrinsic_fn = ctx.module.get_function("__nac3_j0").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_j0", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "j0")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}
