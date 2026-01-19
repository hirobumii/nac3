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

use inkwell::{
    types::{BasicType, IntType},
    values::{IntValue, PointerValue},
};

use super::{
    CodeGenContext,
    stmt::{gen_array_var, gen_var},
    values::{ArraySliceValue, ProxyValue},
};
pub use exception::*;
pub use list::*;
pub use option::*;
pub use range::*;
pub use string::*;
pub use tuple::*;

mod exception;
mod list;
pub mod ndarray;
mod option;
mod range;
mod string;
pub mod structure;
mod tuple;
pub mod slice;

/// A LLVM type that is used to represent a corresponding type in NAC3.
pub trait ProxyType<'ctx>: Into<Self::Base> {
    /// The ABI type of which values of this type possess.
    type ABI: BasicType<'ctx>;

    /// The LLVM type of which values of this type possess.
    type Base: BasicType<'ctx>;

    /// The type of values represented by this type.
    type Value: ProxyValue<'ctx, Type = Self>;

    /// Checks whether `llvm_ty` can be represented by this [`ProxyType`].
    fn is_representable(
        llvm_ty: impl BasicType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String>;

    /// Checks whether the type represented by `ty` expresses the same type represented by this
    /// [`ProxyType`].
    fn has_same_repr(ty: Self::Base, llvm_usize: IntType<'ctx>) -> Result<(), String>;

    /// Returns the type that should be used in `alloca` IR statements.
    fn alloca_type(&self) -> impl BasicType<'ctx>;

    /// Creates a new value of this type by invoking `alloca` at the current builder location,
    /// returning a [`PointerValue`] instance representing the allocated value.
    fn raw_alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> PointerValue<'ctx> {
        ctx.builder
            .build_alloca(self.alloca_type().as_basic_type_enum(), name.unwrap_or_default())
            .unwrap()
    }

    /// Creates a new value of this type by invoking `alloca` at the beginning of the function,
    /// returning a [`PointerValue`] instance representing the allocated value.
    fn raw_alloca_var(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> PointerValue<'ctx> {
        gen_var(ctx, self.alloca_type().as_basic_type_enum(), name).unwrap()
    }

    /// Creates a new array value of this type by invoking `alloca` at the current builder location,
    /// returning an [`ArraySliceValue`] encapsulating the resulting array.
    fn array_alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx> {
        ArraySliceValue::from_ptr_val(
            ctx.builder
                .build_array_alloca(
                    self.alloca_type().as_basic_type_enum(),
                    size,
                    name.unwrap_or_default(),
                )
                .unwrap(),
            size,
            name,
        )
    }

    /// Creates a new array value of this type by invoking `alloca` at the beginning of the
    /// function, returning an [`ArraySliceValue`] encapsulating the resulting array.
    fn array_alloca_var(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx> {
        gen_array_var(ctx, self.alloca_type().as_basic_type_enum(), size, name).unwrap()
    }

    /// Returns the [base type][Self::Base] of this proxy.
    fn as_base_type(&self) -> Self::Base;

    /// Returns this proxy as its ABI type, i.e. the expected type representation if a value of this
    /// [`ProxyType`] is being passed into or returned from a function.
    ///
    /// See [`CodeGenContext::get_llvm_abi_type`].
    fn as_abi_type(&self) -> Self::ABI;
}
