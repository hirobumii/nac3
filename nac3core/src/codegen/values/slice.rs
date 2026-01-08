use inkwell::{
    types::IntType,
    values::{IntValue, PointerValue, StructValue},
};

use nac3parser::ast::Expr;

use crate::{
    codegen::{
        CodeGenContext, CodeGenerator,
        stmt::gen_var,
        types::{
            structure::{StructField, StructProxyType},
            slice::SliceType,
        },
        values::{ProxyValue, structure::StructProxyValue},
    },
    typecheck::typedef::Type,
};

/// An IRRT representation of an (unresolved) slice.
#[derive(Copy, Clone)]
pub struct SliceValue<'ctx> {
    value: PointerValue<'ctx>,
    int_ty: IntType<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> SliceValue<'ctx> {
    /// Creates an [`SliceValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        int_ty: IntType<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let pval =
            gen_var(ctx, val.get_type().into(), name.map(|name| format!("{name}.addr")).as_deref())
                .unwrap();
        ctx.builder.build_store(pval, val).unwrap();
        Self::from_pointer_value(pval, int_ty, llvm_usize, name)
    }

    /// Creates an [`SliceValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        int_ty: IntType<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        Self { value: ptr, int_ty, llvm_usize, name }
    }

    fn start_defined_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().start_defined
    }

    pub fn load_start_defined(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.start_defined_field().load(ctx, self.value, self.name)
    }

    fn start_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().start
    }

    pub fn load_start(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.start_field().load(ctx, self.value, self.name)
    }

    pub fn store_start(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: Option<IntValue<'ctx>>) {
        match value {
            Some(start) => {
                self.start_defined_field().store(
                    ctx,
                    self.value,
                    ctx.i1.const_all_ones(),
                    self.name,
                );
                self.start_field().store(ctx, self.value, start, self.name);
            }

            None => {
                self.start_defined_field().store(ctx, self.value, ctx.i1.const_zero(), self.name);
            }
        }
    }

    fn stop_defined_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().stop_defined
    }

    pub fn load_stop_defined(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.stop_defined_field().load(ctx, self.value, self.name)
    }

    fn stop_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().stop
    }

    pub fn load_stop(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.stop_field().load(ctx, self.value, self.name)
    }

    pub fn store_stop(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: Option<IntValue<'ctx>>) {
        match value {
            Some(stop) => {
                self.stop_defined_field().store(
                    ctx,
                    self.value,
                    ctx.i1.const_all_ones(),
                    self.name,
                );
                self.stop_field().store(ctx, self.value, stop, self.name);
            }

            None => {
                self.stop_defined_field().store(ctx, self.value, ctx.i1.const_zero(), self.name);
            }
        }
    }

    fn step_defined_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().step_defined
    }

    pub fn load_step_defined(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.step_defined_field().load(ctx, self.value, self.name)
    }

    fn step_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().step
    }

    pub fn load_step(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.step_field().load(ctx, self.value, self.name)
    }

    pub fn store_step(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: Option<IntValue<'ctx>>) {
        match value {
            Some(step) => {
                self.step_defined_field().store(
                    ctx,
                    self.value,
                    ctx.i1.const_all_ones(),
                    self.name,
                );
                self.step_field().store(ctx, self.value, step, self.name);
            }

            None => {
                self.step_defined_field().store(ctx, self.value, ctx.i1.const_zero(), self.name);
            }
        }
    }
}

impl<'ctx> ProxyValue<'ctx> for SliceValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = SliceType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_pointer_type(self.value.get_type(), self.int_ty, self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for SliceValue<'ctx> {}

impl<'ctx> From<SliceValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: SliceValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// A slice represented in compile-time by `start`, `stop` and `step`, all held as LLVM values.
// TODO: Rename this to CTConstSlice
#[derive(Debug, Copy, Clone)]
pub struct RustSlice<'ctx> {
    int_ty: IntType<'ctx>,
    start: Option<IntValue<'ctx>>,
    stop: Option<IntValue<'ctx>>,
    step: Option<IntValue<'ctx>>,
}

impl<'ctx> RustSlice<'ctx> {
    /// Generate LLVM IR for an [`ExprKind::Slice`] and convert it into a [`RustSlice`].
    #[allow(clippy::type_complexity)]
    pub fn from_slice_expr<G: CodeGenerator>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        lower: &Option<Box<Expr<Option<Type>>>>,
        upper: &Option<Box<Expr<Option<Type>>>>,
        step: &Option<Box<Expr<Option<Type>>>>,
    ) -> Result<Self, String> {
        let mut value_mapper = |value_expr: &Option<Box<Expr<Option<Type>>>>| -> Result<_, String> {
            Ok(match value_expr {
                None => None,
                Some(value_expr) => {
                    let value = generator.gen_expr(ctx, value_expr)?.to_basic_value_enum(ctx)?;

                    Some(value.into_int_value())
                }
            })
        };

        let start = value_mapper(lower)?;
        let stop = value_mapper(upper)?;
        let step = value_mapper(step)?;

        Ok(RustSlice { int_ty: ctx.i32, start, stop, step })
    }

    /// Write the contents to an LLVM [`SliceValue`].
    pub fn write_to_slice(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dst_slice_ptr: SliceValue<'ctx>,
    ) {
        assert_eq!(self.int_ty, dst_slice_ptr.int_ty);

        dst_slice_ptr.store_start(ctx, self.start);
        dst_slice_ptr.store_stop(ctx, self.stop);
        dst_slice_ptr.store_step(ctx, self.step);
    }
}
