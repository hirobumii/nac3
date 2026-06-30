use std::borrow::Cow;

use inkwell::{
    IntPredicate,
    types::BasicTypeEnum,
    values::{IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        AllocationScope, CodeGenContext, ModuleContext,
        types::{
            ProxyTypeBase, RefCountedArrayType, RefCountedArrayValue, TypedRefCountedType,
            TypedRefCountedValue, Value, WithTypeinfo, builtin::BuiltinStruct, field,
            refcounted_fields_for_struct, structure::StructField,
        },
    },
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

#[derive(Clone, Copy, StructFields)]
pub struct ListStructFields<'ctx> {
    /// A pointer to the [reference-counted array][`RefCountedArrayValue`] storing the list content.
    #[value_type(ptr)]
    items: StructField<'ctx, PointerValue<'ctx>>,

    /// Number of items in the array.
    #[value_type(size_t)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

/// Type of a `list`.
#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct RawListType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ListStructFields<'ctx>>,
    pub item_ty: Type,
}

impl<'ctx> RawListType<'ctx> {
    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, item_ty: Type) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "list"), item_ty }
    }

    /// Creates an [`ListType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type and extract `item_type`
        let elem_type = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }

            _ => panic!("Expected `list` type, but got {}", ctx.unifier.stringify(ty)),
        };

        Self::new(ctx, elem_type)
    }
}

impl<'ctx> ListType<'ctx> {
    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn create(ctx: &ModuleContext<'ctx>, item_ty: Type) -> Self {
        Self::new(ctx, RawListType::new(ctx, item_ty))
    }

    /// Creates an [`ListType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type and extract `item_type`
        let elem_type = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }

            _ => panic!("Expected `list` type, but got {}", ctx.unifier.stringify(ty)),
        };

        Self::create(ctx, elem_type)
    }

    /// Allocates a new list with the given length.
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<ListValue<'ctx>> {
        let list = self.allocate(ctx, AllocationScope::Default, name)?;

        let len = ctx.builder.build_int_z_extend(len, ctx.size_t, "")?;
        list.inner_value(ctx)?.store(ctx, field!(len), len)?;

        let len_eqz =
            ctx.builder.build_int_compare(IntPredicate::EQ, len, ctx.size_t.const_zero(), "")?;
        let null = ctx.ptr.const_null();

        let data =
            if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(self.object.item_ty) {
                // Generate a runtime assertion if allocating a non-empty list with unknown element type
                if ctx.registry.codegen_options.debug {
                    ctx.make_assert(
                        len_eqz,
                        "0:AssertionError",
                        "Cannot allocate a non-empty list with unknown element type",
                        [None, None, None],
                    )?;
                }
                null
            } else {
                let ty = ctx.get_llvm_type(self.object.item_ty);
                let array = RefCountedArrayType::new(ctx, ty, None).allocate(ctx, len, None)?.value;
                ctx.builder.build_select(len_eqz, null, array, "")?.into_pointer_value()
            };

        list.inner_value(ctx)?.store(ctx, field!(items), data)?;
        Ok(list)
    }
}

impl<'ctx> WithTypeinfo<'ctx> for RawListType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_list")
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        refcounted_fields_for_struct(ctx, vec![ctx.i32.const_zero()])
    }
}

pub type ListType<'ctx> = TypedRefCountedType<'ctx, RawListType<'ctx>>;
pub type RawListValue<'ctx> = Value<'ctx, RawListType<'ctx>>;
pub type ListValue<'ctx> = TypedRefCountedValue<'ctx, RawListType<'ctx>>;

impl<'ctx> RawListValue<'ctx> {
    /// Returns the data of this list as an [`ArraySliceValue`].
    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, BasicTypeEnum<'ctx>>> {
        let item_ty = if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(self.ty.item_ty)
        {
            // Use a placeholder type.
            ctx.i8.into()
        } else {
            ctx.get_llvm_type(self.ty.item_ty)
        };

        Ok(RefCountedArrayType::new(ctx, item_ty, None)
            .map_value(self.load(ctx, field!(items))?, self.name))
    }

    /// Creates an empty list with the given item type.
    ///
    /// This is special because `item_ty` can be anything, including completely
    /// unbounded type variables.
    pub fn new_empty(
        ctx: &mut CodeGenContext<'ctx, '_>,
        item_ty: Type,
        name: Option<&'static str>,
    ) -> anyhow::Result<ListValue<'ctx>> {
        let list_ty = ListType::create(ctx, item_ty);
        list_ty.construct(ctx, ctx.size_t.const_zero(), name)
    }
}
