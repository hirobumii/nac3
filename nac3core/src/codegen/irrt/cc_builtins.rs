use inkwell::{
    attributes::{Attribute, AttributeLoc},
    values::{BasicValueEnum, FloatValue, IntValue},
};

use crate::codegen::{expr::infer_and_call_function, CodeGenContext};

/// Generates a call to [`isinf`](https://en.cppreference.com/w/c/numeric/math/isinf) in IR. Returns
/// an `i1` representing the result.
pub fn call_isinf<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(ctx, "__nac3_isinf", Some(llvm_i1.into()), &[v.into()], name, None)
        .map(BasicValueEnum::into_int_value)
        .unwrap()
}

/// Generates a call to [`isnan`](https://en.cppreference.com/w/c/numeric/math/isnan) in IR. Returns
/// an `i1` representing the result.
pub fn call_isnan<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
    name: Option<&str>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_f64 = ctx.ctx.f64_type();

    assert_eq!(v.get_type(), llvm_f64);

    infer_and_call_function(ctx, "__nac3_isnan", Some(llvm_i1.into()), &[v.into()], name, None)
        .map(BasicValueEnum::into_int_value)
        .unwrap()
}

/// Macro to generate N-ary functions accepting an arbitrary number of `f64` as arguments and
/// returning `f64`.
///
/// Arguments:
///
/// - `$fn_name:ident`: The name of the Rust function to be generated.
/// - `$builtin_fn:ident`: The name of the builtin function to be invoked in the body of the
///   generated function. The corresponding function in IRRT must be prefixed with `__nac3_`.
/// - `$(,$args:ident)*`: The parameter name(s) to the IRRT function.
macro_rules! generate_f64_nary_fn {
    ($fn_name:ident, $builtin_fn:ident $(,$args:ident)* $(,)?) => {
        #[doc = concat!("Generates a call to [`", stringify!($builtin_fn), "`](https://en.cppreference.com/w/c/numeric/math/", stringify!($builtin_fn), ") in IR." )]
        pub fn $fn_name<'ctx>(
            ctx: &CodeGenContext<'ctx, '_>,
            $($args: FloatValue<'ctx>,)*
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            const FN_NAME: &str = concat!("__nac3_", stringify!($builtin_fn));

            let llvm_f64 = ctx.ctx.f64_type();
            $(debug_assert_eq!($args.get_type(), llvm_f64);)*

            infer_and_call_function(
                ctx,
                FN_NAME,
                Some(llvm_f64.into()),
                &[$($args.into()),*],
                name,
                Some(&|func| {
                    func.add_attribute(
                        AttributeLoc::Function,
                        ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0)
                    );
                }),
            )
            .map(BasicValueEnum::into_float_value)
            .unwrap()
        }
    };
}

generate_f64_nary_fn!(call_tan, tan, arg);
generate_f64_nary_fn!(call_asin, asin, arg);
generate_f64_nary_fn!(call_acos, acos, arg);
generate_f64_nary_fn!(call_atan, atan, arg);
generate_f64_nary_fn!(call_sinh, sinh, arg);
generate_f64_nary_fn!(call_cosh, cosh, arg);
generate_f64_nary_fn!(call_tanh, tanh, arg);
generate_f64_nary_fn!(call_asinh, asinh, arg);
generate_f64_nary_fn!(call_acosh, acosh, arg);
generate_f64_nary_fn!(call_atanh, atanh, arg);
generate_f64_nary_fn!(call_expm1, expm1, arg);
generate_f64_nary_fn!(call_cbrt, cbrt, arg);
generate_f64_nary_fn!(call_erf, erf, arg);
generate_f64_nary_fn!(call_erfc, erfc, arg);
generate_f64_nary_fn!(call_atan2, atan2, y, x);
generate_f64_nary_fn!(call_hypot, hypot, x, y);
generate_f64_nary_fn!(call_nextafter, nextafter, from, to);

/// Invokes the [`ldexp`](https://en.cppreference.com/w/c/numeric/math/ldexp) function.
pub fn call_ldexp<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    exp: IntValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "__nac3_ldexp";

    let llvm_f64 = ctx.ctx.f64_type();
    let llvm_i32 = ctx.ctx.i32_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);
    debug_assert_eq!(exp.get_type(), llvm_i32);

    infer_and_call_function(
        ctx,
        FN_NAME,
        Some(llvm_f64.into()),
        &[arg.into(), exp.into()],
        name,
        Some(&|func| {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
            );
        }),
    )
    .map(BasicValueEnum::into_float_value)
    .unwrap()
}
