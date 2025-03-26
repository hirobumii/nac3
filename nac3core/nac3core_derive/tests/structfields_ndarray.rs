use nac3core::{
    codegen::types::structure::StructField,
    inkwell::{
        AddressSpace,
        values::{IntValue, PointerValue},
    },
};
use nac3core_derive::StructFields;

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct NDArrayValue<'ctx> {
    #[value_type(usize)]
    ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    data: StructField<'ctx, PointerValue<'ctx>>,
}

fn main() {}
