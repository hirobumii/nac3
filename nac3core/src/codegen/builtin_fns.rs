use inkwell::{FloatPredicate, IntPredicate, OptimizationLevel};
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;
use itertools::Itertools;

use crate::codegen::{CodeGenContext, CodeGenerator, extern_fns, irrt, llvm_intrinsics, numpy};
use crate::codegen::classes::{NDArrayValue, UntypedArrayLikeAccessor};
use crate::codegen::numpy::ndarray_elementwise_unaryop_impl;
use crate::codegen::stmt::gen_for_callback_incrementing;
use crate::toplevel::helper::PRIMITIVE_DEF_IDS;
use crate::toplevel::numpy::unpack_ndarray_var_tys;
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

            ctx.builder
                .build_int_z_extend(n, llvm_i32, "zext")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 32 => {
            debug_assert!([
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!([
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            ctx.builder
                .build_int_truncate(n, llvm_i32, "trunc")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let to_int64 = ctx.builder
                .build_float_to_signed_int(n, ctx.ctx.i64_type(), "")
                .unwrap();
            ctx.builder
                .build_int_truncate(to_int64, llvm_i32, "conv")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.int32,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_int32(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, "int32", &[n_ty])
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
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder
                    .build_int_s_extend(n, llvm_i64, "sext")
                    .map(Into::into)
                    .unwrap() 
            } else {
                ctx.builder
                    .build_int_z_extend(n, llvm_i64, "zext")
                    .map(Into::into)
                    .unwrap()
            }
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!([
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            ctx.builder
                .build_float_to_signed_int(n, ctx.ctx.i64_type(), "fptosi")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.int64,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_int64(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, "int64", &[n_ty])
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

            ctx.builder
                .build_int_z_extend(n, llvm_i32, "zext")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 32 => {
            debug_assert!([
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!(
                ctx.unifier.unioned(n_ty, ctx.primitives.int64)
                    || ctx.unifier.unioned(n_ty, ctx.primitives.uint64)
            );

            ctx.builder
                .build_int_truncate(n, llvm_i32, "trunc")
                .map(Into::into)
                .unwrap()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let n_gez = ctx.builder
                .build_float_compare(FloatPredicate::OGE, n, n.get_type().const_zero(), "")
                .unwrap();

            let to_int32 = ctx.builder
                .build_float_to_signed_int(n, llvm_i32, "")
                .unwrap();
            let to_uint64 = ctx.builder
                .build_float_to_unsigned_int(n, ctx.ctx.i64_type(), "")
                .unwrap();

            ctx.builder
                .build_select(
                    n_gez,
                    ctx.builder.build_int_truncate(to_uint64, llvm_i32, "").unwrap(),
                    to_int32,
                    "conv",
                )
                .unwrap()
        }

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.uint32,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_uint32(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, "uint32", &[n_ty])
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
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder
                    .build_int_s_extend(n, llvm_i64, "sext")
                    .map(Into::into)
                    .unwrap()
            } else {
                ctx.builder
                    .build_int_z_extend(n, llvm_i64, "zext")
                    .map(Into::into)
                    .unwrap()
            }
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!([
                ctx.primitives.int64,
                ctx.primitives.uint64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            n.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val_gez = ctx.builder
                .build_float_compare(FloatPredicate::OGE, n, n.get_type().const_zero(), "")
                .unwrap();

            let to_int64 = ctx.builder
                .build_float_to_signed_int(n, llvm_i64, "")
                .unwrap();
            let to_uint64 = ctx.builder
                .build_float_to_unsigned_int(n, llvm_i64, "")
                .unwrap();

            ctx.builder
                .build_select(val_gez, to_uint64, to_int64, "conv")
                .unwrap()
        }

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.uint64,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_uint64(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, "uint64", &[n_ty])
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
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

            if [
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)) {
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

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.float,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_float(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, "float", &[n_ty])
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

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_round(generator, ctx, (elem_ty, val), ret_elem_ty)
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
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

            llvm_intrinsics::call_float_roundeven(ctx, n, None).into()
        }

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.float,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_round(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
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
            ].iter().any(|ty| ctx.unifier.unioned(n_ty, *ty)));

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

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.bool,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    let elem = call_bool(
                        generator,
                        ctx,
                        (elem_ty, val),
                    )?;

                    Ok(generator.bool_to_i8(ctx, elem.into_int_value()).into())
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
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

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_floor(generator, ctx, (elem_ty, val), ret_elem_ty)
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
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

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_floor(generator, ctx, (elem_ty, val), ret_elem_ty)
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
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
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)));

            if [
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)) {
                llvm_intrinsics::call_int_smin(ctx, m, n, Some(FN_NAME)).into()
            } else {
                llvm_intrinsics::call_int_umin(ctx, m, n, Some(FN_NAME)).into()
            }
        }

        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_minnum(ctx, m, n, Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    }
}

/// Invokes the `np_min` builtin function.
pub fn call_numpy_min<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    a: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_min";

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
            ].iter().any(|ty| ctx.unifier.unioned(a_ty, *ty)));

            a
        }

        BasicValueEnum::PointerValue(n) if a_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, a_ty);
            let llvm_ndarray_ty = ctx.get_llvm_type(generator, elem_ty);

            let n = NDArrayValue::from_ptr_val(n, llvm_usize, None);
            let n_sz = irrt::call_ndarray_calc_size(generator, ctx, &n.dim_sizes());
            if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
                let n_sz_eqz = ctx.builder
                    .build_int_compare(
                        IntPredicate::NE,
                        n_sz,
                        n_sz.get_type().const_zero(),
                        "",
                    )
                    .unwrap();

                ctx.make_assert(
                    generator,
                    n_sz_eqz,
                    "0:ValueError",
                    "zero-size array to reduction operation minimum which has no identity",
                    [None, None, None],
                    ctx.current_loc,
                );
            }

            let accumulator_addr = generator.gen_var_alloc(ctx, llvm_ndarray_ty, None)?;
            unsafe {
                let identity = n.data()
                    .get_unchecked(ctx, generator, &llvm_usize.const_zero(), None);
                ctx.builder.build_store(accumulator_addr, identity).unwrap();
            }

            gen_for_callback_incrementing(
                generator,
                ctx,
                llvm_usize.const_int(1, false),
                (n_sz, false),
                |generator, ctx, idx| {
                    let elem = unsafe {
                        n.data().get_unchecked(ctx, generator, &idx, None) 
                    };

                    let accumulator = ctx.builder.build_load(accumulator_addr, "").unwrap();
                    let result = call_min(ctx, (elem_ty, accumulator), (elem_ty, elem));
                    ctx.builder.build_store(accumulator_addr, result).unwrap();

                    Ok(())
                },
                llvm_usize.const_int(1, false),
            )?;

            let accumulator = ctx.builder.build_load(accumulator_addr, "").unwrap();
            accumulator
        }

        _ => unsupported_type(ctx, FN_NAME, &[a_ty])
    })
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

    let common_ty = if ctx.unifier.unioned(x1_ty, x2_ty) {
        Some(x1_ty)
    } else {
        None
    };

    Ok(match (x1, x2) {
        (BasicValueEnum::IntValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
                ctx.primitives.float,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty.unwrap(), *ty)));

            call_min(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(common_ty.unwrap(), ctx.primitives.float));

            call_min(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)));

            if [
                ctx.primitives.int32,
                ctx.primitives.int64,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty, *ty)) {
                llvm_intrinsics::call_int_smax(ctx, m, n, Some(FN_NAME)).into()
            } else {
                llvm_intrinsics::call_int_umax(ctx, m, n, Some(FN_NAME)).into()
            }
        }

        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_maxnum(ctx, m, n, Some(FN_NAME)).into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    }
}

/// Invokes the `np_max` builtin function.
pub fn call_numpy_max<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    a: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_max";

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
            ].iter().any(|ty| ctx.unifier.unioned(a_ty, *ty)));

            a
        }

        BasicValueEnum::PointerValue(n) if a_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, a_ty);
            let llvm_ndarray_ty = ctx.get_llvm_type(generator, elem_ty);

            let n = NDArrayValue::from_ptr_val(n, llvm_usize, None);
            let n_sz = irrt::call_ndarray_calc_size(generator, ctx, &n.dim_sizes());
            if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
                let n_sz_eqz = ctx.builder
                    .build_int_compare(
                        IntPredicate::NE,
                        n_sz,
                        n_sz.get_type().const_zero(),
                        "",
                    )
                    .unwrap();

                ctx.make_assert(
                    generator,
                    n_sz_eqz,
                    "0:ValueError",
                    "zero-size array to reduction operation minimum which has no identity",
                    [None, None, None],
                    ctx.current_loc,
                );
            }

            let accumulator_addr = generator.gen_var_alloc(ctx, llvm_ndarray_ty, None)?;
            unsafe {
                let identity = n.data()
                    .get_unchecked(ctx, generator, &llvm_usize.const_zero(), None);
                ctx.builder.build_store(accumulator_addr, identity).unwrap();
            }

            gen_for_callback_incrementing(
                generator,
                ctx,
                llvm_usize.const_int(1, false),
                (n_sz, false),
                |generator, ctx, idx| {
                    let elem = unsafe {
                        n.data().get_unchecked(ctx, generator, &idx, None)
                    };

                    let accumulator = ctx.builder.build_load(accumulator_addr, "").unwrap();
                    let result = call_max(ctx, (elem_ty, accumulator), (elem_ty, elem));
                    ctx.builder.build_store(accumulator_addr, result).unwrap();

                    Ok(())
                },
                llvm_usize.const_int(1, false),
            )?;

            let accumulator = ctx.builder.build_load(accumulator_addr, "").unwrap();
            accumulator
        }

        _ => unsupported_type(ctx, FN_NAME, &[a_ty])
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

    let common_ty = if ctx.unifier.unioned(x1_ty, x2_ty) {
        Some(x1_ty)
    } else {
        None
    };

    Ok(match (x1, x2) {
        (BasicValueEnum::IntValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!([
                ctx.primitives.bool,
                ctx.primitives.int32,
                ctx.primitives.uint32,
                ctx.primitives.int64,
                ctx.primitives.uint64,
                ctx.primitives.float,
            ].iter().any(|ty| ctx.unifier.unioned(common_ty.unwrap(), *ty)));

            call_max(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(common_ty.unwrap(), ctx.primitives.float));

            call_max(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))
        }

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
    })
}

/// Invokes the `abs` builtin function.
pub fn call_abs<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "abs";

    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (n_ty, n) = n;

    Ok(match n {
        BasicValueEnum::IntValue(n) => {
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
                    n,
                    llvm_i1.const_zero(),
                    Some(FN_NAME),
                ).into()
            } else {
                n.into()
            }
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_fabs(ctx, n, Some(FN_NAME)).into()
        }

        BasicValueEnum::PointerValue(n) if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(n, llvm_usize, None),
                |generator, ctx, val| {
                    call_abs(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty])
    })
}

/// Invokes the `np_isnan` builtin function.
pub fn call_numpy_isnan<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_isnan";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            irrt::call_isnan(generator, ctx, x).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.bool,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    let val = call_numpy_isnan(generator, ctx, (elem_ty, val))?;

                    Ok(generator.bool_to_i8(ctx, val.into_int_value()).into())
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_isinf` builtin function.
pub fn call_numpy_isinf<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_isinf";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            irrt::call_isinf(generator, ctx, x).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.bool,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    let val = call_numpy_isinf(generator, ctx, (elem_ty, val))?;

                    Ok(generator.bool_to_i8(ctx, val.into_int_value()).into())
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_sin` builtin function.
pub fn call_numpy_sin<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_sin";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_sin(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_sin(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_cos` builtin function.
pub fn call_numpy_cos<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_cos";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_cos(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_cos(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_exp` builtin function.
pub fn call_numpy_exp<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_exp";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_exp(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_exp(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_exp2` builtin function.
pub fn call_numpy_exp2<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_exp2";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_exp2(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_exp2(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_log` builtin function.
pub fn call_numpy_log<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_log";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_log(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_log(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_log10` builtin function.
pub fn call_numpy_log10<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_log10";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_log10(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_log10(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_log2` builtin function.
pub fn call_numpy_log2<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_log2";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_log2(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_log2(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_fabs` builtin function.
pub fn call_numpy_fabs<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_fabs";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_fabs(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_fabs(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_sqrt` builtin function.
pub fn call_numpy_sqrt<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_sqrt";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_sqrt(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_sqrt(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_rint` builtin function.
pub fn call_numpy_rint<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_rint";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_roundeven(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_rint(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_tan` builtin function.
pub fn call_numpy_tan<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_tan";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_tan(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_tan(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_arcsin` builtin function.
pub fn call_numpy_arcsin<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arcsin";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_asin(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_arcsin(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_arccos` builtin function.
pub fn call_numpy_arccos<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arccos";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));
            
            extern_fns::call_acos(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_arccos(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_arctan` builtin function.
pub fn call_numpy_arctan<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arctan";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_atan(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_arctan(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_sinh` builtin function.
pub fn call_numpy_sinh<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_sinh";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_sinh(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_sinh(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_cosh` builtin function.
pub fn call_numpy_cosh<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_cosh";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_cosh(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_cosh(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_tanh` builtin function.
pub fn call_numpy_tanh<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_tanh";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_tanh(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_tanh(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_arcsinh` builtin function.
pub fn call_numpy_arcsinh<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arcsinh";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_asinh(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_arcsinh(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_arccosh` builtin function.
pub fn call_numpy_arccosh<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arccosh";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_acosh(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_arccosh(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_arctanh` builtin function.
pub fn call_numpy_arctanh<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arctanh";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_atanh(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_arctanh(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_expm1` builtin function.
pub fn call_numpy_expm1<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_expm1";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_expm1(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_expm1(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `np_cbrt` builtin function.
pub fn call_numpy_cbrt<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_cbrt";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_cbrt(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_numpy_cbrt(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `sp_spec_erf` builtin function.
pub fn call_scipy_special_erf<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    z: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_spec_erf";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (z_ty, z) = z;

    Ok(match z {
        BasicValueEnum::FloatValue(z) => {
            debug_assert!(ctx.unifier.unioned(z_ty, ctx.primitives.float));

            extern_fns::call_erf(ctx, z, None).into()
        }

        BasicValueEnum::PointerValue(z) if z_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, z_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(z, llvm_usize, None),
                |generator, ctx, val| {
                    call_scipy_special_erf(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[z_ty])
    })
}

/// Invokes the `sp_spec_erfc` builtin function.
pub fn call_scipy_special_erfc<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_spec_erfc";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_erfc(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_scipy_special_erfc(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `sp_spec_gamma` builtin function.
pub fn call_scipy_special_gamma<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    z: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_spec_gamma";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (z_ty, z) = z;

    Ok(match z {
        BasicValueEnum::FloatValue(z) => {
            debug_assert!(ctx.unifier.unioned(z_ty, ctx.primitives.float));

            irrt::call_gamma(ctx, z).into()
        }

        BasicValueEnum::PointerValue(z) if z_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, z_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(z, llvm_usize, None),
                |generator, ctx, val| {
                    call_scipy_special_gamma(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[z_ty])
    })
}

/// Invokes the `sp_spec_gammaln` builtin function.
pub fn call_scipy_special_gammaln<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_spec_gammaln";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            irrt::call_gammaln(ctx, x).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_scipy_special_gammaln(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `sp_spec_j0` builtin function.
pub fn call_scipy_special_j0<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_spec_j0";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            irrt::call_j0(ctx, x).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_scipy_special_j0(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

/// Invokes the `sp_spec_j1` builtin function.
pub fn call_scipy_special_j1<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_spec_j1";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let (x_ty, x) = x;

    Ok(match x {
        BasicValueEnum::FloatValue(x) => {
            debug_assert!(ctx.unifier.unioned(x_ty, ctx.primitives.float));

            extern_fns::call_j1(ctx, x, None).into()
        }

        BasicValueEnum::PointerValue(x) if x_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray) => {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                elem_ty,
                None,
                NDArrayValue::from_ptr_val(x, llvm_usize, None),
                |generator, ctx, val| {
                    call_scipy_special_j1(generator, ctx, (elem_ty, val))
                },
            )?;

            ndarray.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x_ty])
    })
}

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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty); 

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0 
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else {
                x1_ty
            };

            let x1_scalar_ty = dtype;
            let x2_scalar_ty = if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
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

        (x1, x2) if [&x1_ty, &x2_ty].into_iter().any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray)) => {
            let is_ndarray1 = x1_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);
            let is_ndarray2 = x2_ty.obj_id(&ctx.unifier)
                .is_some_and(|id| id == PRIMITIVE_DEF_IDS.ndarray);

            let dtype = if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty);

                debug_assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                ndarray_dtype1
            } else if is_ndarray1 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty).0
            } else if is_ndarray2 {
                unpack_ndarray_var_tys(&mut ctx.unifier, x2_ty).0
            } else { unreachable!() };

            let x1_scalar_ty = if is_ndarray1 {
                dtype
            } else {
                x1_ty
            };
            let x2_scalar_ty = if is_ndarray2 {
                dtype
            } else {
                x2_ty
            };

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
            )?.as_ptr_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
    })
}