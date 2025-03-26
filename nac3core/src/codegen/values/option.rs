use inkwell::{
    types::IntType,
    values::{BasicValueEnum, IntValue, PointerValue},
};

use super::ProxyValue;
use crate::codegen::{CodeGenContext, types::OptionType};

/// Proxy type for accessing a `Option` value in LLVM.
#[derive(Copy, Clone)]
pub struct OptionValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> OptionValue<'ctx> {
    /// Creates an [`OptionValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        Self { value: ptr, llvm_usize, name }
    }

    /// Returns an `i1` indicating if this `Option` instance does not hold a value.
    #[must_use]
    pub fn is_none(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        ctx.builder.build_is_null(self.value, "").unwrap()
    }

    /// Returns an `i1` indicating if this `Option` instance contains a value.
    #[must_use]
    pub fn is_some(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        ctx.builder.build_is_not_null(self.value, "").unwrap()
    }

    /// Loads the value present in this `Option` instance.
    ///
    /// # Safety
    ///
    /// The caller must ensure that this `option` value [contains a value][Self::is_some].
    #[must_use]
    pub unsafe fn load(&self, ctx: &CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx> {
        ctx.builder.build_load(self.value, "").unwrap()
    }
}

impl<'ctx> ProxyValue<'ctx> for OptionValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = OptionType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_pointer_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> From<OptionValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: OptionValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
