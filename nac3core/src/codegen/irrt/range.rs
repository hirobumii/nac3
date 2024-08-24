use inkwell::{
    values::{BasicValueEnum, CallSiteValue, IntValue},
    IntPredicate,
};
use itertools::Either;

use crate::codegen::{CodeGenContext, CodeGenerator};

pub fn calculate_len_for_slice_range<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    start: IntValue<'ctx>,
    end: IntValue<'ctx>,
    step: IntValue<'ctx>,
) -> IntValue<'ctx> {
    const SYMBOL: &str = "__nac3_range_slice_len";
    let len_func = ctx.module.get_function(SYMBOL).unwrap_or_else(|| {
        let i32_t = ctx.ctx.i32_type();
        let fn_t = i32_t.fn_type(&[i32_t.into(), i32_t.into(), i32_t.into()], false);
        ctx.module.add_function(SYMBOL, fn_t, None)
    });

    // assert step != 0, throw exception if not
    let not_zero = ctx
        .builder
        .build_int_compare(IntPredicate::NE, step, step.get_type().const_zero(), "range_step_ne")
        .unwrap();
    ctx.make_assert(
        generator,
        not_zero,
        "0:ValueError",
        "step must not be zero",
        [None, None, None],
        ctx.current_loc,
    );
    ctx.builder
        .build_call(len_func, &[start.into(), end.into(), step.into()], "calc_len")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}
