use std::marker::PhantomData;

use inkwell::{
    context::AsContextRef,
    types::{BasicTypeEnum, IntType, StructType},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue, StructValue},
};

use crate::codegen::CodeGenContext;

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
pub trait StructFields<'ctx>: Eq + Copy {
    /// Creates an instance of [`StructFields`] using the given `ctx` and `size_t` types.
    fn new(ctx: impl AsContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self;

    /// Returns a [`Vec`] that contains the fields of the structure in the order as they appear in
    /// the type definition.
    #[must_use]
    fn to_vec(&self) -> Vec<(&'static str, BasicTypeEnum<'ctx>)>;

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
}

/// A single field of an LLVM structure.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StructField<'ctx, Value>
where
    Value: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error = ()>,
{
    /// The index of this field within the structure.
    index: u32,

    /// The name of this field.
    name: &'static str,

    /// The type of this field.
    ty: BasicTypeEnum<'ctx>,

    /// Instance of [`PhantomData`] containing [`Value`], used to implement automatic downcasts.
    _value_ty: PhantomData<Value>,
}

impl<'ctx, Value> StructField<'ctx, Value>
where
    Value: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error = ()>,
{
    /// Creates an instance of [`StructField`].
    ///
    /// * `idx_counter` - The instance of [`FieldIndexCounter`] used to track the current field
    ///   index.
    /// * `name` - Name of the field.
    /// * `ty` - The type of this field.
    pub fn create(
        idx_counter: &mut FieldIndexCounter,
        name: &'static str,
        ty: impl Into<BasicTypeEnum<'ctx>>,
    ) -> Self {
        StructField { index: idx_counter.increment(), name, ty: ty.into(), _value_ty: PhantomData }
    }

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
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Creates a pointer to this field in an arbitrary structure by performing a `getelementptr i32
    /// {idx...}, i32 {self.index}`.
    pub fn ptr_by_array_gep(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        pobj: PointerValue<'ctx>,
        idx: &[IntValue<'ctx>],
    ) -> PointerValue<'ctx> {
        unsafe {
            ctx.builder.build_in_bounds_gep(
                pobj,
                &[idx, &[ctx.ctx.i32_type().const_int(u64::from(self.index), false)]].concat(),
                "",
            )
        }
        .unwrap()
    }

    /// Creates a pointer to this field in an arbitrary structure by performing the equivalent of
    /// `getelementptr i32 0, i32 {self.index}`.
    pub fn ptr_by_gep(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        pobj: PointerValue<'ctx>,
        obj_name: Option<&'ctx str>,
    ) -> PointerValue<'ctx> {
        ctx.builder
            .build_struct_gep(
                pobj,
                self.index,
                &obj_name.map(|name| format!("{name}.{}.addr", self.name)).unwrap_or_default(),
            )
            .unwrap()
    }

    /// Gets the value of this field for a given `obj`.
    #[must_use]
    pub fn get_from_value(&self, obj: StructValue<'ctx>) -> Value {
        obj.get_field_at_index(self.index).and_then(|value| Value::try_from(value).ok()).unwrap()
    }

    /// Sets the value of this field for a given `obj`.
    pub fn set_for_value(&self, obj: StructValue<'ctx>, value: Value) {
        obj.set_field_at_index(self.index, value);
    }

    /// Gets the value of this field for a pointer-to-structure.
    pub fn get(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        pobj: PointerValue<'ctx>,
        obj_name: Option<&'ctx str>,
    ) -> Value {
        ctx.builder
            .build_load(
                self.ptr_by_gep(ctx, pobj, obj_name),
                &obj_name.map(|name| format!("{name}.{}", self.name)).unwrap_or_default(),
            )
            .map_err(|_| ())
            .and_then(|value| Value::try_from(value))
            .unwrap()
    }

    /// Sets the value of this field for a pointer-to-structure.
    pub fn set(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        pobj: PointerValue<'ctx>,
        value: Value,
        obj_name: Option<&'ctx str>,
    ) {
        ctx.builder.build_store(self.ptr_by_gep(ctx, pobj, obj_name), value).unwrap();
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

/// A counter that tracks the next index of a field using a monotonically increasing counter.
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct FieldIndexCounter(u32);

impl FieldIndexCounter {
    /// Increments the number stored by this counter, returning the previous value.
    ///
    /// Functionally equivalent to `i++` in C-based languages.
    pub fn increment(&mut self) -> u32 {
        let v = self.0;
        self.0 += 1;
        v
    }
}

type FieldTypeVerifier<'ctx> = dyn Fn(BasicTypeEnum<'ctx>) -> Result<(), String>;

/// Checks whether [`llvm_ty`][StructType] contains the fields described by the given
/// [`StructFields`] instance.
///
/// By default, this function will compare the type of each field in `expected_fields` against
/// `llvm_ty`. To override this behavior for individual fields, pass in overrides to
/// `custom_verifiers`, which will use the specified verifier when a field with the matching field
/// name is being checked.
pub(super) fn check_struct_type_matches_fields<'ctx>(
    expected_fields: impl StructFields<'ctx>,
    llvm_ty: StructType<'ctx>,
    ty_name: &'static str,
    custom_verifiers: &[(&str, &FieldTypeVerifier<'ctx>)],
) -> Result<(), String> {
    let expected_fields = expected_fields.to_vec();

    if llvm_ty.count_fields() != u32::try_from(expected_fields.len()).unwrap() {
        return Err(format!(
            "Expected {} fields in `{ty_name}`, got {}",
            expected_fields.len(),
            llvm_ty.count_fields(),
        ));
    }

    expected_fields
        .into_iter()
        .enumerate()
        .map(|(i, (field_name, expected_ty))| {
            (field_name, expected_ty, llvm_ty.get_field_type_at_index(i as u32).unwrap())
        })
        .try_for_each(|(field_name, expected_ty, actual_ty)| {
            if let Some((_, verifier)) =
                custom_verifiers.iter().find(|verifier| verifier.0 == field_name)
            {
                verifier(actual_ty)
            } else if expected_ty == actual_ty {
                Ok(())
            } else {
                Err(format!("Expected {expected_ty} for `{ty_name}.{field_name}`, got {actual_ty}"))
            }
        })?;

    Ok(())
}
