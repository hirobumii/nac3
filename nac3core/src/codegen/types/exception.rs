use inkwell::values::{IntValue, StructValue};
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        CodeGenContext, ModuleContext,
        types::{Value, builtin::BuiltinStruct, structure::StructField},
    },
    typecheck::typedef::{Type, TypeEnum},
};

#[derive(Clone, Copy, StructFields)]
pub struct ExceptionStructFields<'ctx> {
    /// The ID of the exception name.
    #[value_type(i32)]
    pub name: StructField<'ctx, IntValue<'ctx>>,

    /// The file where the exception originated from.
    #[value_type(ctx.get_struct_type("str").unwrap())]
    pub file: StructField<'ctx, StructValue<'ctx>>,

    /// The line number where the exception originated from.
    #[value_type(i32)]
    pub line: StructField<'ctx, IntValue<'ctx>>,

    /// The column number where the exception originated from.
    #[value_type(i32)]
    pub col: StructField<'ctx, IntValue<'ctx>>,

    /// The function name where the exception originated from.
    #[value_type(ctx.get_struct_type("str").unwrap())]
    pub func: StructField<'ctx, StructValue<'ctx>>,

    /// The exception message.
    #[value_type(ctx.get_struct_type("str").unwrap())]
    pub message: StructField<'ctx, StructValue<'ctx>>,

    #[value_type(i64)]
    pub param0: StructField<'ctx, IntValue<'ctx>>,

    #[value_type(i64)]
    pub param1: StructField<'ctx, IntValue<'ctx>>,

    #[value_type(i64)]
    pub param2: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct ExceptionType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ExceptionStructFields<'ctx>>,
}

impl<'ctx> ExceptionType<'ctx> {
    /// Creates an instance of [`ExceptionType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "exception") }
    }

    /// Creates an [`ExceptionType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        assert!(
            matches!(&*ctx.unifier.get_ty_immutable(ty), TypeEnum::TObj { obj_id, .. } if *obj_id == ctx.primitives.exception.obj_id(&ctx.unifier).unwrap())
        );
        Self::new(ctx)
    }
}

pub type ExceptionValue<'ctx> = Value<'ctx, ExceptionType<'ctx>>;
