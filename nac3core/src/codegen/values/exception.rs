use inkwell::{
    types::IntType,
    values::{IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use nac3parser::ast::Location;

use super::{ProxyValue, StringValue, structure::StructProxyValue};
use crate::codegen::{
    CodeGenContext, CodeGenerator,
    types::{
        ExceptionType,
        structure::{StructField, StructProxyType},
    },
};

/// Proxy type for accessing an `Exception` value in LLVM.
#[derive(Copy, Clone)]
pub struct ExceptionValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> ExceptionValue<'ctx> {
    /// Creates an [`ExceptionValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let pval = generator
            .gen_var_alloc(
                ctx,
                val.get_type().into(),
                name.map(|name| format!("{name}.addr")).as_deref(),
            )
            .unwrap();
        ctx.builder.build_store(pval, val).unwrap();
        Self::from_pointer_value(pval, llvm_usize, name)
    }

    /// Creates an [`ExceptionValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        Self { value: ptr, llvm_usize, name }
    }

    fn name_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().name
    }

    /// Stores the ID of the exception name into this instance.
    pub fn store_name(&self, ctx: &CodeGenContext<'ctx, '_>, name: IntValue<'ctx>) {
        debug_assert_eq!(name.get_type(), ctx.ctx.i32_type());

        self.name_field().store(ctx, self.value, name, self.name);
    }

    fn file_field(&self) -> StructField<'ctx, StructValue<'ctx>> {
        self.get_type().get_fields().file
    }

    /// Stores the file name of the exception source into this instance.
    pub fn store_file(&self, ctx: &CodeGenContext<'ctx, '_>, file: StructValue<'ctx>) {
        debug_assert!(StringValue::is_instance(file, self.llvm_usize).is_ok());

        self.file_field().store(ctx, self.value, file, self.name);
    }

    fn line_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().line
    }

    fn col_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().col
    }

    /// Stores the [location][Location] of the exception source into this instance.
    pub fn store_location<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        location: Location,
    ) {
        let llvm_i32 = ctx.ctx.i32_type();

        let filename = ctx.gen_string(generator, location.file.0);
        self.store_file(ctx, filename);

        self.line_field().store(
            ctx,
            self.value,
            llvm_i32.const_int(location.row as u64, false),
            self.name,
        );
        self.col_field().store(
            ctx,
            self.value,
            llvm_i32.const_int(location.column as u64, false),
            self.name,
        );
    }

    fn func_field(&self) -> StructField<'ctx, StructValue<'ctx>> {
        self.get_type().get_fields().func
    }

    /// Stores the function name of the exception source into this instance.
    pub fn store_func(&self, ctx: &CodeGenContext<'ctx, '_>, func: StructValue<'ctx>) {
        debug_assert!(StringValue::is_instance(func, self.llvm_usize).is_ok());

        self.func_field().store(ctx, self.value, func, self.name);
    }

    fn message_field(&self) -> StructField<'ctx, StructValue<'ctx>> {
        self.get_type().get_fields().message
    }

    /// Stores the exception message into this instance.
    pub fn store_message(&self, ctx: &CodeGenContext<'ctx, '_>, message: StructValue<'ctx>) {
        debug_assert!(StringValue::is_instance(message, self.llvm_usize).is_ok());

        self.message_field().store(ctx, self.value, message, self.name);
    }

    fn param0_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().param0
    }

    fn param1_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().param1
    }

    fn param2_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().param2
    }

    /// Stores the parameters of the exception into this instance.
    ///
    /// If the parameter does not exist, pass `i64 0` in the parameter slot.
    pub fn store_params(&self, ctx: &CodeGenContext<'ctx, '_>, params: &[IntValue<'ctx>; 3]) {
        debug_assert!(params.iter().all(|p| p.get_type() == ctx.ctx.i64_type()));

        [self.param0_field(), self.param1_field(), self.param2_field()]
            .into_iter()
            .zip_eq(params)
            .for_each(|(field, param)| {
                field.store(ctx, self.value, *param, self.name);
            });
    }
}

impl<'ctx> ProxyValue<'ctx> for ExceptionValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = ExceptionType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_pointer_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for ExceptionValue<'ctx> {}

impl<'ctx> From<ExceptionValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ExceptionValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
