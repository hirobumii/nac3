use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;
use inkwell::{FloatPredicate, IntPredicate, OptimizationLevel};
use itertools::Itertools;

use crate::codegen::classes::{NDArrayValue, ProxyValue, UntypedArrayLikeAccessor};
use crate::codegen::numpy::ndarray_elementwise_unaryop_impl;
use crate::codegen::stmt::gen_for_callback_incrementing;
use crate::codegen::{extern_fns, irrt, llvm_intrinsics, numpy, CodeGenContext, CodeGenerator};
use crate::toplevel::helper::PrimDef;
use crate::toplevel::numpy::unpack_ndarray_var_tys;
use crate::typecheck::typedef::Type;

/// Shorthand for [`unreachable!()`] when a type of argument is not supported.
///
/// The generated message will contain the function name and the name of the unsupported type.
fn unsupported_type(ctx: &CodeGenContext<'_, '_>, fn_name: &str, tys: &[Type]) -> ! {
    unreachable!(
        "{fn_name}() not supported for '{}'",
        tys.iter().map(|ty| format!("'{}'", ctx.unifier.stringify(*ty))).join(", "),
    )
}

/// Invokes the `int32` builtin function.
pub fn call_int32<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            ctx.builder.build_int_z_extend(n, llvm_i32, "zext").map(Into::into).unwrap()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 32 => {
            debug_assert!([ctx.primitives.int32, ctx.primitives.uint32,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!([ctx.primitives.int64, ctx.primitives.uint64,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            ctx.builder.build_int_truncate(n, llvm_i32, "trunc").map(Into::into).unwrap()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let to_int64 =
                ctx.builder.build_float_to_signed_int(n, ctx.ctx.i64_type(), "").unwrap();
            ctx.builder.build_int_truncate(to_int64, llvm_i32, "conv").map(Into::into).unwrap()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.int32,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_int32(generator, ctx, (elem_ty, val)),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, "int32", &[n_ty]),
    })
}

/// Invokes the `int64` builtin function.
pub fn call_int64<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i64 = ctx.ctx.i64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8 | 32) => {
            debug_assert!([ctx.primitives.bool, ctx.primitives.int32, ctx.primitives.uint32,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder.build_int_s_extend(n, llvm_i64, "sext").map(Into::into).unwrap()
            } else {
                ctx.builder.build_int_z_extend(n, llvm_i64, "zext").map(Into::into).unwrap()
            }
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!([ctx.primitives.int64, ctx.primitives.uint64,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            ctx.builder
                .build_float_to_signed_int(n, ctx.ctx.i64_type(), "fptosi")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.int64,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_int64(generator, ctx, (elem_ty, val)),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, "int64", &[n_ty]),
    })
}

/// Invokes the `uint32` builtin function.
pub fn call_uint32<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            ctx.builder.build_int_z_extend(n, llvm_i32, "zext").map(Into::into).unwrap()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 32 => {
            debug_assert!([ctx.primitives.int32, ctx.primitives.uint32,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!(
                ctx.unifier.unioned(n_ty, ctx.primitives.int64)
                    || ctx.unifier.unioned(n_ty, ctx.primitives.uint64)
            );

            ctx.builder.build_int_truncate(n, llvm_i32, "trunc").map(Into::into).unwrap()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let n_gez = ctx
                .builder
                .build_float_compare(FloatPredicate::OGE, n, n.get_type().const_zero(), "")
                .unwrap();

            let to_int32 = ctx.builder.build_float_to_signed_int(n, llvm_i32, "").unwrap();
            let to_uint64 =
                ctx.builder.build_float_to_unsigned_int(n, ctx.ctx.i64_type(), "").unwrap();

            ctx.builder
                .build_select(
                    n_gez,
                    ctx.builder.build_int_truncate(to_uint64, llvm_i32, "").unwrap(),
                    to_int32,
                    "conv",
                )
                .unwrap()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.uint32,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_uint32(generator, ctx, (elem_ty, val)),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, "uint32", &[n_ty]),
    })
}

/// Invokes the `uint64` builtin function.
pub fn call_uint64<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i64 = ctx.ctx.i64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8 | 32) => {
            debug_assert!([ctx.primitives.bool, ctx.primitives.int32, ctx.primitives.uint32,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder.build_int_s_extend(n, llvm_i64, "sext").map(Into::into).unwrap()
            } else {
                ctx.builder.build_int_z_extend(n, llvm_i64, "zext").map(Into::into).unwrap()
            }
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!([ctx.primitives.int64, ctx.primitives.uint64,]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val_gez = ctx
                .builder
                .build_float_compare(FloatPredicate::OGE, n, n.get_type().const_zero(), "")
                .unwrap();

            let to_int64 = ctx.builder.build_float_to_signed_int(n, llvm_i64, "").unwrap();
            let to_uint64 = ctx.builder.build_float_to_unsigned_int(n, llvm_i64, "").unwrap();

            ctx.builder.build_select(val_gez, to_uint64, to_int64, "conv").unwrap()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.uint64,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_uint64(generator, ctx, (elem_ty, val)),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, "uint64", &[n_ty]),
    })
}

/// Invokes the `float` builtin function.
pub fn call_float<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_f64 = ctx.ctx.f64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8 | 32 | 64) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if [ctx.primitives.bool, ctx.primitives.int32, ctx.primitives.int64]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            {
                ctx.builder
                    .build_signed_int_to_float(n, llvm_f64, "sitofp")
                    .map(Into::into)
                    .unwrap()
            } else {
                ctx.builder
                    .build_unsigned_int_to_float(n, llvm_f64, "uitofp")
                    .map(Into::into)
                    .unwrap()
            }
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            n.into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.float,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_float(generator, ctx, (elem_ty, val)),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, "float", &[n_ty]),
    })
}

/// Invokes the `round` builtin function.
pub fn call_round<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "round";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;
    let llvm_ret_elem_ty = ctx.get_llvm_abi_type(generator, ret_elem_ty).into_int_type();

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = llvm_intrinsics::call_float_round(ctx, n, None);
            ctx.builder
                .build_float_to_signed_int(val, llvm_ret_elem_ty, FN_NAME)
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_round(generator, ctx, (elem_ty, val), ret_elem_ty),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `np_round` builtin function.
pub fn call_numpy_round<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_round";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_rint(ctx, n, None).into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.float,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_numpy_round(generator, ctx, (elem_ty, val)),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `bool` builtin function.
pub fn call_bool<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "bool";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            n.into()
        }

        BasicValueEnum::IntValue(n) => {
            debug_assert!([
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            ctx.builder
                .build_int_compare(IntPredicate::NE, n, n.get_type().const_zero(), FN_NAME)
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            ctx.builder
                .build_float_compare(FloatPredicate::UNE, n, n.get_type().const_zero(), FN_NAME)
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.bool,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    let elem = call_bool(generator, ctx, (elem_ty, val))?;

                    Ok(generator.bool_to_i8(ctx, elem.into_int_value()).into())
                },
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `floor` builtin function.
pub fn call_floor<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "floor";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;
    let llvm_ret_elem_ty = ctx.get_llvm_abi_type(generator, ret_elem_ty);

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = llvm_intrinsics::call_float_floor(ctx, n, None);
            if let BasicTypeEnum::IntType(llvm_ret_elem_ty) = llvm_ret_elem_ty {
                ctx.builder
                    .build_float_to_signed_int(val, llvm_ret_elem_ty, FN_NAME)
                    .map(Into::into)
                    .unwrap()
            } else {
                val.into()
            }
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_floor(generator, ctx, (elem_ty, val), ret_elem_ty),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `ceil` builtin function.
pub fn call_ceil<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "ceil";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;
    let llvm_ret_elem_ty = ctx.get_llvm_abi_type(generator, ret_elem_ty);

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = llvm_intrinsics::call_float_ceil(ctx, n, None);
            if let BasicTypeEnum::IntType(llvm_ret_elem_ty) = llvm_ret_elem_ty {
                ctx.builder
                    .build_float_to_signed_int(val, llvm_ret_elem_ty, FN_NAME)
                    .map(Into::into)
                    .unwrap()
            } else {
                val.into()
            }
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| call_floor(generator, ctx, (elem_ty, val), ret_elem_ty),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
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

    let common_ty = if ctx.unifier.unioned(m_ty, n_ty) {
        m_ty
    } else {
        unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    };

    match (m, n) {
        (BasicValueEnum::IntValue(m), BasicValueEnum::IntValue(n)) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(common_ty, *ty)));

            if [ctx.primitives.int32, ctx.primitives.int64]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty, *ty))
            {
                llvm_intrinsics::call_int_smin(ctx, m, n, Some(FN_NAME)).into()
            } else {
                llvm_intrinsics::call_int_umin(ctx, m, n, Some(FN_NAME)).into()
            }
        }

        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_minnum(ctx, m, n, Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty]),
    }
}

/// Invokes the `np_minimum` builtin function.
pub fn call_numpy_minimum<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_minimum";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    let common_ty = if ctx.unifier.unioned(x1_ty, x2_ty) { Some(x1_ty) } else { None };

    Ok(match (x1, x2) {
        (BasicValueEnum::IntValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
                ctx.primitives.float,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(common_ty.unwrap(), *ty)));

            call_min(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(common_ty.unwrap(), ctx.primitives.float));

            call_min(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_minimum(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
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

    let common_ty = if ctx.unifier.unioned(m_ty, n_ty) {
        m_ty
    } else {
        unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    };

    match (m, n) {
        (BasicValueEnum::IntValue(m), BasicValueEnum::IntValue(n)) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(common_ty, *ty)));

            if [ctx.primitives.int32, ctx.primitives.int64]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty, *ty))
            {
                llvm_intrinsics::call_int_smax(ctx, m, n, Some(FN_NAME)).into()
            } else {
                llvm_intrinsics::call_int_umax(ctx, m, n, Some(FN_NAME)).into()
            }
        }

        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_maxnum(ctx, m, n, Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty]),
    }
}

/// Invokes the `np_max`, `np_min`, `np_argmax`, `np_argmin` functions
/// * `fn_name`: Can be one of `"np_argmin"`, `"np_argmax"`, `"np_max"`, `"np_min"`
pub fn call_numpy_max_min<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    a: (Type, BasicValueEnum<'ctx>),
    fn_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    debug_assert!(["np_argmin", "np_argmax", "np_max", "np_min"].iter().any(|f| *f == fn_name));

    let llvm_int64 = ctx.ctx.i64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (a_ty, a) = a;
    Ok(match a {
        BasicValueEnum::IntValue(_) | BasicValueEnum::FloatValue(_) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
                ctx.primitives.float,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(a_ty, *ty)));

            match fn_name {
                "np_argmin" | "np_argmax" => llvm_int64.const_zero().into(),
                "np_max" | "np_min" => a,
                _ => unreachable!(),
            }
        }
        BasicValueEnum::PointerValue(n)
            if a_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, a_ty);
            let llvm_ndarray_ty = ctx.get_llvm_type(generator, elem_ty);

            let n = NDArrayValue::from_ptr_val(n, llvm_usize, None);
            let n_sz = irrt::call_ndarray_calc_size(generator, ctx, &n.dim_sizes(), (None, None));
            if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
                let n_sz_eqz = ctx
                    .builder
                    .build_int_compare(IntPredicate::NE, n_sz, n_sz.get_type().const_zero(), "")
                    .unwrap();

                ctx.make_assert(
                    generator,
                    n_sz_eqz,
                    "0:ValueError",
                    format!("zero-size array to reduction operation {fn_name}").as_str(),
                    [None, None, None],
                    ctx.current_loc,
                );
            }

            let accumulator_addr = generator.gen_var_alloc(ctx, llvm_ndarray_ty, None)?;
            let res_idx = generator.gen_var_alloc(ctx, llvm_int64.into(), None)?;

            unsafe {
                let identity =
                    n.data().get_unchecked(ctx, generator, &llvm_usize.const_zero(), None);
                ctx.builder.build_store(accumulator_addr, identity).unwrap();
                ctx.builder.build_store(res_idx, llvm_int64.const_zero()).unwrap();
            }

            gen_for_callback_incrementing(
                generator,
                ctx,
                None,
                llvm_int64.const_int(1, false),
                (n_sz, false),
                |generator, ctx, _, idx| {
                    let elem = unsafe { n.data().get_unchecked(ctx, generator, &idx, None) };
                    let accumulator = ctx.builder.build_load(accumulator_addr, "").unwrap();
                    let cur_idx = ctx.builder.build_load(res_idx, "").unwrap();

                    let result = match fn_name {
                        "np_argmin" | "np_min" => {
                            call_min(ctx, (elem_ty, accumulator), (elem_ty, elem))
                        }
                        "np_argmax" | "np_max" => {
                            call_max(ctx, (elem_ty, accumulator), (elem_ty, elem))
                        }
                        _ => unreachable!(),
                    };

                    let updated_idx = match (accumulator, result) {
                        (BasicValueEnum::IntValue(m), BasicValueEnum::IntValue(n)) => ctx
                            .builder
                            .build_select(
                                ctx.builder.build_int_compare(IntPredicate::NE, m, n, "").unwrap(),
                                idx.into(),
                                cur_idx,
                                "",
                            )
                            .unwrap(),
                        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => ctx
                            .builder
                            .build_select(
                                ctx.builder
                                    .build_float_compare(FloatPredicate::ONE, m, n, "")
                                    .unwrap(),
                                idx.into(),
                                cur_idx,
                                "",
                            )
                            .unwrap(),
                        _ => unsupported_type(ctx, fn_name, &[elem_ty, elem_ty]),
                    };
                    ctx.builder.build_store(res_idx, updated_idx).unwrap();
                    ctx.builder.build_store(accumulator_addr, result).unwrap();

                    Ok(())
                },
                llvm_int64.const_int(1, false),
            )?;

            match fn_name {
                "np_argmin" | "np_argmax" => ctx.builder.build_load(res_idx, "").unwrap(),
                "np_max" | "np_min" => ctx.builder.build_load(accumulator_addr, "").unwrap(),
                _ => unreachable!(),
            }
        }

        _ => unsupported_type(ctx, fn_name, &[a_ty]),
    })
}

/// Invokes the `np_maximum` builtin function.
pub fn call_numpy_maximum<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_maximum";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    let common_ty = if ctx.unifier.unioned(x1_ty, x2_ty) { Some(x1_ty) } else { None };

    Ok(match (x1, x2) {
        (BasicValueEnum::IntValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
                ctx.primitives.float,
            ]
            .iter()
            .any(|ty| ctx.unifier.unioned(common_ty.unwrap(), *ty)));

            call_max(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(common_ty.unwrap(), ctx.primitives.float));

            call_max(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_maximum(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Helper function to create a built-in elementwise unary numpy function that takes in either an ndarray or a scalar.
///
/// * `(arg_ty, arg_val)`: The [`Type`] and llvm value of the input argument.
/// * `fn_name`: The name of the function, only used when throwing an error with [`unsupported_type`]
/// * `get_ret_elem_type`: A function that takes in the input scalar [`Type`], and returns the function's return scalar [`Type`].
/// Return a constant [`Type`] here if the return type does not depend on the input type.
/// * `on_scalar`: The function that acts on the scalars of the input. Returns [`Option::None`]
/// if the scalar type & value are faulty and should panic with [`unsupported_type`].
fn helper_call_numpy_unary_elementwise<'ctx, OnScalarFn, RetElemFn, G>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (arg_ty, arg_val): (Type, BasicValueEnum<'ctx>),
    fn_name: &str,
    get_ret_elem_type: &RetElemFn,
    on_scalar: &OnScalarFn,
) -> Result<BasicValueEnum<'ctx>, String>
where
    G: CodeGenerator + ?Sized,
    OnScalarFn: Fn(
        &mut G,
        &mut CodeGenContext<'ctx, '_>,
        Type,
        BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>>,
    RetElemFn: Fn(&mut CodeGenContext<'ctx, '_>, Type) -> Type,
{
    let result = match arg_val {
        BasicValueEnum::PointerValue(x)
            if arg_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let llvm_usize = generator.get_size_type(ctx.ctx);
            let (arg_elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, arg_ty);
            let ret_elem_ty = get_ret_elem_type(ctx, arg_elem_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, elem_val| {
                    helper_call_numpy_unary_elementwise(
                        generator,
                        ctx,
                        (arg_elem_ty, elem_val),
                        fn_name,
                        get_ret_elem_type,
                        on_scalar,
                    )
                },
            )?;
            ndarray.as_base_value().into()
        }

        _ => on_scalar(generator, ctx, arg_ty, arg_val)
            .unwrap_or_else(|| unsupported_type(ctx, fn_name, &[arg_ty])),
    };

    Ok(result)
}

pub fn call_abs<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "abs";
    helper_call_numpy_unary_elementwise(
        generator,
        ctx,
        n,
        FN_NAME,
        &|_ctx, elem_ty| elem_ty,
        &|_generator, ctx, val_ty, val| match val {
            BasicValueEnum::IntValue(n) => Some({
                debug_assert!([
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(val_ty, *ty)));

                if [ctx.primitives.int32, ctx.primitives.int64]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(val_ty, *ty))
                {
                    llvm_intrinsics::call_int_abs(
                        ctx,
                        n,
                        ctx.ctx.bool_type().const_zero(),
                        Some(FN_NAME),
                    )
                    .into()
                } else {
                    n.into()
                }
            }),

            BasicValueEnum::FloatValue(n) => Some({
                debug_assert!(ctx.unifier.unioned(val_ty, ctx.primitives.float));

                llvm_intrinsics::call_float_fabs(ctx, n, Some(FN_NAME)).into()
            }),

            _ => None,
        },
    )
}

/// Macro to conveniently generate numpy functions with [`helper_call_numpy_unary_elementwise`].
///
/// Arguments:
/// * `$name:ident`: The identifier of the rust function to be generated.
/// * `$fn_name:literal`: To be passed to the `fn_name` parameter of [`helper_call_numpy_unary_elementwise`]
/// * `$get_ret_elem_type:expr`: To be passed to the `get_ret_elem_type` parameter of [`helper_call_numpy_unary_elementwise`].
/// But there is no need to make it a reference.
/// * `$on_scalar:expr`: To be passed to the `on_scalar` parameter of [`helper_call_numpy_unary_elementwise`].
/// But there is no need to make it a reference.
macro_rules! create_helper_call_numpy_unary_elementwise {
    ($name:ident, $fn_name:literal, $get_ret_elem_type:expr, $on_scalar:expr) => {
        #[allow(clippy::redundant_closure_call)]
        pub fn $name<'ctx, G: CodeGenerator + ?Sized>(
            generator: &mut G,
            ctx: &mut CodeGenContext<'ctx, '_>,
            arg: (Type, BasicValueEnum<'ctx>),
        ) -> Result<BasicValueEnum<'ctx>, String> {
            helper_call_numpy_unary_elementwise(
                generator,
                ctx,
                arg,
                $fn_name,
                &$get_ret_elem_type,
                &$on_scalar,
            )
        }
    };
}

/// A specialized version of [`create_helper_call_numpy_unary_elementwise`] to generate functions that takes in float and returns boolean (as an `i8`) elementwise.
///
/// Arguments:
/// * `$name:ident`: The identifier of the rust function to be generated.
/// * `$fn_name:literal`: To be passed to the `fn_name` parameter of [`helper_call_numpy_unary_elementwise`].
/// * `$on_scalar:expr`: The closure (see below for its type) that acts on float scalar values and returns
/// the boolean results of LLVM type `i1`. The returned `i1` value will be converted into an `i8`.
///
/// ```ignore
/// // Type of `$on_scalar:expr`
/// fn on_scalar<'ctx, G: CodeGenerator + ?Sized>(
///     generator: &mut G,
///     ctx: &mut CodeGenContext<'ctx, '_>,
///     arg: FloatValue<'ctx>
/// ) -> IntValue<'ctx> // of LLVM type `i1`
/// ```
macro_rules! create_helper_call_numpy_unary_elementwise_float_to_bool {
    ($name:ident, $fn_name:literal, $on_scalar:expr) => {
        create_helper_call_numpy_unary_elementwise!(
            $name,
            $fn_name,
            |ctx, _| ctx.primitives.bool,
            |generator, ctx, n_ty, val| {
                match val {
                    BasicValueEnum::FloatValue(n) => {
                        debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

                        let ret = $on_scalar(generator, ctx, n);
                        Some(generator.bool_to_i8(ctx, ret).into())
                    }
                    _ => None,
                }
            }
        );
    };
}

/// A specialized version of [`create_helper_call_numpy_unary_elementwise`] to generate functions that takes in float and returns float elementwise.
///
/// Arguments:
/// * `$name:ident`: The identifier of the rust function to be generated.
/// * `$fn_name:literal`: To be passed to the `fn_name` parameter of [`helper_call_numpy_unary_elementwise`].
/// * `$on_scalar:expr`: The closure (see below for its type) that acts on float scalar values and returns float results.
///
/// ```ignore
/// // Type of `$on_scalar:expr`
/// fn on_scalar<'ctx, G: CodeGenerator + ?Sized>(
///     generator: &mut G,
///     ctx: &mut CodeGenContext<'ctx, '_>,
///     arg: FloatValue<'ctx>
/// ) -> FloatValue<'ctx>
/// ```
macro_rules! create_helper_call_numpy_unary_elementwise_float_to_float {
    ($name:ident, $fn_name:literal, $elem_call:expr) => {
        create_helper_call_numpy_unary_elementwise!(
            $name,
            $fn_name,
            |ctx, _| ctx.primitives.float,
            |_generator, ctx, val_ty, val| {
                match val {
                    BasicValueEnum::FloatValue(n) => {
                        debug_assert!(ctx.unifier.unioned(val_ty, ctx.primitives.float));

                        Some($elem_call(ctx, n, Option::<&str>::None).into())
                    }
                    _ => None,
                }
            }
        );
    };
}

create_helper_call_numpy_unary_elementwise_float_to_bool!(
    call_numpy_isnan,
    "np_isnan",
    irrt::call_isnan
);
create_helper_call_numpy_unary_elementwise_float_to_bool!(
    call_numpy_isinf,
    "np_isinf",
    irrt::call_isinf
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_sin,
    "np_sin",
    llvm_intrinsics::call_float_sin
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_cos,
    "np_cos",
    llvm_intrinsics::call_float_cos
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_tan,
    "np_tan",
    extern_fns::call_tan
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arcsin,
    "np_arcsin",
    extern_fns::call_asin
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arccos,
    "np_arccos",
    extern_fns::call_acos
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arctan,
    "np_arctan",
    extern_fns::call_atan
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_sinh,
    "np_sinh",
    extern_fns::call_sinh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_cosh,
    "np_cosh",
    extern_fns::call_cosh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_tanh,
    "np_tanh",
    extern_fns::call_tanh
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arcsinh,
    "np_arcsinh",
    extern_fns::call_asinh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arccosh,
    "np_arccosh",
    extern_fns::call_acosh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arctanh,
    "np_arctanh",
    extern_fns::call_atanh
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_exp,
    "np_exp",
    llvm_intrinsics::call_float_exp
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_exp2,
    "np_exp2",
    llvm_intrinsics::call_float_exp2
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_expm1,
    "np_expm1",
    extern_fns::call_expm1
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_log,
    "np_log",
    llvm_intrinsics::call_float_log
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_log2,
    "np_log2",
    llvm_intrinsics::call_float_log2
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_log10,
    "np_log10",
    llvm_intrinsics::call_float_log10
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_sqrt,
    "np_sqrt",
    llvm_intrinsics::call_float_sqrt
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_cbrt,
    "np_cbrt",
    extern_fns::call_cbrt
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_fabs,
    "np_fabs",
    llvm_intrinsics::call_float_fabs
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_rint,
    "np_rint",
    llvm_intrinsics::call_float_rint
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_erf,
    "sp_spec_erf",
    extern_fns::call_erf
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_erfc,
    "sp_spec_erfc",
    extern_fns::call_erfc
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_gamma,
    "sp_spec_gamma",
    |ctx, val, _| irrt::call_gamma(ctx, val)
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_gammaln,
    "sp_spec_gammaln",
    |ctx, val, _| irrt::call_gammaln(ctx, val)
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_j0,
    "sp_spec_j0",
    |ctx, val, _| irrt::call_j0(ctx, val)
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_j1,
    "sp_spec_j1",
    extern_fns::call_j1
);

/// Invokes the `np_arctan2` builtin function.
pub fn call_numpy_arctan2<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arctan2";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.float));

            extern_fns::call_atan2(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_arctan2(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `np_copysign` builtin function.
pub fn call_numpy_copysign<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_copysign";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_copysign(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_copysign(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `np_fmax` builtin function.
pub fn call_numpy_fmax<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_fmax";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_maxnum(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_fmax(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `np_fmin` builtin function.
pub fn call_numpy_fmin<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_fmin";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_minnum(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_fmin(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `np_ldexp` builtin function.
pub fn call_numpy_ldexp<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_ldexp";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.int32));

            extern_fns::call_ldexp(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype =
                if is_ndarray1 { unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0 } else { x1_ty };

            let x1_scalar_ty = dtype;
            let x2_scalar_ty =
                if is_ndarray2 { unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0 } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_ldexp(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `np_hypot` builtin function.
pub fn call_numpy_hypot<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_hypot";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.float));

            extern_fns::call_hypot(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_hypot(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `np_nextafter` builtin function.
pub fn call_numpy_nextafter<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_nextafter";

    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    Ok(match (x1, x2) {
        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(x1_ty, ctx.primitives.float));
            debug_assert!(ctx.unifier.unioned(x2_ty, ctx.primitives.float));

            extern_fns::call_nextafter(ctx, x1, x2, None).into()
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let is_ndarray1 =
                x1_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                x2_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                unreachable!()
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1, !is_ndarray1),
                (x2, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    call_numpy_nextafter(generator, ctx, (x1_scalar_ty, lhs), (x2_scalar_ty, rhs))
                },
            )?
            .as_base_value()
            .into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}
