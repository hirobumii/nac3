use inkwell::{types::BasicTypeEnum, values::PointerValue};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext,
    types::{ArraySliceValue, BuiltinStruct, Value, structure::StructField},
};

#[derive(Clone, Copy, StructFields)]
pub struct TypeinfoStructFields<'ctx> {
    /// Pointer to the name of the type, as a [`StringType`] value.
    #[value_type(ptr)]
    pub name: StructField<'ctx, PointerValue<'ctx>>,

    /// Array pointer to content.
    #[value_type(ptr)]
    pub refcounted_fields: StructField<'ctx, PointerValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ty(PointerValue<'ctx>, ctx.ptr)]
pub struct TypeinfoType<'ctx> {
    pub inner: BuiltinStruct<'ctx, TypeinfoStructFields<'ctx>>,
}

impl<'ctx> TypeinfoType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "__nac3_typeinfo") }
    }

    /// Returns the LLVM type used for allocating this type.
    pub fn alloca_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.inner.llvm_ty.into()
    }
}

pub type TypeinfoValue<'ctx> = Value<'ctx, TypeinfoType<'ctx>>;

impl<'ctx> TypeinfoValue<'ctx> {
    /// Returns an [`ArraySliceValue`] representing the number of reference-counted fields and their
    /// byte offsets from the start of the object.
    pub fn refcounted_fields(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        let struct_ty = self.ty.alloca_ty(ctx);
        let ptr = self.ty.inner.fields.refcounted_fields.load(ctx, struct_ty, self.value, None)?;
        let pcount =
            unsafe { ctx.builder.build_gep(ctx.i32, ptr, &[ctx.size_t.const_zero()], "").unwrap() };
        let count = ctx.builder.build_load(ctx.i32, pcount, "")?.into_int_value();
        let sizeof_i32 =
            ctx.builder.build_int_truncate_or_bit_cast(ctx.i32.size_of(), ctx.size_t, "")?;
        let arr_begin = unsafe { ctx.builder.build_gep(ctx.i8, ptr, &[sizeof_i32], "")? };
        Ok(ArraySliceValue::new(ctx.i32.into(), arr_begin, count, self.name))
    }
}
