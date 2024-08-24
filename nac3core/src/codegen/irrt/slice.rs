use inkwell::values::{BasicValueEnum, CallSiteValue, IntValue};
use itertools::Either;

use nac3parser::ast::Expr;

use crate::{
    codegen::{CodeGenContext, CodeGenerator},
    typecheck::typedef::Type,
};

/// this function allows index out of range, since python
/// allows index out of range in slice (`a = [1,2,3]; a[1:10] == [2,3]`).
pub fn handle_slice_index_bound<'ctx, G: CodeGenerator>(
    i: &Expr<Option<Type>>,
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut G,
    length: IntValue<'ctx>,
) -> Result<Option<IntValue<'ctx>>, String> {
    const SYMBOL: &str = "__nac3_slice_index_bound";
    let func = ctx.module.get_function(SYMBOL).unwrap_or_else(|| {
        let i32_t = ctx.ctx.i32_type();
        let fn_t = i32_t.fn_type(&[i32_t.into(), i32_t.into()], false);
        ctx.module.add_function(SYMBOL, fn_t, None)
    });

    let i = if let Some(v) = generator.gen_expr(ctx, i)? {
        v.to_basic_value_enum(ctx, generator, i.custom.unwrap())?
    } else {
        return Ok(None);
    };
    Ok(Some(
        ctx.builder
            .build_call(func, &[i.into(), length.into()], "bounded_ind")
            .map(CallSiteValue::try_as_basic_value)
            .map(|v| v.map_left(BasicValueEnum::into_int_value))
            .map(Either::unwrap_left)
            .unwrap(),
    ))
}
