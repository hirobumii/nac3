use inkwell::{
    AddressSpace,
    types::{BasicType, BasicTypeEnum, IntType, PointerType},
    values::{BasicValue, BasicValueEnum, PointerValue},
};

use super::ProxyType;
use crate::{
    codegen::{CoreContext, CodeGenContext, CodeGenerator, values::OptionValue},
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

/// Proxy type for an `Option` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct OptionType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> OptionType<'ctx> {
    /// Creates an LLVM type corresponding to the expected structure of an `Option`.
    #[must_use]
    fn llvm_type(element_type: &impl BasicType<'ctx>) -> PointerType<'ctx> {
        element_type.ptr_type(AddressSpace::default())
    }

    fn new_impl(element_type: &impl BasicType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_option = Self::llvm_type(element_type);

        Self { ty: llvm_option, llvm_usize }
    }

    /// Creates an instance of [`OptionType`].
    #[must_use]
    pub fn new(ctx: &CoreContext<'ctx>, element_type: &impl BasicType<'ctx>) -> Self {
        Self::new_impl(element_type, ctx.size_t)
    }

    /// Creates an [`OptionType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type and extract `element_type`
        let elem_type = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }

            _ => panic!("Expected `option` type, but got {}", ctx.unifier.stringify(ty)),
        };

        let llvm_usize = ctx.size_t;
        let llvm_elem_type = ctx.get_llvm_type(elem_type);

        Self::new_impl(&llvm_elem_type, llvm_usize)
    }

    /// Creates an [`OptionType`] from a [`PointerType`].
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    /// Returns the element type of this `Option` type.
    #[must_use]
    pub fn element_type(&self) -> BasicTypeEnum<'ctx> {
        BasicTypeEnum::try_from(self.ty.get_element_type()).unwrap()
    }

    /// Allocates an [`OptionValue`] on the stack.
    ///
    /// The returned value will be `Some(v)` if [`value` contains a value][Option::is_some],
    /// otherwise `none` will be returned.
    #[must_use]
    pub fn construct<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: Option<BasicValueEnum<'ctx>>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let ptr = if let Some(v) = value {
            let pvar = self.raw_alloca_var(generator, ctx, name);
            ctx.builder.build_store(pvar, v).unwrap();
            pvar
        } else {
            self.ty.const_null()
        };

        self.map_pointer_value(ptr, name)
    }
    /// Allocates an [`OptionValue`] on the stack.
    ///
    /// The returned value will always be `none`.
    #[must_use]
    pub fn construct_empty<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        self.construct(generator, ctx, None, name)
    }

    /// Allocates an [`OptionValue`] on the stack.
    ///
    /// The returned value will be set to `Some(value)`.
    #[must_use]
    pub fn construct_some_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: &impl BasicValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        self.construct(generator, ctx, Some(value.as_basic_value_enum()), name)
    }

    /// Converts an existing value into a [`OptionValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for OptionType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = OptionValue<'ctx>;

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

    fn has_same_repr(ty: Self::Base, _: IntType<'ctx>) -> Result<(), String> {
        BasicTypeEnum::try_from(ty.get_element_type())
            .map_err(|()| format!("Expected `ty` to be a BasicTypeEnum, got {ty}"))?;

        Ok(())
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.element_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_abi_type(&self) -> Self::ABI {
        self.as_base_type()
    }
}

impl<'ctx> From<OptionType<'ctx>> for PointerType<'ctx> {
    fn from(value: OptionType<'ctx>) -> Self {
        value.as_base_type()
    }
}
