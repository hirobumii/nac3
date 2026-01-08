use inkwell::{
    types::IntType,
    values::{BasicValueEnum, IntValue, PointerValue},
};

use super::ProxyValue;

use crate::codegen::{CodeGenContext, types::EnumerateType};

/// Proxy type for accessing an `enumerate` value in LLVM.
#[derive(Copy, Clone)]
pub struct EnumerateValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> EnumerateValue<'ctx> {
    /// Creates an [`EnumerateValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());
        EnumerateValue { value: ptr, llvm_usize, name }
    }

    fn ptr_to_start(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.i32;

        let var_name = self.name.map(|v| format!("{v}.start.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.value,
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the start value
    pub fn store_start(&self, ctx: &mut CodeGenContext<'ctx, '_>, start: IntValue<'ctx>) {
        let p_idx = self.ptr_to_start(ctx);
        ctx.builder.build_store(p_idx, start).unwrap();
    }

    /// Loads the start value
    pub fn load_start(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let p_idx = self.ptr_to_start(ctx);
        ctx.builder.build_load(p_idx, "start").map(BasicValueEnum::into_int_value).unwrap()
    }

    pub fn ptr_to_iterable(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.i32;

        let var_name = self.name.map(|v| format!("{v}.iterable.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.value,
                    &[llvm_i32.const_zero(), llvm_i32.const_int(0, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    pub fn store_iterable(&self, ctx: &mut CodeGenContext<'ctx, '_>, iterable: PointerValue<'ctx>) {
        let p_iterable = self.ptr_to_iterable(ctx);
        ctx.builder.build_store(p_iterable, iterable).unwrap();
    }

    /// Loads the iterable pointer i8* from the enumerate struct.
    pub fn load_iterable(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let p_iterable = self.ptr_to_iterable(ctx);
        let iterable_struct_ptr = ctx
            .builder
            .build_load(p_iterable, "iterable")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        // The iterable is a struct { i8*, usize }
        let i8_ptr_ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    iterable_struct_ptr,
                    &[ctx.i32.const_zero(), ctx.i32.const_zero()],
                    "iterable.data.addr",
                )
                .unwrap()
        };

        ctx.builder
            .build_load(i8_ptr_ptr, "iterable.data")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    /// Loads the length of the iterable from the enumerate struct.
    pub fn load_iterable_len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let p_iterable = self.ptr_to_iterable(ctx);
        let iterable_struct_ptr = ctx
            .builder
            .build_load(p_iterable, "iterable")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        // The iterable is a struct { i8*, usize }
        let len_ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    iterable_struct_ptr,
                    &[ctx.i32.const_zero(), ctx.i32.const_int(1, false)],
                    "iterable.len.addr",
                )
                .unwrap()
        };

        ctx.builder.build_load(len_ptr, "iterable.len").map(BasicValueEnum::into_int_value).unwrap()
    }
}

impl<'ctx> ProxyValue<'ctx> for EnumerateValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = EnumerateType<'ctx>;

    fn get_type(&self) -> Self::Type {
        EnumerateType::from_pointer_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> From<EnumerateValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: EnumerateValue<'ctx>) -> Self {
        value.as_base_value()
    }
}
