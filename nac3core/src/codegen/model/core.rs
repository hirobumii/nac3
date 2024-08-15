use std::fmt;

use inkwell::{context::Context, types::*, values::*};
use itertools::Itertools;

use super::*;
use crate::codegen::{CodeGenContext, CodeGenerator};

/// A error type for reporting any [`Model`]-related error (e.g., a [`BasicType`] mismatch).
#[derive(Debug, Clone)]
pub struct ModelError(pub String);

impl ModelError {
    // Append a context message to the error.
    pub(super) fn under_context(mut self, context: &str) -> Self {
        self.0.push_str(" ... in ");
        self.0.push_str(context);
        self
    }
}

/// Trait for Rust structs identifying [`BasicType`]s in the context of a known [`CodeGenerator`] and [`CodeGenContext`].
///
/// For instance,
/// - [`Int<Int32>`] identifies an [`IntType`] with 32-bits.
/// - [`Int<SizeT>`] identifies an [`IntType`] with bit-width [`CodeGenerator::get_size_type`].
/// - [`Ptr<Int<SizeT>>`] identifies a [`PointerType`] that points to an [`IntType`] with bit-width [`CodeGenerator::get_size_type`].
/// - [`Int<AnyInt>`] identifies an [`IntType`] with bit-width of whatever is set in the [`AnyInt`] object.
/// - [`Any`] identifies a [`BasicType`] set in the [`Any`] object itself.
///
/// You can get the [`BasicType`] out of a model with [`Model::get_type`].
///
/// Furthermore, [`Instance<'ctx, M>`] is a simple structure that carries a [`BasicValue`] with [`BasicType`] identified by model `M`.
///
/// The main purpose of this abstraction is to have a more Rust type-safe way to use Inkwell and give type-hints for programmers.
///
/// ### Notes on `Default` trait
///
/// For some models like [`Int<Int32>`] or [`Int<SizeT>`], they have a [`Default`] trait since just by looking at their types, it is possible
/// to tell the [`BasicType`]s they are identifying.
///
/// This can be used to create strongly-typed interfaces accepting only values of a specific [`BasicType`] without having to worry about
/// writing debug assertions to check, for example, if the programmer has passed in an [`IntValue`] with the wrong bit-width.
/// ```ignore
/// fn give_me_i32_and_get_a_size_t_back<'ctx>(i32: Instance<'ctx, Int<Int32>>) -> Instance<'ctx, Int<SizeT>> {
///     // code...
/// }
/// ```
///
/// ### Notes on converting between Inkwell and model.
///
/// Suppose you have an [`IntValue`], and you want to pass it into a function that takes a [`Instance<'ctx, Int<Int32>>`]. You can do use
/// [`Model::check_value`] or [`Model::believe_value`].
/// ```ignore
/// let my_value: IntValue<'ctx>;
///
/// let my_value = Int(Int32).check_value(my_value).unwrap(); // Panics if `my_value` is not 32-bit with a descriptive error message.
///
/// // or, if you are absolutely certain that `my_value` is 32-bit and doing extra checks is a waste of time:
/// let my_value = Int(Int32).believe_value(my_value);
/// ```
pub trait Model<'ctx>: fmt::Debug + Clone + Copy {
    /// The [`BasicType`] *variant* this model is identifying.
    type Type: BasicType<'ctx>;

    /// The [`BasicValue`] type of the [`BasicType`] of this model.
    type Value: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>>;

    /// Return the [`BasicType`] of this model.
    #[must_use]
    fn get_type<G: CodeGenerator + ?Sized>(&self, generator: &G, ctx: &'ctx Context) -> Self::Type;

    /// Get the number of bytes of the [`BasicType`] of this model.
    fn sizeof<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> IntValue<'ctx> {
        self.get_type(generator, ctx).size_of().unwrap()
    }

    /// Check if a [`BasicType`] matches the [`BasicType`] of this model.
    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError>;

    /// Create an instance from a value.
    ///
    /// Caller must make sure the type of `value` and the type of this `model` are equivalent.
    #[must_use]
    fn believe_value(&self, value: Self::Value) -> Instance<'ctx, Self> {
        Instance { model: *self, value }
    }

    /// Check if a [`BasicValue`]'s type is equivalent to the type of this model.
    /// Wrap the [`BasicValue`] into an [`Instance`] if it is.
    fn check_value<V: BasicValue<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        value: V,
    ) -> Result<Instance<'ctx, Self>, ModelError> {
        let value = value.as_basic_value_enum();
        self.check_type(generator, ctx, value.get_type())
            .map_err(|err| err.under_context(format!("the value {value:?}").as_str()))?;

        let Ok(value) = Self::Value::try_from(value) else {
            unreachable!("check_type() has bad implementation")
        };
        Ok(self.believe_value(value))
    }

    // Allocate a value on the stack and return its pointer.
    fn alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Ptr<Self>> {
        let p = ctx.builder.build_alloca(self.get_type(generator, ctx.ctx), "").unwrap();
        Ptr(*self).believe_value(p)
    }

    // Allocate an array on the stack and return its pointer.
    fn array_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
    ) -> Instance<'ctx, Ptr<Self>> {
        let p = ctx.builder.build_array_alloca(self.get_type(generator, ctx.ctx), len, "").unwrap();
        Ptr(*self).believe_value(p)
    }

    fn var_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> Result<Instance<'ctx, Ptr<Self>>, String> {
        let ty = self.get_type(generator, ctx.ctx).as_basic_type_enum();
        let p = generator.gen_var_alloc(ctx, ty, name)?;
        Ok(Ptr(*self).believe_value(p))
    }

    fn array_var_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> Result<Instance<'ctx, Ptr<Self>>, String> {
        // TODO: Remove ArraySliceValue
        let ty = self.get_type(generator, ctx.ctx).as_basic_type_enum();
        let p = generator.gen_array_var_alloc(ctx, ty, len, name)?;
        Ok(Ptr(*self).believe_value(PointerValue::from(p)))
    }

    /// Allocate a constant array.
    fn const_array<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        values: &[Instance<'ctx, Self>],
    ) -> Instance<'ctx, Array<AnyLen, Self>> {
        macro_rules! make {
            ($t:expr, $into_value:expr) => {
                $t.const_array(
                    &values
                        .iter()
                        .map(|x| $into_value(x.value.as_basic_value_enum()))
                        .collect_vec(),
                )
            };
        }

        let value = match self.get_type(generator, ctx).as_basic_type_enum() {
            BasicTypeEnum::ArrayType(t) => make!(t, BasicValueEnum::into_array_value),
            BasicTypeEnum::IntType(t) => make!(t, BasicValueEnum::into_int_value),
            BasicTypeEnum::FloatType(t) => make!(t, BasicValueEnum::into_float_value),
            BasicTypeEnum::PointerType(t) => make!(t, BasicValueEnum::into_pointer_value),
            BasicTypeEnum::StructType(t) => make!(t, BasicValueEnum::into_struct_value),
            BasicTypeEnum::VectorType(t) => make!(t, BasicValueEnum::into_vector_value),
        };

        Array { len: AnyLen(values.len() as u32), item: *self }
            .check_value(generator, ctx, value)
            .unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Instance<'ctx, M: Model<'ctx>> {
    /// The model of this instance.
    pub model: M,
    /// The value of this instance.
    ///
    /// It is guaranteed the [`BasicType`] of `value` is consistent with that of `model`.
    pub value: M::Value,
}
