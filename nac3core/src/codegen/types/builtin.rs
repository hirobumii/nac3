use inkwell::types::StructType;

use crate::codegen::{ModuleContext, types::structure::StructFields};

#[derive(Debug, Clone, Copy)]
pub struct BuiltinStruct<'ctx, T> {
    pub fields: T,
    pub llvm_ty: StructType<'ctx>,
}

impl<'ctx, T: StructFields<'ctx>> BuiltinStruct<'ctx, T> {
    pub(in crate::codegen) fn new(ctx: &ModuleContext<'ctx>, name: &str) -> Self {
        Self::from_fields(ctx, T::new(ctx), name)
    }

    pub(in crate::codegen) fn from_fields(
        ctx: &ModuleContext<'ctx>,
        fields: T,
        name: &str,
    ) -> Self {
        let llvm_ty = ctx.ctx.get_struct_type(name).unwrap_or_else(|| {
            let llvm_ty = ctx.ctx.opaque_struct_type(name);
            let field_tys = fields.field_tys();
            llvm_ty.set_body(&field_tys, false);
            llvm_ty
        });
        Self { fields, llvm_ty }
    }
}
