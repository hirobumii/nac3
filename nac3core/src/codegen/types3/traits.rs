use std::{fmt::Debug, marker::PhantomData};

use inkwell::{
    llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef},
    types::AsTypeRef,
    values::AsValueRef,
};

/// Represents any type that loosely corresponds to some type in LLVM.
pub trait TypeTag {
    type Metadata<'ctx>: Clone;
}

/// Types that have a one-to-one correspondence with LLVM types. This accurately describes
/// Inkwell's types.
///
/// # Safety
///
/// The functions [`TypeExt::from_raw`] and [`AsTypeRef::as_type_ref`] should round-trip.
/// The result of [`TypeExt::from_raw`] should follow the invariants of the corresponding
/// type tag.
pub unsafe trait TypeExt<'ctx>: AsTypeRef + Sized {
    type Tag: TypeTag;
    unsafe fn from_raw(llvm: LLVMTypeRef) -> Self;
    unsafe fn transmute_from(llvm: impl AsTypeRef) -> Self {
        unsafe { Self::from_raw(llvm.as_type_ref()) }
    }
}

/// Represents an LLVM type, with some metadata.
pub struct Type<'ctx, T: TypeTag> {
    ty: LLVMTypeRef,
    info: Meta<'ctx, T>,
    _phantom: PhantomData<fn() -> (&'ctx (), T)>,
}
impl<'ctx, T: TypeTag<Metadata<'ctx>: Copy>> Copy for Type<'ctx, T> {}
impl<'ctx, T: TypeTag> Clone for Type<'ctx, T> {
    fn clone(&self) -> Self {
        Self { info: self.info.clone(), ..*self }
    }
}
impl<'ctx, T: TypeTag<Metadata<'ctx>: Debug>> Debug for Type<'ctx, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Type").field("ty", &self.ty).field("info", &self.info).finish()
    }
}

unsafe impl<'ctx, T: TypeTag> AsTypeRef for Type<'ctx, T> {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.ty
    }
}
unsafe impl<'ctx, T: TypeTag<Metadata<'ctx> = ()>> TypeExt<'ctx> for Type<'ctx, T> {
    type Tag = T;
    unsafe fn from_raw(llvm: LLVMTypeRef) -> Self {
        Self { ty: llvm, info: (), _phantom: PhantomData }
    }
}
impl<'ctx, T: TypeTag> Type<'ctx, T> {
    pub fn new<Ty: TypeExt<'ctx, Tag = T>>(ty: Ty) -> Self
    where
        T: TypeTag<Metadata<'ctx> = ()>,
    {
        unsafe { Self::from_raw_parts(ty, ()) }
    }
    pub unsafe fn from_raw_parts<Ty: AsTypeRef>(ty: Ty, info: Meta<'ctx, T>) -> Self {
        Self { ty: ty.as_type_ref(), info, _phantom: PhantomData }
    }
    pub fn get<Ty: TypeExt<'ctx>>(self) -> (Ty, Meta<'ctx, T>)
    where
        T: SubtypeOf<Ty::Tag>,
    {
        unsafe { self.get_unchecked() }
    }
    pub unsafe fn get_unchecked<Ty: TypeExt<'ctx>>(self) -> (Ty, Meta<'ctx, T>) {
        (unsafe { TypeExt::from_raw(self.ty) }, self.info)
    }
    pub fn cast<U: TypeTag>(self) -> Type<'ctx, U>
    where
        T: SubtypeOf<U>,
    {
        Type { ty: self.ty, info: T::cast_metadata(self.info), _phantom: PhantomData }
    }
    pub fn poison(self) -> Value<'ctx, T> {
        let val = unsafe { inkwell::llvm_sys::core::LLVMGetPoison(self.ty) };
        Value { val, info: self.info, _phantom: PhantomData }
    }
}

/// Values that have a one-to-one correspondence with LLVM values. This accurately describes
/// Inkwell's values.
///
/// # Safety
///
/// The functions [`ValueExt::from_raw`] and [`AsValueRef::as_value_ref`] should round-trip.
/// The result of [`ValueExt::from_raw`] should follow the invariants of the corresponding
/// type tag.
pub unsafe trait ValueExt<'ctx>: AsValueRef + Sized {
    type Tag: TypeTag;
    unsafe fn from_raw(llvm: LLVMValueRef) -> Self;
    unsafe fn transmute_from(llvm: impl AsValueRef) -> Self {
        unsafe { Self::from_raw(llvm.as_value_ref()) }
    }
}

pub struct Value<'ctx, T: TypeTag> {
    val: LLVMValueRef,
    pub(crate) info: Meta<'ctx, T>,
    _phantom: PhantomData<(&'ctx ())>,
}

impl<'ctx, T: TypeTag<Metadata<'ctx>: Copy>> Copy for Value<'ctx, T> {}
impl<'ctx, T: TypeTag> Clone for Value<'ctx, T> {
    fn clone(&self) -> Self {
        Self { info: self.info.clone(), ..*self }
    }
}
impl<'ctx, T: TypeTag<Metadata<'ctx>: Debug>> Debug for Value<'ctx, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Value").field("val", &self.val).field("info", &self.info).finish()
    }
}

unsafe impl<'ctx, T: TypeTag> AsValueRef for Value<'ctx, T> {
    fn as_value_ref(&self) -> LLVMValueRef {
        self.val
    }
}
unsafe impl<'ctx, T: TypeTag<Metadata<'ctx> = ()>> ValueExt<'ctx> for Value<'ctx, T> {
    type Tag = T;
    unsafe fn from_raw(llvm: LLVMValueRef) -> Self {
        Self { val: llvm, info: (), _phantom: PhantomData }
    }
}
impl<'ctx, T: TypeTag> Value<'ctx, T> {
    pub fn new<Val: ValueExt<'ctx, Tag = T>>(val: Val) -> Self
    where
        T: TypeTag<Metadata<'ctx> = ()>,
    {
        unsafe { Self::from_raw_parts(val, ()) }
    }
    pub unsafe fn from_raw_parts<Val: AsValueRef>(val: Val, info: Meta<'ctx, T>) -> Self {
        Self { val: val.as_value_ref(), info, _phantom: PhantomData }
    }
    pub fn get<Val: ValueExt<'ctx>>(self) -> (Val, Meta<'ctx, T>)
    where
        T: SubtypeOf<Val::Tag>,
    {
        unsafe { self.get_unchecked() }
    }
    pub unsafe fn get_unchecked<Val: ValueExt<'ctx>>(self) -> (Val, Meta<'ctx, T>) {
        (unsafe { ValueExt::from_raw(self.val) }, self.info)
    }
    pub fn cast<U: TypeTag>(self) -> Value<'ctx, U>
    where
        T: SubtypeOf<U>,
    {
        Value { val: self.val, info: T::cast_metadata(self.info), _phantom: PhantomData }
    }
    pub fn ty(self) -> Type<'ctx, T> {
        let ty = unsafe { inkwell::llvm_sys::core::LLVMTypeOf(self.val) };
        Type { ty, info: self.info, _phantom: PhantomData }
    }
}

pub type Meta<'ctx, T> = <T as TypeTag>::Metadata<'ctx>;

/// Rules of variance.
pub unsafe trait SubtypeOf<U: TypeTag>: TypeTag {
    fn cast_metadata<'ctx>(meta: Meta<'ctx, Self>) -> Meta<'ctx, U>;
}

#[macro_export]
#[doc(hidden)]
macro_rules! __codegen_type_tag {
    ($tag:ident : $($t:ty),*) => {
        impl $crate::codegen::types3::TypeTag for $tag {
            type Metadata<'ctx> = ();
        }
        unsafe impl $crate::codegen::types3::SubtypeOf<$tag> for $tag {
            fn cast_metadata<'ctx>(meta: $crate::codegen::types3::Meta<'ctx, Self>) -> $crate::codegen::types3::Meta<'ctx, Self> { meta }
        }
        $(unsafe impl $crate::codegen::types3::SubtypeOf<$t> for $tag {
            fn cast_metadata<'ctx>(meta: $crate::codegen::types3::Meta<'ctx, Self>) -> $crate::codegen::types3::Meta<'ctx, $t> { meta }
        })*
    }
}

#[doc(inline)]
pub use __codegen_type_tag as type_tag;

#[macro_export]
#[doc(hidden)]
macro_rules! __codegen_type_tag_generic {
    ($tag:ident : $($t:ty),*) => {
        $(unsafe impl<T: TypeTag> $crate::codegen::types3::SubtypeOf<$t> for $tag<T> {
            fn cast_metadata<'ctx>(meta: $crate::codegen::types3::Meta<'ctx, Self>) -> $crate::codegen::types3::Meta<'ctx, $t> { () }
        })*
    }
}

#[doc(inline)]
pub use __codegen_type_tag_generic as type_tag_generic;
