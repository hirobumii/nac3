use inkwell::{
    context::Context,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::IntValue,
    AddressSpace,
};

use super::ProxyType;
use crate::codegen::{
    values::{ArraySliceValue, ProxyValue, RangeValue},
    {CodeGenContext, CodeGenerator},
};

/// Proxy type for a `range` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RangeType<'ctx> {
    ty: PointerType<'ctx>,
}

impl<'ctx> RangeType<'ctx> {
    /// Checks whether `llvm_ty` represents a `range` type, returning [Err] if it does not.
    pub fn is_representable(llvm_ty: PointerType<'ctx>) -> Result<(), String> {
        let llvm_range_ty = llvm_ty.get_element_type();
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

    /// Creates an instance of [`RangeType`].
    #[must_use]
    pub fn new(ctx: &'ctx Context) -> Self {
        let llvm_i32 = ctx.i32_type();
        let llvm_range = llvm_i32.array_type(3).ptr_type(AddressSpace::default());

        RangeType::from_type(llvm_range)
    }

    /// Creates an [`RangeType`] from a [`PointerType`].
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>) -> Self {
        debug_assert!(Self::is_representable(ptr_ty).is_ok());

        RangeType { ty: ptr_ty }
    }

    /// Returns the type of all fields of this `range` type.
    #[must_use]
    pub fn value_type(&self) -> IntType<'ctx> {
        self.as_base_type().get_element_type().into_array_type().get_element_type().into_int_type()
    }
}

impl<'ctx> ProxyType<'ctx> for RangeType<'ctx> {
    type Base = PointerType<'ctx>;
    type Value = RangeValue<'ctx>;

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
        _: &G,
        _: &'ctx Context,
        llvm_ty: Self::Base,
    ) -> Result<(), String> {
        Self::is_representable(llvm_ty)
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
        debug_assert_eq!(value.get_type(), self.as_base_type());

        RangeValue::from_pointer_value(value, name)
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }
}

impl<'ctx> From<RangeType<'ctx>> for PointerType<'ctx> {
    fn from(value: RangeType<'ctx>) -> Self {
        value.as_base_type()
    }
}
