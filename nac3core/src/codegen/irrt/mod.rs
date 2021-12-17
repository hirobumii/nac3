use super::*;
use inkwell::{
    attributes::AttributeLoc,
    memory_buffer::MemoryBuffer,
    module::{Linkage, Module},
    values::IntValue,
};

pub struct IrrtSymbolTable;
impl IrrtSymbolTable {
    const LEN: &'static str = "__nac3_irrt_range_slice_len";
    const POWER_I32: &'static str = "__nac3_irrt_int_exp_int32_t";
    const POWER_I64: &'static str = "__nac3_irrt_int_exp_int64_t";
}
pub const ALL_IRRT_SYMBOLS: &[&str] =
    &[IrrtSymbolTable::LEN, IrrtSymbolTable::POWER_I32, IrrtSymbolTable::POWER_I64];

fn load_irrt<'ctx, 'a>(ctx: &CodeGenContext<'ctx, 'a>, fun: &str) -> FunctionValue<'ctx> {
    let bitcode_buf = MemoryBuffer::create_from_memory_range(
        include_bytes!(concat!(env!("OUT_DIR"), "/irrt.bc")),
        "irrt_bitcode_buffer",
    );
    let irrt_mod = Module::parse_bitcode_from_buffer(&bitcode_buf, ctx.ctx).unwrap();
    irrt_mod.set_data_layout(&ctx.module.get_data_layout());
    irrt_mod.set_triple(&ctx.module.get_triple());
    ctx.module.link_in_module(irrt_mod).unwrap();
    for f in ALL_IRRT_SYMBOLS {
        let fun = ctx.module.get_function(f).unwrap();
        fun.set_linkage(Linkage::Private);
        if f == &IrrtSymbolTable::POWER_I32 || f == &IrrtSymbolTable::POWER_I64 {
            // add alwaysinline attributes to power function to help them get inlined
            // alwaysinline enum = 1, see release/13.x/llvm/include/llvm/IR/Attributes.td
            fun.add_attribute(AttributeLoc::Function, ctx.ctx.create_enum_attribute(1, 0));
        }
    }
    ctx.module.get_function(fun).unwrap()
}

// equivalent code:
// def length(start, end, step != 0):
//     diff = end - start
//     if diff > 0 and step > 0:
//          return ((diff - 1) // step) + 1
//     elif diff < 0 and step < 0:
//          return ((diff + 1) // step) + 1
//     else:
//          return 0
pub fn calculate_len_for_slice_range<'ctx, 'a>(
    ctx: &mut CodeGenContext<'ctx, 'a>,
    start: IntValue<'ctx>,
    end: IntValue<'ctx>,
    step: IntValue<'ctx>,
) -> IntValue<'ctx> {
    const FUN_SYMBOL: &str = IrrtSymbolTable::LEN;
    let len_func =
        ctx.module.get_function(FUN_SYMBOL).unwrap_or_else(|| load_irrt(ctx, FUN_SYMBOL));

    // TODO: throw exception when step == 0
    ctx.builder
        .build_call(len_func, &[start.into(), end.into(), step.into()], "calc_len")
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value()
}
// repeated squaring method adapted from GNU Scientific Library:
// https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
pub fn integer_power<'ctx, 'a>(
    ctx: &mut CodeGenContext<'ctx, 'a>,
    base: IntValue<'ctx>,
    exp: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let symbol = match (base.get_type().get_bit_width(), exp.get_type().get_bit_width()) {
        (32, 32) => IrrtSymbolTable::POWER_I32,
        (64, 64) => IrrtSymbolTable::POWER_I64,
        _ => unreachable!(),
    };
    let pow_fun = ctx.module.get_function(symbol).unwrap_or_else(|| load_irrt(ctx, symbol));
    // TODO: throw exception when exp < 0
    ctx.builder
        .build_call(pow_fun, &[base.into(), exp.into()], "call_int_pow")
        .try_as_basic_value()
        .unwrap_left()
        .into_int_value()
}
