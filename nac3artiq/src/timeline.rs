use nac3core::{
    codegen::{CodeGenContext, expr::call_extern},
    inkwell::{AtomicOrdering, values::BasicValueEnum},
};

/// Trait for emitting LLVM IR for ARTIQ timeline operations.
///
/// Different implementations target different hardware backends: `NowPinningTimeFns64`
/// directly reads/writes split 32-bit CSR registers on `VexRiscv`, while `ExternTimeFns`
/// calls external C functions (used for host-mode execution and `runkernel`).
pub trait TimeFns {
    /// Emits LLVM IR for `now_mu`.
    fn emit_now_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>>;

    /// Emits LLVM IR for `at_mu`.
    fn emit_at_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        t: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()>;

    /// Emits LLVM IR for `delay_mu`.
    fn emit_delay_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dt: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()>;
}

pub struct NowPinningTimeFns64 {}

// For FPGA design reasons, on VexRiscv with 64-bit data bus, the "now" CSR is split into two 32-bit
// values that are each padded to 64-bits.
impl TimeFns for NowPinningTimeFns64 {
    fn emit_now_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let i64_type = ctx.i64;
        let i32_type = ctx.i32;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = now.as_pointer_value();

        let now_loptr = unsafe {
            ctx.builder.build_gep(
                i32_type,
                now_hiptr,
                &[i32_type.const_int(2, false)],
                "now.lo.addr",
            )?
        };

        let now_hi = ctx.builder.build_load(i32_type, now_hiptr, "now.hi")?.into_int_value();
        let now_lo = ctx.builder.build_load(i32_type, now_loptr, "now.lo")?.into_int_value();

        let zext_hi = ctx.builder.build_int_z_extend(now_hi, i64_type, "")?;
        let shifted_hi =
            ctx.builder.build_left_shift(zext_hi, i64_type.const_int(32, false), "")?;
        let zext_lo = ctx.builder.build_int_z_extend(now_lo, i64_type, "")?;
        Ok(ctx.builder.build_or(shifted_hi, zext_lo, "now_mu").map(Into::into)?)
    }

    fn emit_at_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        t: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        let i32_type = ctx.i32;
        let i64_type = ctx.i64;

        let i64_32 = i64_type.const_int(32, false);
        let time = t.into_int_value();

        let time_hi = ctx.builder.build_int_truncate(
            ctx.builder.build_right_shift(time, i64_32, false, "time.hi")?,
            i32_type,
            "",
        )?;
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "time.lo")?;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = now.as_pointer_value();

        let now_loptr = unsafe {
            ctx.builder.build_gep(
                i32_type,
                now_hiptr,
                &[i32_type.const_int(2, false)],
                "now.lo.addr",
            )?
        };
        ctx.builder
            .build_store(now_hiptr, time_hi)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;
        ctx.builder
            .build_store(now_loptr, time_lo)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;

        Ok(())
    }

    fn emit_delay_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dt: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        let i64_type = ctx.i64;
        let i32_type = ctx.i32;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = now.as_pointer_value();

        let now_loptr = unsafe {
            ctx.builder.build_gep(
                i32_type,
                now_hiptr,
                &[i32_type.const_int(2, false)],
                "now.lo.addr",
            )?
        };

        let now_hi = ctx.builder.build_load(i32_type, now_hiptr, "now.hi")?.into_int_value();
        let now_lo = ctx.builder.build_load(i32_type, now_loptr, "now.lo")?.into_int_value();
        let dt = dt.into_int_value();

        let zext_hi = ctx.builder.build_int_z_extend(now_hi, i64_type, "")?;
        let shifted_hi =
            ctx.builder.build_left_shift(zext_hi, i64_type.const_int(32, false), "")?;
        let zext_lo = ctx.builder.build_int_z_extend(now_lo, i64_type, "")?;
        let now_val = ctx.builder.build_or(shifted_hi, zext_lo, "now")?;

        let time = ctx.builder.build_int_add(now_val, dt, "time")?;
        let time_hi = ctx.builder.build_int_truncate(
            ctx.builder.build_right_shift(time, i64_type.const_int(32, false), false, "")?,
            i32_type,
            "time.hi",
        )?;
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "time.lo")?;

        ctx.builder
            .build_store(now_hiptr, time_hi)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;
        ctx.builder
            .build_store(now_loptr, time_lo)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;

        Ok(())
    }
}

pub static NOW_PINNING_TIME_FNS_64: NowPinningTimeFns64 = NowPinningTimeFns64 {};

pub struct NowPinningTimeFns {}

impl TimeFns for NowPinningTimeFns {
    fn emit_now_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let i64_type = ctx.i64;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_raw =
            ctx.builder.build_load(i64_type, now.as_pointer_value(), "now")?.into_int_value();

        let i64_32 = i64_type.const_int(32, false);
        let now_lo = ctx.builder.build_left_shift(now_raw, i64_32, "now.lo")?;
        let now_hi = ctx.builder.build_right_shift(now_raw, i64_32, false, "now.hi")?;
        Ok(ctx.builder.build_or(now_lo, now_hi, "now_mu")?.into())
    }

    fn emit_at_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        t: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        let i32_type = ctx.i32;
        let i64_type = ctx.i64;
        let i64_32 = i64_type.const_int(32, false);

        let time = t.into_int_value();

        let time_hi = ctx.builder.build_int_truncate(
            ctx.builder.build_right_shift(time, i64_32, false, "")?,
            i32_type,
            "time.hi",
        )?;
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "now_trunc")?;
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_hiptr = now.as_pointer_value();

        let now_loptr = unsafe {
            ctx.builder.build_gep(
                i32_type,
                now_hiptr,
                &[i32_type.const_int(1, false)],
                "now.lo.addr",
            )?
        };
        ctx.builder
            .build_store(now_hiptr, time_hi)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;
        ctx.builder
            .build_store(now_loptr, time_lo)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;

        Ok(())
    }

    fn emit_delay_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dt: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        let i32_type = ctx.i32;
        let i64_type = ctx.i64;
        let i64_32 = i64_type.const_int(32, false);
        let now = ctx
            .module
            .get_global("now")
            .unwrap_or_else(|| ctx.module.add_global(i64_type, None, "now"));
        let now_raw =
            ctx.builder.build_load(i64_type, now.as_pointer_value(), "")?.into_int_value();

        let dt = dt.into_int_value();

        let now_lo = ctx.builder.build_left_shift(now_raw, i64_32, "now.lo")?;
        let now_hi = ctx.builder.build_right_shift(now_raw, i64_32, false, "now.hi")?;
        let now_val = ctx.builder.build_or(now_lo, now_hi, "now_val")?;
        let time = ctx.builder.build_int_add(now_val, dt, "time")?;
        let time_hi = ctx.builder.build_int_truncate(
            ctx.builder.build_right_shift(time, i64_32, false, "time.hi")?,
            i32_type,
            "now_trunc",
        )?;
        let time_lo = ctx.builder.build_int_truncate(time, i32_type, "time.lo")?;
        let now_hiptr = now.as_pointer_value();

        let now_loptr = unsafe {
            ctx.builder.build_gep(
                i32_type,
                now_hiptr,
                &[i32_type.const_int(1, false)],
                "now.lo.addr",
            )?
        };
        ctx.builder
            .build_store(now_hiptr, time_hi)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;
        ctx.builder
            .build_store(now_loptr, time_lo)?
            .set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)?;

        Ok(())
    }
}

pub static NOW_PINNING_TIME_FNS: NowPinningTimeFns = NowPinningTimeFns {};

pub struct ExternTimeFns {}

impl TimeFns for ExternTimeFns {
    fn emit_now_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        Ok(call_extern!(ctx: (ctx.i64) "now_mu" = "now_mu"())?.into())
    }

    fn emit_at_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        t: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        assert_eq!(t.get_type(), ctx.i64.into());
        call_extern!(ctx: void "at_mu" = "at_mu"(t))
    }

    fn emit_delay_mu<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dt: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        assert_eq!(dt.get_type(), ctx.i64.into());
        call_extern!(ctx: void "delay_mu" = "delay_mu"(dt))
    }
}

pub static EXTERN_TIME_FNS: ExternTimeFns = ExternTimeFns {};
