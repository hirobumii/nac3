use inkwell::{
    IntPredicate,
    values::{FloatValue, IntValue},
};

use crate::codegen::{CodeGenContext, expr::call_extern, macros::codegen_unreachable};

// repeated squaring method adapted from GNU Scientific Library:
// https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
pub fn integer_power<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    base: IntValue<'ctx>,
    exp: IntValue<'ctx>,
    signed: bool,
) -> anyhow::Result<IntValue<'ctx>> {
    let base_type = base.get_type();

    let symbol = match (base_type.get_bit_width(), exp.get_type().get_bit_width(), signed) {
        (32, 32, true) => "__nac3_int_exp_int32_t",
        (64, 64, true) => "__nac3_int_exp_int64_t",
        (32, 32, false) => "__nac3_int_exp_uint32_t",
        (64, 64, false) => "__nac3_int_exp_uint64_t",
        _ => codegen_unreachable!(ctx),
    };

    // throw exception when exp < 0
    let ge_zero = ctx.builder.build_int_compare(
        IntPredicate::SGE,
        exp,
        exp.get_type().const_zero(),
        "assert_int_pow_ge_0",
    )?;
    ctx.make_assert(
        ge_zero,
        "0:ValueError",
        "integer power must be positive or zero",
        [None, None, None],
    )?;

    call_extern!(ctx: base_type "call_int_pow" = symbol(base, exp))
}

/// Generates a call to `gammaln` in IR. Returns an `f64` representing the result.
pub fn call_gammaln<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> anyhow::Result<FloatValue<'ctx>> {
    let llvm_f64 = ctx.f64;
    assert_eq!(v.get_type(), llvm_f64);
    call_extern!(ctx: llvm_f64 "gammaln" = "__nac3_gammaln"(v))
}

/// Generates a call to `j0` in IR. Returns an `f64` representing the result.
pub fn call_j0<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> anyhow::Result<FloatValue<'ctx>> {
    let llvm_f64 = ctx.f64;
    assert_eq!(v.get_type(), llvm_f64);
    call_extern!(ctx: llvm_f64 "j0" = "__nac3_j0"(v))
}
