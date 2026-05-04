use std::borrow::Cow;

use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::ProxyType;

use crate::{
    codegen::{
        CodeGenContext, ModuleContext,
        types::{
            OpaqueRefCountedType, OpaqueRefCountedValue, ProxyType, ProxyTypeBase,
            RefCountedArrayType, RefCountedArrayValue, RefCountedType, RefCountedValue, Value,
            WithTypeinfo,
            reference::{ObjectHeaderType, ObjectHeaderValue},
        },
    },
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

/// The heap-allocated payload of an `Option[T]`, modeled as a single-element refcounted array.
///
/// Layout: `{ ObjectHeader, { SizeT count, T[1] } }` — reuses [`RefCountedArrayType`] with
/// `static_size = 1`.
///
/// The `SizeT count` field doubles as a refcount-walk indicator:
/// - Pointer (refcounted) elements: count = 1 → IRRT walks the element via `REFCOUNT_ARRAY_MAGIC`
/// - Non-pointer elements: count = 0 → IRRT skips (no refcounted children)
#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.alloca_ty(ctx))]
pub struct OptionSomeType<'ctx> {
    inner: RefCountedArrayType<'ctx, BasicTypeEnum<'ctx>>,
}

impl<'ctx> WithTypeinfo<'ctx> for OptionSomeType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        self.inner.typename()
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        self.inner.refcounted_fields_data(ctx)
    }
}

impl<'ctx> OptionSomeType<'ctx> {
    /// Creates an instance of [`OptionSomeType`] for elements with LLVM type `elem_llvm_ty`.
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, elem_llvm_ty: BasicTypeEnum<'ctx>) -> Self {
        Self { inner: RefCountedArrayType::new(ctx, elem_llvm_ty, Some(1)) }
    }

    /// Heap-allocates a new `__nac3_some` payload and returns it as an [`OptionSomeValue`].
    ///
    /// This initializes the [`ObjectHeader`][ObjectHeaderType] with `refcount = 1` and stores
    /// the element count.
    pub fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<OptionSomeValue<'ctx>> {
        let arr = self.inner.allocate(ctx, ctx.size_t.const_int(1, false), name)?;
        Ok(self.map_value(arr.value, name))
    }
}

impl<'ctx> RefCountedType<'ctx> for OptionSomeType<'ctx> {}

pub type OptionSomeValue<'ctx> = Value<'ctx, OptionSomeType<'ctx>>;

impl<'ctx> OptionSomeValue<'ctx> {
    /// Returns a view of this value as the underlying [`RefCountedArrayValue`].
    fn as_array(
        &self,
        ctx: &ModuleContext<'ctx>,
    ) -> RefCountedArrayValue<'ctx, BasicTypeEnum<'ctx>> {
        RefCountedArrayType::new(ctx, self.ty.inner.elem, Some(1)).map_value(self.value, self.name)
    }

    /// Loads the element stored in this `__nac3_some` payload.
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let arr_data = self.as_array(ctx).inner_value(ctx, None)?;
        let elem_llvm_ty = self.ty.inner.elem.llvm_ty(ctx);
        Ok(ctx.builder.build_load(elem_llvm_ty, arr_data.value.0, name.unwrap_or(""))?)
    }

    /// Stores `val` into this `__nac3_some` payload.
    pub fn set(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        let arr_data = self.as_array(ctx).inner_value(ctx, None)?;
        ctx.builder.build_store(arr_data.value.0, val)?;
        Ok(())
    }
}

impl<'ctx> RefCountedValue<'ctx> for OptionSomeValue<'ctx> {
    fn as_opaque(&self, ctx: &ModuleContext<'ctx>) -> OpaqueRefCountedValue<'ctx> {
        OpaqueRefCountedType::new(ctx).map_value(self.value, self.name)
    }

    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx> {
        ObjectHeaderType::new(ctx).map_value(self.value, self.name)
    }

    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>> {
        Ok(self.as_array(ctx).inner_value(ctx, None)?.value.0)
    }
}

// =============================================================================
// OptionType / OptionValue — the public API for Option[T]
// =============================================================================

/// Proxy type for `Option[T]`.
///
/// At runtime, an `Option[T]` value is a nullable `__nac3_some*`:
/// - `None` = null pointer (no heap allocation)
/// - `Some(val)` = pointer to a heap-allocated [`OptionSomeType`] with `refcount = 1`
#[derive(Clone, Copy)]
pub struct OptionType<'ctx> {
    some_ty: OptionSomeType<'ctx>,
    pub elem_ty: Type,
}

impl<'ctx> OptionType<'ctx> {
    /// Creates an [`OptionType`] from an element LLVM type and unifier type.
    #[must_use]
    pub fn new(
        ctx: &ModuleContext<'ctx>,
        elem_ty: Type,
        elem_llvm_ty: BasicTypeEnum<'ctx>,
    ) -> Self {
        Self { some_ty: OptionSomeType::new(ctx, elem_llvm_ty), elem_ty }
    }

    /// Creates an [`OptionType`] from a [unifier type][Type].
    ///
    /// Panics if `ty` is not an Option type.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let elem_ty = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }
            _ => panic!("Expected `option` type, but got {}", ctx.unifier.stringify(ty)),
        };
        let elem_llvm_ty = ctx.get_llvm_type(elem_ty);
        Self::new(ctx, elem_ty, elem_llvm_ty)
    }

    /// Returns the underlying [`OptionSomeType`].
    #[must_use]
    pub const fn some_ty(&self) -> OptionSomeType<'ctx> {
        self.some_ty
    }

    /// Constructs a runtime optional value.
    ///
    /// - `Some(val)`: Heap-allocates a `__nac3_some` payload with `refcount = 1`.
    /// - `None`: Returns a null pointer (no allocation).
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: Option<BasicValueEnum<'ctx>>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<OptionValue<'ctx>> {
        match value {
            Some(val) => {
                let some = self.some_ty.allocate(ctx, name)?;
                some.set(ctx, val)?;
                Ok(self.map_value(some.value, name))
            }
            None => Ok(self.map_value(ctx.ptr.const_null(), name)),
        }
    }
}

impl<'ctx> ProxyTypeBase<'ctx> for OptionType<'ctx> {
    type Value = PointerValue<'ctx>;

    fn map_value(&self, value: Self::Value, name: Option<&'ctx str>) -> Value<'ctx, Self>
    where
        Self: Sized + Copy,
    {
        Value { ty: *self, value, name }
    }
}

impl<'ctx> ProxyType<'ctx> for OptionType<'ctx> {
    fn llvm_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        OpaqueRefCountedType::new(ctx).llvm_ty(ctx)
    }
}

impl<'ctx> RefCountedType<'ctx> for OptionType<'ctx> {}

/// A runtime `Option[T]` value — a nullable `__nac3_some*` pointer.
pub type OptionValue<'ctx> = Value<'ctx, OptionType<'ctx>>;

impl<'ctx> OptionValue<'ctx> {
    /// Returns whether this option contains a value (`Some`).
    pub fn is_some(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        Ok(ctx.builder.build_is_not_null(self.value, "is_some")?)
    }

    /// Returns this value as an [`OptionSomeValue`] for accessing the payload.
    ///
    /// The caller must ensure that [`is_some`][Self::is_some] is true.
    #[must_use]
    pub fn as_some(&self) -> OptionSomeValue<'ctx> {
        self.ty.some_ty.map_value(self.value, self.name)
    }

    /// Loads the element stored in this `Some` option.
    ///
    /// The caller must ensure that [`is_some`][Self::is_some] is true.
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        self.as_some().get(ctx, name)
    }
}
