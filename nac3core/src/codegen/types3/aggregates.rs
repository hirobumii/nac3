use std::marker::PhantomData;

use inkwell::{
    builder::Builder,
    context::Context,
    values::{ArrayValue, PointerValue, StructValue},
};

use super::{
    Any, Basic, BasicTag, Memory, Ref, Struct, SubtypeOf, Type, TypeTag, TypedArray, Value,
};

pub struct LlvmArrayField<'ctx, T: TypeTag> {
    offset: u32,
    _phantom: PhantomData<fn() -> (&'ctx (), T)>,
}

impl<'ctx, T: TypeTag> LlvmArrayField<'ctx, T> {
    pub unsafe fn new(offset: u32) -> Self {
        Self { offset, _phantom: PhantomData }
    }

    pub fn extract(
        self,
        builder: &Builder<'ctx>,
        value: Value<'ctx, TypedArray<T>>,
    ) -> Value<'ctx, T> {
        let (value, info) = unsafe { value.get_unchecked::<ArrayValue>() };
        let result = builder.build_extract_value(value, self.offset, "").unwrap();
        unsafe { Value::from_raw_parts(result, info) }
    }

    pub fn insert(
        self,
        builder: &Builder<'ctx>,
        value: Value<'ctx, TypedArray<T>>,
        elem: Value<'ctx, T>,
    ) -> Value<'ctx, TypedArray<T>>
    where
        T: BasicTag,
    {
        let (value, info) = unsafe { value.get_unchecked::<ArrayValue>() };
        let value = builder
            .build_insert_value(value, elem.get::<Value<Basic>>().0, self.offset, "")
            .unwrap();
        unsafe { Value::from_raw_parts(value, info) }
    }

    pub fn offset(
        self,
        ctx: &'ctx Context,
        builder: &Builder<'ctx>,
        value: Value<'ctx, Ref<Memory<T>>>,
    ) -> Value<'ctx, Ref<Memory<T>>>
    where
        T: BasicTag,
    {
        let (ptr, base_ty) = value.get::<PointerValue>();
        let ty = base_ty.clone().elem_type().get::<Type<Basic>>().0;

        let int = ctx.i32_type();
        unsafe {
            let result =
                builder.build_gep(ty, ptr, &[int.const_int(self.offset as _, false)], "").unwrap();
            Value::from_raw_parts(result, base_ty)
        }
    }
}

pub struct LlvmStructField<'ctx, Base, T: TypeTag> {
    offset: u32,
    ty: Type<'ctx, T>,
    _phantom: PhantomData<fn(Base) -> T>,
}
impl<'ctx, Base: TypeTag, T: TypeTag> LlvmStructField<'ctx, Base, T> {
    pub unsafe fn new(offset: u32, ty: Type<'ctx, T>) -> Self {
        Self { offset, ty, _phantom: PhantomData }
    }

    pub fn extract(self, builder: &Builder<'ctx>, value: Value<'ctx, Base>) -> Value<'ctx, T>
    where
        Base: SubtypeOf<Struct>,
    {
        let value =
            builder.build_extract_value(value.get::<StructValue>().0, self.offset, "").unwrap();
        unsafe { Value::from_raw_parts(value, self.ty.get::<Type<Any>>().1) }
    }

    pub fn insert(
        self,
        builder: &Builder<'ctx>,
        value: Value<'ctx, Base>,
        elem: Value<'ctx, T>,
    ) -> Value<'ctx, Base>
    where
        Base: SubtypeOf<Struct>,
        T: BasicTag,
    {
        let (value, info) = value.get::<StructValue>();
        let value = builder
            .build_insert_value(value, elem.get::<Value<Basic>>().0, self.offset, "")
            .unwrap();
        unsafe { Value::from_raw_parts(value, info) }
    }

    pub fn gep(
        self,
        ctx: &'ctx Context,
        builder: &Builder<'ctx>,
        value: Value<'ctx, Ref<Base>>,
    ) -> Value<'ctx, Ref<T>>
    where
        Base: BasicTag,
    {
        let (ptr, base_ty) = value.get::<PointerValue>();
        let int = ctx.i32_type();
        unsafe {
            let result = builder
                .build_gep(
                    base_ty.get::<Type<Basic>>().0,
                    ptr,
                    &[int.const_zero(), int.const_int(self.offset as _, false)],
                    "",
                )
                .unwrap();
            Value::from_raw_parts(result, self.ty)
        }
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! __codegen_define_structs_impl {
    (@count_tts ) => { 0u32 };
    (@count_tts $odd:tt $($a:tt $b:tt)*) => { ($crate::__codegen_define_structs_impl!(@count_tts $($a)*) << 1u32) | 1u32 };
    (@count_tts $($a:tt $even:tt)*) => { $crate::__codegen_define_structs_impl!(@count_tts $($a)*) << 1u32 };

    ([$(($(#[$attr:meta])* $v:vis $field:ident ($count:expr): $field_ty:ty))*] $s:ident $sfields:ident {}) => {
        pub struct $sfields<'ctx> {
            $($(#[$attr])* $v $field: $crate::codegen::types3::LlvmStructField<'ctx, $s, $field_ty>,)*
        }

        impl $s {
            pub fn get_type<'ctx>(ctx: &'ctx Context, $($field: Type<'ctx, $field_ty>),*)
                -> (Type<'ctx, Self>, $sfields<'ctx>)
            {
                // SAFETY: We know that the $count-th field is exactly of type `$field`.
                let fields = unsafe {
                    $sfields {
                        $($field: $crate::codegen::types3::LlvmStructField::new($count, $field)),*
                    }
                };
                let opaque = ctx.opaque_struct_type(stringify!($s));
                opaque.set_body(&[$($field.get::<BasicTypeEnum>().0),*], false);
                (unsafe { Type::from_raw_parts(opaque, ()) }, fields)
            }
        }
    };
    ([$($t:tt)*] $s:ident $sfields:ident { $(#[$attr:meta])* $v:vis  $field:ident: $field_ty:ty, $($rest:tt)* }) => {
        $crate::__codegen_define_structs_impl! {
            [$($t)* ($(#[$attr])* $v $field ($crate::__codegen_define_structs_impl!(@count_tts $($t)*)): $field_ty)]
            $s $sfields { $($rest)* }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __codegen_define_structs {
    {$(
        #[fields($struct_fields_ty:ident)]
        $(#[$struct_attr:meta])*
        struct $struct_name:ident {$(
            $(#[$field_attr:meta])*
            $vis:vis $field:ident: $field_ty:ty,
        )*}
    )*} => {$(
        $(#[$struct_attr])*
        pub enum $struct_name {}
        $crate::codegen::types3::type_tag!($struct_name : Struct, Basic);

        $crate::__codegen_define_structs_impl! { [] $struct_name $struct_fields_ty { $($(#[$field_attr])* $vis $field: $field_ty,)* } }
    )*};
}
#[doc(inline)]
pub use crate::__codegen_define_structs as make_structs;
