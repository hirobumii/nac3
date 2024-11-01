use inkwell::{context::Context, values::BasicValue};

use super::types::ProxyType;
use crate::codegen::CodeGenerator;
pub use array::*;
pub use list::*;
pub use ndarray::*;
pub use range::*;

mod array;
mod list;
mod ndarray;
mod range;

/// A LLVM type that is used to represent a non-primitive value in NAC3.
pub trait ProxyValue<'ctx>: Into<Self::Base> {
    /// The type of LLVM values represented by this instance. This is usually the
    /// [LLVM pointer type][PointerValue].
    type Base: BasicValue<'ctx>;

    /// The type of this value.
    type Type: ProxyType<'ctx, Value = Self>;

    /// Checks whether `value` can be represented by this [`ProxyValue`].
    fn is_instance<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        value: impl BasicValue<'ctx>,
    ) -> Result<(), String> {
        Self::Type::is_type(generator, ctx, value.as_basic_value_enum().get_type())
    }

    /// Checks whether `value` can be represented by this [`ProxyValue`].
    fn is_representable<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        value: Self::Base,
    ) -> Result<(), String> {
        Self::is_instance(generator, ctx, value.as_basic_value_enum())
    }

    /// Returns the [type][ProxyType] of this value.
    fn get_type(&self) -> Self::Type;

    /// Returns the [base value][Self::Base] of this proxy.
    fn as_base_value(&self) -> Self::Base;
}
