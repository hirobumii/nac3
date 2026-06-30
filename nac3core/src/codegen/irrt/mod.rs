use inkwell::{
    IntPredicate,
    context::ContextRef,
    memory_buffer::MemoryBuffer,
    module::Module,
    values::{BasicValue, BasicValueEnum, IntValue},
};
use nac3parser::ast::Expr;

use crate::{
    codegen::{CodeGenContext, CodeGenerator, TargetMachineOptions},
    symbol_resolver::SymbolResolver,
    typecheck::typedef::Type,
};

pub use cc_builtins::*;
pub use list::*;
pub use math::*;
pub use range::*;
pub use slice::*;
pub use string::*;

mod cc_builtins;
mod list;
mod math;
mod range;
mod slice;
mod string;

#[must_use]
pub fn load_irrt<'ctx>(
    ctx: ContextRef<'ctx>,
    target: &TargetMachineOptions,
    symbol_resolver: &dyn SymbolResolver,
) -> Module<'ctx> {
    let target = target.create_target_machine();
    let size_t = ctx.ptr_sized_int_type(&target.get_target_data(), None);
    let ir_bytes: &[u8] = if size_t == ctx.i64_type() {
        include_bytes!(concat!(env!("OUT_DIR"), "/irrt64.bc"))
    } else if size_t == ctx.i32_type() {
        include_bytes!(concat!(env!("OUT_DIR"), "/irrt32.bc"))
    } else {
        unreachable!("Unsupported size_t type bit width, must be either 32-bit or 64-bit")
    };

    let mut ir_bytes = ir_bytes.to_vec();
    // inkwell 0.9 requires nul-terminated input
    ir_bytes.push(b'\0');
    let ir_buf = MemoryBuffer::create_from_memory_range_copy(&ir_bytes, "irrt_ir_buffer");
    let irrt_mod = ctx.create_module_from_ir(ir_buf).unwrap();

    // Initialize all global `EXN_*` exception IDs in IRRT with the [`SymbolResolver`].
    let exn_id_type = ctx.i32_type();
    let errors = &[
        ("EXN_INDEX_ERROR", "0:IndexError"),
        ("EXN_VALUE_ERROR", "0:ValueError"),
        ("EXN_ASSERTION_ERROR", "0:AssertionError"),
        ("EXN_TYPE_ERROR", "0:TypeError"),
    ];
    for (irrt_name, symbol_name) in errors {
        let exn_id = symbol_resolver.get_string_id(symbol_name);
        let exn_id = exn_id_type.const_int(exn_id as u64, false).as_basic_value_enum();

        let global = irrt_mod.get_global(irrt_name).unwrap_or_else(|| {
            panic!("Exception symbol name '{irrt_name}' should exist in the IRRT LLVM module")
        });
        global.set_initializer(&exn_id);
    }

    irrt_mod
}

/// NOTE: the output value of the end index of this function should be compared ***inclusively***,
/// because python allows `a[2::-1]`, whose semantic is `[a[2], a[1], a[0]]`, which is equivalent to
/// NO numeric slice in python.
///
/// equivalent code:
/// ```pseudo_code
/// match (start, end, step):
///     case (s, e, None | Some(step)) if step > 0:
///         return (
///             match s:
///                 case None:
///                     0
///                 case Some(s):
///                     handle_in_bound(s)
///             ,match e:
///                 case None:
///                     length - 1
///                 case Some(e):
///                     handle_in_bound(e) - 1
///             ,step == None ? 1 : step
///         )
///     case (s, e, Some(step)) if step < 0:
///         return (
///             match s:
///                 case None:
///                     length - 1
///                 case Some(s):
///                     s = handle_in_bound(s)
///                     if s == length:
///                         s - 1
///                     else:
///                         s
///             ,match e:
///                 case None:
///                     0
///                 case Some(e):
///                     handle_in_bound(e) + 1
///             ,step
///         )
/// ```
#[allow(clippy::too_long_first_doc_paragraph)]
pub fn handle_slice_indices<'ctx, G: CodeGenerator>(
    start: &Option<Box<Expr<Option<Type>>>>,
    end: &Option<Box<Expr<Option<Type>>>>,
    step: &Option<Box<Expr<Option<Type>>>>,
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut G,
    length: IntValue<'ctx>,
) -> anyhow::Result<Option<(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)>> {
    let llvm_i32 = ctx.i32;

    let zero = llvm_i32.const_zero();
    let one = llvm_i32.const_int(1, false);
    let length = ctx.builder.build_int_truncate_or_bit_cast(length, llvm_i32, "leni32")?;
    Ok(Some(match (start, end, step) {
        (s, e, None) => (
            if let Some(s) = s.as_ref() {
                match handle_slice_index_bound(s, ctx, generator, length)? {
                    Some(v) => v,
                    None => return Ok(None),
                }
            } else {
                llvm_i32.const_zero()
            },
            {
                let e = if let Some(s) = e.as_ref() {
                    match handle_slice_index_bound(s, ctx, generator, length)? {
                        Some(v) => v,
                        None => return Ok(None),
                    }
                } else {
                    length
                };
                ctx.builder.build_int_sub(e, one, "final_end")?
            },
            one,
        ),
        (s, e, Some(step)) => {
            let step = generator.gen_expr(ctx, step)?.to_basic_value_enum(ctx)?.into_int_value();

            // assert step != 0, throw exception if not
            let not_zero = ctx.builder.build_int_compare(
                IntPredicate::NE,
                step,
                step.get_type().const_zero(),
                "range_step_ne",
            )?;
            ctx.make_assert(
                not_zero,
                "0:ValueError",
                "slice step cannot be zero",
                [None, None, None],
            )?;
            let len_id = ctx.builder.build_int_sub(length, one, "lenmin1")?;
            let neg =
                ctx.builder.build_int_compare(IntPredicate::SLT, step, zero, "step_is_neg")?;
            (
                match s {
                    Some(s) => {
                        let Some(s) = handle_slice_index_bound(s, ctx, generator, length)? else {
                            return Ok(None);
                        };
                        ctx.builder
                            .build_select(
                                ctx.builder.build_and(
                                    ctx.builder.build_int_compare(
                                        IntPredicate::EQ,
                                        s,
                                        length,
                                        "s_eq_len",
                                    )?,
                                    neg,
                                    "should_minus_one",
                                )?,
                                ctx.builder.build_int_sub(s, one, "s_min")?,
                                s,
                                "final_start",
                            )
                            .map(BasicValueEnum::into_int_value)?
                    }
                    None => ctx
                        .builder
                        .build_select(neg, len_id, zero, "stt")
                        .map(BasicValueEnum::into_int_value)?,
                },
                match e {
                    Some(e) => {
                        let Some(e) = handle_slice_index_bound(e, ctx, generator, length)? else {
                            return Ok(None);
                        };
                        ctx.builder
                            .build_select(
                                neg,
                                ctx.builder.build_int_add(e, one, "end_add_one")?,
                                ctx.builder.build_int_sub(e, one, "end_sub_one")?,
                                "final_end",
                            )
                            .map(BasicValueEnum::into_int_value)?
                    }
                    None => ctx
                        .builder
                        .build_select(neg, zero, len_id, "end")
                        .map(BasicValueEnum::into_int_value)?,
                },
                step,
            )
        }
    }))
}
