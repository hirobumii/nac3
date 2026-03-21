use std::borrow::Cow;

use inkwell::{
    AddressSpace,
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
mod reference;
mod string;
mod structure;
mod tuple;
mod typeinfo;

pub use array::{ArrayLikeIndexer, ArraySliceType, ArraySliceValue};
pub use builtin::BuiltinStruct;
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
pub use option::{OptionType, OptionValue};
pub use range::{RangeField, RangeType, RangeValue};
pub use reference::{
    OpaqueRefCountedType, OpaqueRefCountedValue, RefCountedArrayType, RefCountedArrayValue,
    RefCountedType, RefCountedValue, TypedRefCountedType, TypedRefCountedValue,
    is_obj_id_refcounted, is_refcounted_type,
};
pub use string::{StringType, StringValue};
pub use tuple::{TupleType, TupleValue};
pub use typeinfo::{TypeinfoType, TypeinfoValue};

/// Represents a wrapper type on top of this kind of `Value`.
pub trait ProxyTypeBase<'ctx> {
    type Value;

    /// Allocates a new instance of this type on the stack.
    ///
    /// Note that this allocates space for the type itself and does not initialize any of
    /// its fields.
    #[deprecated = "Use ProxyTypeExt::allocate instead."]
    fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        let alloca = self.alloca_ty(ctx);
        let ptr = ctx.build_allocate(AllocationScope::StackStartOfFunc, alloca, name)?;
        let ptr = ctx.builder.build_pointer_cast(ptr, ctx.ptr, "ptr_cast")?;
        Ok(Value { ty: *self, value: ptr, name })
    }

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
        let ptr = ctx.builder.build_pointer_cast(ptr, ctx.ptr, name.unwrap_or_default())?;
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
// TODO(Derppening): Consider adding `typename()` and `refcounted_fields()` methods to this trait
// and move the corresponding methods from `typeinfo.rs` here.
pub trait WithTypeinfo<'ctx> {
    /// Returns a global instance of [`TypeinfoValue`] representing the type information of this
    /// reference type.
    fn typeinfo(&self, ctx: &ModuleContext<'ctx>) -> TypeinfoValue<'ctx> {
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

                let refcounted_field_offsets = self.refcounted_field_offset(ctx);
                let refcounted_fields = ctx
                    .module
                    .get_global(&format!("refcounted_fields array for {typename}"))
                    .unwrap_or_else(|| {
                        let refcounted_fields = ctx.module.add_global(
                            ctx.i32.array_type(refcounted_field_offsets.len() as u32 + 1),
                            None,
                            &format!("refcounted_fields array for {typename}"),
                        );
                        refcounted_fields.set_linkage(Linkage::WeakAny);
                        refcounted_fields.set_initializer(
                            &ctx.i32.const_array(
                                &[
                                    &[ctx
                                        .i32
                                        .const_int(refcounted_field_offsets.len() as u64, false)],
                                    refcounted_field_offsets.as_slice(),
                                ]
                                .concat(),
                            ),
                        );
                        refcounted_fields.set_constant(true);

                        refcounted_fields
                    });

                let llvm_typeinfo = TypeinfoType::new(ctx).alloca_ty(ctx).into_struct_type();

                let value =
                    ctx.module.add_global(llvm_typeinfo, None, &format!("typeinfo for {typename}"));
                value.set_linkage(Linkage::WeakAny);
                value.set_initializer(
                    &llvm_typeinfo.const_named_struct(&[
                        name.as_pointer_value()
                            .const_cast(ctx.i8.ptr_type(AddressSpace::default()))
                            .into(),
                        refcounted_fields
                            .as_pointer_value()
                            .const_cast(ctx.i32.ptr_type(AddressSpace::default()))
                            .into(),
                    ]),
                );
                value.set_constant(true);
                value
            });
        TypeinfoType::new(ctx).map_value(global.as_pointer_value(), None)
    }

    /// Returns the name of this type, which is used for debugging and error messages.
    fn typename(&self) -> Cow<'static, str>;

    /// Returns a vector of byte offsets of the reference-counted fields in this type.
    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>>;
}

/// Represents a type that is passed around by pointer.
// TODO(Derppening): Uncomment the following line when all types implement `typeinfo`
// pub trait RefType<'ctx>: ProxyType<'ctx, Value = PointerValue<'ctx>> + WithTypeinfo<'ctx> {
pub trait RefType<'ctx>: ProxyType<'ctx, Value = PointerValue<'ctx>> {
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
