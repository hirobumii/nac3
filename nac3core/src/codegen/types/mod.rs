use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValue, BasicValueEnum, PointerValue},
};

use crate::codegen::{CodeGenContext, ModuleContext, stmt::gen_var, types::structure::StructField};

/// Internal helper macro to implement [`ProxyType`] (and [`RefType`]) for various types.
///
/// This is only used by the [`ProxyType`][nac3core_derive::ProxyType] derive macro.
/// See that macro for more details.
macro_rules! impl_proxy_type {
    ([$($pre:tt)*] [$($post:tt)*] |$self:ident, $ctx:ident| llvm_ty($llvm_val:ty, $llvm_ty:expr)) => {
        impl $($pre)* $crate::codegen::types::ProxyTypeMarker<'ctx> for $($post)* {
            type Value = $llvm_val;
        }

        impl $($pre)* $crate::codegen::types::ProxyType<'ctx> for $($post)* {
            #[allow(unused_variables)]
            fn llvm_ty(&$self, $ctx: &$crate::codegen::types::ModuleContext<'ctx>) -> $crate::inkwell::types::BasicTypeEnum<'ctx> {
                ::core::convert::Into::<$crate::inkwell::types::BasicTypeEnum<'ctx>>::into($llvm_ty)
            }
        }
    };
    ([$($pre:tt)*] [$($post:tt)*] |$self:ident, $ctx:ident| llvm_ref($alloca_ty:expr)) => {
        $crate::codegen::types::impl_proxy_type! {
            [$($pre)*] [$($post)*] |self, ctx| llvm_ty($crate::inkwell::values::PointerValue<'ctx>, ctx.ptr)
        }

        impl $($pre)* $crate::codegen::types::RefType<'ctx> for $($post)* {
            #[allow(unused_variables)]
            fn alloca_ty(
                &$self,
                $ctx: &mut $crate::codegen::types::CodeGenContext<'ctx, '_>,
            ) -> $crate::inkwell::types::BasicTypeEnum<'ctx> {
                ::core::convert::Into::<$crate::inkwell::types::BasicTypeEnum<'ctx>>::into($alloca_ty)
            }
        }
    };
}
use impl_proxy_type;

/// Macro to access fields of a struct type.
///
/// This relies on the convention that struct types have an `inner` field
/// of type `BuiltinStruct`, which in turn wraps a `StructFields` struct
/// containing the actual fields.
///
/// Returns a closure of type `FnOnce(&T) -> StructField<'ctx, B>`,
/// which can be passed into [`Value::load`] and [`Value::store`].
///
/// # Example
///
/// Retrieving a field from a builtin struct type passed around by pointer:
///
/// ```
/// # use nac3core::codegen::{CodeGenContext, types::{ListValue, field}};
/// # use inkwell::values::IntValue;
/// fn get_list_len<'ctx>(
///     ctx: &mut CodeGenContext<'ctx, '_>,
///     list: &ListValue<'ctx>)
/// -> IntValue<'ctx> {
///     list.load(ctx, field!(len)).unwrap()
/// }
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __codegen_type_field {
    ($field:ident) => {{ |this| this.inner.fields.$field }};
}

#[doc(inline)]
pub use crate::__codegen_type_field as field;

mod array;
mod builtin;
mod enumerate;
mod exception;
mod list;
mod ndarray;
mod option;
mod range;
mod string;
mod structure;
mod tuple;

pub use array::{ArrayLikeIndexer, ArraySliceType, ArraySliceValue};
pub use builtin::BuiltinStruct;
pub use enumerate::{EnumerateType, EnumerateValue};
pub use exception::{ExceptionType, ExceptionValue};
pub use list::{ListStructFields, ListType, ListValue};
pub use ndarray::{
    BroadcastAllResult, ContiguousNDArrayType, ContiguousNDArrayValue, NDArrayLikeType, NDArrayOut,
    NDArrayType, NDArrayValue, NDIndexType, NDIndexValue, NDIterType, NDIterValue, RustNDIndex,
    ScalarOrNDArray, assert_ndarray_can_be_written_by_out, broadcast, broadcast_starmap,
    make_contiguous_strides, parse_numpy_int_sequence,
};
pub use option::{OptionType, OptionValue};
pub use range::{RangeField, RangeType, RangeValue};
pub use string::{StringType, StringValue};
pub use tuple::{TupleType, TupleValue};

/// Extension trait for types.
pub trait ProxyTypeExt {
    /// Allocates a new instance of this type on the stack.
    ///
    /// Note that this allocates space for the type itself and does not initialize any of
    /// its fields.
    fn alloca<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'static str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        let alloca = self.alloca_ty(ctx);
        let ptr = gen_var(ctx, alloca, name)?;
        let ptr = ctx.builder.build_pointer_cast(ptr, ctx.ptr, "ptr_cast")?;
        Ok(Value { ty: *self, value: ptr, name })
    }

    /// Maps an existing value of the underlying LLVM type to a typed value.
    fn map_value<'ctx, V>(&self, value: V, name: Option<&'static str>) -> Value<'ctx, Self>
    where
        Self: ProxyTypeMarker<'ctx, Value = V> + Copy,
    {
        Value { ty: *self, value, name }
    }
}

impl<T: ?Sized> ProxyTypeExt for T {}

/// Represents a wrapper type on top of this kind of `Value`.
pub trait ProxyTypeMarker<'ctx> {
    type Value;
}

/// Represents a type that is passed around with a single LLVM value.
pub trait ProxyType<'ctx>: ProxyTypeMarker<'ctx> {
    /// Returns the LLVM type that represents how this value is passed around.
    fn llvm_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx>;
}

/// Represents a type that is passed around by pointer.
pub trait RefType<'ctx>: ProxyType<'ctx, Value = PointerValue<'ctx>> {
    /// Returns the LLVM type used for allocating this reference type.
    fn alloca_ty(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> BasicTypeEnum<'ctx>;
}

#[derive(Clone, Copy)]
pub struct Value<'ctx, T: ProxyTypeMarker<'ctx>> {
    pub ty: T,
    pub value: T::Value,
    pub name: Option<&'static str>,
}

impl<'ctx, T: ProxyTypeMarker<'ctx, Value = PointerValue<'ctx>>> Value<'ctx, T> {
    /// Loads the value of the specified field.
    ///
    /// Use the [`field!`][crate::codegen::types::field] macro to specify the field.
    pub fn load<B>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        field: impl FnOnce(&T) -> StructField<'ctx, B>,
    ) -> anyhow::Result<B>
    where
        T: RefType<'ctx>,
        B: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error = ()>,
    {
        let struct_ty = self.ty.alloca_ty(ctx);
        field(&self.ty).load(ctx, struct_ty, self.value, self.name)
    }

    /// Stores the value into the specified field.
    ///
    /// Use the [`field!`][crate::codegen::types::field] macro to specify the field.
    pub fn store<B>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        field: impl FnOnce(&T) -> StructField<'ctx, B>,
        value: B,
    ) -> anyhow::Result<()>
    where
        T: RefType<'ctx>,
        B: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug>,
    {
        let struct_ty = self.ty.alloca_ty(ctx);
        field(&self.ty).store(ctx, struct_ty, self.value, value, self.name)
    }
}
