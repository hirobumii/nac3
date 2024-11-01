use inkwell::{
    context::Context,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::IntValue,
    AddressSpace,
};

use super::ProxyType;
use crate::codegen::{
    values::{ArraySliceValue, ListValue, ProxyValue},
    CodeGenContext, CodeGenerator,
};

/// Proxy type for a `list` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ListType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> ListType<'ctx> {
    /// Checks whether `llvm_ty` represents a `list` type, returning [Err] if it does not.
    pub fn is_representable(
        llvm_ty: PointerType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let llvm_list_ty = llvm_ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_list_ty) = llvm_list_ty else {
            return Err(format!("Expected struct type for `list` type, got {llvm_list_ty}"));
        };
        if llvm_list_ty.count_fields() != 2 {
            return Err(format!(
                "Expected 2 fields in `list`, got {}",
                llvm_list_ty.count_fields()
            ));
        }

        let list_size_ty = llvm_list_ty.get_field_type_at_index(0).unwrap();
        let Ok(_) = PointerType::try_from(list_size_ty) else {
            return Err(format!("Expected pointer type for `list.0`, got {list_size_ty}"));
        };

        let list_data_ty = llvm_list_ty.get_field_type_at_index(1).unwrap();
        let Ok(list_data_ty) = IntType::try_from(list_data_ty) else {
            return Err(format!("Expected int type for `list.1`, got {list_data_ty}"));
        };
        if list_data_ty.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!(
                "Expected {}-bit int type for `list.1`, got {}-bit int",
                llvm_usize.get_bit_width(),
                list_data_ty.get_bit_width()
            ));
        }

        Ok(())
    }

    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        element_type: BasicTypeEnum<'ctx>,
    ) -> Self {
        let llvm_usize = generator.get_size_type(ctx);
        let llvm_list = ctx
            .struct_type(
                &[element_type.ptr_type(AddressSpace::default()).into(), llvm_usize.into()],
                false,
            )
            .ptr_type(AddressSpace::default());

        ListType::from_type(llvm_list, llvm_usize)
    }

    /// Creates an [`ListType`] from a [`PointerType`].
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

        ListType { ty: ptr_ty, llvm_usize }
    }

    /// Returns the type of the `size` field of this `list` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(1)
            .map(BasicTypeEnum::into_int_type)
            .unwrap()
    }

    /// Returns the element type of this `list` type.
    #[must_use]
    pub fn element_type(&self) -> AnyTypeEnum<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(0)
            .map(BasicTypeEnum::into_pointer_type)
            .map(PointerType::get_element_type)
            .unwrap()
    }
}

impl<'ctx> ProxyType<'ctx> for ListType<'ctx> {
    type Base = PointerType<'ctx>;
    type Value = ListValue<'ctx>;

    fn is_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: impl BasicType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            <Self as ProxyType<'ctx>>::is_representable(generator, ctx, ty)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn is_representable<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: Self::Base,
    ) -> Result<(), String> {
        Self::is_representable(llvm_ty, generator.get_size_type(ctx))
    }

    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        self.map_value(
            generator
                .gen_var_alloc(
                    ctx,
                    self.as_base_type().get_element_type().into_struct_type().into(),
                    name,
                )
                .unwrap(),
            name,
        )
    }

    fn new_array_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx> {
        generator
            .gen_array_var_alloc(
                ctx,
                self.as_base_type().get_element_type().into_struct_type().into(),
                size,
                name,
            )
            .unwrap()
    }

    fn map_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        Self::Value::from_pointer_value(value, self.llvm_usize, name)
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }
}

impl<'ctx> From<ListType<'ctx>> for PointerType<'ctx> {
    fn from(value: ListType<'ctx>) -> Self {
        value.as_base_type()
    }
}
