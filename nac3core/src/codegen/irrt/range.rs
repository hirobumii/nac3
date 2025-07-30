use inkwell::{IntPredicate, values::IntValue};

use crate::codegen::{CodeGenContext, CodeGenerator, expr::call_extern};

/// Invokes the `__nac3_range_slice_len` in IRRT.
///
/// - `start`: The `i32` start value for the slice.
/// - `end`: The `i32` end value for the slice.
/// - `step`: The `i32` step value for the slice.
///
/// Returns an `i32` value of the length of the slice.
pub fn calculate_len_for_slice_range<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    start: IntValue<'ctx>,
    end: IntValue<'ctx>,
    step: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i32 = ctx.i32;
    assert_eq!(start.get_type(), llvm_i32);
    assert_eq!(end.get_type(), llvm_i32);
    assert_eq!(step.get_type(), llvm_i32);

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

    call_extern!(ctx: llvm_i32 "calc_len" = "__nac3_range_slice_len"(start, end, step))
}
