use inkwell::{
    context::Context,
    types::{BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{GlobalValue, IntValue, PointerValue, StructValue},
    AddressSpace,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use super::{
    structure::{check_struct_type_matches_fields, StructField, StructFields},
    ProxyType,
};
use crate::codegen::{values::StringValue, CodeGenContext, CodeGenerator};

/// Proxy type for a `str` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StringType<'ctx> {
    ty: StructType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct StringStructFields<'ctx> {
    /// Pointer to the first character of the string.
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub ptr: StructField<'ctx, PointerValue<'ctx>>,

    /// Length of the string.
    #[value_type(usize)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

impl<'ctx> StringType<'ctx> {
    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(llvm_usize: IntType<'ctx>) -> StringStructFields<'ctx> {
        StringStructFields::new(llvm_usize.get_context(), llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of a `str`.
    #[must_use]
    fn llvm_type(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> StructType<'ctx> {
        const NAME: &str = "str";

        if let Some(t) = ctx.get_struct_type(NAME) {
            t
        } else {
            let str_ty = ctx.opaque_struct_type(NAME);
            let field_tys = Self::fields(llvm_usize).into_iter().map(|field| field.1).collect_vec();
            str_ty.set_body(&field_tys, false);
            str_ty
        }
    }

    fn new_impl(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_str = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_str, llvm_usize }
    }

    /// Creates an instance of [`StringType`].
    #[must_use]
    pub fn new(ctx: &CodeGenContext<'ctx, '_>) -> Self {
        Self::new_impl(ctx.ctx, ctx.get_size_type())
    }

    /// Creates an instance of [`StringType`].
    #[must_use]
    pub fn new_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
    ) -> Self {
        Self::new_impl(ctx, generator.get_size_type(ctx))
    }

    /// Creates an [`StringType`] from a [`StructType`] representing a `str`.
    #[must_use]
    pub fn from_struct_type(ty: StructType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ty, llvm_usize).is_ok());

        Self { ty, llvm_usize }
    }

    /// Creates an [`StringType`] from a [`PointerType`] representing a `str`.
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        Self::from_struct_type(ptr_ty.get_element_type().into_struct_type(), llvm_usize)
    }

    /// Returns the fields present in this [`StringType`].
    #[must_use]
    pub fn get_fields(&self) -> StringStructFields<'ctx> {
        Self::fields(self.llvm_usize)
    }

    /// Constructs a global constant string.
    #[must_use]
    pub fn construct_constant(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        v: &str,
        name: Option<&'ctx str>,
    ) -> StringValue<'ctx> {
        let str_ptr = ctx
            .builder
            .build_global_string_ptr(v, "const")
            .map(GlobalValue::as_pointer_value)
            .unwrap();
        let size = ctx.get_size_type().const_int(v.len() as u64, false);
        self.map_struct_value(
            self.as_abi_type().const_named_struct(&[str_ptr.into(), size.into()]),
            name,
        )
    }

    /// Converts an existing value into a [`StringValue`].
    #[must_use]
    pub fn map_struct_value(
        &self,
        value: StructValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_struct_value(value, self.llvm_usize, name)
    }

    /// Converts an existing value into a [`StringValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(ctx, value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for StringType<'ctx> {
    type ABI = StructType<'ctx>;
    type Base = StructType<'ctx>;
    type Value = StringValue<'ctx>;

    fn is_representable(
        llvm_ty: impl BasicType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::StructType(ty) = llvm_ty.as_basic_type_enum() {
            Self::has_same_repr(ty, llvm_usize)
        } else {
            Err(format!("Expected structure type, got {llvm_ty:?}"))
        }
    }

    fn has_same_repr(ty: Self::Base, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        check_struct_type_matches_fields(Self::fields(llvm_usize), ty, "str", &[])
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.as_abi_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_abi_type(&self) -> Self::ABI {
        self.as_base_type()
    }
}

impl<'ctx> From<StringType<'ctx>> for StructType<'ctx> {
    fn from(value: StringType<'ctx>) -> Self {
        value.as_base_type()
    }
}
