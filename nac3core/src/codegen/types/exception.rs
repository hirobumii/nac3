use inkwell::{
    AddressSpace,
    context::{AsContextRef, Context},
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use super::{
    ProxyType,
    structure::{StructField, StructFields, StructProxyType, check_struct_type_matches_fields},
};
use crate::{
    codegen::{CodeGenContext, CodeGenerator, values::ExceptionValue},
    typecheck::typedef::{Type, TypeEnum},
};

/// Proxy type for an `Exception` in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ExceptionType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct ExceptionStructFields<'ctx> {
    /// The ID of the exception name.
    #[value_type(i32_type())]
    pub name: StructField<'ctx, IntValue<'ctx>>,

    /// The file where the exception originated from.
    #[value_type(get_struct_type("str").unwrap())]
    pub file: StructField<'ctx, StructValue<'ctx>>,

    /// The line number where the exception originated from.
    #[value_type(i32_type())]
    pub line: StructField<'ctx, IntValue<'ctx>>,

    /// The column number where the exception originated from.
    #[value_type(i32_type())]
    pub col: StructField<'ctx, IntValue<'ctx>>,

    /// The function name where the exception originated from.
    #[value_type(get_struct_type("str").unwrap())]
    pub func: StructField<'ctx, StructValue<'ctx>>,

    /// The exception message.
    #[value_type(get_struct_type("str").unwrap())]
    pub message: StructField<'ctx, StructValue<'ctx>>,

    #[value_type(i64_type())]
    pub param0: StructField<'ctx, IntValue<'ctx>>,

    #[value_type(i64_type())]
    pub param1: StructField<'ctx, IntValue<'ctx>>,

    #[value_type(i64_type())]
    pub param2: StructField<'ctx, IntValue<'ctx>>,
}

impl<'ctx> ExceptionType<'ctx> {
    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(
        ctx: impl AsContextRef<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> ExceptionStructFields<'ctx> {
        ExceptionStructFields::new(ctx, llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of an `Exception`.
    #[must_use]
    fn llvm_type(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        const NAME: &str = "Exception";

        assert!(ctx.get_struct_type("str").is_some());

        if let Some(t) = ctx.get_struct_type(NAME) {
            t.ptr_type(AddressSpace::default())
        } else {
            let exn_ty = ctx.opaque_struct_type(NAME);
            let field_tys =
                Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();
            exn_ty.set_body(&field_tys, false);
            exn_ty.ptr_type(AddressSpace::default())
        }
    }

    fn new_impl(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_str = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_str, llvm_usize }
    }

    /// Creates an instance of [`ExceptionType`].
    #[must_use]
    pub fn new(ctx: &CodeGenContext<'ctx, '_>) -> Self {
        Self::new_impl(ctx.ctx, ctx.get_size_type())
    }

    /// Creates an instance of [`ExceptionType`].
    #[must_use]
    pub fn new_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
    ) -> Self {
        Self::new_impl(ctx, generator.get_size_type(ctx))
    }

    /// Creates an [`ExceptionType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type
        assert!(
            matches!(&*ctx.unifier.get_ty_immutable(ty), TypeEnum::TObj { obj_id, .. } if *obj_id == ctx.primitives.exception.obj_id(&ctx.unifier).unwrap())
        );

        Self::new_impl(ctx.ctx, ctx.get_size_type())
    }

    /// Creates an [`ExceptionType`] from a [`StructType`] representing an `Exception`.
    #[must_use]
    pub fn from_struct_type(ty: StructType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        Self::from_pointer_type(ty.ptr_type(AddressSpace::default()), llvm_usize)
    }

    /// Creates an [`ExceptionType`] from a [`PointerType`] representing an `Exception`.
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    /// Returns an instance of [`ExceptionType`] by obtaining the LLVM representation of the builtin
    /// `Exception` type.
    #[must_use]
    pub fn get_instance<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Self {
        Self::from_pointer_type(
            ctx.get_llvm_type(generator, ctx.primitives.exception).into_pointer_type(),
            ctx.get_size_type(),
        )
    }

    /// Allocates an instance of [`ExceptionValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca`].
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

    /// Allocates an instance of [`ExceptionValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca_var`].
    #[must_use]
    pub fn alloca_var<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca_var(generator, ctx, name),
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`ExceptionValue`].
    #[must_use]
    pub fn map_struct_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: StructValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_struct_value(
            generator,
            ctx,
            value,
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`ExceptionValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for ExceptionType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = ExceptionValue<'ctx>;

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

        let llvm_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ty) = llvm_ty else {
            return Err(format!("Expected struct type for `list` type, got {llvm_ty}"));
        };

        check_struct_type_matches_fields(Self::fields(ctx, llvm_usize), llvm_ty, "exception", &[])
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

impl<'ctx> StructProxyType<'ctx> for ExceptionType<'ctx> {
    type StructFields = ExceptionStructFields<'ctx>;

    fn get_fields(&self) -> Self::StructFields {
        Self::fields(self.ty.get_context(), self.llvm_usize)
    }
}

impl<'ctx> From<ExceptionType<'ctx>> for PointerType<'ctx> {
    fn from(value: ExceptionType<'ctx>) -> Self {
        value.as_base_type()
    }
}
