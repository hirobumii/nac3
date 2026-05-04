use std::borrow::Cow;

use inkwell::{types::ArrayType, values::IntValue};
use nac3core_derive::ProxyType;

use crate::{
    codegen::{
        CodeGenContext, ModuleContext,
        types::{
            Value, WithTypeinfo,
            array::{ArrayLikeIndexer as _, ArraySliceValue},
            refcounted_fields_for_struct,
        },
    },
    toplevel::helper::PrimDef,
    typecheck::typedef::{Type, TypeEnum},
};

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.llvm_ty)]
pub struct RangeType<'ctx> {
    llvm_ty: ArrayType<'ctx>,
}

impl<'ctx> WithTypeinfo<'ctx> for RangeType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_range")
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        refcounted_fields_for_struct(ctx, Vec::new())
    }
}

impl<'ctx> RangeType<'ctx> {
    /// Creates an instance of [`RangeType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        // typedef int32_t Range[3];
        Self { llvm_ty: ctx.i32.array_type(3) }
    }

    /// Creates an [`RangeType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type
        assert!(
            matches!(&*ctx.unifier.get_ty_immutable(ty), TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Range.id()),
        );

        Self::new(ctx)
    }
}

pub type RangeValue<'ctx> = Value<'ctx, RangeType<'ctx>>;

impl<'ctx> RangeValue<'ctx> {
    #[must_use]
    fn field_ptr(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        field: RangeField,
    ) -> (Option<String>, ArraySliceValue<'ctx>, IntValue<'ctx>) {
        let arr =
            ArraySliceValue::new(ctx.i32.into(), self.value, ctx.size_t.const_int(3, false), None);
        let idx = ctx.i32.const_int(field as _, false);
        let name = self.name.map(|v| format!("{v}.{}", field.name()));
        (name, arr, idx)
    }

    /// Loads the value of the specified `Range` field.
    ///
    /// Due to a slightly different internal representation, the [`Value::load`] method cannot
    /// be used directly here. Instead, use the [`RangeField`] enum to specify which field to load.
    pub fn load_field(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        field: RangeField,
    ) -> anyhow::Result<IntValue<'ctx>> {
        let (name, arr, idx) = self.field_ptr(ctx, field);
        arr.get_unchecked(ctx, &idx, name.as_deref())
    }

    /// Stores the value into the specified `Range` field.
    ///
    /// Due to a slightly different internal representation, the [`Value::store`] method cannot
    /// be used directly here. Instead, use the [`RangeField`] enum to specify which field to store.
    pub fn store_field(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        field: RangeField,
        value: IntValue<'ctx>,
    ) -> anyhow::Result<()> {
        let (name, arr, idx) = self.field_ptr(ctx, field);
        arr.set_unchecked(ctx, &idx, value, name.as_deref())
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum RangeField {
    Start = 0,
    End = 1,
    Step = 2,
}

impl RangeField {
    const fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Step => "step",
        }
    }
}
