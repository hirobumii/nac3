//! This module defines various traits and types for exposing a high-level interface for types and
//! values in LLVM.
//!
//! # Types
//!
//! The primary abstraction over LLVM types is the [`ProxyType`] trait, which represents a
//! higher-level type that represents either specific built-in types (e.g. `str`, `list`, etc.) or
//! user-defined struct types.
//!
//! Reference types (i.e. types that are passed around by pointer and have a global `typeinfo`)
//! contain a object header to allow for reference counting and metadata tracking. To aid in code
//! reuse, reference types are separated into two categories:
//!
//! - Types prefixed with `Raw` (e.g. [`RawListType`]) represent the raw struct type that only
//!   contains the fields of the type itself and lacks an object header.
//! - Types without the `Raw` prefix (e.g. [`ListType`]) represent the full struct type that contains
//!   an object header and the fields of the type. These types are often implemented as type aliases
//!   of `TypedRefCountedType<'_, RawType>`.
//!
//! To obtain an instance of a non-raw reference type, use one of the `construct`-family of
//! functions on the reference type.
//!
//! # Values
//!
//! Values are implemented using the [`Value`] struct, which is used to represent a pair of a value
//! and its respective type.
//!
//! Similar to types, values of reference types are also separated into raw and non-raw variants
//! with the same naming convention. Since the object header of reference values can be stripped to
//! obtain the raw reference value, some operations on reference values may be implemented directly
//! on the raw reference value. In such cases, [`TypedRefCountedValue::inner_value`] can be used to
//! obtain the raw reference value.

use std::{borrow::Cow, iter};

use inkwell::{
    module::Linkage,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
};
use itertools::Itertools as _;

use crate::codegen::{
    AllocationScope, CodeGenContext, ModuleContext, types::structure::StructField,
};

/// Internal helper macro to implement [`ProxyType`] (and [`RefType`]) for various types.
///
/// This is only used by the [`ProxyType`][nac3core_derive::ProxyType] derive macro.
/// See that macro for more details.
macro_rules! impl_proxy_type {
    ([$($pre:tt)*] [$($post:tt)*] |$self:ident, $ctx:ident| llvm_ty($llvm_val:ty, $llvm_ty:expr)) => {
        impl $($pre)* $crate::codegen::types::ProxyTypeBase<'ctx> for $($post)* {
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
                $ctx: &$crate::codegen::types::ModuleContext<'ctx>,
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
/// # use nac3core::codegen::{CodeGenContext, types::{RawListValue, field}};
/// # use inkwell::values::IntValue;
/// fn get_list_len<'ctx>(
///     ctx: &mut CodeGenContext<'ctx, '_>,
///     list: &RawListValue<'ctx>)
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
mod class;
mod enumerate;
mod exception;
mod list;
mod ndarray;
mod option;
mod range;
mod reference;
mod string;
mod structure;
mod tuple;
mod typeinfo;

pub use array::{ArrayLikeIndexer, ArraySliceType, ArraySliceValue};
pub use builtin::BuiltinStruct;
pub use class::{ClassType, ClassValue, RawClassType, RawClassValue};
pub use enumerate::{EnumerateType, EnumerateValue};
pub use exception::{ExceptionType, ExceptionValue};
pub use list::{ListStructFields, ListType, ListValue, RawListType, RawListValue};
pub use ndarray::{
    BroadcastAllResult, ContiguousNDArrayType, ContiguousNDArrayValue, NDArrayLikeType, NDArrayOut,
    NDArrayType, NDArrayValue, NDIndexType, NDIndexValue, NDIterType, NDIterValue,
    RawContiguousNDArrayType, RawContiguousNDArrayValue, RawNDArrayType, RawNDArrayValue,
    RawNDIterType, RawNDIterValue, RustNDIndex, ScalarOrNDArray,
    assert_ndarray_can_be_written_by_out, broadcast, broadcast_starmap, make_contiguous_strides,
    parse_numpy_int_sequence,
};
pub use option::{OptionSomeType, OptionSomeValue, OptionType, OptionValue};
pub use range::{RangeField, RangeType, RangeValue};
pub use reference::{
    ObjectHeaderType, OpaqueRefCountedType, OpaqueRefCountedValue, RefCountedArrayType,
    RefCountedArrayValue, RefCountedType, RefCountedValue, TypedRefCountedType,
    TypedRefCountedValue, is_obj_id_refcounted, is_refcounted_type,
};
pub use string::{StringType, StringValue};
pub use tuple::{TupleType, TupleValue};
pub use typeinfo::{TypeinfoType, TypeinfoValue};

/// Represents a wrapper type on top of this kind of `Value`.
pub trait ProxyTypeBase<'ctx> {
    type Value;

    /// Allocates a new instance of this type in the
    /// [default allocation scope][`AllocationScope::Default`].
    ///
    /// Note that this allocates space for the type itself and does not initialize any of
    /// its fields.
    fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        let alloca = self.alloca_ty(ctx);
        let ptr = ctx.build_allocate(
            AllocationScope::Default,
            alloca,
            name.map(|n| format!("{n}.alloc")).as_deref(),
        )?;
        Ok(Value { ty: *self, value: ptr, name })
    }

    /// Maps an existing value of the underlying LLVM type to a typed value.
    fn map_value(&self, value: Self::Value, name: Option<&'ctx str>) -> Value<'ctx, Self>
    where
        Self: Sized + Copy,
    {
        Value { ty: *self, value, name }
    }
}

/// Represents a type that is passed around with a single LLVM value.
pub trait ProxyType<'ctx>: ProxyTypeBase<'ctx> {
    /// Returns the LLVM type that represents how this value is passed around.
    fn llvm_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx>;
}

/// Represents a type with a `typeinfo` global structure.
pub trait WithTypeinfo<'ctx> {
    /// Returns a global instance of [`TypeinfoValue`] representing the type information of this
    /// reference type.
    fn typeinfo(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> TypeinfoValue<'ctx> {
        let typename = self.typename();

        let global =
            ctx.module.get_global(&format!("typeinfo for {typename}")).unwrap_or_else(|| {
                let name_data = ctx
                    .module
                    .get_global(&format!("typename array for {typename}"))
                    .unwrap_or_else(|| {
                        let name_data = ctx.module.add_global(
                            ctx.i8.array_type(typename.len() as u32),
                            None,
                            &format!("typename array for {typename}"),
                        );
                        name_data.set_linkage(Linkage::WeakAny);
                        name_data.set_initializer(
                            &ctx.i8.const_array(
                                &typename
                                    .as_bytes()
                                    .iter()
                                    .map(|&b| ctx.i8.const_int(u64::from(b), false))
                                    .collect_vec(),
                            ),
                        );
                        name_data.set_constant(true);

                        name_data
                    });

                let name = ctx
                    .module
                    .get_global(&format!("typename for {typename}"))
                    .unwrap_or_else(|| {
                        let llvm_str = StringType::new(ctx).llvm_ty(ctx).into_struct_type();
                        let name = ctx.module.add_global(
                            llvm_str,
                            None,
                            &format!("typename for {typename}"),
                        );
                        name.set_linkage(Linkage::WeakAny);
                        name.set_initializer(&llvm_str.const_named_struct(&[
                            name_data.as_pointer_value().into(),
                            ctx.size_t.const_int(typename.len() as u64, false).into(),
                        ]));
                        name.set_constant(true);

                        name
                    });

                let refcounted_fields_data = self.refcounted_fields_data(ctx);
                let refcounted_fields = ctx
                    .module
                    .get_global(&format!("refcounted_fields array for {typename}"))
                    .unwrap_or_else(|| {
                        let refcounted_fields = ctx.module.add_global(
                            ctx.i32.array_type(refcounted_fields_data.len() as u32),
                            None,
                            &format!("refcounted_fields array for {typename}"),
                        );
                        refcounted_fields.set_linkage(Linkage::WeakAny);
                        refcounted_fields
                            .set_initializer(&ctx.i32.const_array(&refcounted_fields_data));
                        refcounted_fields.set_constant(true);

                        refcounted_fields
                    });

                let llvm_typeinfo = TypeinfoType::new(ctx).alloca_ty(ctx).into_struct_type();

                let value =
                    ctx.module.add_global(llvm_typeinfo, None, &format!("typeinfo for {typename}"));
                value.set_linkage(Linkage::WeakAny);
                value.set_initializer(&llvm_typeinfo.const_named_struct(&[
                    name.as_pointer_value().into(),
                    refcounted_fields.as_pointer_value().into(),
                ]));
                value.set_constant(true);
                value
            });
        TypeinfoType::new(ctx).map_value(global.as_pointer_value(), None)
    }

    /// Returns the name of this type, which is used for debugging and error messages.
    fn typename(&self) -> Cow<'static, str>;

    /// Returns the complete `refcounted_fields` array payload for this type.
    ///
    /// The first element distinguishes the type's IRRT layout:
    /// - `0xFFFFFFFF` (`REFCOUNT_ARRAY_MAGIC`): an array of refcounted pointer elements.
    /// - `0xFFFFFFFE` (`REFCOUNT_ARRAY_INLINE_MAGIC`): an array of inline refcounted objects;
    ///   the second element is the byte stride between elements.
    /// - Any other value `N`: a struct with `N` refcounted fields - the subsequent `N` elements
    ///   contains the byte offsets of the refcounted fields relative to the start of the struct.
    ///
    /// Struct-flavored implementations can use [`refcounted_fields_for_struct`] to build a payload
    /// containing the prepended count of offsets.
    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>>;
}

/// Builds a `refcounted_fields` payload for a struct-flavored [`WithTypeinfo`] implementation by
/// prepending the count of `offsets` to the slice itself.
#[must_use]
pub fn refcounted_fields_for_struct<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    offsets: Vec<IntValue<'ctx>>,
) -> Vec<IntValue<'ctx>> {
    let count = ctx.i32.const_int(offsets.len() as u64, false);
    iter::once(count).chain(offsets).collect()
}

/// Represents a type that is passed around by pointer and contains a global `typeinfo` instance
/// containing the type information of this type.
pub trait RefType<'ctx>: ProxyType<'ctx, Value = PointerValue<'ctx>> + WithTypeinfo<'ctx> {
    /// Returns the LLVM type used for allocating this reference type.
    fn alloca_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx>;

    /// Creates a [`TypedRefCountedType`] for this reference type.
    fn refcounted_type(&self, ctx: &ModuleContext<'ctx>) -> TypedRefCountedType<'ctx, Self>
    where
        Self: Copy,
    {
        TypedRefCountedType::new(ctx, *self)
    }
}

#[derive(Clone, Copy)]
pub struct Value<'ctx, T: ProxyTypeBase<'ctx>> {
    pub ty: T,
    pub value: T::Value,
    pub name: Option<&'ctx str>,
}

impl<'ctx, T: ProxyTypeBase<'ctx, Value = PointerValue<'ctx>>> Value<'ctx, T> {
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

/// Implements [`ProxyType`] for simple wrapper types, such as [`IntType`][inkwell::types::IntType]
/// and [`FloatType`][inkwell::types::FloatType].
macro_rules! impl_proxytype_for_simple_type {
    ($type:ty, $value:ty $(,)?) => {
        impl<'ctx> $crate::codegen::types::ProxyTypeBase<'ctx> for $type {
            type Value = $value;
        }

        impl<'ctx> $crate::codegen::types::ProxyType<'ctx> for $type {
            fn llvm_ty(
                &self,
                _ctx: &$crate::codegen::ModuleContext<'ctx>,
            ) -> inkwell::types::BasicTypeEnum<'ctx> {
                self.as_basic_type_enum()
            }
        }
    };
}

impl_proxytype_for_simple_type!(inkwell::types::ArrayType<'ctx>, inkwell::values::ArrayValue<'ctx>);
impl_proxytype_for_simple_type!(inkwell::types::IntType<'ctx>, inkwell::values::IntValue<'ctx>);
impl_proxytype_for_simple_type!(inkwell::types::FloatType<'ctx>, inkwell::values::FloatValue<'ctx>);
impl_proxytype_for_simple_type!(
    inkwell::types::PointerType<'ctx>,
    inkwell::values::PointerValue<'ctx>
);
impl_proxytype_for_simple_type!(
    inkwell::types::StructType<'ctx>,
    inkwell::values::StructValue<'ctx>
);
impl_proxytype_for_simple_type!(
    inkwell::types::VectorType<'ctx>,
    inkwell::values::VectorValue<'ctx>
);
impl_proxytype_for_simple_type!(
    inkwell::types::ScalableVectorType<'ctx>,
    inkwell::values::ScalableVectorValue<'ctx>
);
impl_proxytype_for_simple_type!(
    inkwell::types::BasicTypeEnum<'ctx>,
    inkwell::values::BasicValueEnum<'ctx>
);
