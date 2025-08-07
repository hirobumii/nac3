use inkwell::{
    AddressSpace,
    context::ContextRef,
    types::{AnyTypeEnum, ArrayType, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{ArrayValue, PointerValue},
};

use super::ProxyType;
use crate::{
    codegen::{ModuleContext, CodeGenContext, CodeGenerator, values::RangeValue},
    typecheck::typedef::{Type, TypeEnum},
};

/// Proxy type for a `range` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RangeType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> RangeType<'ctx> {
    /// Creates an LLVM type corresponding to the expected structure of a `Range`.
    #[must_use]
    fn llvm_type(ctx: ContextRef<'ctx>) -> PointerType<'ctx> {
        // typedef int32_t Range[3];
        let llvm_i32 = ctx.i32_type();
        llvm_i32.array_type(3).ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_range = Self::llvm_type(ctx);

        RangeType { ty: llvm_range, llvm_usize }
    }

    /// Creates an instance of [`RangeType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, ctx.size_t)
    }

    /// Creates an [`RangeType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type
        assert!(
            matches!(&*ctx.unifier.get_ty_immutable(ty), TypeEnum::TObj { obj_id, .. } if *obj_id == ctx.primitives.range.obj_id(&ctx.unifier).unwrap())
        );

        Self::new(ctx)
    }

    /// Creates an [`RangeType`] from a [`ArrayType`].
    #[must_use]
    pub fn from_array_type(arr_ty: ArrayType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        Self::from_pointer_type(arr_ty.ptr_type(AddressSpace::default()), llvm_usize)
    }

    /// Creates an [`RangeType`] from a [`PointerType`].
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        RangeType { ty: ptr_ty, llvm_usize }
    }

    /// Returns the type of all fields of this `range` type.
    #[must_use]
    pub fn value_type(&self) -> IntType<'ctx> {
        self.as_abi_type().get_element_type().into_array_type().get_element_type().into_int_type()
    }

    /// Allocates an instance of [`RangeValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca`].
    #[must_use]
    pub fn alloca<G: CodeGenerator + ?Sized>(
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

    /// Allocates an instance of [`RangeValue`] as if by calling `alloca` on the base type.
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

    /// Converts an existing value into a [`RangeValue`].
    #[must_use]
    pub fn map_array_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: ArrayValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_array_value(
            generator,
            ctx,
            value,
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`RangeValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for RangeType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = RangeValue<'ctx>;

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
        let llvm_range_ty = ty.get_element_type();
        let AnyTypeEnum::ArrayType(llvm_range_ty) = llvm_range_ty else {
            return Err(format!("Expected array type for `range` type, got {llvm_range_ty}"));
        };
        if llvm_range_ty.len() != 3 {
            return Err(format!(
                "Expected 3 elements for `range` type, got {}",
                llvm_range_ty.len()
            ));
        }

        let llvm_range_elem_ty = llvm_range_ty.get_element_type();
        let Ok(llvm_range_elem_ty) = IntType::try_from(llvm_range_elem_ty) else {
            return Err(format!(
                "Expected int type for `range` element type, got {llvm_range_elem_ty}"
            ));
        };
        if llvm_range_elem_ty.get_bit_width() != 32 {
            return Err(format!(
                "Expected 32-bit int type for `range` element type, got {}",
                llvm_range_elem_ty.get_bit_width()
            ));
        }

        Ok(())
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

impl<'ctx> From<RangeType<'ctx>> for PointerType<'ctx> {
    fn from(value: RangeType<'ctx>) -> Self {
        value.as_base_type()
    }
}
