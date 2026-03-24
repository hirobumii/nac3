use std::borrow::Cow;

use inkwell::{
    types::{BasicTypeEnum, StructType},
    values::{BasicValue, BasicValueEnum, IntValue, StructValue},
};
use itertools::Itertools as _;

use crate::{
    codegen::{
        CodeGenContext,
        types::{
            ModuleContext, ProxyType, ProxyTypeBase, RefType, Value, WithTypeinfo,
            reference::{ObjectHeaderType, is_refcounted_type},
        },
    },
    typecheck::typedef::{Type, TypeEnum},
};

/// Represents a tuple type with an [`ObjectHeader`][ObjectHeaderType] prefix.
///
/// The LLVM layout is `{ObjectHeader, {field0, field1, ...}}`, where the outer struct
/// is the full tuple representation and the inner struct contains the actual field values.
///
/// The `ObjectHeader` has `refcount = 0` (tuples are not themselves refcounted), but provides
/// typeinfo so the IRRT can walk refcounted children when tuples are stored inline in
/// containers (e.g., `list[tuple[...]]`).
#[derive(Clone, Copy)]
pub struct TupleType<'ctx> {
    /// The outer struct type: `{ObjectHeader, {field0, field1, ...}}`
    outer: StructType<'ctx>,

    /// The inner fields struct type: `{field0, field1, ...}`
    fields: StructType<'ctx>,

    /// Bitmask of which elements are refcounted (bit `i` = 1 if element `i` is refcounted).
    /// Supports up to 64 elements.
    refcounted_mask: u64,
}

// Manual ProxyTypeBase/ProxyType implementations (cannot use derive macro due to
// the additional fields and the two-level struct layout).

impl<'ctx> ProxyTypeBase<'ctx> for TupleType<'ctx> {
    type Value = StructValue<'ctx>;
}

impl<'ctx> ProxyType<'ctx> for TupleType<'ctx> {
    fn llvm_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.outer.into()
    }
}

impl<'ctx> TupleType<'ctx> {
    /// Creates an instance of [`TupleType`] from LLVM field types.
    ///
    /// The `refcounted_mask` is set to `0` (no refcounted fields assumed).
    /// Use [`from_unifier_type`][Self::from_unifier_type] when unifier type information is
    /// available, which computes the correct mask.
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, field_types: &[BasicTypeEnum<'ctx>]) -> Self {
        let fields = ctx.ctx.struct_type(field_types, false);
        let header_ty = ObjectHeaderType::new(ctx).alloca_ty(ctx).into_struct_type();
        let outer = ctx.ctx.struct_type(&[header_ty.into(), fields.into()], false);
        Self { outer, fields, refcounted_mask: 0 }
    }

    /// Creates a [`TupleType`] from a [unifier type][Type].
    ///
    /// This computes the correct `refcounted_mask` from the element types.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let TypeEnum::TTuple { ty: tys, .. } = &*ctx.unifier.get_ty_immutable(ty) else {
            panic!("Expected type to be a TypeEnum::TTuple, got {}", ctx.unifier.stringify(ty));
        };
        let tys = tys.clone();

        let mut mask = 0u64;
        for (i, t) in tys.iter().enumerate() {
            if is_refcounted_type(&mut ctx.unifier, *t) {
                mask |= 1 << i;
            }
        }

        let llvm_tys = tys.iter().map(|ty| ctx.get_llvm_type(*ty)).collect_vec();
        let mut result = Self::new(ctx, &llvm_tys);
        result.refcounted_mask = mask;
        result
    }

    /// Creates a poison value of this tuple type.
    #[must_use]
    pub fn poison(&self, name: Option<&'static str>) -> TupleValue<'ctx> {
        self.map_value(self.outer.get_poison(), name)
    }

    /// Returns the number of elements in the tuple.
    #[must_use]
    pub fn num_elements(&self) -> u32 {
        self.fields.count_fields()
    }

    /// Returns the inner fields struct type (without the `ObjectHeader`).
    #[must_use]
    pub const fn fields_type(&self) -> StructType<'ctx> {
        self.fields
    }

    /// Returns the bitmask of refcounted elements.
    #[must_use]
    pub const fn refcounted_mask(&self) -> u64 {
        self.refcounted_mask
    }

    /// Initializes the [`ObjectHeader`][ObjectHeaderType] of a tuple stored at the given memory
    /// location.
    ///
    /// The header is initialized with `refcount = 0` (tuples are not refcounted) and a typeinfo
    /// pointer that allows the IRRT to walk refcounted children.
    pub fn init_header(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ptr: inkwell::values::PointerValue<'ctx>,
    ) -> anyhow::Result<()> {
        let header = ObjectHeaderType::new(ctx).map_value(ptr, None);
        header.init(ctx, false, self.typeinfo(ctx))?;
        Ok(())
    }
}

impl<'ctx> WithTypeinfo<'ctx> for TupleType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        // Generate a unique typename based on the inner fields struct layout.
        // This ensures tuples with the same field types share typeinfo globals.
        let struct_str = self.fields.print_to_string();
        let sanitized: String = struct_str
            .to_str()
            .unwrap_or("unknown")
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        Cow::Owned(format!("__nac3_tuple_{sanitized}"))
    }

    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        if self.refcounted_mask == 0 {
            return vec![];
        }

        let data_layout = ctx.target.get_target_data();
        let num_fields = self.fields.count_fields();

        let mut offsets = Vec::new();
        for i in 0..num_fields {
            if self.refcounted_mask & (1 << i) != 0 {
                let offset = data_layout.offset_of_element(&self.fields, i).unwrap();
                offsets.push(ctx.i32.const_int(offset, false));
            }
        }

        offsets
    }
}

pub type TupleValue<'ctx> = Value<'ctx, TupleType<'ctx>>;

impl<'ctx> TupleValue<'ctx> {
    /// Creates a new `TupleValue` directly from the given element values.
    ///
    /// The `ObjectHeader` is left uninitialized (poison). Use
    /// [`TupleType::init_header`] to initialize it when the tuple is stored to memory.
    pub fn new(
        ctx: &mut CodeGenContext<'ctx, '_>,
        values: &[impl BasicValue<'ctx>],
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        let types = values.iter().map(|v| v.as_basic_value_enum().get_type()).collect_vec();
        let tuple_ty = TupleType::new(ctx, &types);

        // Build inner fields struct
        let inner_poison = tuple_ty.fields.get_poison();
        let inner = values.iter().enumerate().try_fold(inner_poison, |acc, (i, v)| {
            Ok::<_, anyhow::Error>(
                ctx.builder
                    .build_insert_value(acc, v.as_basic_value_enum(), i as u32, "")?
                    .into_struct_value(),
            )
        })?;

        // Build outer struct: insert inner fields at index 1 (index 0 is ObjectHeader, left as poison)
        let outer_poison = tuple_ty.outer.get_poison();
        let label = name.unwrap_or("tuple");
        let outer =
            ctx.builder.build_insert_value(outer_poison, inner, 1, label)?.into_struct_value();

        Ok(tuple_ty.map_value(outer, name))
    }

    /// Loads a value from the tuple element at the given `index`.
    pub fn extract(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        index: u32,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        // Extract inner struct (field 1 of outer), then extract the element from it.
        let inner = ctx.builder.build_extract_value(self.value, 1, "tuple.inner")?;
        let name = format!("{}.{}", self.name.unwrap_or("tuple"), index);
        Ok(ctx.builder.build_extract_value(inner.into_struct_value(), index, &name)?)
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
            self.ty.fields.get_field_type_at_index_unchecked(index)
        });
        let name = self.name.unwrap_or_default();

        // Extract inner struct, insert element, put it back into outer
        let inner =
            ctx.builder.build_extract_value(self.value, 1, "tuple.inner")?.into_struct_value();
        let new_inner =
            ctx.builder.build_insert_value(inner, element, index, "")?.into_struct_value();
        let new_outer =
            ctx.builder.build_insert_value(self.value, new_inner, 1, name)?.into_struct_value();
        Ok(self.ty.map_value(new_outer, self.name))
    }
}
