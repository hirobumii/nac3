use inkwell::{
    AddressSpace,
    context::ContextRef,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use crate::codegen::{
    CodeGenContext, ModuleContext,
    types::{
        ProxyType,
        structure::{StructField, StructFields, StructProxyType, check_struct_type_matches_fields},
    },
    values::ndarray::ShapeEntryValue,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ShapeEntryType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct ShapeEntryStructFields<'ctx> {
    #[value_type(usize)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
}

impl<'ctx> ShapeEntryType<'ctx> {
    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> ShapeEntryStructFields<'ctx> {
        ShapeEntryStructFields::new(ctx, llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of a `ShapeEntry`.
    #[must_use]
    fn llvm_type(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_ty = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_ty, llvm_usize }
    }

    /// Creates an instance of [`ShapeEntryType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, ctx.size_t)
    }

    /// Creates a [`ShapeEntryType`] from a [`StructType`] representing an `ShapeEntry`.
    #[must_use]
    pub fn from_struct_type(ty: StructType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        Self::from_pointer_type(ty.ptr_type(AddressSpace::default()), llvm_usize)
    }

    /// Creates a [`ShapeEntryType`] from a [`PointerType`] representing an `ShapeEntry`.
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    /// Allocates an instance of [`ShapeEntryValue`] as if by calling `alloca` on the base type.
    #[must_use]
    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(ctx, name),
            self.llvm_usize,
            name,
        )
    }

    /// Allocates an instance of [`ShapeEntryValue`] as if by calling `alloca` on the base type.
    #[must_use]
    pub fn alloca_var(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca_var(ctx, name),
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`ShapeEntryValue`].
    #[must_use]
    pub fn map_struct_value(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: StructValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_struct_value(ctx, value, self.llvm_usize, name)
    }

    /// Converts an existing value into a [`ShapeEntryValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for ShapeEntryType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = ShapeEntryValue<'ctx>;

    fn is_representable(
        llvm_ty: impl BasicType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            Self::has_same_repr(ty, llvm_usize)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn has_same_repr(ty: Self::Base, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        let ctx = ty.get_context();

        let llvm_ndarray_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ndarray_ty) = llvm_ndarray_ty else {
            return Err(format!(
                "Expected struct type for `ShapeEntry` type, got {llvm_ndarray_ty}"
            ));
        };

        check_struct_type_matches_fields(
            Self::fields(ctx, llvm_usize),
            llvm_ndarray_ty,
            "NDArray",
            &[],
        )
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.as_abi_type().get_element_type().into_struct_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_abi_type(&self) -> Self::ABI {
        self.as_base_type()
    }
}

impl<'ctx> StructProxyType<'ctx> for ShapeEntryType<'ctx> {
    type StructFields = ShapeEntryStructFields<'ctx>;

    fn get_fields(&self) -> Self::StructFields {
        Self::fields(self.ty.get_context(), self.llvm_usize)
    }
}

impl<'ctx> From<ShapeEntryType<'ctx>> for PointerType<'ctx> {
    fn from(value: ShapeEntryType<'ctx>) -> Self {
        value.as_base_type()
    }
}
