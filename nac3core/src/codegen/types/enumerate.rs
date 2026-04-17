use std::borrow::Cow;

use inkwell::values::{IntValue, PointerValue};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    ModuleContext,
    types::{Value, WithTypeinfo, builtin::BuiltinStruct, structure::StructField},
};

#[derive(Clone, Copy, StructFields)]
pub struct EnumerateStructFields<'ctx> {
    /// Pointer to the iterable data.
    #[value_type(ptr)]
    pub iterable: StructField<'ctx, PointerValue<'ctx>>,

    /// Start value for enumeration counter.
    #[value_type(i32)]
    pub start: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct EnumerateType<'ctx> {
    pub inner: BuiltinStruct<'ctx, EnumerateStructFields<'ctx>>,
}

impl<'ctx> WithTypeinfo<'ctx> for EnumerateType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_enumerate")
    }

    fn refcounted_field_offset(&self, _ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        Vec::new()
    }
}

impl<'ctx> EnumerateType<'ctx> {
    /// Creates an instance of [`EnumerateType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "enumerate") }
    }
}

pub type EnumerateValue<'ctx> = Value<'ctx, EnumerateType<'ctx>>;
