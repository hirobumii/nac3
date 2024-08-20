use crate::codegen::{
    stmt::{gen_for_callback_incrementing, BreakContinueHooks},
    CodeGenContext, CodeGenerator,
};

use super::*;

/// Like [`gen_for_callback_incrementing`] with [`Model`] abstractions.
///
/// `stop` is not included.
pub fn gen_for_model<'ctx, 'a, G, F, N>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    start: Instance<'ctx, Int<N>>,
    stop: Instance<'ctx, Int<N>>,
    step: Instance<'ctx, Int<N>>,
    body: F,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    F: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        Instance<'ctx, Int<N>>,
    ) -> Result<(), String>,
    N: IntKind<'ctx> + Default,
{
    let int_model = Int(N::default());
    gen_for_callback_incrementing(
        generator,
        ctx,
        None,
        start.value,
        (stop.value, false),
        |g, ctx, hooks, i| {
            let i = int_model.believe_value(i);
            body(g, ctx, hooks, i)
        },
        step.value,
    )
}
