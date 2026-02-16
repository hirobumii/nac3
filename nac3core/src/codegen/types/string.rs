use inkwell::values::{IntValue, PointerValue, StructValue};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext,
    types::{ProxyTypeExt, Value, builtin::BuiltinStruct, structure::StructField},
};

#[derive(Clone, Copy, StructFields)]
pub struct StringStructFields<'ctx> {
    /// Pointer to the first character of the string.
    #[value_type(ptr)]
    pub ptr: StructField<'ctx, PointerValue<'ctx>>,

    /// Length of the string.
    #[value_type(size_t)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ty(StructValue<'ctx>, self.inner.llvm_ty)]
pub struct StringType<'ctx> {
    pub(crate) inner: BuiltinStruct<'ctx, StringStructFields<'ctx>>,
}

impl<'ctx> StringType<'ctx> {
    /// Creates an instance of [`StringType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "str") }
    }

    pub fn constant(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        v: &str,
        name: Option<&'static str>,
    ) -> anyhow::Result<StringValue<'ctx>> {
        let str_ptr = ctx.builder.build_global_string_ptr(v, "const")?.as_pointer_value();
        let size = ctx.size_t.const_int(v.len() as u64, false);
        let value = self.inner.llvm_ty.const_named_struct(&[str_ptr.into(), size.into()]);
        Ok(self.map_value(value, name))
    }
}

pub type StringValue<'ctx> = Value<'ctx, StringType<'ctx>>;

impl<'ctx> StringValue<'ctx> {
    /// Returns the pointer to the string data.
    pub fn ptr(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>> {
        self.ty.inner.fields.ptr.extract_value(ctx, self.value)
    }

    /// Returns the length of the string.
    pub fn len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        self.ty.inner.fields.len.extract_value(ctx, self.value)
    }
}
