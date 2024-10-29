use inkwell::values::{BasicValueEnum, IntValue, PointerValue};

use super::ProxyValue;
use crate::codegen::{types::RangeType, CodeGenContext};

/// Proxy type for accessing a `range` value in LLVM.
#[derive(Copy, Clone)]
pub struct RangeValue<'ctx> {
    value: PointerValue<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> RangeValue<'ctx> {
    /// Checks whether `value` is an instance of `range`, returning [Err] if `value` is not an instance.
    pub fn is_instance(value: PointerValue<'ctx>) -> Result<(), String> {
        RangeType::is_type(value.get_type())
    }

    /// Creates an [`RangeValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(ptr: PointerValue<'ctx>, name: Option<&'ctx str>) -> Self {
        debug_assert!(Self::is_instance(ptr).is_ok());

        RangeValue { value: ptr, name }
    }

    fn ptr_to_start(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.start.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(0, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    fn ptr_to_end(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.end.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    fn ptr_to_step(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.step.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(2, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the `start` value into this instance.
    pub fn store_start(&self, ctx: &CodeGenContext<'ctx, '_>, start: IntValue<'ctx>) {
        debug_assert_eq!(start.get_type().get_bit_width(), 32);

        let pstart = self.ptr_to_start(ctx);
        ctx.builder.build_store(pstart, start).unwrap();
    }

    /// Returns the `start` value of this `range`.
    pub fn load_start(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
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
    pub fn store_end(&self, ctx: &CodeGenContext<'ctx, '_>, end: IntValue<'ctx>) {
        debug_assert_eq!(end.get_type().get_bit_width(), 32);

        let pend = self.ptr_to_end(ctx);
        ctx.builder.build_store(pend, end).unwrap();
    }

    /// Returns the `end` value of this `range`.
    pub fn load_end(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pend = self.ptr_to_end(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.end")))
            .unwrap_or_default();

        ctx.builder.build_load(pend, var_name.as_str()).map(BasicValueEnum::into_int_value).unwrap()
    }

    /// Stores the `step` value into this instance.
    pub fn store_step(&self, ctx: &CodeGenContext<'ctx, '_>, step: IntValue<'ctx>) {
        debug_assert_eq!(step.get_type().get_bit_width(), 32);

        let pstep = self.ptr_to_step(ctx);
        ctx.builder.build_store(pstep, step).unwrap();
    }

    /// Returns the `step` value of this `range`.
    pub fn load_step(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
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
    type Base = PointerValue<'ctx>;
    type Type = RangeType<'ctx>;

    fn get_type(&self) -> Self::Type {
        RangeType::from_type(self.value.get_type())
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

impl<'ctx> From<RangeValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: RangeValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
