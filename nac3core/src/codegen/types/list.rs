use inkwell::{
    IntPredicate,
    values::{IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        AllocationScope, CodeGenContext, ModuleContext,
        types::{
            TypedRefCountedType, TypedRefCountedValue, TypeinfoValue, Value, WithTypeinfo,
            array::ArraySliceValue, builtin::BuiltinStruct, field, structure::StructField,
        },
    },
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

#[derive(Clone, Copy, StructFields)]
pub struct ListStructFields<'ctx> {
    /// Array pointer to content.
    #[value_type(ptr)]
    pub items: StructField<'ctx, PointerValue<'ctx>>,

    /// Number of items in the array.
    #[value_type(size_t)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct ListType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ListStructFields<'ctx>>,
    pub item_ty: Type,
}

impl<'ctx> ListType<'ctx> {
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

    /// Allocates a new list with the given length.
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<TypedRefCountedValue<'ctx, Self>> {
        let list = TypedRefCountedType::new(ctx, *self).allocate(
            ctx,
            AllocationScope::Default,
            true,
            name,
        )?;

        let len = ctx.builder.build_int_z_extend(len, ctx.size_t, "")?;
        list.inner_value().store(ctx, field!(len), len)?;

        let len_eqz =
            ctx.builder.build_int_compare(IntPredicate::EQ, len, ctx.size_t.const_zero(), "")?;
        let null = ctx.ptr.const_null();

        let data = if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(self.item_ty) {
            // Generate a runtime assertion if allocating a non-empty list with unknown element type
            if ctx.registry.codegen_options.debug {
                ctx.make_assert(
                    len_eqz,
                    "0:AssertionError",
                    "Cannot allocate a non-empty list with unknown element type",
                    [None, None, None],
                    ctx.current_loc,
                )?;
            }
            null
        } else {
            let ty = ctx.get_llvm_type(self.item_ty);
            let array =
                ctx.build_dyn_array_allocate(AllocationScope::Default, ty, len, None)?.value.0;
            ctx.builder.build_select(len_eqz, null, array, "")?.into_pointer_value()
        };

        list.inner_value().store(ctx, field!(items), data)?;
        Ok(list)
    }
}

impl<'ctx> WithTypeinfo<'ctx> for ListType<'ctx> {
    fn typeinfo(ctx: &ModuleContext<'ctx>) -> TypeinfoValue<'ctx> {
        todo!()
    }
}

pub type ListValue<'ctx> = Value<'ctx, ListType<'ctx>>;

impl<'ctx> ListValue<'ctx> {
    /// Returns the data of this list as an [`ArraySliceValue`].
    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        let item_ty = if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(self.ty.item_ty)
        {
            // Use a placeholder type.
            ctx.i8.into()
        } else {
            ctx.get_llvm_type(self.ty.item_ty)
        };

        Ok(ArraySliceValue::new(
            item_ty,
            self.load(ctx, field!(items))?,
            self.load(ctx, field!(len))?,
            self.name,
        ))
    }

    /// Creates an empty list with the given item type.
    ///
    /// This is special because `item_ty` can be anything, including completely
    /// unbounded type variables.
    pub fn new_empty(
        ctx: &mut CodeGenContext<'ctx, '_>,
        item_ty: Type,
        name: Option<&'static str>,
    ) -> anyhow::Result<TypedRefCountedValue<'ctx, ListType<'ctx>>> {
        let list_ty = ListType::new(ctx, item_ty);
        list_ty.construct(ctx, ctx.size_t.const_zero(), name)
    }
}
