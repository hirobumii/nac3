use inkwell::{
    attributes::{Attribute, AttributeLoc},
    context::Context,
    memory_buffer::MemoryBuffer,
    module::Module,
    values::{BasicValue, BasicValueEnum, IntValue},
    IntPredicate,
};

use nac3parser::ast::Expr;

use super::{CodeGenContext, CodeGenerator};
use crate::{symbol_resolver::SymbolResolver, typecheck::typedef::Type};
pub use list::*;
pub use math::*;
pub use slice::*;

mod list;
mod math;
pub mod ndarray;
mod slice;

#[must_use]
pub fn load_irrt<'ctx>(ctx: &'ctx Context, symbol_resolver: &dyn SymbolResolver) -> Module<'ctx> {
    let bitcode_buf = MemoryBuffer::create_from_memory_range(
        include_bytes!(concat!(env!("OUT_DIR"), "/irrt.bc")),
        "irrt_bitcode_buffer",
    );
    let irrt_mod = Module::parse_bitcode_from_buffer(&bitcode_buf, ctx).unwrap();
    let inline_attr = Attribute::get_named_enum_kind_id("alwaysinline");
    for symbol in &[
        "__nac3_int_exp_int32_t",
        "__nac3_int_exp_int64_t",
        "__nac3_range_slice_len",
        "__nac3_slice_index_bound",
    ] {
        let function = irrt_mod.get_function(symbol).unwrap();
        function.add_attribute(AttributeLoc::Function, ctx.create_enum_attribute(inline_attr, 0));
    }

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

/// Returns the name of a function which contains variants for 32-bit and 64-bit `size_t`.
///
/// - When [`TypeContext::size_type`] is 32-bits, the function name is `fn_name}`.
/// - When [`TypeContext::size_type`] is 64-bits, the function name is `{fn_name}64`.
#[must_use]
pub fn get_usize_dependent_function_name<G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'_, '_>,
    name: &str,
) -> String {
    let mut name = name.to_owned();
    match generator.get_size_type(ctx.ctx).get_bit_width() {
        32 => {}
        64 => name.push_str("64"),
        bit_width => {
            panic!("Unsupported int type bit width {bit_width}, must be either 32-bits or 64-bits")
        }
    }
    name
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
pub fn handle_slice_indices<'ctx, G: CodeGenerator>(
    start: &Option<Box<Expr<Option<Type>>>>,
    end: &Option<Box<Expr<Option<Type>>>>,
    step: &Option<Box<Expr<Option<Type>>>>,
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut G,
    length: IntValue<'ctx>,
) -> Result<Option<(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)>, String> {
    let int32 = ctx.ctx.i32_type();
    let zero = int32.const_zero();
    let one = int32.const_int(1, false);
    let length = ctx.builder.build_int_truncate_or_bit_cast(length, int32, "leni32").unwrap();
    Ok(Some(match (start, end, step) {
        (s, e, None) => (
            if let Some(s) = s.as_ref() {
                match handle_slice_index_bound(s, ctx, generator, length)? {
                    Some(v) => v,
                    None => return Ok(None),
                }
            } else {
                int32.const_zero()
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
                ctx.builder.build_int_sub(e, one, "final_end").unwrap()
            },
            one,
        ),
        (s, e, Some(step)) => {
            let step = if let Some(v) = generator.gen_expr(ctx, step)? {
                v.to_basic_value_enum(ctx, generator, ctx.primitives.int32)?.into_int_value()
            } else {
                return Ok(None);
            };
            // assert step != 0, throw exception if not
            let not_zero = ctx
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    step,
                    step.get_type().const_zero(),
                    "range_step_ne",
                )
                .unwrap();
            ctx.make_assert(
                generator,
                not_zero,
                "0:ValueError",
                "slice step cannot be zero",
                [None, None, None],
                ctx.current_loc,
            );
            let len_id = ctx.builder.build_int_sub(length, one, "lenmin1").unwrap();
            let neg = ctx
                .builder
                .build_int_compare(IntPredicate::SLT, step, zero, "step_is_neg")
                .unwrap();
            (
                match s {
                    Some(s) => {
                        let Some(s) = handle_slice_index_bound(s, ctx, generator, length)? else {
                            return Ok(None);
                        };
                        ctx.builder
                            .build_select(
                                ctx.builder
                                    .build_and(
                                        ctx.builder
                                            .build_int_compare(
                                                IntPredicate::EQ,
                                                s,
                                                length,
                                                "s_eq_len",
                                            )
                                            .unwrap(),
                                        neg,
                                        "should_minus_one",
                                    )
                                    .unwrap(),
                                ctx.builder.build_int_sub(s, one, "s_min").unwrap(),
                                s,
                                "final_start",
                            )
                            .map(BasicValueEnum::into_int_value)
                            .unwrap()
                    }
                    None => ctx
                        .builder
                        .build_select(neg, len_id, zero, "stt")
                        .map(BasicValueEnum::into_int_value)
                        .unwrap(),
                },
                match e {
                    Some(e) => {
                        let Some(e) = handle_slice_index_bound(e, ctx, generator, length)? else {
                            return Ok(None);
                        };
                        ctx.builder
                            .build_select(
                                neg,
                                ctx.builder.build_int_add(e, one, "end_add_one").unwrap(),
                                ctx.builder.build_int_sub(e, one, "end_sub_one").unwrap(),
                                "final_end",
                            )
                            .map(BasicValueEnum::into_int_value)
                            .unwrap()
                    }
                    None => ctx
                        .builder
                        .build_select(neg, zero, len_id, "end")
                        .map(BasicValueEnum::into_int_value)
                        .unwrap(),
                },
                step,
            )
        }
    }))
}
