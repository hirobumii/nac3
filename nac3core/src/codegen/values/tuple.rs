use inkwell::{
    types::IntType,
    values::{BasicValue, BasicValueEnum, StructValue},
};

use super::ProxyValue;
use crate::codegen::{types::TupleType, CodeGenContext};

#[derive(Copy, Clone)]
pub struct TupleValue<'ctx> {
    value: StructValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> TupleValue<'ctx> {
    /// Checks whether `value` is an instance of `tuple`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_representable(
        value: StructValue<'ctx>,
        _llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        TupleType::is_representable(value.get_type())
    }

    /// Creates an [`TupleValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value(
        value: StructValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_representable(value, llvm_usize).is_ok());

        Self { value, llvm_usize, name }
    }

    /// Stores a value into the tuple element at the given `index`.
    pub fn store_element(
        &mut self,
        ctx: &CodeGenContext<'ctx, '_>,
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
    pub fn load_element(&self, ctx: &CodeGenContext<'ctx, '_>, index: u32) -> BasicValueEnum<'ctx> {
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
    type Base = StructValue<'ctx>;
    type Type = TupleType<'ctx>;

    fn get_type(&self) -> Self::Type {
        TupleType::from_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

impl<'ctx> From<TupleValue<'ctx>> for StructValue<'ctx> {
    fn from(value: TupleValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
