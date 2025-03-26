use inkwell::values::{BasicValueEnum, IntValue};

use nac3parser::ast::Expr;

use crate::{
    codegen::{CodeGenContext, CodeGenerator, expr::infer_and_call_function},
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

    let llvm_i32 = ctx.ctx.i32_type();
    assert_eq!(length.get_type(), llvm_i32);

    let i = if let Some(v) = generator.gen_expr(ctx, i)? {
        v.to_basic_value_enum(ctx, generator, i.custom.unwrap())?
    } else {
        return Ok(None);
    };

    Ok(Some(
        infer_and_call_function(
            ctx,
            SYMBOL,
            Some(llvm_i32.into()),
            &[i, length.into()],
            Some("bounded_ind"),
            None,
        )
        .map(BasicValueEnum::into_int_value)
        .unwrap(),
    ))
}
