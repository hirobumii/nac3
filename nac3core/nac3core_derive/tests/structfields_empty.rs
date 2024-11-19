use nac3core_derive::StructFields;
use std::marker::PhantomData;

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct EmptyValue<'ctx> {
    _phantom: PhantomData<&'ctx ()>,
}

fn main() {}
