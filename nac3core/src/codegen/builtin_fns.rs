use inkwell::{FloatPredicate, IntPredicate};
use inkwell::types::{BasicTypeEnum, IntType};
use inkwell::values::{BasicValueEnum, FloatValue, IntValue};
use itertools::Itertools;

use crate::codegen::{CodeGenContext, CodeGenerator, extern_fns, irrt, llvm_intrinsics};
use crate::typecheck::typedef::Type;

/// Shorthand for [`unreachable!()`] when a type of argument is not supported.
///
/// The generated message will contain the function name and the name of the unsupported type.
fn unsupported_type(
    ctx: &CodeGenContext<'_, '_>,
    fn_name: &str,
    tys: &[Type],
) -> ! {
    unreachable!(
        "{fn_name}() not supported for '{}'",
        tys.iter().map(|ty| format!("'{}'", ctx.unifier.stringify(*ty))).join(", "),
    )
}

/// Invokes the `int32` builtin function.
pub fn call_int32<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> IntValue<'ctx> {
    let llvm_i32 = ctx.ctx.i32_type();

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(int_ty) if matches!(int_ty.get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            ctx.builder
                .build_int_z_extend(n.into_int_value(), llvm_i32, "zext")
                .unwrap()
        }

        BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 32 => {
            debug_assert!([
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into_int_value()
        }

        BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 64 => {
            debug_assert!([
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            ctx.builder
                .build_int_truncate(n.into_int_value(), llvm_i32, "trunc")
                .unwrap()
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let to_int64 = ctx.builder
                .build_float_to_signed_int(n.into_float_value(), ctx.ctx.i64_type(), "")
                .unwrap();
            ctx.builder
                .build_int_truncate(to_int64, llvm_i32, "conv")
                .unwrap()
        }

        _ => unsupported_type(ctx, "int32", &[n_ty])
    }
}

/// Invokes the `int64` builtin function.
pub fn call_int64<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> IntValue<'ctx> {
    let llvm_i64 = ctx.ctx.i64_type();

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(int_ty) if matches!(int_ty.get_bit_width(), 1 | 8 | 32) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder
                    .build_int_s_extend(n.into_int_value(), llvm_i64, "sext")
                    .unwrap() 
            } else {
                ctx.builder
                    .build_int_z_extend(n.into_int_value(), llvm_i64, "zext")
                    .unwrap()
            }
        }

        BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 64 => {
            debug_assert!([
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into_int_value()
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            ctx.builder
                .build_float_to_signed_int(n.into_float_value(), ctx.ctx.i64_type(), "fptosi")
                .unwrap()
        }

        _ => unsupported_type(ctx, "int64", &[n_ty])
    }
}

/// Invokes the `uint32` builtin function.
pub fn call_uint32<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> IntValue<'ctx> {
    let llvm_i32 = ctx.ctx.i32_type();

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(int_ty) if matches!(int_ty.get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            ctx.builder
                .build_int_z_extend(n.into_int_value(), llvm_i32, "zext")
                .unwrap()
        }

        BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 32 => {
            debug_assert!([
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into_int_value()
        }

        BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 64 => {
            debug_assert!(
                ctx.unifier.unioned(n_ty, ctx.primitives.int64)
                    || ctx.unifier.unioned(n_ty, ctx.primitives.uint64)
            );

            ctx.builder
                .build_int_truncate(n.into_int_value(), llvm_i32, "trunc")
                .unwrap()
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = n.into_float_value();
            let val_gez = ctx.builder
                .build_float_compare(FloatPredicate::OGE, val, val.get_type().const_zero(), "")
                .unwrap();

            let to_int32 = ctx.builder
                .build_float_to_signed_int(val, llvm_i32, "")
                .unwrap();
            let to_uint64 = ctx.builder
                .build_float_to_unsigned_int(val, ctx.ctx.i64_type(), "")
                .unwrap();

            ctx.builder
                .build_select(
                    val_gez,
                    ctx.builder.build_int_truncate(to_uint64, llvm_i32, "").unwrap(),
                    to_int32,
                    "conv",
                )
                .map(BasicValueEnum::into_int_value)
                .unwrap()
        }

        _ => unsupported_type(ctx, "uint32", &[n_ty])
    }
}

/// Invokes the `uint64` builtin function.
pub fn call_uint64<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> IntValue<'ctx> {
    let llvm_i64 = ctx.ctx.i64_type();

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(int_ty) if matches!(int_ty.get_bit_width(), 1 | 8 | 32) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder
                    .build_int_s_extend(n.into_int_value(), llvm_i64, "sext")
                    .unwrap()
            } else {
                ctx.builder
                    .build_int_z_extend(n.into_int_value(), llvm_i64, "zext")
                    .unwrap()
            }
        }

        BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 64 => {
            debug_assert!([
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into_int_value()
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = n.into_float_value();
            let val_gez = ctx.builder
                .build_float_compare(FloatPredicate::OGE, val, val.get_type().const_zero(), "")
                .unwrap();

            let to_int64 = ctx.builder
                .build_float_to_signed_int(val, llvm_i64, "")
                .unwrap();
            let to_uint64 = ctx.builder
                .build_float_to_unsigned_int(val, llvm_i64, "")
                .unwrap();

            ctx.builder
                .build_select(val_gez, to_uint64, to_int64, "conv")
                .map(BasicValueEnum::into_int_value)
                .unwrap()
        }

        _ => unsupported_type(ctx, "uint64", &[n_ty])
    }
}

/// Invokes the `float` builtin function.
pub fn call_float<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(int_ty) if matches!(int_ty.get_bit_width(), 1 | 8 | 32 | 64) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if [
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)) {
                ctx.builder
                    .build_signed_int_to_float(n.into_int_value(), llvm_f64, "sitofp")
                    .unwrap()
            } else {
                ctx.builder
                    .build_unsigned_int_to_float(n.into_int_value(), llvm_f64, "uitofp")
                    .unwrap() 
            }
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            n.into_float_value()
        }

        _ => unsupported_type(ctx, "float", &[n_ty])
    }
}

/// Invokes the `round` builtin function.
pub fn call_round<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, FloatValue<'ctx>),
    llvm_ret_ty: IntType<'ctx>,
) -> IntValue<'ctx> {
    const FN_NAME: &str = "round";

    let (n_ty, n) = n;

    if !ctx.unifier.unioned(n_ty, ctx.primitives.float) {
        unsupported_type(ctx, FN_NAME, &[n_ty])
    }

    let val = llvm_intrinsics::call_float_round(ctx, n, None);
    ctx.builder
        .build_float_to_signed_int(val, llvm_ret_ty, FN_NAME)
        .unwrap()
}

/// Invokes the `np_round` builtin function.
pub fn call_numpy_round<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (n_ty, n) = n;

    if !ctx.unifier.unioned(n_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_round", &[n_ty])
    }

    llvm_intrinsics::call_float_roundeven(ctx, n, None)
}

/// Invokes the `bool` builtin function.
pub fn call_bool<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> IntValue<'ctx> {
    const FN_NAME: &str = "bool";

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(int_ty) if matches!(int_ty.get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            n.into_int_value()
        }

        BasicTypeEnum::IntType(_) => {
            debug_assert!([
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            let val = n.into_int_value();
            ctx.builder
                .build_int_compare(IntPredicate::NE, val, val.get_type().const_zero(), FN_NAME)
                .unwrap()
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = n.into_float_value();
            ctx.builder
                .build_float_compare(FloatPredicate::UNE, val, val.get_type().const_zero(), FN_NAME)
                .unwrap()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
    }
}

/// Invokes the `floor` builtin function.
pub fn call_floor<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, FloatValue<'ctx>),
    llvm_ret_ty: BasicTypeEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "floor";

    let (n_ty, n) = n;
    debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

    let val = llvm_intrinsics::call_float_floor(ctx, n, None);
    match llvm_ret_ty {
        _ if llvm_ret_ty == val.get_type().into() => val.into(),

        BasicTypeEnum::IntType(_) => {
            ctx.builder
                .build_float_to_signed_int(val, llvm_ret_ty.into_int_type(), FN_NAME)
                .map(Into::into)
                .unwrap()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
    }
}

/// Invokes the `ceil` builtin function.
pub fn call_ceil<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, FloatValue<'ctx>),
    llvm_ret_ty: BasicTypeEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "ceil";

    let (n_ty, n) = n;
    debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

    let val = llvm_intrinsics::call_float_ceil(ctx, n, None);
    match llvm_ret_ty {
        _ if llvm_ret_ty == val.get_type().into() => val.into(),

        BasicTypeEnum::IntType(_) => {
            ctx.builder
                .build_float_to_signed_int(val, llvm_ret_ty.into_int_type(), FN_NAME)
                .map(Into::into)
                .unwrap()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
    }
}

/// Invokes the `min` builtin function.
pub fn call_min<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    m: (Type, BasicValueEnum<'ctx>),
    n: (Type, BasicValueEnum<'ctx>),
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "min";

    let (m_ty, m) = m;
    let (n_ty, n) = n;

    if !ctx.unifier.unioned(m_ty, n_ty) {
        unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    }
    debug_assert_eq!(m.get_type(), n.get_type());

    let common_ty = m_ty;
    let llvm_common_ty = m.get_type();

    match llvm_common_ty {
        BasicTypeEnum::IntType(_) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)));

            let (m, n) = (m.into_int_value(), n.into_int_value());

            if [
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)) {
                llvm_intrinsics::call_int_smin(ctx, m, n, Some(FN_NAME)).into()
            } else {
                llvm_intrinsics::call_int_umin(ctx, m, n, Some(FN_NAME)).into()
            }
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            let (m, n) = (m.into_float_value(), n.into_float_value());

            llvm_intrinsics::call_float_minnum(ctx, m, n, Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    }
}

/// Invokes the `max` builtin function.
pub fn call_max<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    m: (Type, BasicValueEnum<'ctx>),
    n: (Type, BasicValueEnum<'ctx>),
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "max";

    let (m_ty, m) = m;
    let (n_ty, n) = n;

    if !ctx.unifier.unioned(m_ty, n_ty) {
        unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    }
    debug_assert_eq!(m.get_type(), n.get_type());

    let common_ty = m_ty;
    let llvm_common_ty = m.get_type();

    match llvm_common_ty {
        BasicTypeEnum::IntType(_) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)));

            let (m, n) = (m.into_int_value(), n.into_int_value());

            if [
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)) {
                llvm_intrinsics::call_int_smax(ctx, m, n, Some(FN_NAME)).into()
            } else {
                llvm_intrinsics::call_int_umax(ctx, m, n, Some(FN_NAME)).into()
            }
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            let (m, n) = (m.into_float_value(), n.into_float_value());

            llvm_intrinsics::call_float_maxnum(ctx, m, n, Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    }
}

/// Invokes the `abs` builtin function.
pub fn call_abs<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "abs";

    let llvm_i1 = ctx.ctx.bool_type();

    let (n_ty, n) = n;

    match n.get_type() {
        BasicTypeEnum::IntType(_) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if [
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)) {
                llvm_intrinsics::call_int_abs(
                    ctx,
                    n.into_int_value(),
                    llvm_i1.const_zero(),
                    Some(FN_NAME),
                ).into()
            } else {
                n
            }
        }

        BasicTypeEnum::FloatType(_) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_fabs(ctx, n.into_float_value(), Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
    }
}

/// Invokes the `np_isnan` builtin function.
pub fn call_numpy_isnan<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> IntValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_isnan", &[x_ty])
    }

    irrt::call_isnan(generator, ctx, x)
}

/// Invokes the `np_isinf` builtin function.
pub fn call_numpy_isinf<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> IntValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_isinf", &[x_ty])
    }

    irrt::call_isinf(generator, ctx, x)
}

/// Invokes the `np_sin` builtin function.
pub fn call_numpy_sin<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_sin", &[x_ty])
    }

    llvm_intrinsics::call_float_sin(ctx, x, None)
}

/// Invokes the `np_cos` builtin function.
pub fn call_numpy_cos<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_cos", &[x_ty])
    }

    llvm_intrinsics::call_float_cos(ctx, x, None)
}

/// Invokes the `np_exp` builtin function.
pub fn call_numpy_exp<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_exp", &[x_ty])
    }

    llvm_intrinsics::call_float_exp(ctx, x, None)
}

/// Invokes the `np_exp2` builtin function.
pub fn call_numpy_exp2<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_exp2", &[x_ty])
    }

    llvm_intrinsics::call_float_exp2(ctx, x, None)
}

/// Invokes the `np_log` builtin function.
pub fn call_numpy_log<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_log", &[x_ty])
    }

    llvm_intrinsics::call_float_log(ctx, x, None)
}

/// Invokes the `np_log10` builtin function.
pub fn call_numpy_log10<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_log10", &[x_ty])
    }

    llvm_intrinsics::call_float_log10(ctx, x, None)
}

/// Invokes the `np_log2` builtin function.
pub fn call_numpy_log2<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_log2", &[x_ty])
    }

    llvm_intrinsics::call_float_log2(ctx, x, None)
}

/// Invokes the `np_sqrt` builtin function.
pub fn call_numpy_fabs<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_fabs", &[x_ty])
    }

    llvm_intrinsics::call_float_fabs(ctx, x, None)
}

/// Invokes the `np_sqrt` builtin function.
pub fn call_numpy_sqrt<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_sqrt", &[x_ty])
    }

    llvm_intrinsics::call_float_sqrt(ctx, x, None)
}

/// Invokes the `np_rint` builtin function.
pub fn call_numpy_rint<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_rint", &[x_ty])
    }

    llvm_intrinsics::call_float_roundeven(ctx, x, None)
}

/// Invokes the `np_tan` builtin function.
pub fn call_numpy_tan<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_tan", &[x_ty])
    }

    extern_fns::call_tan(ctx, x, None)
}

/// Invokes the `np_arcsin` builtin function.
pub fn call_numpy_arcsin<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_arcsin", &[x_ty])
    }

    extern_fns::call_asin(ctx, x, None)
}

/// Invokes the `np_arccos` builtin function.
pub fn call_numpy_arccos<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_arccos", &[x_ty])
    }

    extern_fns::call_acos(ctx, x, None)
}

/// Invokes the `np_arctan` builtin function.
pub fn call_numpy_arctan<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_arctan", &[x_ty])
    }

    extern_fns::call_atan(ctx, x, None)
}

/// Invokes the `np_sinh` builtin function.
pub fn call_numpy_sinh<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_sinh", &[x_ty])
    }

    extern_fns::call_sinh(ctx, x, None)
}

/// Invokes the `np_cosh` builtin function.
pub fn call_numpy_cosh<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_cosh", &[x_ty])
    }

    extern_fns::call_cosh(ctx, x, None)
}

/// Invokes the `np_tanh` builtin function.
pub fn call_numpy_tanh<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_tanh", &[x_ty])
    }

    extern_fns::call_tanh(ctx, x, None)
}

/// Invokes the `np_asinh` builtin function.
pub fn call_numpy_asinh<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_asinh", &[x_ty])
    }

    extern_fns::call_asinh(ctx, x, None)
}

/// Invokes the `np_acosh` builtin function.
pub fn call_numpy_acosh<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_acosh", &[x_ty])
    }

    extern_fns::call_acosh(ctx, x, None)
}

/// Invokes the `np_atanh` builtin function.
pub fn call_numpy_atanh<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_atanh", &[x_ty])
    }

    extern_fns::call_atanh(ctx, x, None)
}

/// Invokes the `np_expm1` builtin function.
pub fn call_numpy_expm1<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_expm1", &[x_ty])
    }

    extern_fns::call_expm1(ctx, x, None)
}

/// Invokes the `np_cbrt` builtin function.
pub fn call_numpy_cbrt<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "np_cbrt", &[x_ty])
    }

    extern_fns::call_cbrt(ctx, x, None)
}

/// Invokes the `sp_spec_erf` builtin function.
pub fn call_scipy_special_erf<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    z: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (z_ty, z) = z;

    if !ctx.unifier.unioned(z_ty, ctx.primitives.float) {
        unsupported_type(ctx, "sp_spec_erf", &[z_ty])
    }

    extern_fns::call_erf(ctx, z, None)
}

/// Invokes the `sp_spec_erfc` builtin function.
pub fn call_scipy_special_erfc<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "sp_spec_erfc", &[x_ty])
    }

    extern_fns::call_erfc(ctx, x, None)
}

/// Invokes the `sp_spec_gamma` builtin function.
pub fn call_scipy_special_gamma<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    z: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (z_ty, z) = z;

    if !ctx.unifier.unioned(z_ty, ctx.primitives.float) {
        unsupported_type(ctx, "sp_spec_gamma", &[z_ty])
    }

    irrt::call_gamma(ctx, z)
}

/// Invokes the `sp_spec_gammaln` builtin function.
pub fn call_scipy_special_gammaln<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "sp_spec_gammaln", &[x_ty])
    }

    irrt::call_gammaln(ctx, x)
}

/// Invokes the `sp_spec_j0` builtin function.
pub fn call_scipy_special_j0<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "sp_spec_j0", &[x_ty])
    }

    irrt::call_j0(ctx, x)
}

/// Invokes the `sp_spec_j1` builtin function.
pub fn call_scipy_special_j1<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x_ty, x) = x;

    if !ctx.unifier.unioned(x_ty, ctx.primitives.float) {
        unsupported_type(ctx, "sp_spec_j1", &[x_ty])
    }

    extern_fns::call_j1(ctx, x, None)
}

/// Invokes the `np_arctan2` builtin function.
pub fn call_numpy_arctan2<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let float_t = ctx.primitives.float;

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    if !ctx.unifier.unioned(x1_ty, float_t) || !ctx.unifier.unioned(x2_ty, float_t) {
        unsupported_type(ctx, "np_atan2", &[x1_ty, x2_ty])
    }

    extern_fns::call_atan2(ctx, x1, x2, None)
}

/// Invokes the `np_copysign` builtin function.
pub fn call_numpy_copysign<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let float_t = ctx.primitives.float;

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;
    debug_assert_eq!(x1.get_type(), x2.get_type());

    if !ctx.unifier.unioned(x1_ty, float_t) || !ctx.unifier.unioned(x2_ty, float_t) {
        unsupported_type(ctx, "np_copysign", &[x1_ty, x2_ty])
    }

    llvm_intrinsics::call_float_copysign(ctx, x1, x2, None)
}

/// Invokes the `np_fmax` builtin function.
pub fn call_numpy_fmax<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let float_t = ctx.primitives.float;

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;
    debug_assert_eq!(x1.get_type(), x2.get_type());

    if !ctx.unifier.unioned(x1_ty, float_t) || !ctx.unifier.unioned(x2_ty, float_t) {
        unsupported_type(ctx, "np_fmax", &[x1_ty, x2_ty])
    }

    llvm_intrinsics::call_float_maxnum(ctx, x1, x2, None)
}

/// Invokes the `np_fmin` builtin function.
pub fn call_numpy_fmin<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let float_t = ctx.primitives.float;

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;
    debug_assert_eq!(x1.get_type(), x2.get_type());

    if !ctx.unifier.unioned(x1_ty, float_t) || !ctx.unifier.unioned(x2_ty, float_t) {
        unsupported_type(ctx, "np_fmin", &[x1_ty, x2_ty])
    }

    llvm_intrinsics::call_float_minnum(ctx, x1, x2, None)
}

/// Invokes the `np_ldexp` builtin function.
pub fn call_numpy_ldexp<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, IntValue<'ctx>),
) -> FloatValue<'ctx> {
    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    if !ctx.unifier.unioned(x1_ty, ctx.primitives.float) {
        unsupported_type(ctx, "fp_ldexp", &[x1_ty, x2_ty])
    }
    if !ctx.unifier.unioned(x2_ty, ctx.primitives.int32) {
        unsupported_type(ctx, "fp_ldexp", &[x1_ty, x2_ty])
    }

    extern_fns::call_ldexp(ctx, x1, x2, None)
}

/// Invokes the `np_hypot` builtin function.
pub fn call_numpy_hypot<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let float_t = ctx.primitives.float;

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    if !ctx.unifier.unioned(x1_ty, float_t) || !ctx.unifier.unioned(x2_ty, float_t) {
        unsupported_type(ctx, "np_hypot", &[x1_ty, x2_ty])
    }

    extern_fns::call_hypot(ctx, x1, x2, None)
}

/// Invokes the `np_nextafter` builtin function.
pub fn call_numpy_nextafter<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, FloatValue<'ctx>),
    x2: (Type, FloatValue<'ctx>),
) -> FloatValue<'ctx> {
    let float_t = ctx.primitives.float;

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    if !ctx.unifier.unioned(x1_ty, float_t) || !ctx.unifier.unioned(x2_ty, float_t) {
        unsupported_type(ctx, "np_nextafter", &[x1_ty, x2_ty])
    }

    extern_fns::call_nextafter(ctx, x1, x2, None)
}