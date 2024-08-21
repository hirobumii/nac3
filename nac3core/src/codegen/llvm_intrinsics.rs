use crate::codegen::CodeGenContext;
use inkwell::context::Context;
use inkwell::intrinsics::Intrinsic;
use inkwell::types::AnyTypeEnum::IntType;
use inkwell::types::FloatType;
use inkwell::values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue, PointerValue};
use inkwell::AddressSpace;
use itertools::Either;

/// Returns the string representation for the floating-point type `ft` when used in intrinsic
/// functions.
fn get_float_intrinsic_repr(ctx: &Context, ft: FloatType) -> &'static str {
    // Standard LLVM floating-point types
    if ft == ctx.f16_type() {
        return "f16";
    }
    if ft == ctx.f32_type() {
        return "f32";
    }
    if ft == ctx.f64_type() {
        return "f64";
    }
    if ft == ctx.f128_type() {
        return "f128";
    }

    // Non-standard floating-point types
    if ft == ctx.x86_f80_type() {
        return "f80";
    }
    if ft == ctx.ppc_f128_type() {
        return "ppcf128";
    }

    unreachable!()
}

/// Invokes the [`llvm.va_start`](https://llvm.org/docs/LangRef.html#llvm-va-start-intrinsic)
/// intrinsic.
pub fn call_va_start<'ctx>(ctx: &CodeGenContext<'ctx, '_>, arglist: PointerValue<'ctx>) {
    const FN_NAME: &str = "llvm.va_start";

    let intrinsic_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i8 = ctx.ctx.i8_type();
        let llvm_p0i8 = llvm_i8.ptr_type(AddressSpace::default());
        let fn_type = llvm_void.fn_type(&[llvm_p0i8.into()], false);

        ctx.module.add_function(FN_NAME, fn_type, None)
    });

    ctx.builder.build_call(intrinsic_fn, &[arglist.into()], "").unwrap();
}

/// Invokes the [`llvm.va_start`](https://llvm.org/docs/LangRef.html#llvm-va-start-intrinsic)
/// intrinsic.
pub fn call_va_end<'ctx>(ctx: &CodeGenContext<'ctx, '_>, arglist: PointerValue<'ctx>) {
    const FN_NAME: &str = "llvm.va_end";

    let intrinsic_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i8 = ctx.ctx.i8_type();
        let llvm_p0i8 = llvm_i8.ptr_type(AddressSpace::default());
        let fn_type = llvm_void.fn_type(&[llvm_p0i8.into()], false);

        ctx.module.add_function(FN_NAME, fn_type, None)
    });

    ctx.builder.build_call(intrinsic_fn, &[arglist.into()], "").unwrap();
}

/// Invokes the [`llvm.stacksave`](https://llvm.org/docs/LangRef.html#llvm-stacksave-intrinsic)
/// intrinsic.
pub fn call_stacksave<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    name: Option<&str>,
) -> PointerValue<'ctx> {
    const FN_NAME: &str = "llvm.stacksave";

    let intrinsic_fn = Intrinsic::find(FN_NAME)
        .and_then(|intrinsic| intrinsic.get_declaration(&ctx.module, &[]))
        .unwrap();

    ctx.builder
        .build_call(intrinsic_fn, &[], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_pointer_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the
/// [`llvm.stackrestore`](https://llvm.org/docs/LangRef.html#llvm-stackrestore-intrinsic) intrinsic.
///
/// - `ptr`: The pointer storing the address to restore the stack to.
pub fn call_stackrestore<'ctx>(ctx: &CodeGenContext<'ctx, '_>, ptr: PointerValue<'ctx>) {
    const FN_NAME: &str = "llvm.stackrestore";

    /*
        SEE https://github.com/TheDan64/inkwell/issues/496

        We want `llvm.stackrestore`, but the following would generate `llvm.stackrestore.p0i8`.
        ```ignore
        let intrinsic_fn = Intrinsic::find(FN_NAME)
            .and_then(|intrinsic| intrinsic.get_declaration(&ctx.module, &[llvm_p0i8.into()]))
            .unwrap();
        ```

        Temp workaround by manually declaring the intrinsic with the correct function name instead.
    */
    let intrinsic_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i8 = ctx.ctx.i8_type();
        let llvm_p0i8 = llvm_i8.ptr_type(AddressSpace::default());
        let fn_type = llvm_void.fn_type(&[llvm_p0i8.into()], false);

        ctx.module.add_function(FN_NAME, fn_type, None)
    });

    ctx.builder.build_call(intrinsic_fn, &[ptr.into()], "").unwrap();
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
    const FN_NAME: &str = "llvm.memcpy";

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

    let intrinsic_fn = Intrinsic::find(FN_NAME)
        .and_then(|intrinsic| {
            intrinsic.get_declaration(
                &ctx.module,
                &[llvm_dest_t.into(), llvm_src_t.into(), llvm_len_t.into()],
            )
        })
        .unwrap();

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

/// Macro to find and generate build call for llvm intrinsic (body of llvm intrinsic function)
///
/// Arguments:
/// * `$ctx:ident`: Reference to the current Code Generation Context
/// * `$name:ident`: Optional name to be assigned to the llvm build call (Option<&str>)
/// * `$llvm_name:literal`: Name of underlying llvm intrinsic function
/// * `$map_fn:ident`: Mapping function to be applied on `BasicValue` (`BasicValue` -> Function Return Type).
///   Use `BasicValueEnum::into_int_value` for Integer return type and `BasicValueEnum::into_float_value` for Float return type
/// * `$llvm_ty:ident`: Type of first operand
/// * `,($val:ident)*`: Comma separated list of operands
macro_rules! generate_llvm_intrinsic_fn_body {
    ($ctx:ident, $name:ident, $llvm_name:literal, $map_fn:expr, $llvm_ty:ident $(,$val:ident)*) => {{
        const FN_NAME: &str = concat!("llvm.", $llvm_name);
        let intrinsic_fn = Intrinsic::find(FN_NAME).and_then(|intrinsic| intrinsic.get_declaration(&$ctx.module, &[$llvm_ty.into()])).unwrap();
        $ctx.builder.build_call(intrinsic_fn, &[$($val.into()),*],  $name.unwrap_or_default()).map(CallSiteValue::try_as_basic_value).map(|v| v.map_left($map_fn)).map(Either::unwrap_left).unwrap()
    }};
}

/// Macro to generate the llvm intrinsic function using [`generate_llvm_intrinsic_fn_body`].
///
/// Arguments:
/// * `float/int`: Indicates the return and argument type of the function
/// * `$fn_name:ident`: The identifier of the rust function to be generated
/// * `$llvm_name:literal`: Name of underlying llvm intrinsic function.
///   Omit "llvm." prefix from the function name i.e. use "ceil" instead of "llvm.ceil"
/// * `$val:ident`: The operand for unary operations
/// * `$val1:ident`, `$val2:ident`: The operands for binary operations
macro_rules! generate_llvm_intrinsic_fn {
    ("float", $fn_name:ident, $llvm_name:literal, $val:ident) => {
        #[doc = concat!("Invokes the [`", stringify!($llvm_name), "`](https://llvm.org/docs/LangRef.html#llvm-", stringify!($llvm_name), "-intrinsic) intrinsic." )]
        pub fn $fn_name<'ctx> (
            ctx: &CodeGenContext<'ctx, '_>,
            $val: FloatValue<'ctx>,
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            let llvm_ty = $val.get_type();
            generate_llvm_intrinsic_fn_body!(ctx, name, $llvm_name, BasicValueEnum::into_float_value, llvm_ty, $val)
        }
    };
    ("float", $fn_name:ident, $llvm_name:literal, $val1:ident, $val2:ident) => {
        #[doc = concat!("Invokes the [`", stringify!($llvm_name), "`](https://llvm.org/docs/LangRef.html#llvm-", stringify!($llvm_name), "-intrinsic) intrinsic." )]
        pub fn $fn_name<'ctx> (
            ctx: &CodeGenContext<'ctx, '_>,
            $val1: FloatValue<'ctx>,
            $val2: FloatValue<'ctx>,
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            debug_assert_eq!($val1.get_type(), $val2.get_type());
            let llvm_ty = $val1.get_type();
            generate_llvm_intrinsic_fn_body!(ctx, name, $llvm_name, BasicValueEnum::into_float_value, llvm_ty, $val1, $val2)
        }
    };
    ("int", $fn_name:ident, $llvm_name:literal, $val1:ident, $val2:ident) => {
        #[doc = concat!("Invokes the [`", stringify!($llvm_name), "`](https://llvm.org/docs/LangRef.html#llvm-", stringify!($llvm_name), "-intrinsic) intrinsic." )]
        pub fn $fn_name<'ctx> (
            ctx: &CodeGenContext<'ctx, '_>,
            $val1: IntValue<'ctx>,
            $val2: IntValue<'ctx>,
            name: Option<&str>,
        ) -> IntValue<'ctx> {
            debug_assert_eq!($val1.get_type().get_bit_width(), $val2.get_type().get_bit_width());
            let llvm_ty = $val1.get_type();
            generate_llvm_intrinsic_fn_body!(ctx, name, $llvm_name, BasicValueEnum::into_int_value, llvm_ty, $val1, $val2)
        }
    };
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

    let src_type = src.get_type();
    generate_llvm_intrinsic_fn_body!(
        ctx,
        name,
        "abs",
        BasicValueEnum::into_int_value,
        src_type,
        src,
        is_int_min_poison
    )
}

generate_llvm_intrinsic_fn!("int", call_int_smax, "smax", a, b);
generate_llvm_intrinsic_fn!("int", call_int_smin, "smin", a, b);
generate_llvm_intrinsic_fn!("int", call_int_umax, "umax", a, b);
generate_llvm_intrinsic_fn!("int", call_int_umin, "umin", a, b);
generate_llvm_intrinsic_fn!("int", call_expect, "expect", val, expected_val);

generate_llvm_intrinsic_fn!("float", call_float_sqrt, "sqrt", val);
generate_llvm_intrinsic_fn!("float", call_float_sin, "sin", val);
generate_llvm_intrinsic_fn!("float", call_float_cos, "cos", val);
generate_llvm_intrinsic_fn!("float", call_float_pow, "pow", val, power);
generate_llvm_intrinsic_fn!("float", call_float_exp, "exp", val);
generate_llvm_intrinsic_fn!("float", call_float_exp2, "exp2", val);
generate_llvm_intrinsic_fn!("float", call_float_log, "log", val);
generate_llvm_intrinsic_fn!("float", call_float_log10, "log10", val);
generate_llvm_intrinsic_fn!("float", call_float_log2, "log2", val);
generate_llvm_intrinsic_fn!("float", call_float_fabs, "fabs", src);
generate_llvm_intrinsic_fn!("float", call_float_minnum, "minnum", val, power);
generate_llvm_intrinsic_fn!("float", call_float_maxnum, "maxnum", val, power);
generate_llvm_intrinsic_fn!("float", call_float_copysign, "copysign", mag, sgn);
generate_llvm_intrinsic_fn!("float", call_float_floor, "floor", val);
generate_llvm_intrinsic_fn!("float", call_float_ceil, "ceil", val);
generate_llvm_intrinsic_fn!("float", call_float_round, "round", val);
generate_llvm_intrinsic_fn!("float", call_float_rint, "rint", val);

/// Invokes the [`llvm.powi`](https://llvm.org/docs/LangRef.html#llvm-powi-intrinsic) intrinsic.
pub fn call_float_powi<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    power: IntValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "llvm.powi";

    let llvm_val_t = val.get_type();
    let llvm_power_t = power.get_type();

    let intrinsic_fn = Intrinsic::find(FN_NAME)
        .and_then(|intrinsic| {
            intrinsic.get_declaration(&ctx.module, &[llvm_val_t.into(), llvm_power_t.into()])
        })
        .unwrap();

    ctx.builder
        .build_call(intrinsic_fn, &[val.into(), power.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}
