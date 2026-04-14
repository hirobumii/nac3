use std::marker::PhantomData;

use anyhow::anyhow;
use inkwell::{
    types::BasicTypeEnum,
    values::{AggregateValueEnum, BasicValue, BasicValueEnum, IntValue, PointerValue, StructValue},
};
use itertools::Itertools as _;

use crate::codegen::{CodeGenContext, ModuleContext, typed_load, typed_store};

/// Trait indicating that the structure is a field-wise representation of an LLVM structure.
///
/// # Usage
///
/// For example, for a simple C-slice LLVM structure:
///
/// ```ignore
/// struct CSliceFields<'ctx> {
///     ptr: StructField<'ctx, PointerValue<'ctx>>,
///     len: StructField<'ctx, IntValue<'ctx>>
/// }
/// ```
pub trait StructFields<'ctx>: Copy {
    /// Creates an instance of [`StructFields`] using the given `ctx` and `size_t` types.
    fn new(ctx: &ModuleContext<'ctx>) -> Self;

    /// Returns a [`Vec`] that contains the fields of the structure in the order as they appear in
    /// the type definition.
    #[must_use]
    fn to_vec(&self) -> Vec<(&'static str, BasicTypeEnum<'ctx>)>;

    #[must_use]
    fn field_tys(&self) -> Vec<BasicTypeEnum<'ctx>> {
        self.to_vec().into_iter().map(|(_, ty)| ty).collect()
    }

    /// Returns a [`Iterator`] that contains the fields of the structure in the order as they appear
    /// in the type definition.
    #[must_use]
    fn iter(&self) -> impl Iterator<Item = (&'static str, BasicTypeEnum<'ctx>)> {
        self.to_vec().into_iter()
    }

    /// Returns a [`Vec`] that contains the fields of the structure in the order as they appear in
    /// the type definition.
    #[must_use]
    fn into_vec(self) -> Vec<(&'static str, BasicTypeEnum<'ctx>)>
    where
        Self: Sized,
    {
        self.to_vec()
    }

    /// Returns a [`Iterator`] that contains the fields of the structure in the order as they appear
    /// in the type definition.
    #[must_use]
    fn into_iter(self) -> impl Iterator<Item = (&'static str, BasicTypeEnum<'ctx>)>
    where
        Self: Sized,
    {
        self.into_vec().into_iter()
    }

    /// Returns the field index of a field in this structure.
    fn index_of_field<V>(&self, name: impl FnOnce(&Self) -> StructField<'ctx, V>) -> u32
    where
        V: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error = ()>,
    {
        let field_name = name(self).name;
        self.index_of_field_name(field_name).unwrap()
    }

    /// Returns the field index of a field with the given name in this structure.
    fn index_of_field_name(&self, field_name: &str) -> Option<u32> {
        self.iter().find_position(|(name, _)| *name == field_name).map(|(idx, _)| idx as u32)
    }
}

/// A single field of an LLVM structure.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StructField<'ctx, Value> {
    /// The index of this field within the structure.
    index: u32,

    /// The name of this field.
    name: &'static str,

    /// The type of this field.
    ty: BasicTypeEnum<'ctx>,

    /// Instance of [`PhantomData`] containing [`Value`], used to implement automatic downcasts.
    _value_ty: PhantomData<Value>,
}

impl<'ctx, Value> StructField<'ctx, Value> {
    /// Creates an instance of [`StructField`] with a given index.
    ///
    /// * `index` - The index of this field within its enclosing structure.
    /// * `name` - Name of the field.
    /// * `ty` - The type of this field.
    pub fn create_at(index: u32, name: &'static str, ty: impl Into<BasicTypeEnum<'ctx>>) -> Self {
        StructField { index, name, ty: ty.into(), _value_ty: PhantomData }
    }

    /// Returns the name of this field.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Creates a pointer to this field in an arbitrary structure by performing a `getelementptr i32
    /// {idx...}, i32 {self.index}`.
    pub fn ptr_by_array_gep(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        struct_ty: BasicTypeEnum<'ctx>,
        pobj: PointerValue<'ctx>,
        idx: &[IntValue<'ctx>],
    ) -> anyhow::Result<PointerValue<'ctx>> {
        Ok(unsafe {
            ctx.builder.build_in_bounds_gep(
                struct_ty,
                pobj,
                &[idx, &[ctx.i32.const_int(u64::from(self.index), false)]].concat(),
                "",
            )?
        })
    }

    /// Creates a pointer to this field in an arbitrary structure by performing the equivalent of
    /// `getelementptr i32 0, i32 {self.index}`.
    pub fn ptr_by_gep(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        struct_ty: BasicTypeEnum<'ctx>,
        pobj: PointerValue<'ctx>,
        obj_name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        Ok(ctx.builder.build_struct_gep(
            struct_ty.into_struct_type(),
            pobj,
            self.index,
            &obj_name.map(|name| format!("{name}.{}.addr", self.name)).unwrap_or_default(),
        )?)
    }

    /// Gets the value of this field for a given `obj`.
    pub fn extract_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        obj: StructValue<'ctx>,
    ) -> anyhow::Result<Value>
    where
        Value: TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug>,
    {
        Value::try_from(ctx.builder.build_extract_value(
            obj,
            self.index,
            &format!("{}.{}", obj.get_name().to_str().unwrap(), self.name),
        )?)
        .map_err(|e| anyhow!("{e:?}"))
    }

    /// Sets the value of this field for a given `obj`.
    pub fn insert_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        obj: StructValue<'ctx>,
        value: Value,
    ) -> anyhow::Result<StructValue<'ctx>>
    where
        Value: BasicValue<'ctx>,
    {
        let obj_name = obj.get_name().to_str().unwrap();
        let new_obj_name = if obj_name.chars().all(char::is_numeric) { "" } else { obj_name };

        Ok(ctx
            .builder
            .build_insert_value(obj, value, self.index, new_obj_name)
            .map(AggregateValueEnum::into_struct_value)?)
    }

    /// Loads the value of this field for a pointer-to-structure.
    pub fn load(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        struct_ty: BasicTypeEnum<'ctx>,
        pobj: PointerValue<'ctx>,
        obj_name: Option<&str>,
    ) -> anyhow::Result<Value>
    where
        Value: TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug>,
    {
        typed_load(
            ctx.builder,
            self.ptr_by_gep(ctx, struct_ty, pobj, obj_name)?,
            self.ty,
            &obj_name.map(|name| format!("{name}.{}", self.name)).unwrap_or_default(),
        )?
        .try_into()
        .map_err(|e| anyhow!("{e:?}"))
    }

    /// Stores the value of this field for a pointer-to-structure.
    pub fn store(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        struct_ty: BasicTypeEnum<'ctx>,
        pobj: PointerValue<'ctx>,
        value: Value,
        obj_name: Option<&str>,
    ) -> anyhow::Result<()>
    where
        Value: BasicValue<'ctx>,
    {
        typed_store(ctx.builder, self.ptr_by_gep(ctx, struct_ty, pobj, obj_name)?, value)?;
        Ok(())
    }
}

impl<'ctx, Value> From<StructField<'ctx, Value>> for (&'static str, BasicTypeEnum<'ctx>)
where
    Value: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error = ()>,
{
    fn from(value: StructField<'ctx, Value>) -> Self {
        (value.name, value.ty)
    }
}
