use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue, PointerValue},
    FloatPredicate, IntPredicate, OptimizationLevel,
};
use itertools::Itertools;

use super::{
    expr::destructure_range,
    extern_fns, irrt,
    irrt::calculate_len_for_slice_range,
    llvm_intrinsics,
    macros::codegen_unreachable,
    numpy,
    numpy::ndarray_elementwise_unaryop_impl,
    stmt::gen_for_callback_incrementing,
    types::ndarray::NDArrayType,
    values::{
        ndarray::NDArrayValue, ArrayLikeValue, ProxyValue, RangeValue, TypedArrayLikeAccessor,
        UntypedArrayLikeAccessor,
    },
    CodeGenContext, CodeGenerator,
};
use crate::{
    toplevel::{
        helper::{extract_ndims, PrimDef},
        numpy::unpack_ndarray_var_tys,
    },
    typecheck::typedef::{Type, TypeEnum},
};

/// Shorthand for [`unreachable!()`] when a type of argument is not supported.
///
/// The generated message will contain the function name and the name of the unsupported type.
fn unsupported_type(ctx: &CodeGenContext<'_, '_>, fn_name: &str, tys: &[Type]) -> ! {
    codegen_unreachable!(
        ctx,
        "{fn_name}() not supported for '{}'",
        tys.iter().map(|ty| format!("'{}'", ctx.unifier.stringify(*ty))).join(", "),
    )
}

/// Invokes the `len` builtin function.
pub fn call_len<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (arg_ty, arg): (Type, BasicValueEnum<'ctx>),
) -> Result<IntValue<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();
    let range_ty = ctx.primitives.range;

    Ok(if ctx.unifier.unioned(arg_ty, range_ty) {
        let arg = RangeValue::from_pointer_value(arg.into_pointer_value(), Some("range"));
        let (start, end, step) = destructure_range(ctx, arg);
        calculate_len_for_slice_range(generator, ctx, start, end, step)
    } else {
        match &*ctx.unifier.get_ty_immutable(arg_ty) {
            TypeEnum::TTuple { ty, .. } => llvm_i32.const_int(ty.len() as u64, false),
            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::List.id() => {
                let zero = llvm_i32.const_zero();
                let len = ctx
                    .build_gep_and_load(
                        arg.into_pointer_value(),
                        &[zero, llvm_i32.const_int(1, false)],
                        None,
                    )
                    .into_int_value();
                ctx.builder.build_int_truncate_or_bit_cast(len, llvm_i32, "len").unwrap()
            }
            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                let llvm_usize = generator.get_size_type(ctx.ctx);
                let arg = NDArrayType::from_unifier_type(generator, ctx, arg_ty)
                    .map_value(arg.into_pointer_value(), None);

                let ndims = arg.shape().size(ctx, generator);
                ctx.make_assert(
                    generator,
                    ctx.builder
                        .build_int_compare(IntPredicate::NE, ndims, llvm_usize.const_zero(), "")
                        .unwrap(),
                    "0:TypeError",
                    "len() of unsized object",
                    [None, None, None],
                    ctx.current_loc,
                );

                let len = unsafe {
                    arg.shape().get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
                };

                ctx.builder.build_int_truncate_or_bit_cast(len, llvm_i32, "len").unwrap()
            }
            _ => codegen_unreachable!(ctx),
        }
    })
}

/// Invokes the `int32` builtin function.
pub fn call_int32<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.int32,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i64 = ctx.ctx.i64_type();

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.int64,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i32 = ctx.ctx.i32_type();

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.uint32,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_i64 = ctx.ctx.i64_type();

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.uint64,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let llvm_f64 = ctx.ctx.f64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

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
            let (elem_ty, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndims = extract_ndims(&ctx.unifier, ndims);
            let llvm_elem_ty = ctx.get_llvm_type(generator, elem_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.float,
                None,
                NDArrayValue::from_pointer_value(n, llvm_elem_ty, Some(ndims), llvm_usize, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "round";

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_round";

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_rint(ctx, n, None).into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.float,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "bool";

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ctx.primitives.bool,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "floor";

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                llvm_ndarray_ty.map_value(n, None),
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
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "ceil";

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
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, n_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                llvm_ndarray_ty.map_value(n, None),
                |generator, ctx, val| call_ceil(generator, ctx, (elem_ty, val), ret_elem_ty),
            )?;

            ndarray.as_base_value().into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `min` builtin function.
pub fn call_min<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (m_ty, m): (Type, BasicValueEnum<'ctx>),
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "min";

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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_minimum";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (m_ty, m): (Type, BasicValueEnum<'ctx>),
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> BasicValueEnum<'ctx> {
    const FN_NAME: &str = "max";

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
    (a_ty, a): (Type, BasicValueEnum<'ctx>),
    fn_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    debug_assert!(["np_argmin", "np_argmax", "np_max", "np_min"].iter().any(|f| *f == fn_name));

    let llvm_int64 = ctx.ctx.i64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

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
                _ => codegen_unreachable!(ctx),
            }
        }
        BasicValueEnum::PointerValue(n)
            if a_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, a_ty);
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, a_ty);

            let n = llvm_ndarray_ty.map_value(n, None);
            let n_sz = n.size(generator, ctx);
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

            let accumulator_addr =
                generator.gen_var_alloc(ctx, llvm_ndarray_ty.element_type(), None)?;
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
                    let elem = unsafe {
                        n.data().get_unchecked(
                            ctx,
                            generator,
                            &ctx.builder
                                .build_int_truncate_or_bit_cast(idx, llvm_usize, "")
                                .unwrap(),
                            None,
                        )
                    };
                    let accumulator = ctx.builder.build_load(accumulator_addr, "").unwrap();
                    let cur_idx = ctx.builder.build_load(res_idx, "").unwrap();

                    let result = match fn_name {
                        "np_argmin" | "np_min" => {
                            call_min(ctx, (elem_ty, accumulator), (elem_ty, elem))
                        }
                        "np_argmax" | "np_max" => {
                            call_max(ctx, (elem_ty, accumulator), (elem_ty, elem))
                        }
                        _ => codegen_unreachable!(ctx),
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
                _ => codegen_unreachable!(ctx),
            }
        }

        _ => unsupported_type(ctx, fn_name, &[a_ty]),
    })
}

/// Invokes the `np_maximum` builtin function.
pub fn call_numpy_maximum<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_maximum";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
///   Return a constant [`Type`] here if the return type does not depend on the input type.
/// * `on_scalar`: The function that acts on the scalars of the input. Returns [`Option::None`]
///   if the scalar type & value are faulty and should panic with [`unsupported_type`].
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
            let (arg_elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, arg_ty);
            let llvm_ndarray_ty = NDArrayType::from_unifier_type(generator, ctx, arg_ty);
            let ret_elem_ty = get_ret_elem_type(ctx, arg_elem_ty);

            let ndarray = ndarray_elementwise_unaryop_impl(
                generator,
                ctx,
                ret_elem_ty,
                None,
                llvm_ndarray_ty.map_value(x, None),
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
///   But there is no need to make it a reference.
/// * `$on_scalar:expr`: To be passed to the `on_scalar` parameter of [`helper_call_numpy_unary_elementwise`].
///   But there is no need to make it a reference.
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
///   the boolean results of LLVM type `i1`. The returned `i1` value will be converted into an `i8`.
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_arctan2";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_copysign";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_fmax";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_fmin";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_ldexp";

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
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_hypot";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_nextafter";

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
                codegen_unreachable!(ctx)
            };

            let x1_scalar_ty = if is_ndarray1 { dtype } else { x1_ty };
            let x2_scalar_ty = if is_ndarray2 { dtype } else { x2_ty };

            numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                dtype,
                None,
                (x1_ty, x1, !is_ndarray1),
                (x2_ty, x2, !is_ndarray2),
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

/// Allocates a struct with the fields specified by `out_matrices` and returns a pointer to it
fn build_output_struct<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    out_matrices: &[BasicValueEnum<'ctx>],
) -> PointerValue<'ctx> {
    let field_ty = out_matrices.iter().map(BasicValueEnum::get_type).collect_vec();
    let out_ty = ctx.ctx.struct_type(&field_ty, false);
    let out_ptr = ctx.builder.build_alloca(out_ty, "").unwrap();

    for (i, v) in out_matrices.iter().enumerate() {
        unsafe {
            let ptr = ctx
                .builder
                .build_in_bounds_gep(
                    out_ptr,
                    &[
                        ctx.ctx.i32_type().const_zero(),
                        ctx.ctx.i32_type().const_int(i as u64, false),
                    ],
                    "",
                )
                .unwrap();
            ctx.builder.build_store(ptr, *v).unwrap();
        }
    }
    out_ptr
}

/// Invokes the `np_linalg_cholesky` linalg function
pub fn call_np_linalg_cholesky<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_cholesky";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2))
        .construct_uninitialized(generator, ctx, None);
    out.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { out.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let out_c = out.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_np_linalg_cholesky(
        ctx,
        x1_c.as_base_value().into(),
        out_c.as_base_value().into(),
        None,
    );
    Ok(out.as_base_value().into())
}

/// Invokes the `np_linalg_qr` linalg function
pub fn call_np_linalg_qr<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_qr";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape();
    let d0 =
        unsafe { x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
    let d1 = unsafe {
        x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None)
    };
    let dk = llvm_intrinsics::call_int_smin(ctx, d0, d1, None);

    let out_ndarray_ty = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2));
    let q = out_ndarray_ty.construct_dyn_shape(generator, ctx, &[d0, dk], None);
    unsafe { q.create_data(generator, ctx) };

    let r = out_ndarray_ty.construct_dyn_shape(generator, ctx, &[dk, d1], None);
    unsafe { r.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let q_c = q.make_contiguous_ndarray(generator, ctx);
    let r_c = r.make_contiguous_ndarray(generator, ctx);

    extern_fns::call_np_linalg_qr(
        ctx,
        x1_c.as_base_value().into(),
        q_c.as_base_value().into(),
        r_c.as_base_value().into(),
        None,
    );

    let q = q.as_base_value().into();
    let r = r.as_base_value().into();
    let out_ptr = build_output_struct(ctx, &[q, r]);
    Ok(ctx.builder.build_load(out_ptr, "QR_Factorization_result").map(Into::into).unwrap())
}

/// Invokes the `np_linalg_svd` linalg function
pub fn call_np_linalg_svd<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_svd";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape();
    let d0 =
        unsafe { x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
    let d1 = unsafe {
        x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None)
    };
    let dk = llvm_intrinsics::call_int_smin(ctx, d0, d1, None);

    let out_ndarray1_ty = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(1));
    let out_ndarray2_ty = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2));

    let u = out_ndarray2_ty.construct_dyn_shape(generator, ctx, &[d0, d0], None);
    unsafe { u.create_data(generator, ctx) };

    let s = out_ndarray1_ty.construct_dyn_shape(generator, ctx, &[dk], None);
    unsafe { s.create_data(generator, ctx) };

    let vh = out_ndarray2_ty.construct_dyn_shape(generator, ctx, &[d1, d1], None);
    unsafe { vh.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let u_c = u.make_contiguous_ndarray(generator, ctx);
    let s_c = s.make_contiguous_ndarray(generator, ctx);
    let vh_c = vh.make_contiguous_ndarray(generator, ctx);

    extern_fns::call_np_linalg_svd(
        ctx,
        x1_c.as_base_value().into(),
        u_c.as_base_value().into(),
        s_c.as_base_value().into(),
        vh_c.as_base_value().into(),
        None,
    );

    let u = u.as_base_value().into();
    let s = s.as_base_value().into();
    let vh = vh.as_base_value().into();
    let out_ptr = build_output_struct(ctx, &[u, s, vh]);

    Ok(ctx.builder.build_load(out_ptr, "SVD_Factorization_result").map(Into::into).unwrap())
}

/// Invokes the `np_linalg_inv` linalg function
pub fn call_np_linalg_inv<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_inv";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2))
        .construct_uninitialized(generator, ctx, None);
    out.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { out.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let out_c = out.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_np_linalg_inv(
        ctx,
        x1_c.as_base_value().into(),
        out_c.as_base_value().into(),
        None,
    );

    Ok(out.as_base_value().into())
}

/// Invokes the `np_linalg_pinv` linalg function
pub fn call_np_linalg_pinv<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_pinv";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape();
    let d0 =
        unsafe { x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
    let d1 = unsafe {
        x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None)
    };

    let out = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2))
        .construct_dyn_shape(generator, ctx, &[d0, d1], None);
    unsafe { out.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let out_c = out.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_np_linalg_pinv(
        ctx,
        x1_c.as_base_value().into(),
        out_c.as_base_value().into(),
        None,
    );

    Ok(out.as_base_value().into())
}

/// Invokes the `sp_linalg_lu` linalg function
pub fn call_sp_linalg_lu<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_linalg_lu";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape();
    let d0 =
        unsafe { x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
    let d1 = unsafe {
        x1_shape.get_typed_unchecked(ctx, generator, &llvm_usize.const_int(1, false), None)
    };
    let dk = llvm_intrinsics::call_int_smin(ctx, d0, d1, None);

    let out_ndarray_ty = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2));

    let l = out_ndarray_ty.construct_dyn_shape(generator, ctx, &[d0, dk], None);
    unsafe { l.create_data(generator, ctx) };

    let u = out_ndarray_ty.construct_dyn_shape(generator, ctx, &[dk, d1], None);
    unsafe { u.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let l_c = l.make_contiguous_ndarray(generator, ctx);
    let u_c = u.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_sp_linalg_lu(
        ctx,
        x1_c.as_base_value().into(),
        l_c.as_base_value().into(),
        u_c.as_base_value().into(),
        None,
    );

    let l = l.as_base_value().into();
    let u = u.as_base_value().into();
    let out_ptr = build_output_struct(ctx, &[l, u]);
    Ok(ctx.builder.build_load(out_ptr, "LU_Factorization_result").map(Into::into).unwrap())
}

/// Invokes the `np_linalg_matrix_power` linalg function
pub fn call_np_linalg_matrix_power<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_matrix_power";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let BasicValueEnum::PointerValue(x1) = x1 else {
        unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
    };

    let (elem_ty, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
    let ndims = extract_ndims(&ctx.unifier, ndims);
    let x1_elem_ty = ctx.get_llvm_type(generator, elem_ty);
    let x1 = NDArrayValue::from_pointer_value(x1, x1_elem_ty, Some(ndims), llvm_usize, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    // x2 is a float, but we are promoting this to a 1D ndarray (.shape == [1]) for uniformity in function call.
    let x2 = call_float(generator, ctx, (x2_ty, x2))?;
    let BasicValueEnum::FloatValue(x2) = x2 else {
        unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
    };

    let x2 = NDArrayType::new_unsized(generator, ctx.ctx, ctx.ctx.f64_type().into())
        .construct_unsized(generator, ctx, &x2, None); // x2.shape == []
    let x2 = x2.atleast_nd(generator, ctx, 1); // x2.shape == [1]

    let out = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2))
        .construct_uninitialized(generator, ctx, None);
    out.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { out.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let x2_c = x2.make_contiguous_ndarray(generator, ctx);
    let out_c = out.make_contiguous_ndarray(generator, ctx);

    extern_fns::call_np_linalg_matrix_power(
        ctx,
        x1_c.as_base_value().into(),
        x2_c.as_base_value().into(),
        out_c.as_base_value().into(),
        None,
    );

    Ok(out.as_base_value().into())
}

/// Invokes the `np_linalg_det` linalg function
pub fn call_np_linalg_det<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "np_linalg_matrix_power";

    let llvm_usize = generator.get_size_type(ctx.ctx);

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    // The output is a float64, but we are using an ndarray (shape == [1]) for uniformity in function call.
    let det = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(1))
        .construct_const_shape(generator, ctx, &[1], None);
    unsafe { det.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let out_c = det.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_np_linalg_det(
        ctx,
        x1_c.as_base_value().into(),
        out_c.as_base_value().into(),
        None,
    );

    // Get the determinant out of `out`
    let det = unsafe { det.data().get_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
    Ok(det)
}

/// Invokes the `sp_linalg_schur` linalg function
pub fn call_sp_linalg_schur<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_linalg_schur";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);
    assert_eq!(x1.get_type().ndims(), Some(2));

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out_ndarray_ty = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2));

    let t = out_ndarray_ty.construct_uninitialized(generator, ctx, None);
    t.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { t.create_data(generator, ctx) };

    let z = out_ndarray_ty.construct_uninitialized(generator, ctx, None);
    z.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { z.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let t_c = t.make_contiguous_ndarray(generator, ctx);
    let z_c = z.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_sp_linalg_schur(
        ctx,
        x1_c.as_base_value().into(),
        t_c.as_base_value().into(),
        z_c.as_base_value().into(),
        None,
    );

    let t = t.as_base_value().into();
    let z = z.as_base_value().into();
    let out_ptr = build_output_struct(ctx, &[t, z]);
    Ok(ctx.builder.build_load(out_ptr, "Schur_Factorization_result").map(Into::into).unwrap())
}

/// Invokes the `sp_linalg_hessenberg` linalg function
pub fn call_sp_linalg_hessenberg<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "sp_linalg_hessenberg";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(x1, None);
    assert_eq!(x1.get_type().ndims(), Some(2));

    if !x1.get_type().element_type().is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out_ndarray_ty = NDArrayType::new(generator, ctx.ctx, ctx.ctx.f64_type().into(), Some(2));

    let h = out_ndarray_ty.construct_uninitialized(generator, ctx, None);
    h.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { h.create_data(generator, ctx) };

    let q = out_ndarray_ty.construct_uninitialized(generator, ctx, None);
    q.copy_shape_from_ndarray(generator, ctx, x1);
    unsafe { q.create_data(generator, ctx) };

    let x1_c = x1.make_contiguous_ndarray(generator, ctx);
    let h_c = h.make_contiguous_ndarray(generator, ctx);
    let q_c = q.make_contiguous_ndarray(generator, ctx);
    extern_fns::call_sp_linalg_hessenberg(
        ctx,
        x1_c.as_base_value().into(),
        h_c.as_base_value().into(),
        q_c.as_base_value().into(),
        None,
    );

    let h = h.as_base_value().into();
    let q = q.as_base_value().into();
    let out_ptr = build_output_struct(ctx, &[h, q]);
    Ok(ctx.builder.build_load(out_ptr, "Hessenberg_decomposition_result").map(Into::into).unwrap())
}
