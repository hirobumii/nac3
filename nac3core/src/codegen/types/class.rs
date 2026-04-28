use std::borrow::Cow;

use inkwell::{
    types::{BasicTypeEnum, StructType},
    values::{IntValue, PointerValue},
};
use itertools::Itertools as _;
use nac3parser::ast::StrRef;

use crate::{
    codegen::{
        CodeGenContext,
        types::{
            ModuleContext, ProxyType, ProxyTypeBase, RefType, TypedRefCountedType,
            TypedRefCountedValue, Value, WithTypeinfo, reference::is_refcounted_type,
        },
    },
    toplevel::{DefinitionId, TopLevelDef},
    typecheck::typedef::{Type, TypeEnum},
};

/// The raw inner type of a user-defined class.
///
/// Stores the LLVM struct type for the class fields and precomputed refcount metadata.
#[derive(Clone, Copy)]
pub struct RawClassType<'ctx> {
    /// The LLVM inner struct type: `{field0, field1, ...}`
    inner: StructType<'ctx>,

    /// The class definition ID.
    class_id: DefinitionId,

    /// Bitmask of which fields are refcounted pointer types (bit `i` = 1).
    /// Supports up to 64 fields.
    refcounted_mask: u64,

    /// The name of the class type.
    name: StrRef,
}

// Manual ProxyTypeBase/ProxyType/RefType implementations.
// Classes are passed by pointer (like List/Option/NDArray).

impl<'ctx> ProxyTypeBase<'ctx> for RawClassType<'ctx> {
    type Value = PointerValue<'ctx>;
}

impl<'ctx> ProxyType<'ctx> for RawClassType<'ctx> {
    fn llvm_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        ctx.ptr.into()
    }
}

impl<'ctx> RefType<'ctx> for RawClassType<'ctx> {
    fn alloca_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.inner.into()
    }
}

impl<'ctx> RawClassType<'ctx> {
    /// Creates an instance of [`RawClassType`] from precomputed components.
    #[must_use]
    pub const fn new(
        inner: StructType<'ctx>,
        class_id: DefinitionId,
        refcounted_mask: u64,
        class_name: StrRef,
    ) -> Self {
        Self { inner, class_id, refcounted_mask, name: class_name }
    }

    /// Creates a [`RawClassType`] from a [unifier type][Type].
    ///
    /// Resolves the class definition, builds the LLVM struct type for its fields,
    /// and precomputes the refcounted field mask.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let TypeEnum::TObj { obj_id, fields, .. } = &*ctx.unifier.get_ty(ty) else {
            panic!("Expected TObj, got {}", ctx.unifier.stringify(ty));
        };
        let obj_id = *obj_id;
        let fields = fields.clone();

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
        let struct_type = if let Some(t) = ctx.module.get_struct_type(&name) {
            t
        } else {
            let struct_type = ctx.ctx.opaque_struct_type(&name);
            let llvm_fields =
                fields_list.iter().map(|f| ctx.get_llvm_type(fields[&f.0].0)).collect_vec();
            struct_type.set_body(&llvm_fields, false);
            struct_type
        };

        let mut mask = 0u64;
        for (i, f) in fields_list.iter().enumerate() {
            if is_refcounted_type(&mut ctx.unifier, fields[&f.0].0) {
                mask |= 1 << i;
            }
        }

        Self::new(struct_type, obj_id, mask, class_name)
    }

    /// Returns the inner struct type (class fields without `ObjectHeader`).
    #[must_use]
    pub const fn inner_type(&self) -> StructType<'ctx> {
        self.inner
    }

    /// Returns the class [`DefinitionId`].
    #[must_use]
    pub const fn class_id(&self) -> DefinitionId {
        self.class_id
    }
}

pub type ClassType<'ctx> = TypedRefCountedType<'ctx, RawClassType<'ctx>>;

impl<'ctx> ClassType<'ctx> {
    /// Creates an instance of [`ClassType`].
    #[must_use]
    pub fn create(
        ctx: &ModuleContext<'ctx>,
        inner: StructType<'ctx>,
        class_id: DefinitionId,
        refcounted_mask: u64,
        class_name: StrRef,
    ) -> Self {
        Self::new(ctx, RawClassType::new(inner, class_id, refcounted_mask, class_name))
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
        // Include class name and struct layout to distinguish monomorphized generic classes.
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

    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        if self.refcounted_mask == 0 {
            return Vec::new();
        }

        let data_layout = ctx.target.get_target_data();
        let num_fields = self.inner.count_fields();

        let mut offsets = Vec::new();
        for i in 0..num_fields {
            if self.refcounted_mask & (1 << i) != 0 {
                let offset = data_layout.offset_of_element(&self.inner, i).unwrap();
                offsets.push(ctx.i32.const_int(offset, false));
            }
        }

        offsets
    }
}

// TODO(Derppening): RawClassValue and ClassValue seems to be unused
pub type RawClassValue<'ctx> = Value<'ctx, RawClassType<'ctx>>;
pub type ClassValue<'ctx> = TypedRefCountedValue<'ctx, RawClassType<'ctx>>;
