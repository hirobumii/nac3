use inkwell::values::BasicValue;

use super::types::ProxyType;
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
    type Type: ProxyType<'ctx>;

    /// Returns the [type][ProxyType] of this value.
    fn get_type(&self) -> Self::Type;

    /// Returns the [base value][Self::Base] of this proxy.
    fn as_base_value(&self) -> Self::Base;
}
