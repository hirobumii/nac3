use std::marker::PhantomData;

use inkwell::{builder::Builder, context::Context, types::ArrayType, values::PointerValue};

use super::*;

pub struct Ref<T>(Void, PhantomData<fn(T) -> T>);
impl<T: TypeTag> TypeTag for Ref<T> {
    // *** Typed pointers! ***
    type Metadata<'ctx> = Type<'ctx, T>;
}
type_tag_generic!(Ref : RawPointer, Basic);
impl<T: TypeTag> Ref<T> {
    pub fn ty<'ctx>(ctx: &'ctx Context, ty: Type<'ctx, T>) -> Type<'ctx, Self> {
        unsafe { Type::from_raw_parts(RawPointer::ty(ctx), ty) }
    }

    pub fn load<'ctx>(builder: &Builder<'ctx>, src: Value<'ctx, Self>) -> Value<'ctx, T>
    where
        T: BasicTag,
    {
        let (ptr, ty) = src.get();
        let (ty, info) = ty.get::<Type<Basic>>();
        unsafe { Value::from_raw_parts(builder.build_load(ty, ptr, "").unwrap(), info) }
    }
    pub fn store<'ctx>(builder: &Builder<'ctx>, dst: Value<'ctx, Self>, val: Value<'ctx, T>)
    where
        T: BasicTag,
    {
        builder.build_store(dst.get().0, val.get::<Value<Basic>>().0);
    }
}

// Somewhat corresponds to C arrays with unknown size. This does not involve LLVM's array type at all;
// it's just a type-level wrapper over T itself.
pub struct Memory<T>(Void, PhantomData<fn() -> T>);
impl<T: TypeTag> TypeTag for Memory<T> {
    type Metadata<'ctx> = T::Metadata<'ctx>;
}
unsafe impl<T: SubtypeOf<U>, U: TypeTag> SubtypeOf<Memory<U>> for Memory<T> {
    fn cast_metadata<'ctx>(meta: Self::Metadata<'ctx>) -> <Memory<U> as TypeTag>::Metadata<'ctx> {
        T::cast_metadata(meta)
    }
}
impl<'ctx, T: TypeTag> Type<'ctx, Memory<T>> {
    pub fn elem_type(self) -> Type<'ctx, T> {
        let (ty, info) = unsafe { self.get_unchecked::<Type<Any>>() };
        unsafe { Type::from_raw_parts(ty, info) }
    }
}
impl<'ctx, T: TypeTag> Value<'ctx, Ref<Memory<T>>> {
    /// Get a reference to the element at this position.
    pub unsafe fn elem_ref(self) -> Value<'ctx, Ref<T>> {
        let (val, info) = unsafe { self.get_unchecked::<Value<Any>>() };
        unsafe { Value::from_raw_parts(val, info.elem_type()) }
    }
}

pub struct TypedArray<T>(Void, PhantomData<fn() -> T>);
impl<T: TypeTag> TypeTag for TypedArray<T> {
    type Metadata<'ctx> = T::Metadata<'ctx>;
}
unsafe impl<T: SubtypeOf<U>, U: TypeTag> SubtypeOf<TypedArray<U>> for TypedArray<T> {
    fn cast_metadata<'ctx>(
        meta: Self::Metadata<'ctx>,
    ) -> <TypedArray<U> as TypeTag>::Metadata<'ctx> {
        T::cast_metadata(meta)
    }
}
type_tag_generic!(TypedArray : Array, Basic);
impl<'ctx, T: TypeTag> Value<'ctx, Ref<TypedArray<T>>> {
    fn as_mem(self) -> Value<'ctx, Ref<Memory<T>>> {
        let (val, arr_ty) = self.get::<PointerValue>();
        let (ty, info) = arr_ty.get::<ArrayType>();
        let new_ty = unsafe { Type::<Memory<T>>::from_raw_parts(ty.get_element_type(), info) };
        unsafe { Value::from_raw_parts(val, new_ty) }
    }
}
