use inkwell::values::IntValue;

use nac3parser::ast::Expr;

use crate::{
    codegen::{CodeGenContext, CodeGenerator, expr::call_extern},
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
    let llvm_i32 = ctx.i32;
    assert_eq!(length.get_type(), llvm_i32);

    let i = generator.gen_expr(ctx, i)?.to_basic_value_enum(ctx, generator)?;

    Ok(Some(call_extern!(ctx: llvm_i32 "bounded_ind" = "__nac3_slice_index_bound"(i, length))))
}
