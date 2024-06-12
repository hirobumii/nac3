use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue};
use itertools::Either;

use crate::codegen::CodeGenContext;

/// Invokes the [`tan`](https://en.cppreference.com/w/c/numeric/math/tan) function.
pub fn call_tan<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "tan";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`asin`](https://en.cppreference.com/w/c/numeric/math/asin) function.
pub fn call_asin<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "asin";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`acos`](https://en.cppreference.com/w/c/numeric/math/acos) function.
pub fn call_acos<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "acos";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`atan`](https://en.cppreference.com/w/c/numeric/math/atan) function.
pub fn call_atan<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "atan";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`sinh`](https://en.cppreference.com/w/c/numeric/math/sinh) function.
pub fn call_sinh<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "sinh";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`cosh`](https://en.cppreference.com/w/c/numeric/math/cosh) function.
pub fn call_cosh<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "cosh";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`tanh`](https://en.cppreference.com/w/c/numeric/math/tanh) function.
pub fn call_tanh<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "tanh";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`asinh`](https://en.cppreference.com/w/c/numeric/math/asinh) function.
pub fn call_asinh<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "asinh";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`acosh`](https://en.cppreference.com/w/c/numeric/math/acosh) function.
pub fn call_acosh<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "acosh";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`atanh`](https://en.cppreference.com/w/c/numeric/math/atanh) function.
pub fn call_atanh<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "atanh";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`expm1`](https://en.cppreference.com/w/c/numeric/math/expm1) function.
pub fn call_expm1<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "expm1";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`cbrt`](https://en.cppreference.com/w/c/numeric/math/cbrt) function.
pub fn call_cbrt<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "cbrt";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nosync", "nounwind", "readonly", "willreturn"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`erf`](https://en.cppreference.com/w/c/numeric/math/erf) function.
pub fn call_erf<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "erf";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        func.add_attribute(
            AttributeLoc::Function,
            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`erfc`](https://en.cppreference.com/w/c/numeric/math/erfc) function.
pub fn call_erfc<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "erfc";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        func.add_attribute(
            AttributeLoc::Function,
            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`j1`](https://www.gnu.org/software/libc/manual/html_node/Special-Functions.html#index-j1)
/// function.
pub fn call_j1<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "j1";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(arg.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        func.add_attribute(
            AttributeLoc::Function,
            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );

        func
    });

    ctx.builder
        .build_call(extern_fn, &[arg.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`atan2`](https://en.cppreference.com/w/c/numeric/math/atan2) function.
pub fn call_atan2<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    y: FloatValue<'ctx>,
    x: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "atan2";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(y.get_type(), llvm_f64);
    debug_assert_eq!(x.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into(), llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in ["mustprogress", "nofree", "nounwind", "willreturn", "writeonly"] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[y.into(), x.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

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

/// Invokes the [`hypot`](https://en.cppreference.com/w/c/numeric/math/hypot) function.
pub fn call_hypot<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    x: FloatValue<'ctx>,
    y: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "hypot";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(x.get_type(), llvm_f64);
    debug_assert_eq!(y.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into(), llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        func.add_attribute(
            AttributeLoc::Function,
            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );

        func
    });

    ctx.builder
        .build_call(extern_fn, &[x.into(), y.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes the [`nextafter`](https://en.cppreference.com/w/c/numeric/math/nextafter) function.
pub fn call_nextafter<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    from: FloatValue<'ctx>,
    to: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    const FN_NAME: &str = "nextafter";

    let llvm_f64 = ctx.ctx.f64_type();
    debug_assert_eq!(from.get_type(), llvm_f64);
    debug_assert_eq!(to.get_type(), llvm_f64);

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into(), llvm_f64.into()], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        func.add_attribute(
            AttributeLoc::Function,
            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );

        func
    });

    ctx.builder
        .build_call(extern_fn, &[from.into(), to.into()], name.unwrap_or_default())
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}
