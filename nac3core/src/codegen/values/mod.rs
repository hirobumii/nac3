use inkwell::{types::IntType, values::BasicValue};

use super::{types::ProxyType, CodeGenContext};
pub use array::*;
pub use list::*;
pub use option::*;
pub use range::*;
pub use string::*;
pub use tuple::*;

mod array;
mod list;
pub mod ndarray;
mod option;
mod range;
mod string;
pub mod structure;
mod tuple;
pub mod utils;

/// A LLVM type that is used to represent a non-primitive value in NAC3.
pub trait ProxyValue<'ctx>: Into<Self::Base> {
    /// The ABI type of LLVM values represented by this instance.
    type ABI: BasicValue<'ctx>;

    /// The type of LLVM values represented by this instance.
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

    /// Returns this proxy as its ABI value, i.e. the expected value representation if a value
    /// represented by this [`ProxyValue`] is being passed into or returned from a function.
    ///
    /// See [`CodeGenContext::get_llvm_abi_type`].
    fn as_abi_value(&self, ctx: &CodeGenContext<'ctx, '_>) -> Self::ABI;
}
