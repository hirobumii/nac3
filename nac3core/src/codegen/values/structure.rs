use inkwell::values::{BasicValueEnum, PointerValue, StructValue};

use super::ProxyValue;
use crate::codegen::{types::structure::StructProxyType, CodeGenContext};

/// An LLVM value that is used to represent a corresponding structure-like value in NAC3.
pub trait StructProxyValue<'ctx>:
    ProxyValue<'ctx, Base = PointerValue<'ctx>, Type: StructProxyType<'ctx, Value = Self>>
{
    /// Returns this value as a [`StructValue`].
    #[must_use]
    fn get_struct_value(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructValue<'ctx> {
        ctx.builder
            .build_load(self.get_pointer_value(ctx), "")
            .map(BasicValueEnum::into_struct_value)
            .unwrap()
    }

    /// Returns this value as a [`PointerValue`].
    #[must_use]
    fn get_pointer_value(&self, _: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.as_base_value()
    }
}
