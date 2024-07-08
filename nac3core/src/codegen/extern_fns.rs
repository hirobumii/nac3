use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue};
use itertools::Either;

use crate::codegen::CodeGenContext;

/// Macro to conveniently generate extern function call
/// Generates a public function with `FloatValue` return type
///
/// Arguments:
/// * `$fn_name:ident`: The identifier of the rust function to be generated
/// * `$extern_fn:literal`: Name of underlying extern function
/// * `$(,$args:ident)*`: Operands of the function
/// The data type of these operands must be `FloatValue`
/// * `$(,$attributes:literal)*)`: Attributes linked with the extern function
///  
macro_rules! helper_generate_extern_fn_call {
    ($fn_name:ident, $extern_fn:literal $(,$args:ident)* $(,$attributes:literal)*) => {
        #[doc = concat!("Invokes the [`", stringify!($extern_fn), "`](https://en.cppreference.com/w/c/numeric/math/", stringify!($llvm_name), ") function." )]
        pub fn $fn_name<'ctx>(
            ctx: &CodeGenContext<'ctx, '_>
            $(,$args: FloatValue<'ctx>)*,
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            const FN_NAME: &str = $extern_fn;

            let llvm_f64 = ctx.ctx.f64_type();
            $(debug_assert_eq!($args.get_type(), llvm_f64);)*

            let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
                let fn_type = llvm_f64.fn_type(&[$($args.get_type().into()),*], false);
                let func = ctx.module.add_function(FN_NAME, fn_type, None);
                for attr in [$($attributes),*] {
                    func.add_attribute(
                        AttributeLoc::Function,
                        ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
                    );
                }
                func
            });

            ctx.builder
                .build_call(extern_fn, &[$($args.into()),*], name.unwrap_or_default())
                .map(CallSiteValue::try_as_basic_value)
                .map(|v| v.map_left(BasicValueEnum::into_float_value))
                .map(Either::unwrap_left)
                .unwrap()
        }
    };
}

/// Macro to conveniently generate extern function call with [`helper_generate_extern_fn_call`].
/// Handles unary extern functions only (Use `generate_extern_binary_fn_call` for binary functions)
/// Both function return type and function parameter type are `FloatValue`
///
/// Arguments:
/// * `$fn_name:ident`: The identifier of the rust function to be generated
/// * `$extern_fn:literal`: Name of underlying extern function
/// * `$(,$attributes:literal)*)`: Attributes linked with the extern function
/// The default attributes are "mustprogress", "nofree", "nounwind", "willreturn", and "writeonly"
/// These will be used unless other attributes are specified
///  
macro_rules! generate_extern_unary_fn_call {
    ($fn_name:ident, $extern_fn:literal) => {
        helper_generate_extern_fn_call!($fn_name, $extern_fn, arg, "mustprogress", "nofree", "nounwind", "willreturn", "writeonly");
    };
    ($fn_name:ident, $extern_fn:literal $(,$attributes:literal)*) => {
        helper_generate_extern_fn_call!($fn_name, $extern_fn, arg $(,$attributes)*);
    };
}

/// Macro to conveniently generate extern function call with [`helper_generate_extern_fn_call`].
/// Handles binary extern functions only (Use `generate_extern_unary_fn_call` for unary functions)
/// Both function return type and function parameter type are `FloatValue`
///
/// Arguments:
/// * `$fn_name:ident`: The identifier of the rust function to be generated
/// * `$extern_fn:literal`: Name of underlying extern function
/// * `$(,$attributes:literal)*)`: Attributes linked with the extern function
/// The default attributes are "mustprogress", "nofree", "nounwind", "willreturn", and "writeonly"
/// These will be used unless other attributes are specified
///  
macro_rules! generate_extern_binary_fn_call {
    ($fn_name:ident, $extern_fn:literal) => {
        helper_generate_extern_fn_call!($fn_name, $extern_fn, arg1, arg2, "mustprogress", "nofree", "nounwind", "willreturn", "writeonly");
    };
    ($fn_name:ident, $extern_fn:literal $(,$attributes:literal)*) => {
        helper_generate_extern_fn_call!($fn_name, $extern_fn, arg1, arg2 $(,$attributes)*);
    };
}

generate_extern_unary_fn_call!(call_tan, "tan");
generate_extern_unary_fn_call!(call_asin, "asin");
generate_extern_unary_fn_call!(call_acos, "acos");
generate_extern_unary_fn_call!(call_atan, "atan");
generate_extern_unary_fn_call!(call_sinh, "sinh");
generate_extern_unary_fn_call!(call_cosh, "cosh");
generate_extern_unary_fn_call!(call_tanh, "tanh");
generate_extern_unary_fn_call!(call_asinh, "asinh");
generate_extern_unary_fn_call!(call_acosh, "acosh");
generate_extern_unary_fn_call!(call_atanh, "atanh");
generate_extern_unary_fn_call!(call_expm1, "expm1");
generate_extern_unary_fn_call!(
    call_cbrt,
    "cbrt",
    "mustprogress",
    "nofree",
    "nosync",
    "nounwind",
    "readonly",
    "willreturn"
);
generate_extern_unary_fn_call!(call_erf, "erf", "nounwind");
generate_extern_unary_fn_call!(call_erfc, "erfc", "nounwind");
generate_extern_unary_fn_call!(call_j1, "j1", "nounwind");

generate_extern_binary_fn_call!(call_atan2, "atan2");
generate_extern_binary_fn_call!(call_hypot, "hypot", "nounwind");
generate_extern_binary_fn_call!(call_nextafter, "nextafter", "nounwind");

/// Invokes the [`ldexp`](https://en.cppreference.com/w/c/numeric/math/ldexp) function.
pub fn call_ldexp<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    exp: IntValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "ldexp";

    let llvm_f64 = ctx.ctx.f64_type();
    let llvm_i32 = ctx.ctx.i32_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);
    debug_assert_eq!(exp.get_type(), llvm_i32);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into(), llvm_i32.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into(), exp.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}
