use inkwell::{
    types::IntType,
    values::{BasicValueEnum, IntValue, PointerValue, StructValue},
};

use crate::codegen::{
    CodeGenContext,
    types::{StringType, structure::StructField},
    values::ProxyValue,
};

/// Proxy type for accessing a `str` value in LLVM.
#[derive(Copy, Clone)]
pub struct StringValue<'ctx> {
    value: StructValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    #[allow(dead_code)]
    name: Option<&'ctx str>,
}

impl<'ctx> StringValue<'ctx> {
    /// Creates an [`StringValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value(
        val: StructValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(val, llvm_usize).is_ok());

        Self { value: val, llvm_usize, name }
    }

    /// Creates an [`StringValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ctx: &CodeGenContext<'ctx, '_>,
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let val = ctx.builder.build_load(ptr, "").map(BasicValueEnum::into_struct_value).unwrap();

        Self::from_struct_value(val, llvm_usize, name)
    }

    fn ptr_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().ptr
    }

    /// Returns the pointer to the beginning of the string.
    pub fn extract_ptr(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.ptr_field().extract_value(ctx, self.value)
    }

    fn len_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().len
    }

    /// Returns the length of the string.
    pub fn extract_len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.len_field().extract_value(ctx, self.value)
    }
}

impl<'ctx> ProxyValue<'ctx> for StringValue<'ctx> {
    type ABI = StructValue<'ctx>;
    type Base = StructValue<'ctx>;
    type Type = StringType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_struct_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> From<StringValue<'ctx>> for StructValue<'ctx> {
    fn from(value: StringValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
