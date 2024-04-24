use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::types::AnyTypeEnum::IntType;
use inkwell::types::FloatType;
use inkwell::values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue, PointerValue};
use itertools::Either;
use crate::codegen::CodeGenContext;

/// Returns the string representation for the floating-point type `ft` when used in intrinsic
/// functions.
fn get_float_intrinsic_repr(ctx: &Context, ft: FloatType) -> &'static str {
    // Standard LLVM floating-point types
    if ft == ctx.f16_type() {
        return "f16"
    }
    if ft == ctx.f32_type() {
        return "f32"
    }
    if ft == ctx.f64_type() {
        return "f64"
    }
    if ft == ctx.f128_type() {
        return "f128"
    }

    // Non-standard floating-point types
    if ft == ctx.x86_f80_type() {
        return "f80"
    }
    if ft == ctx.ppc_f128_type() {
        return "ppcf128"
    }

    unreachable!()
}

/// Invokes the [`llvm.stacksave`](https://llvm.org/docs/LangRef.html#llvm-stacksave-intrinsic)
/// intrinsic.
pub fn call_stacksave<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    name: Option<&str>,
) -> PointerValue<'ctx> {
    const FN_NAME: &str = "llvm.stacksave";

    let intrinsic_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let llvm_i8 = ctx.ctx.i8_type();
        let llvm_p0i8 = llvm_i8.ptr_type(AddressSpace::default());

        let fn_type = llvm_p0i8.fn_type(&[], false);

        ctx.module.add_function(FN_NAME, fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_pointer_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the
/// [`llvm.stackrestore`](https://llvm.org/docs/LangRef.html#llvm-stackrestore-intrinsic) intrinsic.
pub fn call_stackrestore<'ctx>(ctx: &CodeGenContext<'ctx, '_>, ptr: PointerValue<'ctx>) {
    const FN_NAME: &str = "llvm.stackrestore";

    let intrinsic_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i8 = ctx.ctx.i8_type();
        let llvm_p0i8 = llvm_i8.ptr_type(AddressSpace::default());

        let fn_type = llvm_void.fn_type(&[llvm_p0i8.into()], false);

        ctx.module.add_function(FN_NAME, fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[ptr.into()], "")
        .unwrap();
}

/// Invokes the [`llvm.abs`](https://llvm.org/docs/LangRef.html#llvm-abs-intrinsic) intrinsic.
///
/// * `src` - The value for which the absolute value is to be returned.
/// * `is_int_min_poison` - Whether `poison` is to be returned if `src` is `INT_MIN`.
pub fn call_int_abs<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    src: IntValue<'ctx>,
    is_int_min_poison: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    debug_assert_eq!(is_int_min_poison.get_type().get_bit_width(), 1);
    debug_assert!(is_int_min_poison.is_const());

    let llvm_src_t = src.get_type();

    let fn_name = format!("llvm.abs.i{}", llvm_src_t.get_bit_width());

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let llvm_i1 = ctx.ctx.bool_type();

        let fn_type = llvm_src_t.fn_type(&[llvm_src_t.into(), llvm_i1.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[src.into(), is_int_min_poison.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.smax`](https://llvm.org/docs/LangRef.html#llvm-smax-intrinsic) intrinsic.
pub fn call_int_smax<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    a: IntValue<'ctx>,
    b: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    debug_assert_eq!(a.get_type().get_bit_width(), b.get_type().get_bit_width());

    let llvm_int_t = a.get_type();

    let fn_name = format!("llvm.smax.i{}", llvm_int_t.get_bit_width());

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_int_t.fn_type(&[llvm_int_t.into(), llvm_int_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[a.into(), b.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.smin`](https://llvm.org/docs/LangRef.html#llvm-smin-intrinsic) intrinsic.
pub fn call_int_smin<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    a: IntValue<'ctx>,
    b: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    debug_assert_eq!(a.get_type().get_bit_width(), b.get_type().get_bit_width());

    let llvm_int_t = a.get_type();

    let fn_name = format!("llvm.smin.i{}", llvm_int_t.get_bit_width());

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_int_t.fn_type(&[llvm_int_t.into(), llvm_int_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[a.into(), b.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.umax`](https://llvm.org/docs/LangRef.html#llvm-umax-intrinsic) intrinsic.
pub fn call_int_umax<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    a: IntValue<'ctx>,
    b: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    debug_assert_eq!(a.get_type().get_bit_width(), b.get_type().get_bit_width());

    let llvm_int_t = a.get_type();

    let fn_name = format!("llvm.umax.i{}", llvm_int_t.get_bit_width());

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_int_t.fn_type(&[llvm_int_t.into(), llvm_int_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[a.into(), b.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.umin`](https://llvm.org/docs/LangRef.html#llvm-umin-intrinsic) intrinsic.
pub fn call_int_umin<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    a: IntValue<'ctx>,
    b: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    debug_assert_eq!(a.get_type().get_bit_width(), b.get_type().get_bit_width());

    let llvm_int_t = a.get_type();

    let fn_name = format!("llvm.umin.i{}", llvm_int_t.get_bit_width());

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_int_t.fn_type(&[llvm_int_t.into(), llvm_int_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[a.into(), b.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.memcpy`](https://llvm.org/docs/LangRef.html#llvm-memcpy-intrinsic) intrinsic.
///
/// * `dest` - The pointer to the destination. Must be a pointer to an integer type.
/// * `src` - The pointer to the source. Must be a pointer to an integer type.
/// * `len` - The number of bytes to copy.
/// * `is_volatile` - Whether the `memcpy` operation should be `volatile`.
pub fn call_memcpy<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    dest: PointerValue<'ctx>,
    src: PointerValue<'ctx>,
    len: IntValue<'ctx>,
    is_volatile: IntValue<'ctx>,
) {
    debug_assert!(dest.get_type().get_element_type().is_int_type());
    debug_assert!(src.get_type().get_element_type().is_int_type());
    debug_assert_eq!(
        dest.get_type().get_element_type().into_int_type().get_bit_width(),
        src.get_type().get_element_type().into_int_type().get_bit_width(),
    );
    debug_assert!(matches!(len.get_type().get_bit_width(), 32 | 64));
    debug_assert_eq!(is_volatile.get_type().get_bit_width(), 1);

    let llvm_dest_t = dest.get_type();
    let llvm_src_t = src.get_type();
    let llvm_len_t = len.get_type();

    let fn_name = format!(
        "llvm.memcpy.p0i{}.p0i{}.i{}",
        llvm_dest_t.get_element_type().into_int_type().get_bit_width(),
        llvm_src_t.get_element_type().into_int_type().get_bit_width(),
        llvm_len_t.get_bit_width(),
    );

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let llvm_void = ctx.ctx.void_type();

        let fn_type = llvm_void.fn_type(
            &[
                llvm_dest_t.into(),
                llvm_src_t.into(),
                llvm_len_t.into(),
                is_volatile.get_type().into(),
            ],
            false,
        );

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[dest.into(), src.into(), len.into(), is_volatile.into()], "")
        .unwrap();
}

/// Invokes the `llvm.memcpy` intrinsic.
///
/// Unlike [`call_memcpy`], this function accepts any type of pointer value. If `dest` or `src` is
/// not a pointer to an integer, the pointer(s) will be cast to `i8*` before invoking `memcpy`.
pub fn call_memcpy_generic<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    dest: PointerValue<'ctx>,
    src: PointerValue<'ctx>,
    len: IntValue<'ctx>,
    is_volatile: IntValue<'ctx>,
) {
    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_p0i8 = llvm_i8.ptr_type(AddressSpace::default());

    let dest_elem_t = dest.get_type().get_element_type();
    let src_elem_t = src.get_type().get_element_type();

    let dest = if matches!(dest_elem_t, IntType(t) if t.get_bit_width() == 8) {
        dest
    } else {
        ctx.builder
            .build_bitcast(dest, llvm_p0i8, "")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    };
    let src = if matches!(src_elem_t, IntType(t) if t.get_bit_width() == 8) {
        src
    } else {
        ctx.builder
            .build_bitcast(src, llvm_p0i8, "")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    };

    call_memcpy(ctx, dest, src, len, is_volatile);
}

/// Invokes the [`llvm.sqrt`](https://llvm.org/docs/LangRef.html#llvm-sqrt-intrinsic) intrinsic.
pub fn call_float_sqrt<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.sqrt.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.powi`](https://llvm.org/docs/LangRef.html#llvm-powi-intrinsic) intrinsic.
pub fn call_float_powi<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    power: IntValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_val_t = val.get_type();
    let llvm_power_t = power.get_type();

    let fn_name = format!(
        "llvm.powi.{}.i{}",
        get_float_intrinsic_repr(ctx.ctx, llvm_val_t),
        llvm_power_t.get_bit_width(),
    );
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_val_t.fn_type(&[llvm_val_t.into(), llvm_power_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into(), power.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.sin`](https://llvm.org/docs/LangRef.html#llvm-sin-intrinsic) intrinsic.
pub fn call_float_sin<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.sin.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.cos`](https://llvm.org/docs/LangRef.html#llvm-cos-intrinsic) intrinsic.
pub fn call_float_cos<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.cos.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.pow`](https://llvm.org/docs/LangRef.html#llvm-pow-intrinsic) intrinsic.
pub fn call_float_pow<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    power: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    debug_assert_eq!(val.get_type(), power.get_type());

    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.pow.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into(), llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into(), power.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.exp`](https://llvm.org/docs/LangRef.html#llvm-exp-intrinsic) intrinsic.
pub fn call_float_exp<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.exp.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.exp2`](https://llvm.org/docs/LangRef.html#llvm-exp2-intrinsic) intrinsic.
pub fn call_float_exp2<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.exp2.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}


/// Invokes the [`llvm.log`](https://llvm.org/docs/LangRef.html#llvm-log-intrinsic) intrinsic.
pub fn call_float_log<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.log.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.log10`](https://llvm.org/docs/LangRef.html#llvm-log10-intrinsic) intrinsic.
pub fn call_float_log10<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.log10.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.log2`](https://llvm.org/docs/LangRef.html#llvm-log2-intrinsic) intrinsic.
pub fn call_float_log2<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.log2.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.fabs`](https://llvm.org/docs/LangRef.html#llvm-fabs-intrinsic) intrinsic.
pub fn call_float_fabs<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    src: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_src_t = src.get_type();

    let fn_name = format!("llvm.fabs.{}", get_float_intrinsic_repr(ctx.ctx, llvm_src_t));

    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_src_t.fn_type(&[llvm_src_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[src.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.minnum`](https://llvm.org/docs/LangRef.html#llvm-minnum-intrinsic) intrinsic.
pub fn call_float_minnum<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val1: FloatValue<'ctx>,
    val2: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    debug_assert_eq!(val1.get_type(), val2.get_type());

    let llvm_float_t = val1.get_type();

    let fn_name = format!("llvm.minnum.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into(), llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val1.into(), val2.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.maxnum`](https://llvm.org/docs/LangRef.html#llvm-maxnum-intrinsic) intrinsic.
pub fn call_float_maxnum<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val1: FloatValue<'ctx>,
    val2: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    debug_assert_eq!(val1.get_type(), val2.get_type());

    let llvm_float_t = val1.get_type();

    let fn_name = format!("llvm.maxnum.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into(), llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val1.into(), val2.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.copysign`](https://llvm.org/docs/LangRef.html#llvm-copysign-intrinsic) intrinsic.
pub fn call_float_copysign<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    mag: FloatValue<'ctx>,
    sgn: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    debug_assert_eq!(mag.get_type(), sgn.get_type());

    let llvm_float_t = mag.get_type();

    let fn_name = format!("llvm.copysign.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into(), llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[mag.into(), sgn.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.floor`](https://llvm.org/docs/LangRef.html#llvm-floor-intrinsic) intrinsic.
pub fn call_float_floor<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.floor.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.ceil`](https://llvm.org/docs/LangRef.html#llvm-ceil-intrinsic) intrinsic.
pub fn call_float_ceil<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.ceil.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.round`](https://llvm.org/docs/LangRef.html#llvm-round-intrinsic) intrinsic.
pub fn call_float_round<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.round.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the 
/// [`llvm.roundeven`](https://llvm.org/docs/LangRef.html#llvm-roundeven-intrinsic) intrinsic.
pub fn call_float_roundeven<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_float_t = val.get_type();

    let fn_name = format!("llvm.roundeven.{}", get_float_intrinsic_repr(ctx.ctx, llvm_float_t));
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_float_t.fn_type(&[llvm_float_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`llvm.expect`](https://llvm.org/docs/LangRef.html#llvm-expect-intrinsic) intrinsic.
pub fn call_expect<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: IntValue<'ctx>,
    expected_val: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    debug_assert_eq!(val.get_type().get_bit_width(), expected_val.get_type().get_bit_width());

    let llvm_int_t = val.get_type();

    let fn_name = format!("llvm.expect.i{}", llvm_int_t.get_bit_width());
    let intrinsic_fn = ctx.module.get_function(fn_name.as_str()).unwrap_or_else(|| {
        let fn_type = llvm_int_t.fn_type(&[llvm_int_t.into(), llvm_int_t.into()], false);

        ctx.module.add_function(fn_name.as_str(), fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[val.into(), expected_val.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}
