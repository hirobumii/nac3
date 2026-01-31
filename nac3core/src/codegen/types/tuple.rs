use inkwell::{
    types::{BasicTypeEnum, StructType},
    values::{BasicValue, BasicValueEnum, StructValue},
};
use itertools::Itertools as _;
use nac3core_derive::ProxyType;

use crate::{
    codegen::{
        CodeGenContext,
        types::{ModuleContext, ProxyTypeBase, Value},
    },
    typecheck::typedef::{Type, TypeEnum},
};

#[derive(Clone, Copy, ProxyType)]
#[llvm_ty(StructValue<'ctx>, self.inner)]
pub struct TupleType<'ctx> {
    inner: StructType<'ctx>,
}

pub type TupleValue<'ctx> = Value<'ctx, TupleType<'ctx>>;

impl<'ctx> TupleType<'ctx> {
    /// Creates an instance of [`TupleType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, field_types: &[BasicTypeEnum<'ctx>]) -> Self {
        Self { inner: ctx.ctx.struct_type(field_types, false) }
    }

    /// Creates an [`TupleType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Sanity check on object type.
        let TypeEnum::TTuple { ty: tys, .. } = &*ctx.unifier.get_ty_immutable(ty) else {
            panic!("Expected type to be a TypeEnum::TTuple, got {}", ctx.unifier.stringify(ty));
        };

        let llvm_tys = tys.iter().map(|ty| ctx.get_llvm_type(*ty)).collect_vec();
        Self::new(ctx, &llvm_tys)
    }

    /// Creates a poison value of this tuple type.
    #[must_use]
    pub fn poison(&self, name: Option<&'static str>) -> TupleValue<'ctx> {
        self.map_value(self.inner.get_poison(), name)
    }

    /// Returns the number of elements in the tuple.
    #[must_use]
    pub fn num_elements(&self) -> u32 {
        self.inner.count_fields()
    }
}

impl<'ctx> TupleValue<'ctx> {
    /// Creates a new `TupleValue` directly from the given element values.
    pub fn new(
        ctx: &mut CodeGenContext<'ctx, '_>,
        values: &[impl BasicValue<'ctx>],
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        let types = values.iter().map(|v| v.as_basic_value_enum().get_type()).collect_vec();
        let value = TupleType::new(ctx, &types).poison(name);
        values
            .iter()
            .enumerate()
            .try_fold(value, |acc, (i, v)| acc.insert(ctx, i as u32, v.as_basic_value_enum()))
    }

    /// Loads a value from the tuple element at the given `index`.
    pub fn extract(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        index: u32,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let name = format!("{}.{}", self.name.unwrap_or("tuple"), index);
        Ok(ctx.builder.build_extract_value(self.value, index, &name)?)
    }

    /// Stores a value into the tuple element at the given `index`.
    pub fn insert(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        index: u32,
        element: impl BasicValue<'ctx>,
    ) -> anyhow::Result<Self> {
        assert!(index < self.ty.num_elements());
        assert_eq!(element.as_basic_value_enum().get_type(), unsafe {
            self.ty.inner.get_field_type_at_index_unchecked(index)
        });
        let name = self.name.unwrap_or_default();

        let new_value = ctx.builder.build_insert_value(self.value, element, index, name)?;
        Ok(self.ty.map_value(new_value.into_struct_value(), self.name))
    }
}
