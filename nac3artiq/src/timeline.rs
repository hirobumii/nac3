use nac3core::{
    codegen::{CodeGenContext, expr::call_extern},
    inkwell::{AddressSpace, AtomicOrdering, values::BasicValueEnum},
};

/// Functions for manipulating the timeline.
pub trait TimeFns {
    /// Emits LLVM IR for `now_mu`.
    fn emit_now_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx>;

    /// Emits LLVM IR for `at_mu`.
    fn emit_at_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, t: BasicValueEnum<'ctx>);

    /// Emits LLVM IR for `delay_mu`.
    fn emit_delay_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, dt: BasicValueEnum<'ctx>);
}

pub struct NowPinningTimeFns64 {}

// For FPGA design reasons, on VexRiscv with 64-bit data bus, the "now" CSR is split into two 32-bit
// values that are each padded to 64-bits.
impl TimeFns for NowPinningTimeFns64 {
    fn emit_now_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx> {
        let i64_type = ctx.i64;
        let i32_type = ctx.i32;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = ctx
            .builder
            .build_bit_cast(now, i32_type.ptr_type(AddressSpace::default()), "now.hi.addr")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        let now_loptr = unsafe {
            ctx.builder.build_gep(now_hiptr, &[i32_type.const_int(2, false)], "now.lo.addr")
        }
        .unwrap();

        let now_hi = ctx
            .builder
            .build_load(now_hiptr, "now.hi")
            .map(BasicValueEnum::into_int_value)
            .unwrap();
        let now_lo = ctx
            .builder
            .build_load(now_loptr, "now.lo")
            .map(BasicValueEnum::into_int_value)
            .unwrap();

        let zext_hi = ctx.builder.build_int_z_extend(now_hi, i64_type, "").unwrap();
        let shifted_hi =
            ctx.builder.build_left_shift(zext_hi, i64_type.const_int(32, false), "").unwrap();
        let zext_lo = ctx.builder.build_int_z_extend(now_lo, i64_type, "").unwrap();
        ctx.builder.build_or(shifted_hi, zext_lo, "now_mu").map(Into::into).unwrap()
    }

    fn emit_at_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, t: BasicValueEnum<'ctx>) {
        let i32_type = ctx.i32;
        let i64_type = ctx.i64;

        let i64_32 = i64_type.const_int(32, false);
        let time = t.into_int_value();

        let time_hi = ctx
            .builder
            .build_int_truncate(
                ctx.builder.build_right_shift(time, i64_32, false, "time.hi").unwrap(),
                i32_type,
                "",
            )
            .unwrap();
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "time.lo").unwrap();
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = ctx
            .builder
            .build_bit_cast(now, i32_type.ptr_type(AddressSpace::default()), "now.hi.addr")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        let now_loptr = unsafe {
            ctx.builder.build_gep(now_hiptr, &[i32_type.const_int(2, false)], "now.lo.addr")
        }
        .unwrap();
        ctx.builder
            .build_store(now_hiptr, time_hi)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
        ctx.builder
            .build_store(now_loptr, time_lo)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
    }

    fn emit_delay_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, dt: BasicValueEnum<'ctx>) {
        let i64_type = ctx.i64;
        let i32_type = ctx.i32;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = ctx
            .builder
            .build_bit_cast(now, i32_type.ptr_type(AddressSpace::default()), "now.hi.addr")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        let now_loptr = unsafe {
            ctx.builder.build_gep(now_hiptr, &[i32_type.const_int(2, false)], "now.lo.addr")
        }
        .unwrap();

        let now_hi = ctx
            .builder
            .build_load(now_hiptr, "now.hi")
            .map(BasicValueEnum::into_int_value)
            .unwrap();
        let now_lo = ctx
            .builder
            .build_load(now_loptr, "now.lo")
            .map(BasicValueEnum::into_int_value)
            .unwrap();
        let dt = dt.into_int_value();

        let zext_hi = ctx.builder.build_int_z_extend(now_hi, i64_type, "").unwrap();
        let shifted_hi =
            ctx.builder.build_left_shift(zext_hi, i64_type.const_int(32, false), "").unwrap();
        let zext_lo = ctx.builder.build_int_z_extend(now_lo, i64_type, "").unwrap();
        let now_val = ctx.builder.build_or(shifted_hi, zext_lo, "now").unwrap();

        let time = ctx.builder.build_int_add(now_val, dt, "time").unwrap();
        let time_hi = ctx
            .builder
            .build_int_truncate(
                ctx.builder
                    .build_right_shift(time, i64_type.const_int(32, false), false, "")
                    .unwrap(),
                i32_type,
                "time.hi",
            )
            .unwrap();
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "time.lo").unwrap();

        ctx.builder
            .build_store(now_hiptr, time_hi)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
        ctx.builder
            .build_store(now_loptr, time_lo)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
    }
}

pub static NOW_PINNING_TIME_FNS_64: NowPinningTimeFns64 = NowPinningTimeFns64 {};

pub struct NowPinningTimeFns {}

impl TimeFns for NowPinningTimeFns {
    fn emit_now_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx> {
        let i64_type = ctx.i64;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_raw = ctx
            .builder
            .build_load(now.as_pointer_value(), "now")
            .map(BasicValueEnum::into_int_value)
            .unwrap();

        let i64_32 = i64_type.const_int(32, false);
        let now_lo = ctx.builder.build_left_shift(now_raw, i64_32, "now.lo").unwrap();
        let now_hi = ctx.builder.build_right_shift(now_raw, i64_32, false, "now.hi").unwrap();
        ctx.builder.build_or(now_lo, now_hi, "now_mu").map(Into::into).unwrap()
    }

    fn emit_at_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, t: BasicValueEnum<'ctx>) {
        let i32_type = ctx.i32;
        let i64_type = ctx.i64;
        let i64_32 = i64_type.const_int(32, false);

        let time = t.into_int_value();

        let time_hi = ctx
            .builder
            .build_int_truncate(
                ctx.builder.build_right_shift(time, i64_32, false, "").unwrap(),
                i32_type,
                "time.hi",
            )
            .unwrap();
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "now_trunc").unwrap();
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = ctx
            .builder
            .build_bit_cast(now, i32_type.ptr_type(AddressSpace::default()), "now.hi.addr")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        let now_loptr = unsafe {
            ctx.builder.build_gep(now_hiptr, &[i32_type.const_int(1, false)], "now.lo.addr")
        }
        .unwrap();
        ctx.builder
            .build_store(now_hiptr, time_hi)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
        ctx.builder
            .build_store(now_loptr, time_lo)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
    }

    fn emit_delay_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, dt: BasicValueEnum<'ctx>) {
        let i32_type = ctx.i32;
        let i64_type = ctx.i64;
        let i64_32 = i64_type.const_int(32, false);
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_raw = ctx
            .builder
            .build_load(now.as_pointer_value(), "")
            .map(BasicValueEnum::into_int_value)
            .unwrap();

        let dt = dt.into_int_value();

        let now_lo = ctx.builder.build_left_shift(now_raw, i64_32, "now.lo").unwrap();
        let now_hi = ctx.builder.build_right_shift(now_raw, i64_32, false, "now.hi").unwrap();
        let now_val = ctx.builder.build_or(now_lo, now_hi, "now_val").unwrap();
        let time = ctx.builder.build_int_add(now_val, dt, "time").unwrap();
        let time_hi = ctx
            .builder
            .build_int_truncate(
                ctx.builder.build_right_shift(time, i64_32, false, "time.hi").unwrap(),
                i32_type,
                "now_trunc",
            )
            .unwrap();
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "time.lo").unwrap();
        let now_hiptr = ctx
            .builder
            .build_bit_cast(now, i32_type.ptr_type(AddressSpace::default()), "now.hi.addr")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();

        let now_loptr = unsafe {
            ctx.builder.build_gep(now_hiptr, &[i32_type.const_int(1, false)], "now.lo.addr")
        }
        .unwrap();
        ctx.builder
            .build_store(now_hiptr, time_hi)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
        ctx.builder
            .build_store(now_loptr, time_lo)
            .unwrap()
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
            .unwrap();
    }
}

pub static NOW_PINNING_TIME_FNS: NowPinningTimeFns = NowPinningTimeFns {};

pub struct ExternTimeFns {}

impl TimeFns for ExternTimeFns {
    fn emit_now_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx> {
        call_extern!(ctx: (ctx.i64) "now_mu" = "now_mu"()).into()
    }

    fn emit_at_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, t: BasicValueEnum<'ctx>) {
        assert_eq!(t.get_type(), ctx.i64.into());
        call_extern!(ctx: void "at_mu" = "at_mu"(t));
    }

    fn emit_delay_mu<'ctx>(&self, ctx: &mut CodeGenContext<'ctx, '_>, dt: BasicValueEnum<'ctx>) {
        assert_eq!(dt.get_type(), ctx.i64.into());
        call_extern!(ctx: void "delay_mu" = "delay_mu"(dt));
    }
}

pub static EXTERN_TIME_FNS: ExternTimeFns = ExternTimeFns {};
