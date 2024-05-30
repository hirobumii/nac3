use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue, PointerValue};
use itertools::Either;

use crate::codegen::{CodeGenContext, CodeGenerator};

/// Invokes `dbl_nan` in the demo library.
pub fn call_dbl_nan<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>) -> FloatValue<'ctx> {
    const FN_NAME: &str = "dbl_nan";

    assert!(ctx.registry.codegen_options.use_demo_lib);

    let llvm_f64 = ctx.ctx.f64_type();

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in [
            "mustprogress",
            "nofree",
            "norecurse",
            "nosync",
            "nounwind",
            "sspstrong",
            "willreturn",
            "readnone",
        ] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[], "")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes `dbl_inf` in the demo library.
pub fn call_dbl_inf<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>) -> FloatValue<'ctx> {
    const FN_NAME: &str = "dbl_inf";

    assert!(ctx.registry.codegen_options.use_demo_lib);

    let llvm_f64 = ctx.ctx.f64_type();

    let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[], false);
        let func = ctx.module.add_function(FN_NAME, fn_type, None);
        for attr in [
            "mustprogress",
            "nofree",
            "norecurse",
            "nosync",
            "nounwind",
            "sspstrong",
            "willreturn",
            "readnone",
        ] {
            func.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
            );
        }

        func
    });

    ctx.builder
        .build_call(extern_fn, &[], "")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Invokes `output_bool` in the demo library.
pub fn call_output_bool<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
    const FN_NAME: &str = "output_bool";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i1 = ctx.ctx.bool_type();

        debug_assert_eq!(value.get_type().get_bit_width(), llvm_i1.get_bit_width());

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_i1.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}

/// Invokes `output_int32` in the demo library.
pub fn call_output_int32<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
    const FN_NAME: &str = "output_int32";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i32 = ctx.ctx.i32_type();

        debug_assert_eq!(value.get_type().get_bit_width(), llvm_i32.get_bit_width());

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_i32.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}

/// Invokes `output_int64` in the demo library.
pub fn call_output_int64<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
    const FN_NAME: &str = "output_int64";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i64 = ctx.ctx.i64_type();

        debug_assert_eq!(value.get_type().get_bit_width(), llvm_i64.get_bit_width());

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_i64.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}

/// Invokes `output_uint32` in the demo library.
pub fn call_output_uint32<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
    const FN_NAME: &str = "output_uint32";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i32 = ctx.ctx.i32_type();

        debug_assert_eq!(value.get_type().get_bit_width(), llvm_i32.get_bit_width());

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_i32.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}

/// Invokes `output_uint64` in the demo library.
pub fn call_output_uint64<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
    const FN_NAME: &str = "output_uint64";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_i64 = ctx.ctx.i64_type();

        debug_assert_eq!(value.get_type().get_bit_width(), llvm_i64.get_bit_width());

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_i64.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}

/// Invokes `output_float64` in the demo library.
pub fn call_output_float64<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, value: FloatValue<'ctx>) {
    const FN_NAME: &str = "output_float64";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_f64 = ctx.ctx.f64_type();

        debug_assert_eq!(value.get_type(), llvm_f64);

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_f64.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}

/// Invokes `output_str` in the demo library.
pub fn call_output_str<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    value: PointerValue<'ctx>,
) {
    const FN_NAME: &str = "output_str";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_str = ctx.get_llvm_type(generator, ctx.primitives.str).into_pointer_type();

        debug_assert_eq!(value.get_type(), llvm_str);

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_str.into()], false);
            let func = ctx.module.add_function(FN_NAME, fn_type, None);
            for attr in [
                "nofree",
                "nounwind",
                "sspstrong",
            ] {
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                );
            }

            func
        });

        ctx.builder
            .build_call(extern_fn, &[value.into()], "")
            .unwrap();
    }
}