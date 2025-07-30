use inkwell::{
    context::ContextRef,
    types::{BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{BasicValueEnum, PointerValue, StructValue},
};
use itertools::Itertools;

use super::ProxyType;
use crate::{
    codegen::{CoreContext, CodeGenContext, values::TupleValue},
    typecheck::typedef::{Type, TypeEnum},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TupleType<'ctx> {
    ty: StructType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> TupleType<'ctx> {
    /// Creates an LLVM type corresponding to the expected structure of a tuple.
    #[must_use]
    fn llvm_type(ctx: ContextRef<'ctx>, tys: &[BasicTypeEnum<'ctx>]) -> StructType<'ctx> {
        ctx.struct_type(tys, false)
    }

    fn new_impl(
        ctx: ContextRef<'ctx>,
        tys: &[BasicTypeEnum<'ctx>],
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        let llvm_tuple = Self::llvm_type(ctx, tys);

        Self { ty: llvm_tuple, llvm_usize }
    }

    /// Creates an instance of [`TupleType`].
    #[must_use]
    pub fn new(ctx: &CoreContext<'ctx>, tys: &[impl BasicType<'ctx>]) -> Self {
        Self::new_impl(
            ctx.ctx,
            &tys.iter().map(BasicType::as_basic_type_enum).collect_vec(),
            ctx.size_t,
        )
    }

    /// Creates an [`TupleType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let llvm_usize = ctx.size_t;

        // Sanity check on object type.
        let TypeEnum::TTuple { ty: tys, .. } = &*ctx.unifier.get_ty_immutable(ty) else {
            panic!("Expected type to be a TypeEnum::TTuple, got {}", ctx.unifier.stringify(ty));
        };

        let llvm_tys = tys.iter().map(|ty| ctx.get_llvm_type(*ty)).collect_vec();
        Self { ty: Self::llvm_type(ctx.ctx, &llvm_tys), llvm_usize }
    }

    /// Creates an [`TupleType`] from a [`StructType`].
    #[must_use]
    pub fn from_struct_type(struct_ty: StructType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(struct_ty, llvm_usize).is_ok());

        TupleType { ty: struct_ty, llvm_usize }
    }

    /// Creates an [`TupleType`] from a [`PointerType`].
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        Self::from_struct_type(ptr_ty.get_element_type().into_struct_type(), llvm_usize)
    }

    /// Returns the number of elements present in this [`TupleType`].
    #[must_use]
    pub fn num_elements(&self) -> u32 {
        self.ty.count_fields()
    }

    /// Returns the type of the tuple element at the given `index`, or [`None`] if `index` is out of
    /// range.
    #[must_use]
    pub fn type_at_index(&self, index: u32) -> Option<BasicTypeEnum<'ctx>> {
        if index < self.num_elements() {
            Some(unsafe { self.type_at_index_unchecked(index) })
        } else {
            None
        }
    }

    /// Returns the type of the tuple element at the given `index`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the index is valid.
    #[must_use]
    pub unsafe fn type_at_index_unchecked(&self, index: u32) -> BasicTypeEnum<'ctx> {
        unsafe { self.ty.get_field_type_at_index_unchecked(index) }
    }

    /// Constructs a [`TupleValue`] from this type by zero-initializing the tuple value.
    #[must_use]
    pub fn construct(&self, name: Option<&'ctx str>) -> <Self as ProxyType<'ctx>>::Value {
        self.map_struct_value(self.as_abi_type().const_zero(), name)
    }

    /// Constructs a [`TupleValue`] from `objects`. The resulting tuple preserves the order of
    /// objects.
    #[must_use]
    pub fn construct_from_objects<I: IntoIterator<Item = BasicValueEnum<'ctx>>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        objects: I,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let values = objects.into_iter().collect_vec();

        assert_eq!(values.len(), self.num_elements() as usize);
        assert!(
            values.iter().enumerate().all(|(i, v)| {
                v.get_type() == unsafe { self.type_at_index_unchecked(i as u32) }
            })
        );

        let mut value = self.construct(name);
        for (i, val) in values.into_iter().enumerate() {
            value.insert_element(ctx, i as u32, val);
        }

        value
    }

    /// Converts an existing value into a [`ListValue`].
    #[must_use]
    pub fn map_struct_value(
        &self,
        value: StructValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_struct_value(value, self.llvm_usize, name)
    }

    /// Converts an existing value into a [`TupleValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(ctx, value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for TupleType<'ctx> {
    type ABI = StructType<'ctx>;
    type Base = StructType<'ctx>;
    type Value = TupleValue<'ctx>;

    fn is_representable(
        llvm_ty: impl BasicType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::StructType(ty) = llvm_ty.as_basic_type_enum() {
            Self::has_same_repr(ty, llvm_usize)
        } else {
            Err(format!("Expected struct type, got {llvm_ty:?}"))
        }
    }

    fn has_same_repr(_: Self::Base, _: IntType<'ctx>) -> Result<(), String> {
        Ok(())
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.as_base_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_abi_type(&self) -> Self::ABI {
        self.as_base_type()
    }
}

impl<'ctx> From<TupleType<'ctx>> for StructType<'ctx> {
    fn from(value: TupleType<'ctx>) -> Self {
        value.as_base_type()
    }
}
