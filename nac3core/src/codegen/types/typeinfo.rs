use inkwell::values::{PointerValue, StructValue};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext, typed_load,
    types::{ArraySliceValue, BuiltinStruct, ProxyTypeExt, Value, structure::StructField},
};

#[derive(Clone, Copy, StructFields)]
pub struct TypeinfoStructFields<'ctx> {
    /// Array pointer to content.
    #[value_type(ptr)]
    pub refcounted_fields: StructField<'ctx, PointerValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ty(StructValue<'ctx>, self.inner.llvm_ty)]
pub struct TypeinfoType<'ctx> {
    pub inner: BuiltinStruct<'ctx, TypeinfoStructFields<'ctx>>,
}

impl<'ctx> TypeinfoType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "__nac3_typeinfo") }
    }

    /// Creates a constant value of this type.
    #[must_use]
    pub fn constant(
        &self,
        _ctx: &CodeGenContext<'ctx, '_>,
        refcounted_fields_arr: PointerValue<'ctx>,
        name: Option<&'static str>,
    ) -> TypeinfoValue<'ctx> {
        let value = self.inner.llvm_ty.const_named_struct(&[refcounted_fields_arr.into()]);
        self.map_value(value, name)
    }
}

pub type TypeinfoValue<'ctx> = Value<'ctx, TypeinfoType<'ctx>>;

impl<'ctx> TypeinfoValue<'ctx> {
    /// Returns an [`ArraySliceValue`] representing the number of reference-counted fields and their
    /// byte offsets from the start of the object.
    pub fn refcounted_fields(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> ArraySliceValue<'ctx> {
        let ptr = self.ty.inner.fields.refcounted_fields.extract_value(ctx, self.value);
        let pcount = unsafe { ctx.builder.build_gep(ptr, &[ctx.size_t.const_zero()], "").unwrap() };
        let count = typed_load(&ctx.builder, pcount, ctx.i32.into(), "").into_int_value();
        let arr_begin = unsafe {
            ctx.builder
                .build_gep(ptr, &[ctx.i32.size_of().const_cast(ctx.size_t, false)], "")
                .unwrap()
        };
        ArraySliceValue::new(ctx.i32.into(), arr_begin, count, self.name)
    }
}
