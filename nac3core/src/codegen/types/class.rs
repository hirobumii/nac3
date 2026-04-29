use std::borrow::Cow;

use inkwell::{types::StructType, values::IntValue};
use itertools::Itertools as _;
use nac3core_derive::ProxyType;
use nac3parser::ast::StrRef;

use crate::{
    codegen::{
        CodeGenContext,
        types::{
            ModuleContext, TypedRefCountedType, TypedRefCountedValue, Value, WithTypeinfo,
            reference::is_refcounted_type,
        },
    },
    toplevel::TopLevelDef,
    typecheck::typedef::{Type, TypeEnum},
};

/// The raw inner type of a user-defined class.
///
/// Stores the LLVM struct type for the class fields and precomputed refcount metadata.
#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner)]
pub struct RawClassType<'ctx> {
    /// The LLVM inner struct type: `{field0, field1, ...}`
    inner: StructType<'ctx>,

    /// The [unifier type][Type] of this class.
    unifier_ty: Type,

    /// The name of the class type.
    name: StrRef,
}

impl<'ctx> RawClassType<'ctx> {
    /// Creates an instance of [`RawClassType`] from precomputed components.
    #[must_use]
    pub const fn new(inner: StructType<'ctx>, unifier_ty: Type, class_name: StrRef) -> Self {
        Self { inner, unifier_ty, name: class_name }
    }

    /// Creates a [`RawClassType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let TypeEnum::TObj { obj_id, fields, .. } = &*ctx.unifier.get_ty(ty) else {
            panic!("Expected TObj, got {}", ctx.unifier.stringify(ty));
        };

        let (class_name, fields_list) = {
            let top_level_defs = ctx.top_level.definitions.read();
            let TopLevelDef::Class { name, fields: fields_list, .. } =
                &*top_level_defs[obj_id.0].read()
            else {
                unreachable!()
            };
            (*name, fields_list.clone())
        };

        // Build the LLVM struct type and compute refcounted mask
        let name = ctx.unifier.stringify(ty);
        let struct_type = ctx.module.get_struct_type(&name).unwrap_or_else(|| {
            let struct_type = ctx.ctx.opaque_struct_type(&name);
            let llvm_fields =
                fields_list.iter().map(|f| ctx.get_llvm_type(fields[&f.0].0)).collect_vec();
            struct_type.set_body(&llvm_fields, false);
            struct_type
        });

        Self::new(struct_type, ty, class_name)
    }

    /// Returns the inner struct type.
    #[must_use]
    pub const fn inner_type(&self) -> StructType<'ctx> {
        self.inner
    }
}

pub type ClassType<'ctx> = TypedRefCountedType<'ctx, RawClassType<'ctx>>;

impl<'ctx> ClassType<'ctx> {
    /// Creates an instance of [`ClassType`].
    #[must_use]
    pub fn create(
        ctx: &ModuleContext<'ctx>,
        inner: StructType<'ctx>,
        unifier_ty: Type,
        class_name: StrRef,
    ) -> Self {
        Self::new(ctx, RawClassType::new(inner, unifier_ty, class_name))
    }

    /// Creates a [`ClassType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let raw = RawClassType::from_unifier_type(ctx, ty);
        Self::new(ctx, raw)
    }
}

impl<'ctx> WithTypeinfo<'ctx> for RawClassType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        // Include class name and struct layout to distinguish monomorphized generic classes
        let struct_str = self.inner.print_to_string();
        let sanitized: String = struct_str
            .to_str()
            .unwrap_or_else(|_| {
                panic!("Failed to convert struct type to string: {}", struct_str.to_string_lossy())
            })
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        Cow::Owned(format!("{}_{sanitized}", self.name))
    }

    fn refcounted_field_offset(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        let mut offsets = Vec::new();

        let TypeEnum::TObj { obj_id, fields, .. } = &*ctx.unifier.get_ty(self.unifier_ty) else {
            panic!("Expected TObj, got {}", ctx.unifier.stringify(self.unifier_ty));
        };

        let fields_list = {
            let top_level_defs = ctx.top_level.definitions.read();
            let TopLevelDef::Class { fields: fields_list, .. } = &*top_level_defs[obj_id.0].read()
            else {
                unreachable!()
            };
            fields_list.clone()
        };

        for (i, f) in fields_list.iter().enumerate() {
            if is_refcounted_type(&mut ctx.unifier, fields[&f.0].0) {
                let offset =
                    ctx.target.get_target_data().offset_of_element(&self.inner, i as u32).unwrap();
                offsets.push(ctx.i32.const_int(offset, false));
            }
        }

        offsets
    }
}

// TODO(Derppening): RawClassValue and ClassValue seems to be unused
pub type RawClassValue<'ctx> = Value<'ctx, RawClassType<'ctx>>;
pub type ClassValue<'ctx> = TypedRefCountedValue<'ctx, RawClassType<'ctx>>;
