//! This module contains abstraction over all intrinsic composite types of NAC3.
//!
//! # `raw_alloca` vs `alloca` vs `construct`
//!
//! There are three ways of creating a new object instance using the abstractions provided by this
//! module.
//!
//! - `raw_alloca`: Allocates the object on the stack, returning an instance of
//!   [`impl BasicValue`][inkwell::values::BasicValue]. This is similar to a `malloc` expression in
//!   C++ but the object is allocated on the stack.
//! - `alloca`: Similar to `raw_alloca`, but also wraps the allocated object with
//!   [`<Self as ProxyType<'ctx>>::Value`][ProxyValue], and returns the wrapped object. The returned
//!   object will not initialize any value or fields. This is similar to a type-safe `malloc`
//!   expression in C++ but the object is allocated on the stack.
//! - `construct`: Similar to `alloca`, but performs some initialization on the value or fields of
//!   the returned object. This is similar to a `new` expression in C++ but the object is allocated
//!   on the stack.

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

    /// Creates a new value of this type, returning the LLVM instance of this value.
    fn raw_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self::Value as ProxyValue<'ctx>>::Base;

    /// Creates a new array value of this type, returning an [`ArraySliceValue`] encapsulating the
    /// resulting array.
    fn array_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx>;

    /// Returns the [base type][Self::Base] of this proxy.
    fn as_base_type(&self) -> Self::Base;
}
