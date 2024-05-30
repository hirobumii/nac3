use inkwell::AddressSpace;
use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue, PointerValue};
use itertools::Either;

use crate::codegen::{CodeGenContext, CodeGenerator, llvm_intrinsics};

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

/// Helper function for invoking `output_bool` with a compile-time constant value.
pub fn call_output_bool_const(ctx: &mut CodeGenContext<'_, '_>, value: bool) {
    let llvm_i1 = ctx.ctx.bool_type();

    call_output_bool(ctx, llvm_i1.const_int(value as u64, false));
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

/// Helper function for invoking `output_int32` with a compile-time constant value.
pub fn call_output_int32_const(ctx: &mut CodeGenContext<'_, '_>, value: i32) {
    let llvm_i32 = ctx.ctx.i32_type();

    call_output_int32(ctx, llvm_i32.const_int(value as u64, true));
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

/// Helper function for invoking `output_int64` with a compile-time constant value.
pub fn call_output_int64_const(ctx: &mut CodeGenContext<'_, '_>, value: i64) {
    let llvm_i64 = ctx.ctx.i64_type();

    call_output_int64(ctx, llvm_i64.const_int(value as u64, true));
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

/// Helper function for invoking `output_uint32` with a compile-time constant value.
pub fn call_output_uint32_const(ctx: &mut CodeGenContext<'_, '_>, value: u32) {
    let llvm_i32 = ctx.ctx.i32_type();

    call_output_uint32(ctx, llvm_i32.const_int(value as u64, false));
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

/// Helper function for invoking `output_uint64` with a compile-time constant value.
pub fn call_output_uint64_const(ctx: &mut CodeGenContext<'_, '_>, value: u64) {
    let llvm_i64 = ctx.ctx.i64_type();

    call_output_uint64(ctx, llvm_i64.const_int(value, false));
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

/// Helper function for invoking `output_float64` with a compile-time constant value.
pub fn call_output_float64_const(ctx: &mut CodeGenContext<'_, '_>, value: f64) {
    let llvm_f64 = ctx.ctx.f64_type();

    call_output_float64(ctx, llvm_f64.const_float(value));
}

/// Invokes `output_str` in the demo library.
pub fn call_output_str<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    value: PointerValue<'ctx>,
) {
    const FN_NAME: &str = "output_str";

    if ctx.registry.codegen_options.use_demo_lib {
        let llvm_void = ctx.ctx.void_type();
        let llvm_str = ctx.get_llvm_type(generator, ctx.primitives.str).into_struct_type();
        let llvm_pstr = llvm_str.ptr_type(AddressSpace::default());

        debug_assert_eq!(value.get_type(), llvm_pstr);

        let extern_fn = ctx.module.get_function(FN_NAME).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(&[llvm_pstr.into()], false);
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

/// Helper function for invoking `output_str` with a compile-time constant value.
pub fn call_output_str_const<G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    value: &str,
) {
    let str = ctx.gen_string(generator, value).into_struct_value();

    // The alloca is necessary to form a pointer to the string struct, but since we only need it for
    // the call to `call_output_str`, do a stack save-and-restore to ensure that this can be called
    // anywhere
    let stack_addr = llvm_intrinsics::call_stacksave(ctx, None);
    let pstr = ctx.builder.build_alloca(str.get_type(), "").unwrap();
    ctx.builder.build_store(pstr, str).unwrap();
    call_output_str(generator, ctx, pstr);
    llvm_intrinsics::call_stackrestore(ctx, stack_addr);
}