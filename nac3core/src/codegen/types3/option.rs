use std::marker::PhantomData;

use inkwell::{builder::Builder, context::Context, types::PointerType, values::PointerValue};

use super::{Basic, Bool, RawPointer, Ref, Type, TypeTag, Value, ValueExt, Void, type_tag_generic};

pub struct Optional<T>(Void, PhantomData<fn(T) -> T>);
impl<T: TypeTag> TypeTag for Optional<T> {
    // *** Typed pointers! ***
    type Metadata<'ctx> = Type<'ctx, T>;
}
type_tag_generic!(Optional : RawPointer, Basic);
impl<T: TypeTag> Optional<T> {
    fn ty<'ctx>(ctx: &'ctx Context, ty: Type<'ctx, T>) -> Type<'ctx, Self> {
        unsafe { Type::from_raw_parts(RawPointer::ty(ctx), ty) }
    }
    fn none<'ctx>(ty: Type<'ctx, Self>) -> Value<'ctx, Self> {
        let (p, info) = ty.get::<PointerType>();
        unsafe { Value::from_raw_parts(p.const_null(), info) }
    }
    fn some<'ctx>(val: Value<'ctx, Ref<T>>) -> Value<'ctx, Self> {
        let (p, info) = val.get::<PointerValue>();
        unsafe { Value::from_raw_parts(p, info) }
    }
    fn is_some<'ctx>(builder: &Builder<'ctx>, val: Value<'ctx, Self>) -> Value<'ctx, Bool> {
        let p = val.get::<PointerValue>().0;
        let result = builder.build_is_not_null(p, "").unwrap();
        unsafe { Value::transmute_from(result) }
    }
    fn unwrap<'ctx>(val: Value<'ctx, Self>) -> Value<'ctx, Ref<T>> {
        let (p, info) = val.get::<PointerValue>();
        unsafe { Value::from_raw_parts(p, info) }
    }
}
