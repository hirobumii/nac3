use inkwell::{
    types::IntType,
    values::{ArrayValue, BasicValueEnum, IntValue, PointerValue},
};

use super::ProxyValue;
use crate::codegen::{CodeGenContext, CodeGenerator, types::RangeType};

/// Proxy type for accessing a `range` value in LLVM.
#[derive(Copy, Clone)]
pub struct RangeValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> RangeValue<'ctx> {
    /// Creates an [`RangeValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_array_value<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: ArrayValue<'ctx>,
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

    /// Creates an [`RangeValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        RangeValue { value: ptr, llvm_usize, name }
    }

    fn ptr_to_start(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.i32;
        let var_name = self.name.map(|v| format!("{v}.start.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_abi_value(ctx),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(0, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    fn ptr_to_end(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.i32;
        let var_name = self.name.map(|v| format!("{v}.end.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_abi_value(ctx),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    fn ptr_to_step(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.i32;
        let var_name = self.name.map(|v| format!("{v}.step.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_abi_value(ctx),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(2, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the `start` value into this instance.
    pub fn store_start(&self, ctx: &mut CodeGenContext<'ctx, '_>, start: IntValue<'ctx>) {
        debug_assert_eq!(start.get_type().get_bit_width(), 32);

        let pstart = self.ptr_to_start(ctx);
        ctx.builder.build_store(pstart, start).unwrap();
    }

    /// Returns the `start` value of this `range`.
    pub fn load_start(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> IntValue<'ctx> {
        let pstart = self.ptr_to_start(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.start")))
            .unwrap_or_default();

        ctx.builder
            .build_load(pstart, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }

    /// Stores the `end` value into this instance.
    pub fn store_end(&self, ctx: &mut CodeGenContext<'ctx, '_>, end: IntValue<'ctx>) {
        debug_assert_eq!(end.get_type().get_bit_width(), 32);

        let pend = self.ptr_to_end(ctx);
        ctx.builder.build_store(pend, end).unwrap();
    }

    /// Returns the `end` value of this `range`.
    pub fn load_end(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> IntValue<'ctx> {
        let pend = self.ptr_to_end(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.end")))
            .unwrap_or_default();

        ctx.builder.build_load(pend, var_name.as_str()).map(BasicValueEnum::into_int_value).unwrap()
    }

    /// Stores the `step` value into this instance.
    pub fn store_step(&self, ctx: &mut CodeGenContext<'ctx, '_>, step: IntValue<'ctx>) {
        debug_assert_eq!(step.get_type().get_bit_width(), 32);

        let pstep = self.ptr_to_step(ctx);
        ctx.builder.build_store(pstep, step).unwrap();
    }

    /// Returns the `step` value of this `range`.
    pub fn load_step(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&str>,
    ) -> IntValue<'ctx> {
        let pstep = self.ptr_to_step(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.step")))
            .unwrap_or_default();

        ctx.builder
            .build_load(pstep, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }
}

impl<'ctx> ProxyValue<'ctx> for RangeValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = RangeType<'ctx>;

    fn get_type(&self) -> Self::Type {
        RangeType::from_pointer_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> From<RangeValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: RangeValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
