use inkwell::values::{BasicValueEnum, IntValue};
use nac3core_derive::ProxyType;

use crate::{
    codegen::{
        CodeGenContext, ModuleContext, typed_load, typed_store,
        types::{ProxyTypeBase, RefType, Value},
    },
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(ctx.get_llvm_type(self.elem_ty))]
pub struct OptionType {
    elem_ty: Type,
}

impl OptionType {
    /// Creates an instance of [`OptionType`].
    pub const fn new(_ctx: &ModuleContext<'_>, element_type: Type) -> Self {
        Self { elem_ty: element_type }
    }

    /// Decodes a [`Type`] into an [`OptionType`].
    ///
    /// Panics if `ty` is not an Option type.
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'_, '_>, ty: Type) -> Self {
        // Check unifier type and extract `element_type`
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

    /// Constructs a runtime optional value from an optional `BasicValueEnum`.
    pub fn construct<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: Option<BasicValueEnum<'ctx>>,
        name: Option<&'static str>,
    ) -> anyhow::Result<OptionValue<'ctx>> {
        match value {
            Some(v) => {
                let value = self.alloca(ctx, name)?;
                typed_store(ctx.builder, value.value, v)?;
                Ok(value)
            }
            None => Ok(self.map_value(ctx.ptr.const_null(), name)),
        }
    }
}

pub type OptionValue<'ctx> = Value<'ctx, OptionType>;

impl<'ctx> OptionValue<'ctx> {
    /// Returns whether this `Option` instance contains a value.
    pub fn is_some(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        Ok(ctx.builder.build_is_not_null(self.value, "")?)
    }

    /// Loads the value present in this `Option` instance.
    ///
    /// The caller must ensure that this `option` value [contains a value][Self::is_some].
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let ty = self.ty.alloca_ty(ctx);
        typed_load(ctx.builder, self.value, ty, name.or(self.name).unwrap_or(""))
    }
}
