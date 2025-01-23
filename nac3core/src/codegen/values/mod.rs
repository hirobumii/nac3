use inkwell::{types::IntType, values::BasicValue};

use super::types::ProxyType;
pub use array::*;
pub use list::*;
pub use range::*;
pub use tuple::*;

mod array;
mod list;
pub mod ndarray;
mod range;
mod tuple;
pub mod utils;

/// A LLVM type that is used to represent a non-primitive value in NAC3.
pub trait ProxyValue<'ctx>: Into<Self::Base> {
    /// The type of LLVM values represented by this instance. This is usually the
    /// [LLVM pointer type][PointerValue].
    type Base: BasicValue<'ctx>;

    /// The type of this value.
    type Type: ProxyType<'ctx, Value = Self>;

    /// Checks whether `value` can be represented by this [`ProxyValue`].
    fn is_instance(value: impl BasicValue<'ctx>, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        Self::Type::is_representable(value.as_basic_value_enum().get_type(), llvm_usize)
    }

    /// Returns the [type][ProxyType] of this value.
    fn get_type(&self) -> Self::Type;

    /// Returns the [base value][Self::Base] of this proxy.
    fn as_base_value(&self) -> Self::Base;
}
