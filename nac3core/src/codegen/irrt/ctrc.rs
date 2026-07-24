use inkwell::{
    builder::Builder,
    values::{BasicValueEnum, IntValue, PointerValue},
};

use crate::codegen::CodeGenContext;

/// Invokes `__nac3_alloc` in IRRT, allocating `size` bytes of memory with `align` bytes of
/// alignment for an object.
///
/// In CTRC Mode:
///
/// - The allocation is served by the CTRC slab allocator.
/// - The maximum allocation size is `CTRC_CELL_SIZE` (governed in IRRT).
/// - The `align` argument must be a power of two and less than or equal to `CTRC_CELL_SIZE`.
///
/// In non-CTRC Mode:
///
/// - The allocation is served by the system allocator (i.e. `malloc`).
/// - The maximum allocation size is governed by the system allocator.
/// - The `align` argument is ignored; the system allocator will align based on the allocation size.
///
/// The returned pointer is never null: if the allocation cannot be satisfied - an oversized object
/// or unsatisfiable alignment in CTRC mode, or memory exhaustion - IRRT raises `0:MemoryError` at
/// the point of allocation rather than returning to the caller.
///
/// Note: This function manually builds the call to `__nac3_alloc` because `call_extern!` requires
/// `&mut CodeGenContext`, which cannot be satisfied when the `CodeGenContext` is already borrowed
/// by the `Builder`.
pub fn call_alloc<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    builder: &Builder<'ctx>,
    size: IntValue<'ctx>,
    align: u32,
    name: &str,
) -> anyhow::Result<PointerValue<'ctx>> {
    const FUNC_NAME: &str = "__nac3_alloc";

    let f = ctx.module.get_function(FUNC_NAME).unwrap_or_else(|| {
        ctx.module.add_function(
            FUNC_NAME,
            ctx.ptr.fn_type(&[ctx.size_t.into(), ctx.size_t.into()], false),
            None,
        )
    });
    let align = ctx.size_t.const_int(u64::from(align), false);
    Ok(builder
        .build_call(f, &[size.into(), align.into()], name)?
        .try_as_basic_value()
        .basic()
        .map(BasicValueEnum::into_pointer_value)
        .unwrap())
}
