use std::borrow::Cow;

use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        AllocationScope, CodeGenContext, ModuleContext, typed_load, typed_store,
        types::{
            OpaqueRefCountedType, OpaqueRefCountedValue, ProxyType, ProxyTypeBase,
            RefCountedArrayType, RefCountedArrayValue, RefCountedType, RefCountedValue,
            TypedRefCountedType, TypedRefCountedValue, Value, WithTypeinfo,
            builtin::BuiltinStruct,
            field,
            reference::{ObjectHeaderType, ObjectHeaderValue},
            structure::StructField,
        },
    },
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

/// The heap-allocated `__nac3_some` payload of an `Option[T]`.
///
/// Layout: `{ ObjectHeader, { usize, T[1] } }` — reuses [`RefCountedArrayType`] with
/// `static_size = 1`. The `usize` field holds the element count for IRRT refcount traversal.
///
/// - `None` is represented by a null `some_ptr` in the enclosing [`RawOptionType`].
/// - `Some(val)` allocates one of these on the heap (`refcount = 1`) and stores `val` in `T[1]`.
#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.alloca_ty(ctx))]
pub struct OptionSomeType<'ctx> {
    inner: RefCountedArrayType<'ctx, BasicTypeEnum<'ctx>>,
}

impl<'ctx> OptionSomeType<'ctx> {
    /// Creates an instance of [`OptionSomeType`] for elements with LLVM type `elem_llvm_ty`.
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, elem_llvm_ty: BasicTypeEnum<'ctx>) -> Self {
        Self { inner: RefCountedArrayType::new(ctx, elem_llvm_ty, Some(1)) }
    }

    /// Heap-allocates a new `__nac3_some` payload and returns it as an [`OptionSomeValue`].
    ///
    /// This initializes the [`ObjectHeader`][ObjectHeaderType] and stores the element count.
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
        RefCountedArrayType::new(ctx, self.ty.inner.elem, None).map_value(self.value, self.name)
    }

    /// Loads the element stored in this `__nac3_some` payload.
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let arr_data = self.as_array(ctx).inner_value(ctx)?;
        let elem_llvm_ty = self.ty.inner.elem.llvm_ty(ctx);
        typed_load(ctx.builder, arr_data.value.0, elem_llvm_ty, name.unwrap_or(""))
    }

    /// Stores `val` into this `__nac3_some` payload.
    pub fn set(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        let arr_data = self.as_array(ctx).inner_value(ctx)?;
        typed_store(ctx.builder, arr_data.value.0, val)?;
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

    /// Returns a pointer to the element `T` stored in this `__nac3_some` payload.
    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>> {
        Ok(self.as_array(ctx).inner_value(ctx)?.value.0)
    }
}

#[derive(Clone, Copy, StructFields)]
pub struct OptionStructFields<'ctx> {
    /// Pointer to the `__nac3_some` payload (null = None).
    #[value_type(ptr)]
    pub some_ptr: StructField<'ctx, PointerValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct RawOptionType<'ctx> {
    pub inner: BuiltinStruct<'ctx, OptionStructFields<'ctx>>,
    pub elem_ty: Type,
}

impl<'ctx> RawOptionType<'ctx> {
    /// Creates an instance of [`RawOptionType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, elem_ty: Type) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "__nac3_option_inner"), elem_ty }
    }

    /// Creates a [`RawOptionType`] from a [unifier type][Type].
    ///
    /// Panics if `ty` is not an Option type.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let elem_type = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }

            _ => panic!("Expected `option` type, but got {}", ctx.unifier.stringify(ty)),
        };
        Self::new(ctx, elem_type)
    }
}

impl<'ctx> WithTypeinfo<'ctx> for RawOptionType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_option")
    }

    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        vec![ctx.i32.const_zero()]
    }
}

pub type OptionType<'ctx> = TypedRefCountedType<'ctx, RawOptionType<'ctx>>;
pub type RawOptionValue<'ctx> = Value<'ctx, RawOptionType<'ctx>>;
pub type OptionValue<'ctx> = TypedRefCountedValue<'ctx, RawOptionType<'ctx>>;

impl<'ctx> RawOptionValue<'ctx> {
    /// Loads the `some_ptr` field and wraps it as an [`OptionSomeValue`].
    ///
    /// The caller must ensure the option [contains a value][OptionValue::is_some] before
    /// calling methods that dereference the returned value.
    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<OptionSomeValue<'ctx>> {
        let some_ptr = self.load(ctx, field!(some_ptr))?;
        let elem_llvm_ty = ctx.get_llvm_type(self.ty.elem_ty);
        Ok(OptionSomeType::new(ctx, elem_llvm_ty).map_value(some_ptr, self.name))
    }
}

impl<'ctx> OptionType<'ctx> {
    /// Creates an instance of [`OptionType`].
    #[must_use]
    pub fn create(ctx: &ModuleContext<'ctx>, elem_ty: Type) -> Self {
        Self::new(ctx, RawOptionType::new(ctx, elem_ty))
    }

    /// Creates an [`OptionType`] from a [unifier type][Type].
    ///
    /// Panics if `ty` is not an Option type.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let raw = RawOptionType::from_unifier_type(ctx, ty);
        Self::new(ctx, raw)
    }

    /// Constructs a runtime optional value from an optional `BasicValueEnum`.
    ///
    /// The outer [`OptionValue`] is stack-allocated with `refcount = 0`. If `value` is `Some`,
    /// a `__nac3_some` payload is heap-allocated (refcounted) and linked via the `some_ptr` field.
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: Option<BasicValueEnum<'ctx>>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<OptionValue<'ctx>> {
        // Stack-allocate the outer OptionValue with refcount=0.
        let option = self.allocate(ctx, AllocationScope::StackCurrentLoc, name)?;
        let inner = option.inner_value(ctx)?;

        let some_ptr = match value {
            Some(val) => {
                let elem_llvm_ty = ctx.get_llvm_type(self.object.elem_ty);
                let some = OptionSomeType::new(ctx, elem_llvm_ty).allocate(ctx, None)?;
                some.set(ctx, val)?;
                some.value
            }
            None => ctx.ptr.const_null(),
        };

        inner.store(ctx, field!(some_ptr), some_ptr)?;
        Ok(option)
    }
}

impl<'ctx> OptionValue<'ctx> {
    /// Returns whether this `Option` instance contains a value.
    pub fn is_some(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        let some_data = self.inner_value(ctx)?.data(ctx)?;
        Ok(ctx.builder.build_is_not_null(some_data.value, "")?)
    }

    /// Loads the value present in this `Option` instance.
    ///
    /// The caller must ensure that this `option` value [contains a value][Self::is_some].
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        self.inner_value(ctx)?.data(ctx)?.get(ctx, name)
    }
}
