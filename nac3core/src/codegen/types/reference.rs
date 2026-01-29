use inkwell::{
    types::{BasicType, BasicTypeEnum, StructType},
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext, typed_load,
    types::{BuiltinStruct, ProxyType, ProxyTypeBase, RefType, Value, structure::StructField},
};

#[derive(Clone, Copy, StructFields)]
pub struct ObjectHeaderStructFields<'ctx> {
    /// The reference count of this object.
    #[value_type(i32)]
    pub refcount: StructField<'ctx, IntValue<'ctx>>,

    /// The offset of the `typeinfo` global structure from `__nac3_global_begin`.
    #[value_type(i32)]
    pub typeinfo_offset: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct ObjectHeaderType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ObjectHeaderStructFields<'ctx>>,
}

impl<'ctx> ObjectHeaderType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "__nac3_object_header") }
    }
}

pub type ObjectHeaderValue<'ctx> = Value<'ctx, ObjectHeaderType<'ctx>>;

/// A trait indicating that a type is reference-counted.
pub trait RefCountedType<'ctx> {}

/// A trait indicating that a value is reference-counted.
pub trait RefCountedValue<'ctx> {
    /// Returns an opaque variant of this refcounted value.
    fn as_opaque(&self, ctx: &ModuleContext<'ctx>) -> OpaqueRefCountedValue<'ctx>;

    /// Gets the object header of this value.
    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx>;

    /// Returns a pointer to the inner data of this refcounted object.
    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx>;

    /// Returns a loaded value of the inner data of this refcounted object.
    fn inner_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        inner_ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        typed_load(ctx.builder, self.inner_ptr(ctx), inner_ty, name)
    }
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner)]
pub struct OpaqueRefCountedType<'ctx> {
    pub inner: StructType<'ctx>,
}

impl<'ctx> OpaqueRefCountedType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self {
            inner: ObjectHeaderType::new(ctx)
                .llvm_ty(ctx)
                .into_pointer_type()
                .get_element_type()
                .into_struct_type(),
        }
    }
}

impl<'ctx> RefCountedType<'ctx> for OpaqueRefCountedType<'ctx> {}

pub type OpaqueRefCountedValue<'ctx> = Value<'ctx, OpaqueRefCountedType<'ctx>>;

impl<'ctx> RefCountedValue<'ctx> for OpaqueRefCountedValue<'ctx> {
    fn as_opaque(&self, _ctx: &ModuleContext<'ctx>) -> Self {
        *self
    }

    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx> {
        ObjectHeaderType::new(ctx).map_value(self.value, self.name)
    }

    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let obj_header = ctx.builder.build_pointer_cast(self.value, ctx.ptr, "").unwrap();
        unsafe {
            ctx.builder
                .build_gep(
                    obj_header,
                    &[ObjectHeaderType::new(ctx)
                        .llvm_ty(ctx)
                        .size_of()
                        .unwrap()
                        .const_cast(ctx.size_t, false)],
                    "",
                )
                .unwrap()
        }
    }
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner)]
pub struct TypedRefCountedType<'ctx, T: RefType<'ctx> + Copy> {
    pub inner: StructType<'ctx>,
    pub object: T,
}

impl<'ctx, T: RefType<'ctx> + Copy> TypedRefCountedType<'ctx, T> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &mut CodeGenContext<'ctx, '_>, object_ty: T) -> Self {
        let object = object_ty.alloca_ty(ctx);

        Self {
            inner: ctx.ctx.struct_type(
                &[
                    ObjectHeaderType::new(ctx)
                        .llvm_ty(ctx)
                        .into_pointer_type()
                        .get_element_type()
                        .into_struct_type()
                        .into(),
                    object,
                ],
                false,
            ),
            object: object_ty,
        }
    }
}

impl<'ctx, T: RefType<'ctx> + Copy> RefCountedType<'ctx> for TypedRefCountedType<'ctx, T> {}

pub type TypedRefCountedValue<'ctx, T> = Value<'ctx, TypedRefCountedType<'ctx, T>>;

impl<'ctx, T: RefType<'ctx> + Copy> TypedRefCountedValue<'ctx, T> {
    /// Returns a loaded value of the inner data of this refcounted object.
    pub fn inner_value(&self) -> Value<'ctx, T> {
        self.ty.object.map_value(self.value, self.name)
    }
}

impl<'ctx, T: RefType<'ctx> + Copy> RefCountedValue<'ctx> for TypedRefCountedValue<'ctx, T> {
    fn as_opaque(&self, ctx: &ModuleContext<'ctx>) -> OpaqueRefCountedValue<'ctx> {
        OpaqueRefCountedType::new(ctx).map_value(self.value, self.name)
    }

    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx> {
        ObjectHeaderType::new(ctx).map_value(self.value, self.name)
    }

    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let obj_header = ctx.builder.build_pointer_cast(self.value, ctx.ptr, "").unwrap();
        unsafe {
            ctx.builder
                .build_gep(
                    obj_header,
                    &[ObjectHeaderType::new(ctx)
                        .llvm_ty(ctx)
                        .size_of()
                        .unwrap()
                        .const_cast(ctx.size_t, false)],
                    "",
                )
                .unwrap()
        }
    }
}
