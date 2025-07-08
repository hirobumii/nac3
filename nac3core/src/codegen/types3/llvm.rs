use inkwell::llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef};

use super::{Meta, SubtypeOf, Type, TypeTag, Value, type_tag};
use inkwell::{types::*, values::*};

macro_rules! impl_for_inkwell {
    ([ty] $tag:ident, $ty:ident) => {
        unsafe impl<'ctx> super::TypeExt<'ctx> for $ty<'ctx> {
            type Tag = $tag;
            unsafe fn from_raw(llvm: LLVMTypeRef) -> Self {
                unsafe { Self::new(llvm) }
            }
        }
    };
    ([val] $tag:ident, $val:ident) => {
        unsafe impl<'ctx> super::ValueExt<'ctx> for $val<'ctx> {
            type Tag = $tag;
            unsafe fn from_raw(llvm: LLVMValueRef) -> Self {
                unsafe { Self::new(llvm) }
            }
        }
    };
    ($tag:ident, $(ty $ty:ident)? $(val $val:ident)? : $($t:ty),*) => {
        pub enum $tag {}
        type_tag!($tag : $($t),*);
        $(impl_for_inkwell!([ty] $tag, $ty);)?
        $(impl_for_inkwell!([val] $tag, $val);)?
    };
}

pub enum Any {}
impl TypeTag for Any {
    type Metadata<'ctx> = ();
}
impl_for_inkwell!([ty] Any, AnyTypeEnum);
impl_for_inkwell!([val] Any, AnyValueEnum);

impl_for_inkwell!(Basic, ty BasicTypeEnum val BasicValueEnum : );

impl_for_inkwell!(Array, ty ArrayType val ArrayValue : Basic);

impl_for_inkwell!(Int, ty IntType val IntValue : Basic);

impl_for_inkwell!(Float, ty FloatType val FloatValue : Basic);

impl_for_inkwell!(RawPointer, ty PointerType val PointerValue : Basic);

impl_for_inkwell!(Struct, ty StructType val StructValue : Basic);

impl_for_inkwell!(Vector, ty VectorType val VectorValue : Basic);

impl_for_inkwell!(ScalableVector, ty ScalableVectorType val ScalableVectorValue : Basic);

impl_for_inkwell!(Metadata, ty MetadataType val MetadataValue : Basic);

// FunctionValue has a weird constructor that gives an Option.
impl_for_inkwell!(Function, ty FunctionType : Basic);

unsafe impl<'ctx> super::ValueExt<'ctx> for FunctionValue<'ctx> {
    type Tag = Function;
    unsafe fn from_raw(llvm: LLVMValueRef) -> Self {
        unsafe { Self::new(llvm).unwrap_unchecked() }
    }
}

impl_for_inkwell!(AggregateValue, val AggregateValueEnum : Basic);

unsafe impl<'ctx, T: SubtypeOf<Basic> + SubtypeOf<Any> + TypeTag<Metadata<'ctx>: std::fmt::Debug>>
    BasicType<'ctx> for Type<'ctx, T>
{
}
unsafe impl<'ctx, T: SubtypeOf<Any> + TypeTag<Metadata<'ctx>: std::fmt::Debug>> AnyType<'ctx>
    for Type<'ctx, T>
{
}
unsafe impl<'ctx, T: SubtypeOf<Basic> + SubtypeOf<Any> + TypeTag<Metadata<'ctx>: std::fmt::Debug>>
    BasicValue<'ctx> for Value<'ctx, T>
{
}
unsafe impl<'ctx, T: SubtypeOf<Any> + TypeTag<Metadata<'ctx>: std::fmt::Debug>> AnyValue<'ctx>
    for Value<'ctx, T>
{
}
pub trait BasicTag:
    SubtypeOf<Basic> + SubtypeOf<Any> + for<'ctx> TypeTag<Metadata<'ctx>: std::fmt::Debug>
{
}
impl<T: SubtypeOf<Basic> + SubtypeOf<Any> + for<'ctx> TypeTag<Metadata<'ctx>: std::fmt::Debug>>
    BasicTag for T
{
}

unsafe impl<T: TypeTag> SubtypeOf<Any> for T {
    fn cast_metadata<'ctx>(
        meta: Meta<'ctx, Self>,
    ) -> <Any as super::traits::TypeTag>::Metadata<'ctx> {
        ()
    }
}
