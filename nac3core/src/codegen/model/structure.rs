use std::fmt;

use inkwell::{
    context::Context,
    types::{BasicType, BasicTypeEnum, StructType},
    values::{BasicValueEnum, StructValue},
};

use crate::codegen::{CodeGenContext, CodeGenerator};

use super::*;

/// A traveral that traverses a Rust `struct` that is used to declare an LLVM's struct's field types.
pub trait FieldTraversal<'ctx> {
    /// Output type of [`FieldTraversal::add`].
    type Out<M>;

    /// Traverse through the type of a declared field and do something with it.
    ///
    /// * `name` - The cosmetic name of the LLVM field. Used for debugging.
    /// * `model` - The [`Model`] representing the LLVM type of this field.
    fn add<M: Model<'ctx>>(&mut self, name: &'static str, model: M) -> Self::Out<M>;

    /// Like [`FieldTraversal::add`] but [`Model`] is automatically inferred from its [`Default`] trait.
    fn add_auto<M: Model<'ctx> + Default>(&mut self, name: &'static str) -> Self::Out<M> {
        self.add(name, M::default())
    }
}

/// Descriptor of an LLVM struct field.
#[derive(Debug, Clone, Copy)]
pub struct GepField<M> {
    /// The GEP index of this field. This is the index to use with `build_gep`.
    pub gep_index: u64,
    /// The cosmetic name of this field.
    pub name: &'static str,
    /// The [`Model`] of this field's type.
    pub model: M,
}

/// A traversal to calculate the GEP index of fields.
pub struct GepFieldTraversal {
    /// The current GEP index.
    gep_index_counter: u64,
}

impl<'ctx> FieldTraversal<'ctx> for GepFieldTraversal {
    type Out<M> = GepField<M>;

    fn add<M: Model<'ctx>>(&mut self, name: &'static str, model: M) -> Self::Out<M> {
        let gep_index = self.gep_index_counter;
        self.gep_index_counter += 1;
        Self::Out { gep_index, name, model }
    }
}

/// A traversal to collect the field types of a struct.
///
/// This is used to collect field types and construct the LLVM struct type with [`Context::struct_type`].
struct TypeFieldTraversal<'ctx, 'a, G: CodeGenerator + ?Sized> {
    generator: &'a G,
    ctx: &'ctx Context,
    /// The collected field types so far in exact order.
    field_types: Vec<BasicTypeEnum<'ctx>>,
}

impl<'ctx, 'a, G: CodeGenerator + ?Sized> FieldTraversal<'ctx> for TypeFieldTraversal<'ctx, 'a, G> {
    type Out<M> = (); // Checking types return nothing.

    fn add<M: Model<'ctx>>(&mut self, _name: &'static str, model: M) -> Self::Out<M> {
        let t = model.get_type(self.generator, self.ctx).as_basic_type_enum();
        self.field_types.push(t);
    }
}

/// A traversal to check the types of fields.
struct CheckTypeFieldTraversal<'ctx, 'a, G: CodeGenerator + ?Sized> {
    generator: &'a mut G,
    ctx: &'ctx Context,
    /// The current GEP index, so we can tell the index of the field we are checking
    /// and report the GEP index.
    gep_index_counter: u32,
    /// The [`StructType`] to check.
    scrutinee: StructType<'ctx>,
    /// The list of collected errors so far.
    errors: Vec<ModelError>,
}

impl<'ctx, 'a, G: CodeGenerator + ?Sized> FieldTraversal<'ctx>
    for CheckTypeFieldTraversal<'ctx, 'a, G>
{
    type Out<M> = (); // Checking types return nothing.

    fn add<M: Model<'ctx>>(&mut self, name: &'static str, model: M) -> Self::Out<M> {
        let gep_index = self.gep_index_counter;
        self.gep_index_counter += 1;

        if let Some(t) = self.scrutinee.get_field_type_at_index(gep_index) {
            if let Err(err) = model.check_type(self.generator, self.ctx, t) {
                self.errors
                    .push(err.under_context(format!("field #{gep_index} '{name}'").as_str()));
            }
        } // Otherwise, it will be caught by Struct's `check_type`.
    }
}

/// A trait for Rust structs identifying LLVM structures.
///
/// ### Example
///
/// Suppose you want to define this structure:
/// ```c
/// template <typename T>
/// struct ContiguousNDArray {
///     size_t ndims;
///     size_t* shape;
///     T* data;
/// }
/// ```
///
/// This is how it should be done:
/// ```ignore
/// pub struct ContiguousNDArrayFields<'ctx, F: FieldTraversal<'ctx>, Item: Model<'ctx>> {
///     pub ndims: F::Out<Int<SizeT>>,
///     pub shape: F::Out<Ptr<Int<SizeT>>>,
///     pub data: F::Out<Ptr<Item>>,
/// }
///
/// /// An ndarray without strides and non-opaque `data` field in NAC3.
/// #[derive(Debug, Clone, Copy)]
/// pub struct ContiguousNDArray<M> {
///     /// [`Model`] of the items.
///     pub item: M,
/// }
///
/// impl<'ctx, Item: Model<'ctx>> StructKind<'ctx> for ContiguousNDArray<Item> {
///     type Fields<F: FieldTraversal<'ctx>> = ContiguousNDArrayFields<'ctx, F, Item>;
///
///     fn traverse_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
///         // The order of `traversal.add*` is important
///         Self::Fields {
///             ndims: traversal.add_auto("ndims"),
///             shape: traversal.add_auto("shape"),
///             data: traversal.add("data", Ptr(self.item)),
///         }
///     }
/// }
/// ```
///
/// The [`FieldTraversal`] here is a mechanism to allow the fields of `ContiguousNDArrayFields` to be
/// traversed to do useful work such as:
///
/// - To create the [`StructType`] of `ContiguousNDArray` by collecting [`BasicType`]s of the fields.
/// - To enable the `.gep(ctx, |f| f.ndims).store(ctx, ...)` syntax.
///
/// Suppose now that you have defined `ContiguousNDArray` and you want to allocate a `ContiguousNDArray`
/// with dtype `float64` in LLVM, this is how you do it:
/// ```ignore
/// type F64NDArray = Struct<ContiguousNDArray<Float<Float64>>>; // Type alias for leaner documentation
/// let model: F64NDArray = Struct(ContigousNDArray { item: Float(Float64) });
/// let ndarray: Instance<'ctx, Ptr<F64NDArray>> = model.alloca(generator, ctx);
/// ```
///
/// ...and here is how you may manipulate/access `ndarray`:
///
/// (NOTE: some arguments have been omitted)
///
/// ```ignore
/// // Get `&ndarray->data`
/// ndarray.gep(|f| f.data); // type: Instance<'ctx, Ptr<Float<Float64>>>
///
/// // Get `ndarray->ndims`
/// ndarray.get(|f| f.ndims); // type: Instance<'ctx, Int<SizeT>>
///
/// // Get `&ndarray->ndims`
/// ndarray.gep(|f| f.ndims); // type: Instance<'ctx, Ptr<Int<SizeT>>>
///
/// // Get `ndarray->shape[0]`
/// ndarray.get(|f| f.shape).get_index_const(0); // Instance<'ctx, Int<SizeT>>
///
/// // Get `&ndarray->shape[2]`
/// ndarray.get(|f| f.shape).offset_const(2); // Instance<'ctx, Ptr<Int<SizeT>>>
///
/// // Do `ndarray->ndims = 3;`
/// let num_3 = Int(SizeT).const_int(3);
/// ndarray.set(|f| f.ndims, num_3);
/// ```
pub trait StructKind<'ctx>: fmt::Debug + Clone + Copy {
    /// The associated fields of this struct.
    type Fields<F: FieldTraversal<'ctx>>;

    /// Traverse through all fields of this [`StructKind`].
    ///
    /// Only used internally in this module for implementing other components.
    fn traverse_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F>;

    /// Get a convenience structure to get a struct field's GEP index through its corresponding Rust field.
    ///
    /// Only used internally in this module for implementing other components.
    fn fields(&self) -> Self::Fields<GepFieldTraversal> {
        self.traverse_fields(&mut GepFieldTraversal { gep_index_counter: 0 })
    }

    /// Get the LLVM [`StructType`] of this [`StructKind`].
    fn get_struct_type<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &'ctx Context,
    ) -> StructType<'ctx> {
        let mut traversal = TypeFieldTraversal { generator, ctx, field_types: Vec::new() };
        self.traverse_fields(&mut traversal);

        ctx.struct_type(&traversal.field_types, false)
    }
}

/// A model for LLVM struct.
///
/// `S` should be of a [`StructKind`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Struct<S>(pub S);

impl<'ctx, S: StructKind<'ctx>> Struct<S> {
    /// Create a constant struct value from its fields.
    ///
    /// This function also validates `fields` and panic when there is something wrong.
    pub fn const_struct<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        fields: &[BasicValueEnum<'ctx>],
    ) -> Instance<'ctx, Self> {
        // NOTE: There *could* have been a functor `F<M> = Instance<'ctx, M>` for `S::Fields<F>`
        // to create a more user-friendly interface, but Rust's type system is not sophisticated enough
        // and if you try doing that Rust would force you put lifetimes everywhere.
        let val = ctx.const_struct(fields, false);
        self.check_value(generator, ctx, val).unwrap()
    }
}

impl<'ctx, S: StructKind<'ctx>> Model<'ctx> for Struct<S> {
    type Value = StructValue<'ctx>;
    type Type = StructType<'ctx>;

    fn get_type<G: CodeGenerator + ?Sized>(&self, generator: &G, ctx: &'ctx Context) -> Self::Type {
        self.0.get_struct_type(generator, ctx)
    }

    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError> {
        let ty = ty.as_basic_type_enum();
        let Ok(ty) = StructType::try_from(ty) else {
            return Err(ModelError(format!("Expecting StructType, but got {ty:?}")));
        };

        // Check each field individually.
        let mut traversal = CheckTypeFieldTraversal {
            generator,
            ctx,
            gep_index_counter: 0,
            errors: Vec::new(),
            scrutinee: ty,
        };
        self.0.traverse_fields(&mut traversal);

        // Check the number of fields.
        let exp_num_fields = traversal.gep_index_counter;
        let got_num_fields = u32::try_from(ty.get_field_types().len()).unwrap();
        if exp_num_fields != got_num_fields {
            return Err(ModelError(format!(
                "Expecting StructType with {exp_num_fields} field(s), but got {got_num_fields}"
            )));
        }

        if !traversal.errors.is_empty() {
            // Currently, only the first error is reported.
            return Err(traversal.errors[0].clone());
        }

        Ok(())
    }
}

impl<'ctx, S: StructKind<'ctx>> Instance<'ctx, Struct<S>> {
    /// Get a field with [`StructValue::get_field_at_index`].
    pub fn get_field<G: CodeGenerator + ?Sized, M, GetField>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        get_field: GetField,
    ) -> Instance<'ctx, M>
    where
        M: Model<'ctx>,
        GetField: FnOnce(S::Fields<GepFieldTraversal>) -> GepField<M>,
    {
        let field = get_field(self.model.0.fields());
        let val = self.value.get_field_at_index(field.gep_index as u32).unwrap();
        field.model.check_value(generator, ctx, val).unwrap()
    }
}

impl<'ctx, S: StructKind<'ctx>> Instance<'ctx, Ptr<Struct<S>>> {
    /// Get a pointer to a field with [`Builder::build_in_bounds_gep`].
    pub fn gep<M, GetField>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        get_field: GetField,
    ) -> Instance<'ctx, Ptr<M>>
    where
        M: Model<'ctx>,
        GetField: FnOnce(S::Fields<GepFieldTraversal>) -> GepField<M>,
    {
        let field = get_field(self.model.0 .0.fields());
        let llvm_i32 = ctx.ctx.i32_type();

        let ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.value,
                    &[llvm_i32.const_zero(), llvm_i32.const_int(field.gep_index, false)],
                    field.name,
                )
                .unwrap()
        };

        Ptr(field.model).believe_value(ptr)
    }

    /// Convenience function equivalent to `.gep(...).load(...)`.
    pub fn get<M, GetField, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        get_field: GetField,
    ) -> Instance<'ctx, M>
    where
        M: Model<'ctx>,
        GetField: FnOnce(S::Fields<GepFieldTraversal>) -> GepField<M>,
    {
        self.gep(ctx, get_field).load(generator, ctx)
    }

    /// Convenience function equivalent to `.gep(...).store(...)`.
    pub fn set<M, GetField>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        get_field: GetField,
        value: Instance<'ctx, M>,
    ) where
        M: Model<'ctx>,
        GetField: FnOnce(S::Fields<GepFieldTraversal>) -> GepField<M>,
    {
        self.gep(ctx, get_field).store(ctx, value);
    }
}
