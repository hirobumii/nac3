use inkwell::{
    intrinsics::Intrinsic,
    types::{AnyTypeEnum::IntType, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, FloatValue, IntValue, PointerValue},
};

use super::CodeGenContext;

fn call_intrinsic_impl<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    intrin: &str,
    // These are *type parameters* for overloaded functions (the ".i8" part of
    // "llvm.ctpop.i8"), not the types of the parameters of the function!
    type_params: &[BasicTypeEnum<'ctx>],
    args: &[BasicMetadataValueEnum<'ctx>],
    call_name: Option<&str>,
) -> Option<BasicValueEnum<'ctx>> {
    let intrin = Intrinsic::find(intrin)
        .and_then(|intrinsic| intrinsic.get_declaration(&ctx.module, type_params))
        .expect("intrinsic not found");
    let result = ctx.builder.build_call(intrin, args, call_name.unwrap_or_default()).unwrap();
    result.try_as_basic_value().basic()
}

macro_rules! call_intrinsic {
    ($ctx: expr, $call_name: expr, $intrin:literal $([$($type_param:expr),*])? ($($arg:expr),*)) => {{
        call_intrinsic_impl($ctx, concat!("llvm.", $intrin), &[$($($type_param.into()),*)?], &[$($arg.into()),*], $call_name)
    }};
    ($ctx: expr, $call_name: expr, $intrin:literal $([$($type_param:expr),*])? ($($arg:expr),*) -> void) => {{
        assert!(call_intrinsic!($ctx, $call_name, $intrin $([$($type_param),*])? ($($arg),*)).is_none())
    }};
    ($ctx: expr, $call_name: expr, $intrin:literal $([$($type_param:expr),*])? ($($arg:expr),*) -> int) => {{
        call_intrinsic!($ctx, $call_name, $intrin $([$($type_param),*])? ($($arg),*)).unwrap().into_int_value()
    }};
    ($ctx: expr, $call_name: expr, $intrin:literal $([$($type_param:expr),*])? ($($arg:expr),*) -> float) => {{
        call_intrinsic!($ctx, $call_name, $intrin $([$($type_param),*])? ($($arg),*)).unwrap().into_float_value()
    }};
    ($ctx: expr, $call_name: expr, $intrin:literal $([$($type_param:expr),*])? ($($arg:expr),*) -> ptr) => {{
        call_intrinsic!($ctx, $call_name, $intrin $([$($type_param),*])? ($($arg),*)).unwrap().into_pointer_value()
    }};
}

macro_rules! llvm_doc {
    ($llvm_name:literal) => {
        concat!(
            "Invokes the [`llvm.",
            $llvm_name,
            "`](https://llvm.org/docs/LangRef.html#llvm-",
            $llvm_name,
            "-intrinsic) intrinsic."
        )
    };
}

#[doc = llvm_doc!("va_start")]
pub fn call_va_start<'ctx>(ctx: &CodeGenContext<'ctx, '_>, arglist: PointerValue<'ctx>) {
    call_intrinsic!(ctx, None, "va_start"(arglist) -> void);
}

#[doc = llvm_doc!("va_end")]
pub fn call_va_end<'ctx>(ctx: &CodeGenContext<'ctx, '_>, arglist: PointerValue<'ctx>) {
    call_intrinsic!(ctx, None, "va_end"(arglist) -> void);
}

#[doc = llvm_doc!("va_stacksave")]
pub fn call_stacksave<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    name: Option<&str>,
) -> PointerValue<'ctx> {
    call_intrinsic!(ctx, name, "stacksave"() -> ptr)
}

#[doc = llvm_doc!("va_stackrestore")]
///
/// - `ptr`: The pointer storing the address to restore the stack to.
pub fn call_stackrestore<'ctx>(ctx: &CodeGenContext<'ctx, '_>, ptr: PointerValue<'ctx>) {
    call_intrinsic!(ctx, None, "stackrestore"(ptr) -> void);
}

#[doc = llvm_doc!("memcpy")]
///
/// * `dest` - The pointer to the destination. Must be a pointer to an integer type.
/// * `src` - The pointer to the source. Must be a pointer to an integer type.
/// * `len` - The number of bytes to copy.
pub fn call_memcpy<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    dest: PointerValue<'ctx>,
    src: PointerValue<'ctx>,
    len: IntValue<'ctx>,
) {
    debug_assert!(dest.get_type().get_element_type().is_int_type());
    debug_assert!(src.get_type().get_element_type().is_int_type());
    debug_assert_eq!(
        dest.get_type().get_element_type().into_int_type().get_bit_width(),
        src.get_type().get_element_type().into_int_type().get_bit_width(),
    );
    debug_assert_eq!(len.get_type(), ctx.size_t);

    let llvm_dest_t = dest.get_type();
    let llvm_src_t = src.get_type();

    let target_data = ctx.target.get_target_data();
    let dest_alignment = target_data.get_abi_alignment(&llvm_dest_t);
    let src_alignment = target_data.get_abi_alignment(&llvm_src_t);

    ctx.builder.build_memcpy(dest, dest_alignment, src, src_alignment, len).unwrap();
}

#[doc = llvm_doc!("memcpy")]
///
/// Unlike [`call_memcpy`], this function accepts any type of pointer value. If `dest` or `src` is
/// not a pointer to an integer, the pointer(s) will be cast to `i8*` before invoking `memcpy`.
pub fn call_memcpy_generic<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    dest: PointerValue<'ctx>,
    src: PointerValue<'ctx>,
    len: IntValue<'ctx>,
) {
    let llvm_p0i8 = ctx.ptr;

    let dest_elem_t = dest.get_type().get_element_type();
    let src_elem_t = src.get_type().get_element_type();

    let dest = if matches!(dest_elem_t, IntType(t) if t.get_bit_width() == 8) {
        dest
    } else {
        ctx.builder
            .build_bit_cast(dest, llvm_p0i8, "")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    };
    let src = if matches!(src_elem_t, IntType(t) if t.get_bit_width() == 8) {
        src
    } else {
        ctx.builder
            .build_bit_cast(src, llvm_p0i8, "")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    };

    call_memcpy(ctx, dest, src, len);
}

#[doc = llvm_doc!("memcpy")]
///
/// Unlike [`call_memcpy`], this function accepts any type of pointer value. If `dest` or `src` is
/// not a pointer to an integer, the pointer(s) will be cast to `i8*` before invoking `memcpy`.
/// Moreover, `len` now refers to the number of elements to copy (rather than number of bytes to
/// copy).
pub fn call_memcpy_generic_array<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    dest: PointerValue<'ctx>,
    src: PointerValue<'ctx>,
    len: IntValue<'ctx>,
) {
    let llvm_p0i8 = ctx.ptr;
    let llvm_usize = ctx.size_t;

    let dest_elem_t = dest.get_type().get_element_type();
    let src_elem_t = src.get_type().get_element_type();

    let dest = if matches!(dest_elem_t, IntType(t) if t.get_bit_width() == 8) {
        dest
    } else {
        ctx.builder
            .build_bit_cast(dest, llvm_p0i8, "")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    };
    let src = if matches!(src_elem_t, IntType(t) if t.get_bit_width() == 8) {
        src
    } else {
        ctx.builder
            .build_bit_cast(src, llvm_p0i8, "")
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    };

    let sizeof_elem = ctx
        .builder
        .build_int_truncate_or_bit_cast(src_elem_t.size_of().unwrap(), llvm_usize, "")
        .unwrap();
    let len = ctx.builder.build_int_mul(len, sizeof_elem, "").unwrap();

    call_memcpy(ctx, dest, src, len);
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
    (float $fn_name:ident: $llvm_name:literal ($val:ident)) => {
        #[doc = llvm_doc!($llvm_name)]
        pub fn $fn_name<'ctx> (
            ctx: &CodeGenContext<'ctx, '_>,
            $val: FloatValue<'ctx>,
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            call_intrinsic!(ctx, name, $llvm_name[$val.get_type()]($val) -> float)
        }
    };
    (float $fn_name:ident: $llvm_name:literal ($val1:ident, $val2:ident)) => {
        #[doc = llvm_doc!($llvm_name)]
        pub fn $fn_name<'ctx> (
            ctx: &CodeGenContext<'ctx, '_>,
            $val1: FloatValue<'ctx>,
            $val2: FloatValue<'ctx>,
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            debug_assert_eq!($val1.get_type(), $val2.get_type());
            call_intrinsic!(ctx, name, $llvm_name[$val1.get_type()]($val1, $val2) -> float)
        }
    };
    (int $fn_name:ident: $llvm_name:literal ($val1:ident, $val2:ident)) => {
        #[doc = llvm_doc!($llvm_name)]
        pub fn $fn_name<'ctx> (
            ctx: &CodeGenContext<'ctx, '_>,
            $val1: IntValue<'ctx>,
            $val2: IntValue<'ctx>,
            name: Option<&str>,
        ) -> IntValue<'ctx> {
            debug_assert_eq!($val1.get_type().get_bit_width(), $val2.get_type().get_bit_width());
            call_intrinsic!(ctx, name, $llvm_name[$val1.get_type()]($val1, $val2) -> int)
        }
    };
}

#[doc = llvm_doc!("abs")]
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

    call_intrinsic!(ctx, name, "abs"[src.get_type()](src, is_int_min_poison) -> int)
}

generate_llvm_intrinsic_fn!(int call_int_smax: "smax"(a, b));
generate_llvm_intrinsic_fn!(int call_int_smin: "smin"(a, b));
generate_llvm_intrinsic_fn!(int call_int_umax: "umax"(a, b));
generate_llvm_intrinsic_fn!(int call_int_umin: "umin"(a, b));
generate_llvm_intrinsic_fn!(int call_expect: "expect"(val, expected_val));

generate_llvm_intrinsic_fn!(float call_float_sqrt: "sqrt"(val));
generate_llvm_intrinsic_fn!(float call_float_sin: "sin"(val));
generate_llvm_intrinsic_fn!(float call_float_cos: "cos"(val));
generate_llvm_intrinsic_fn!(float call_float_pow: "pow"(val, power));
generate_llvm_intrinsic_fn!(float call_float_exp: "exp"(val));
generate_llvm_intrinsic_fn!(float call_float_exp2: "exp2"(val));
generate_llvm_intrinsic_fn!(float call_float_log: "log"(val));
generate_llvm_intrinsic_fn!(float call_float_log10: "log10"(val));
generate_llvm_intrinsic_fn!(float call_float_log2: "log2"(val));
generate_llvm_intrinsic_fn!(float call_float_fabs: "fabs"(src));
generate_llvm_intrinsic_fn!(float call_float_minnum: "minnum"(val, power));
generate_llvm_intrinsic_fn!(float call_float_maxnum: "maxnum"(val, power));
generate_llvm_intrinsic_fn!(float call_float_copysign: "copysign"(mag, sgn));
generate_llvm_intrinsic_fn!(float call_float_floor: "floor"(val));
generate_llvm_intrinsic_fn!(float call_float_ceil: "ceil"(val));
generate_llvm_intrinsic_fn!(float call_float_round: "round"(val));
generate_llvm_intrinsic_fn!(float call_float_rint: "rint"(val));

#[doc = llvm_doc!("powi")]
pub fn call_float_powi<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    val: FloatValue<'ctx>,
    power: IntValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    call_intrinsic!(ctx, name, "powi"[val.get_type(), power.get_type()](val, power) -> float)
}

#[doc = llvm_doc!("ctpop")]
pub fn call_int_ctpop<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    src: IntValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    call_intrinsic!(ctx, name, "ctpop"[src.get_type()](src) -> int)
}
