use nac3core::{
    codegen::types::structure::StructField,
    inkwell::{
        AddressSpace,
        values::{IntValue, PointerValue},
    },
};
use nac3core_derive::StructFields;

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct SliceValue<'ctx> {
    #[value_type(ctx.i8_type().ptr_type(AddressSpace::default()))]
    ptr: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(usize)]
    len: StructField<'ctx, IntValue<'ctx>>,
}

fn main() {}
