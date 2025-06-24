use inkwell::{
    types::IntType,
    values::{BasicValue, BasicValueEnum, PointerValue, StructValue},
};

use super::ProxyValue;
use crate::codegen::{CodeGenContext, types::TupleType};

#[derive(Copy, Clone)]
pub struct TupleValue<'ctx> {
    value: StructValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> TupleValue<'ctx> {
    /// Creates an [`TupleValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value(
        value: StructValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(value, llvm_usize).is_ok());

        Self { value, llvm_usize, name }
    }

    /// Creates an [`TupleValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        Self::from_struct_value(
            ctx.builder
                .build_load(ptr, name.unwrap_or_default())
                .map(BasicValueEnum::into_struct_value)
                .unwrap(),
            llvm_usize,
            name,
        )
    }

    /// Stores a value into the tuple element at the given `index`.
    pub fn insert_element(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        index: u32,
        element: impl BasicValue<'ctx>,
    ) {
        assert_eq!(element.as_basic_value_enum().get_type(), unsafe {
            self.get_type().type_at_index_unchecked(index)
        });

        let new_value = ctx
            .builder
            .build_insert_value(self.value, element, index, self.name.unwrap_or_default())
            .unwrap();
        self.value = new_value.into_struct_value();
    }

    /// Loads a value from the tuple element at the given `index`.
    pub fn extract_element(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        index: u32,
    ) -> BasicValueEnum<'ctx> {
        ctx.builder
            .build_extract_value(
                self.value,
                index,
                &format!("{}[{{i}}]", self.name.unwrap_or("tuple")),
            )
            .unwrap()
    }
}

impl<'ctx> ProxyValue<'ctx> for TupleValue<'ctx> {
    type ABI = StructValue<'ctx>;
    type Base = StructValue<'ctx>;
    type Type = TupleType<'ctx>;

    fn get_type(&self) -> Self::Type {
        TupleType::from_struct_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> From<TupleValue<'ctx>> for StructValue<'ctx> {
    fn from(value: TupleValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
