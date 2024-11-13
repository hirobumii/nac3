use inkwell::{context::Context, types::BasicType, values::IntValue};

use super::{
    values::{ArraySliceValue, ProxyValue},
    {CodeGenContext, CodeGenerator},
};
pub use list::*;
pub use ndarray::*;
pub use range::*;

mod list;
mod ndarray;
mod range;
pub mod structure;

/// A LLVM type that is used to represent a corresponding type in NAC3.
pub trait ProxyType<'ctx>: Into<Self::Base> {
    /// The LLVM type of which values of this type possess. This is usually a
    /// [LLVM pointer type][PointerType] for any non-primitive types.
    type Base: BasicType<'ctx>;

    /// The type of values represented by this type.
    type Value: ProxyValue<'ctx, Type = Self>;

    fn is_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: impl BasicType<'ctx>,
    ) -> Result<(), String>;

    /// Checks whether `llvm_ty` can be represented by this [`ProxyType`].
    fn is_representable<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: Self::Base,
    ) -> Result<(), String>;

    /// Creates a new value of this type.
    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value;

    /// Creates a new array value of this type.
    fn new_array_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx>;

    /// Converts an existing value into a [`ProxyValue`] of this type.
    fn map_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value;

    /// Returns the [base type][Self::Base] of this proxy.
    fn as_base_type(&self) -> Self::Base;
}
