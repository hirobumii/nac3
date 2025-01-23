use inkwell::{
    types::IntType,
    values::{IntValue, PointerValue},
};

use nac3parser::ast::Expr;

use crate::{
    codegen::{
        types::{structure::StructField, utils::SliceType},
        values::ProxyValue,
        CodeGenContext, CodeGenerator,
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

    pub fn load_start_defined(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.start_defined_field().get(ctx, self.value, self.name)
    }

    fn start_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().start
    }

    pub fn load_start(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.start_field().get(ctx, self.value, self.name)
    }

    pub fn store_start(&self, ctx: &CodeGenContext<'ctx, '_>, value: Option<IntValue<'ctx>>) {
        match value {
            Some(start) => {
                self.start_defined_field().set(
                    ctx,
                    self.value,
                    ctx.ctx.bool_type().const_all_ones(),
                    self.name,
                );
                self.start_field().set(ctx, self.value, start, self.name);
            }

            None => self.start_defined_field().set(
                ctx,
                self.value,
                ctx.ctx.bool_type().const_zero(),
                self.name,
            ),
        }
    }

    fn stop_defined_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().stop_defined
    }

    pub fn load_stop_defined(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.stop_defined_field().get(ctx, self.value, self.name)
    }

    fn stop_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().stop
    }

    pub fn load_stop(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.stop_field().get(ctx, self.value, self.name)
    }

    pub fn store_stop(&self, ctx: &CodeGenContext<'ctx, '_>, value: Option<IntValue<'ctx>>) {
        match value {
            Some(stop) => {
                self.stop_defined_field().set(
                    ctx,
                    self.value,
                    ctx.ctx.bool_type().const_all_ones(),
                    self.name,
                );
                self.stop_field().set(ctx, self.value, stop, self.name);
            }

            None => self.stop_defined_field().set(
                ctx,
                self.value,
                ctx.ctx.bool_type().const_zero(),
                self.name,
            ),
        }
    }

    fn step_defined_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().step_defined
    }

    pub fn load_step_defined(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.step_defined_field().get(ctx, self.value, self.name)
    }

    fn step_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().step
    }

    pub fn load_step(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.step_field().get(ctx, self.value, self.name)
    }

    pub fn store_step(&self, ctx: &CodeGenContext<'ctx, '_>, value: Option<IntValue<'ctx>>) {
        match value {
            Some(step) => {
                self.step_defined_field().set(
                    ctx,
                    self.value,
                    ctx.ctx.bool_type().const_all_ones(),
                    self.name,
                );
                self.step_field().set(ctx, self.value, step, self.name);
            }

            None => self.step_defined_field().set(
                ctx,
                self.value,
                ctx.ctx.bool_type().const_zero(),
                self.name,
            ),
        }
    }
}

impl<'ctx> ProxyValue<'ctx> for SliceValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Type = SliceType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_type(self.value.get_type(), self.int_ty, self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

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
    ) -> Result<RustSlice<'ctx>, String> {
        let mut value_mapper = |value_expr: &Option<Box<Expr<Option<Type>>>>| -> Result<_, String> {
            Ok(match value_expr {
                None => None,
                Some(value_expr) => {
                    let value_expr = generator
                        .gen_expr(ctx, value_expr)?
                        .map(|value| {
                            value.to_basic_value_enum(ctx, generator, ctx.primitives.int32)
                        })
                        .unwrap()?;

                    Some(value_expr.into_int_value())
                }
            })
        };

        let start = value_mapper(lower)?;
        let stop = value_mapper(upper)?;
        let step = value_mapper(step)?;

        Ok(RustSlice { int_ty: ctx.ctx.i32_type(), start, stop, step })
    }

    /// Write the contents to an LLVM [`SliceValue`].
    pub fn write_to_slice(&self, ctx: &CodeGenContext<'ctx, '_>, dst_slice_ptr: SliceValue<'ctx>) {
        assert_eq!(self.int_ty, dst_slice_ptr.int_ty);

        dst_slice_ptr.store_start(ctx, self.start);
        dst_slice_ptr.store_stop(ctx, self.stop);
        dst_slice_ptr.store_step(ctx, self.step);
    }
}
