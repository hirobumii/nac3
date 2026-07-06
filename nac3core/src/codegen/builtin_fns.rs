use inkwell::{
    FloatPredicate, IntPredicate,
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue},
};
use itertools::Itertools as _;

use crate::{
    codegen::{
        CodeGenContext, bool_to_i8,
        expr::destructure_range,
        extern_fns,
        irrt::{self, calculate_len_for_slice_range},
        llvm_intrinsics,
        macros::codegen_unreachable,
        types::{
            ArrayLikeIndexer, NDArrayOut, NDArrayType, ProxyTypeBase, RangeType, RawListType,
            RawNDArrayValue, ScalarOrNDArray, TupleType, TupleValue, TypedRefCountedType,
            broadcast_starmap, field,
        },
    },
    toplevel::{
        helper::{PrimDef, arraylike_flatten_element_type, extract_ndims},
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
pub fn call_len<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (arg_ty, arg): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<IntValue<'ctx>> {
    let range_ty = ctx.primitives.range;

    Ok(if ctx.unifier.unioned(arg_ty, range_ty) {
        let arg = RangeType::new(ctx).map_value(arg.into_pointer_value(), Some("range"));
        let (start, end, step) = destructure_range(ctx, arg)?;
        calculate_len_for_slice_range(ctx, start, end, step)?
    } else {
        match &*ctx.unifier.get_ty_immutable(arg_ty) {
            TypeEnum::TTuple { .. } => {
                let tuple = TupleType::from_unifier_type(ctx, arg_ty)
                    .map_value(arg.into_struct_value(), None);
                ctx.i32.const_int(tuple.ty.num_elements().into(), false)
            }

            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let ndarray = NDArrayType::from_unifier_type(ctx, arg_ty)
                    .map_value(arg.into_pointer_value(), None);
                let len = ndarray.len(ctx)?;
                ctx.builder.build_int_truncate_or_bit_cast(len, ctx.i32, "len")?
            }

            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                let list_ty = RawListType::from_unifier_type(ctx, arg_ty);
                let list = TypedRefCountedType::new(ctx, list_ty)
                    .map_value(arg.into_pointer_value(), None);
                let size = list.inner_value(ctx)?.load(ctx, field!(len))?;
                ctx.builder.build_int_truncate_or_bit_cast(size, ctx.i32, "len")?
            }

            _ => unsupported_type(ctx, "len", &[arg_ty]),
        }
    })
}

/// Invokes the `int32` builtin function.
pub fn call_int32<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            ctx.builder.build_int_z_extend(n, ctx.i32, "zext")?.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 32 => {
            debug_assert!(
                [ctx.primitives.int32, ctx.primitives.uint32,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            n.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!(
                [ctx.primitives.int64, ctx.primitives.uint64,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            ctx.builder.build_int_truncate(n, ctx.i32, "trunc")?.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let to_int64 = ctx.builder.build_float_to_signed_int(n, ctx.i64, "")?;
            ctx.builder.build_int_truncate(to_int64, ctx.i32, "conv")?.into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.i32.into() },
                |ctx, scalar| call_int32(ctx, (elem_ty, scalar)),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, "int32", &[n_ty]),
    })
}

/// Invokes the `int64` builtin function.
pub fn call_int64<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8 | 32) => {
            debug_assert!(
                [ctx.primitives.bool, ctx.primitives.int32, ctx.primitives.uint32,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder.build_int_s_extend(n, ctx.i64, "sext")?.into()
            } else {
                ctx.builder.build_int_z_extend(n, ctx.i64, "zext")?.into()
            }
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!(
                [ctx.primitives.int64, ctx.primitives.uint64,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            n.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            ctx.builder.build_float_to_signed_int(n, ctx.i64, "fptosi")?.into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.i64.into() },
                |ctx, scalar| call_int64(ctx, (elem_ty, scalar)),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, "int64", &[n_ty]),
    })
}

/// Invokes the `uint32` builtin function.
pub fn call_uint32<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            ctx.builder.build_int_z_extend(n, ctx.i32, "zext")?.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 32 => {
            debug_assert!(
                [ctx.primitives.int32, ctx.primitives.uint32,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            n.into()
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!(
                ctx.unifier.unioned(n_ty, ctx.primitives.int64)
                    || ctx.unifier.unioned(n_ty, ctx.primitives.uint64)
            );

            ctx.builder.build_int_truncate(n, ctx.i32, "trunc")?.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let n_gez = ctx.builder.build_float_compare(
                FloatPredicate::OGE,
                n,
                n.get_type().const_zero(),
                "",
            )?;

            let to_int32 = ctx.builder.build_float_to_signed_int(n, ctx.i32, "")?;
            let to_uint64 = ctx.builder.build_float_to_unsigned_int(n, ctx.i64, "")?;

            ctx.builder.build_select(
                n_gez,
                ctx.builder.build_int_truncate(to_uint64, ctx.i32, "")?,
                to_int32,
                "conv",
            )?
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.i32.into() },
                |ctx, scalar| call_uint32(ctx, (elem_ty, scalar)),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, "uint32", &[n_ty]),
    })
}

/// Invokes the `uint64` builtin function.
pub fn call_uint64<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8 | 32) => {
            debug_assert!(
                [ctx.primitives.bool, ctx.primitives.int32, ctx.primitives.uint32,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            if ctx.unifier.unioned(n_ty, ctx.primitives.int32) {
                ctx.builder.build_int_s_extend(n, ctx.i64, "sext")?.into()
            } else {
                ctx.builder.build_int_z_extend(n, ctx.i64, "zext")?.into()
            }
        }

        BasicValueEnum::IntValue(n) if n.get_type().get_bit_width() == 64 => {
            debug_assert!(
                [ctx.primitives.int64, ctx.primitives.uint64,]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            n.into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val_gez = ctx.builder.build_float_compare(
                FloatPredicate::OGE,
                n,
                n.get_type().const_zero(),
                "",
            )?;

            let to_int64 = ctx.builder.build_float_to_signed_int(n, ctx.i64, "")?;
            let to_uint64 = ctx.builder.build_float_to_unsigned_int(n, ctx.i64, "")?;

            ctx.builder.build_select(val_gez, to_uint64, to_int64, "conv")?
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.i64.into() },
                |ctx, scalar| call_uint64(ctx, (elem_ty, scalar)),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, "uint64", &[n_ty]),
    })
}

/// Invokes the `float` builtin function.
pub fn call_float<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8 | 32 | 64) => {
            debug_assert!(
                [
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            if [ctx.primitives.bool, ctx.primitives.int32, ctx.primitives.int64]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            {
                ctx.builder.build_signed_int_to_float(n, ctx.f64, "sitofp")?.into()
            } else {
                ctx.builder.build_unsigned_int_to_float(n, ctx.f64, "uitofp")?.into()
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
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.f64.into() },
                |ctx, scalar| call_float(ctx, (elem_ty, scalar)),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, "float", &[n_ty]),
    })
}

/// Invokes the `round` builtin function.
pub fn call_round<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "round";

    let llvm_ret_elem_ty = ctx.get_llvm_abi_type(ret_elem_ty).into_int_type();

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = llvm_intrinsics::call_float_round(ctx, n, None)?;
            ctx.builder.build_float_to_signed_int(val, llvm_ret_elem_ty, FN_NAME)?.into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: llvm_ret_elem_ty.into() },
                |ctx, scalar| call_round(ctx, (elem_ty, scalar), ret_elem_ty),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `np_round` builtin function.
pub fn call_numpy_round<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_round";

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_rint(ctx, n, None)?.into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.f64.into() },
                |ctx, scalar| call_numpy_round(ctx, (elem_ty, scalar)),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `bool` builtin function.
pub fn call_bool<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "bool";

    Ok(match n {
        BasicValueEnum::IntValue(n) if matches!(n.get_type().get_bit_width(), 1 | 8) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.bool));

            n.into()
        }

        BasicValueEnum::IntValue(n) => {
            debug_assert!(
                [
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(n_ty, *ty))
            );

            ctx.builder
                .build_int_compare(IntPredicate::NE, n, n.get_type().const_zero(), FN_NAME)?
                .into()
        }

        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            ctx.builder
                .build_float_compare(FloatPredicate::UNE, n, n.get_type().const_zero(), FN_NAME)?
                .into()
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: ctx.i8.into() },
                |ctx, scalar| {
                    let elem = call_bool(ctx, (elem_ty, scalar))?;
                    Ok(bool_to_i8(ctx, elem.into_int_value())?.into())
                },
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `floor` builtin function.
pub fn call_floor<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "floor";

    let llvm_ret_elem_ty = ctx.get_llvm_abi_type(ret_elem_ty);

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = llvm_intrinsics::call_float_floor(ctx, n, None)?;
            if let BasicTypeEnum::IntType(llvm_ret_elem_ty) = llvm_ret_elem_ty {
                ctx.builder.build_float_to_signed_int(val, llvm_ret_elem_ty, FN_NAME)?.into()
            } else {
                val.into()
            }
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: llvm_ret_elem_ty },
                |ctx, scalar| call_floor(ctx, (elem_ty, scalar), ret_elem_ty),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `ceil` builtin function.
pub fn call_ceil<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
    ret_elem_ty: Type,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "ceil";

    let llvm_ret_elem_ty = ctx.get_llvm_abi_type(ret_elem_ty);

    Ok(match n {
        BasicValueEnum::FloatValue(n) => {
            debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

            let val = llvm_intrinsics::call_float_ceil(ctx, n, None)?;
            if let BasicTypeEnum::IntType(llvm_ret_elem_ty) = llvm_ret_elem_ty {
                ctx.builder.build_float_to_signed_int(val, llvm_ret_elem_ty, FN_NAME)?.into()
            } else {
                val.into()
            }
        }

        BasicValueEnum::PointerValue(n)
            if n_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, n_ty);
            let ndarray = NDArrayType::from_unifier_type(ctx, n_ty).map_value(n, None);

            let result = ndarray.map(
                ctx,
                NDArrayOut::NewNDArray { dtype: llvm_ret_elem_ty },
                |ctx, scalar| call_ceil(ctx, (elem_ty, scalar), ret_elem_ty),
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[n_ty]),
    })
}

/// Invokes the `min` builtin function.
pub fn call_min<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (m_ty, m): (Type, BasicValueEnum<'ctx>),
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "min";

    let common_ty = if ctx.unifier.unioned(m_ty, n_ty) {
        m_ty
    } else {
        unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    };

    Ok(match (m, n) {
        (BasicValueEnum::IntValue(m), BasicValueEnum::IntValue(n)) => {
            debug_assert!(
                [
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty, *ty))
            );

            if [ctx.primitives.int32, ctx.primitives.int64]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty, *ty))
            {
                llvm_intrinsics::call_int_smin(ctx, m, n, Some(FN_NAME))?.into()
            } else {
                llvm_intrinsics::call_int_umin(ctx, m, n, Some(FN_NAME))?.into()
            }
        }

        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_minnum(ctx, m, n, Some(FN_NAME))?.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty]),
    })
}

/// Invokes the `np_minimum` builtin function.
pub fn call_numpy_minimum<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_minimum";

    let common_ty = if ctx.unifier.unioned(x1_ty, x2_ty) { Some(x1_ty) } else { None };

    Ok(match (x1, x2) {
        (BasicValueEnum::IntValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!(
                [
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                    ctx.primitives.float,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty.unwrap(), *ty))
            );

            call_min(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))?
        }

        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(common_ty.unwrap(), ctx.primitives.float));

            call_min(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))?
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1)).to_ndarray(ctx)?;
            let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2)).to_ndarray(ctx)?;

            let x1_dtype = arraylike_flatten_element_type(&mut ctx.unifier, x1_ty);
            let x2_dtype = arraylike_flatten_element_type(&mut ctx.unifier, x2_ty);

            debug_assert!(ctx.unifier.unioned(x1_dtype, x2_dtype));
            let llvm_common_dtype = x1.ty.object.dtype;

            let result = broadcast_starmap(
                ctx,
                &[x1, x2],
                NDArrayOut::NewNDArray { dtype: llvm_common_dtype },
                |ctx, scalars| {
                    let x1_scalar = scalars[0];
                    let x2_scalar = scalars[1];
                    call_min(ctx, (x1_dtype, x1_scalar), (x2_dtype, x2_scalar))
                },
            )?;

            result.value.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
    })
}

/// Invokes the `max` builtin function.
pub fn call_max<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (m_ty, m): (Type, BasicValueEnum<'ctx>),
    (n_ty, n): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "max";

    let common_ty = if ctx.unifier.unioned(m_ty, n_ty) {
        m_ty
    } else {
        unsupported_type(ctx, FN_NAME, &[m_ty, n_ty])
    };

    Ok(match (m, n) {
        (BasicValueEnum::IntValue(m), BasicValueEnum::IntValue(n)) => {
            debug_assert!(
                [
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty, *ty))
            );

            if [ctx.primitives.int32, ctx.primitives.int64]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty, *ty))
            {
                llvm_intrinsics::call_int_smax(ctx, m, n, Some(FN_NAME))?.into()
            } else {
                llvm_intrinsics::call_int_umax(ctx, m, n, Some(FN_NAME))?.into()
            }
        }

        (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
            debug_assert!(ctx.unifier.unioned(common_ty, ctx.primitives.float));

            llvm_intrinsics::call_float_maxnum(ctx, m, n, Some(FN_NAME))?.into()
        }

        _ => unsupported_type(ctx, FN_NAME, &[m_ty, n_ty]),
    })
}

/// Invokes the `np_max`, `np_min`, `np_argmax`, `np_argmin` functions
/// * `fn_name`: Can be one of `"np_argmin"`, `"np_argmax"`, `"np_max"`, `"np_min"`
pub fn call_numpy_max_min<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (a_ty, a): (Type, BasicValueEnum<'ctx>),
    fn_name: &str,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    debug_assert!(["np_argmin", "np_argmax", "np_max", "np_min"].contains(&fn_name));

    Ok(match a {
        BasicValueEnum::IntValue(_) | BasicValueEnum::FloatValue(_) => {
            debug_assert!(
                [
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                    ctx.primitives.float,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(a_ty, *ty))
            );

            match fn_name {
                "np_argmin" | "np_argmax" => ctx.i64.const_zero().into(),
                "np_max" | "np_min" => a,
                _ => codegen_unreachable!(ctx),
            }
        }

        BasicValueEnum::PointerValue(n)
            if a_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) =>
        {
            let (elem_ty, _) = unpack_ndarray_var_tys(&mut ctx.unifier, a_ty);

            let ndarray = NDArrayType::from_unifier_type(ctx, a_ty).map_value(n, None);
            let llvm_dtype = ndarray.ty.object.dtype;

            let zero = ctx.size_t.const_zero();

            if ctx.registry.codegen_options.debug {
                let size = ndarray.size(ctx)?;
                let size_nez = ctx.builder.build_int_compare(IntPredicate::NE, size, zero, "")?;

                ctx.make_assert(
                    size_nez,
                    "0:ValueError",
                    format!("zero-size array to reduction operation {fn_name}").as_str(),
                    [None, None, None],
                )?;
            }

            let extremum = ctx.alloc(llvm_dtype, None)?;
            let extremum_idx = ctx.alloc(ctx.size_t, None)?;

            let first_value = ndarray.first_element(ctx)?;
            ctx.builder.build_store(extremum, first_value)?;
            ctx.builder.build_store(extremum_idx, zero)?;

            // The first element is iterated, but this doesn't matter.
            ndarray.foreach(ctx, |ctx, _, nditer| {
                let old_extremum = ctx.builder.build_load(llvm_dtype, extremum, "")?;
                let old_extremum_idx =
                    ctx.builder.build_load(ctx.size_t, extremum_idx, "")?.into_int_value();

                let curr_value = nditer.inner_value(ctx)?.get_scalar(ctx)?;
                let curr_idx = nditer.inner_value(ctx)?.get_index(ctx)?;

                let new_extremum = match fn_name {
                    "np_argmin" | "np_min" => {
                        call_min(ctx, (elem_ty, old_extremum), (elem_ty, curr_value))?
                    }
                    "np_argmax" | "np_max" => {
                        call_max(ctx, (elem_ty, old_extremum), (elem_ty, curr_value))?
                    }
                    _ => codegen_unreachable!(ctx),
                };

                let new_extremum_idx = match (old_extremum, new_extremum) {
                    (BasicValueEnum::IntValue(m), BasicValueEnum::IntValue(n)) => {
                        ctx.builder.build_select(
                            ctx.builder.build_int_compare(IntPredicate::NE, m, n, "")?,
                            curr_idx,
                            old_extremum_idx,
                            "",
                        )?
                    }
                    (BasicValueEnum::FloatValue(m), BasicValueEnum::FloatValue(n)) => {
                        ctx.builder.build_select(
                            ctx.builder.build_float_compare(FloatPredicate::ONE, m, n, "")?,
                            curr_idx,
                            old_extremum_idx,
                            "",
                        )?
                    }
                    _ =>
                    {
                        #[allow(clippy::tuple_array_conversions)]
                        unsupported_type(ctx, fn_name, &[elem_ty, elem_ty])
                    }
                };

                ctx.builder.build_store(extremum, new_extremum)?;
                ctx.builder.build_store(extremum_idx, new_extremum_idx)?;

                Ok(())
            })?;

            match fn_name {
                "np_argmin" | "np_argmax" => ctx
                    .builder
                    .build_int_s_extend_or_bit_cast(
                        ctx.builder.build_load(ctx.size_t, extremum_idx, "")?.into_int_value(),
                        ctx.i64,
                        "",
                    )?
                    .into(),
                "np_max" | "np_min" => ctx.builder.build_load(llvm_dtype, extremum, "")?,
                _ => codegen_unreachable!(ctx),
            }
        }

        _ => unsupported_type(ctx, fn_name, &[a_ty]),
    })
}

/// Invokes the `np_maximum` builtin function.
pub fn call_numpy_maximum<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_maximum";

    let common_ty = if ctx.unifier.unioned(x1_ty, x2_ty) { Some(x1_ty) } else { None };

    Ok(match (x1, x2) {
        (BasicValueEnum::IntValue(x1), BasicValueEnum::IntValue(x2)) => {
            debug_assert!(
                [
                    ctx.primitives.bool,
                    ctx.primitives.int32,
                    ctx.primitives.uint32,
                    ctx.primitives.int64,
                    ctx.primitives.uint64,
                    ctx.primitives.float,
                ]
                .iter()
                .any(|ty| ctx.unifier.unioned(common_ty.unwrap(), *ty))
            );

            call_max(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))?
        }

        (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
            debug_assert!(ctx.unifier.unioned(common_ty.unwrap(), ctx.primitives.float));

            call_max(ctx, (x1_ty, x1.into()), (x2_ty, x2.into()))?
        }

        (x1, x2)
            if [&x1_ty, &x2_ty].into_iter().any(|ty| {
                ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) =>
        {
            let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1)).to_ndarray(ctx)?;
            let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2)).to_ndarray(ctx)?;

            let x1_dtype = arraylike_flatten_element_type(&mut ctx.unifier, x1_ty);
            let x2_dtype = arraylike_flatten_element_type(&mut ctx.unifier, x2_ty);

            debug_assert!(ctx.unifier.unioned(x1_dtype, x2_dtype));
            let llvm_common_dtype = x1.ty.object.dtype;

            let result = broadcast_starmap(
                ctx,
                &[x1, x2],
                NDArrayOut::NewNDArray { dtype: llvm_common_dtype },
                |ctx, scalars| {
                    let x1_scalar = scalars[0];
                    let x2_scalar = scalars[1];
                    call_max(ctx, (x1_dtype, x1_scalar), (x2_dtype, x2_scalar))
                },
            )?;

            result.value.into()
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
fn helper_call_numpy_unary_elementwise<'ctx, OnScalarFn, RetElemFn>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (arg_ty, arg_val): (Type, BasicValueEnum<'ctx>),
    fn_name: &str,
    get_ret_elem_type: &RetElemFn,
    on_scalar: &OnScalarFn,
) -> anyhow::Result<BasicValueEnum<'ctx>>
where
    OnScalarFn: Fn(
        &mut CodeGenContext<'ctx, '_>,
        Type,
        BasicValueEnum<'ctx>,
    ) -> anyhow::Result<Option<BasicValueEnum<'ctx>>>,
    RetElemFn: Fn(&mut CodeGenContext<'ctx, '_>, Type) -> Type,
{
    let arg = ScalarOrNDArray::from_value(ctx, (arg_ty, arg_val));

    let dtype = arraylike_flatten_element_type(&mut ctx.unifier, arg_ty);

    let ret_ty = get_ret_elem_type(ctx, dtype);
    let llvm_ret_ty = ctx.get_llvm_type(ret_ty);
    let result = arg.map(ctx, llvm_ret_ty, |ctx, scalar| {
        let Some(result) = on_scalar(ctx, dtype, scalar)? else {
            unsupported_type(ctx, fn_name, &[arg_ty])
        };
        Ok(result)
    })?;

    Ok(result.to_basic_value_enum())
}

pub fn call_abs<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    n: (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "abs";

    helper_call_numpy_unary_elementwise(
        ctx,
        n,
        FN_NAME,
        &|_ctx, elem_ty| elem_ty,
        &|ctx, val_ty, val| match val {
            BasicValueEnum::IntValue(n) => {
                debug_assert!(
                    [
                        ctx.primitives.bool,
                        ctx.primitives.int32,
                        ctx.primitives.uint32,
                        ctx.primitives.int64,
                        ctx.primitives.uint64,
                    ]
                    .iter()
                    .any(|ty| ctx.unifier.unioned(val_ty, *ty))
                );

                Ok(Some(
                    if [ctx.primitives.int32, ctx.primitives.int64]
                        .iter()
                        .any(|ty| ctx.unifier.unioned(val_ty, *ty))
                    {
                        llvm_intrinsics::call_int_abs(ctx, n, ctx.i1.const_zero(), Some(FN_NAME))?
                            .into()
                    } else {
                        n.into()
                    },
                ))
            }

            BasicValueEnum::FloatValue(n) => {
                debug_assert!(ctx.unifier.unioned(val_ty, ctx.primitives.float));

                Ok(Some(llvm_intrinsics::call_float_fabs(ctx, n, Some(FN_NAME))?.into()))
            }

            _ => Ok(None),
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
        pub fn $name<'ctx>(
            ctx: &mut CodeGenContext<'ctx, '_>,
            arg: (Type, BasicValueEnum<'ctx>),
        ) -> anyhow::Result<BasicValueEnum<'ctx>> {
            helper_call_numpy_unary_elementwise(
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
/// ```rust,ignore
/// // Type of `$on_scalar:expr`
/// fn on_scalar<'ctx>(
///     ctx: &mut CodeGenContext<'ctx, '_>,
///     arg: FloatValue<'ctx>
/// ) -> anyhow::Result<IntValue<'ctx>> // of LLVM type `i1`
/// ```
macro_rules! create_helper_call_numpy_unary_elementwise_float_to_bool {
    ($name:ident, $fn_name:literal, $on_scalar:expr) => {
        create_helper_call_numpy_unary_elementwise!(
            $name,
            $fn_name,
            |ctx, _| ctx.primitives.bool,
            |ctx, n_ty, val| {
                match val {
                    BasicValueEnum::FloatValue(n) => {
                        debug_assert!(ctx.unifier.unioned(n_ty, ctx.primitives.float));

                        let ret = $on_scalar(ctx, n, Option::<&str>::None)?;
                        Ok(Some(bool_to_i8(ctx, ret)?.into()))
                    }
                    _ => Ok(None),
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
/// ```rust,ignore
/// // Type of `$on_scalar:expr`
/// fn on_scalar<'ctx>(
///
///     ctx: &mut CodeGenContext<'ctx, '_>,
///     arg: FloatValue<'ctx>
/// ) -> anyhow::Result<FloatValue<'ctx>>
/// ```
macro_rules! create_helper_call_numpy_unary_elementwise_float_to_float {
    ($name:ident, $fn_name:literal, $elem_call:expr) => {
        create_helper_call_numpy_unary_elementwise!(
            $name,
            $fn_name,
            |ctx, _| ctx.primitives.float,
            |ctx, val_ty, val| {
                match val {
                    BasicValueEnum::FloatValue(n) => {
                        debug_assert!(ctx.unifier.unioned(val_ty, ctx.primitives.float));

                        Ok(Some($elem_call(ctx, n, Option::<&str>::None)?.into()))
                    }
                    _ => Ok(None),
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
    irrt::call_tan
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arcsin,
    "np_arcsin",
    irrt::call_asin
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arccos,
    "np_arccos",
    irrt::call_acos
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arctan,
    "np_arctan",
    irrt::call_atan
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_sinh,
    "np_sinh",
    irrt::call_sinh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_cosh,
    "np_cosh",
    irrt::call_cosh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_tanh,
    "np_tanh",
    irrt::call_tanh
);

create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arcsinh,
    "np_arcsinh",
    irrt::call_asinh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arccosh,
    "np_arccosh",
    irrt::call_acosh
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_numpy_arctanh,
    "np_arctanh",
    irrt::call_atanh
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
    irrt::call_expm1
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
    irrt::call_cbrt
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
    irrt::call_erf
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_erfc,
    "sp_spec_erfc",
    irrt::call_erfc
);
create_helper_call_numpy_unary_elementwise_float_to_float!(
    call_scipy_special_gamma,
    "sp_spec_gamma",
    irrt::call_gamma
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
pub fn call_numpy_arctan2<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_arctan2";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
                    Ok(irrt::call_atan2(ctx, x1, x2, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_copysign` builtin function.
pub fn call_numpy_copysign<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_copysign";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
                    Ok(llvm_intrinsics::call_float_copysign(ctx, x1, x2, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_fmax` builtin function.
pub fn call_numpy_fmax<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_fmax";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
                    Ok(llvm_intrinsics::call_float_maxnum(ctx, x1, x2, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_fmin` builtin function.
pub fn call_numpy_fmin<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_fmin";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
                    Ok(llvm_intrinsics::call_float_minnum(ctx, x1, x2, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_ldexp` builtin function.
pub fn call_numpy_ldexp<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_ldexp";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1_scalar), BasicValueEnum::IntValue(x2_scalar)) => {
                    debug_assert_eq!(x1.get_dtype(), ctx.f64.into());
                    debug_assert_eq!(x2.get_dtype(), ctx.i32.into());
                    Ok(irrt::call_ldexp(ctx, x1_scalar, x2_scalar, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_hypot` builtin function.
pub fn call_numpy_hypot<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_hypot";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
                    Ok(irrt::call_hypot(ctx, x1, x2, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_nextafter` builtin function.
pub fn call_numpy_nextafter<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_nextafter";

    let x1 = ScalarOrNDArray::from_value(ctx, (x1_ty, x1));
    let x2 = ScalarOrNDArray::from_value(ctx, (x2_ty, x2));

    let result =
        ScalarOrNDArray::broadcasting_starmap(ctx, &[x1, x2], ctx.f64.into(), |ctx, scalars| {
            let x1_scalar = scalars[0];
            let x2_scalar = scalars[1];

            match (x1_scalar, x2_scalar) {
                (BasicValueEnum::FloatValue(x1), BasicValueEnum::FloatValue(x2)) => {
                    Ok(irrt::call_nextafter(ctx, x1, x2, None)?.into())
                }
                _ => unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty]),
            }
        })?;

    Ok(result.to_basic_value_enum())
}

/// Invokes the `np_linalg_cholesky` linalg function
pub fn call_np_linalg_cholesky<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_cholesky";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out = NDArrayType::create(ctx, ctx.f64.into(), 2).construct(ctx, None)?;
    out.copy_shape_from(ctx, &x1)?;
    out.create_data(ctx)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let out_c = out.make_contiguous_ndarray(ctx)?;
    extern_fns::call_np_linalg_cholesky(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        out_c.inner_value(ctx)?.value.into(),
        None,
    )?;
    Ok(out.value.into())
}

/// Invokes the `np_linalg_qr` linalg function
pub fn call_np_linalg_qr<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_qr";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape(ctx)?;
    let d0 = x1_shape.inner_value(ctx, None)?.get_unchecked(ctx, &ctx.size_t.const_zero(), None)?;
    let d1 = x1_shape.inner_value(ctx, None)?.get_unchecked(
        ctx,
        &ctx.size_t.const_int(1, false),
        None,
    )?;
    let dk = llvm_intrinsics::call_int_smin(ctx, d0, d1, None)?;

    let out_ndarray_ty = NDArrayType::create(ctx, ctx.f64.into(), 2);
    let q = out_ndarray_ty.with_shape(ctx, &[d0, dk], None)?;
    let r = out_ndarray_ty.with_shape(ctx, &[dk, d1], None)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let q_c = q.make_contiguous_ndarray(ctx)?;
    let r_c = r.make_contiguous_ndarray(ctx)?;

    extern_fns::call_np_linalg_qr(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        q_c.inner_value(ctx)?.value.into(),
        r_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    let tuple = TupleValue::new(ctx, &[q.value, r.value], None)?;
    Ok(tuple.value.into())
}

/// Invokes the `np_linalg_svd` linalg function
pub fn call_np_linalg_svd<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_svd";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape(ctx)?;
    let d0 = x1_shape.inner_value(ctx, None)?.get_unchecked(ctx, &ctx.size_t.const_zero(), None)?;
    let d1 = x1_shape.inner_value(ctx, None)?.get_unchecked(
        ctx,
        &ctx.size_t.const_int(1, false),
        None,
    )?;
    let dk = llvm_intrinsics::call_int_smin(ctx, d0, d1, None)?;

    let out_ndarray1_ty = NDArrayType::create(ctx, ctx.f64.into(), 1);
    let out_ndarray2_ty = NDArrayType::create(ctx, ctx.f64.into(), 2);

    let u = out_ndarray2_ty.with_shape(ctx, &[d0, d0], None)?;
    let s = out_ndarray1_ty.with_shape(ctx, &[dk], None)?;
    let vh = out_ndarray2_ty.with_shape(ctx, &[d1, d1], None)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let u_c = u.make_contiguous_ndarray(ctx)?;
    let s_c = s.make_contiguous_ndarray(ctx)?;
    let vh_c = vh.make_contiguous_ndarray(ctx)?;

    extern_fns::call_np_linalg_svd(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        u_c.inner_value(ctx)?.value.into(),
        s_c.inner_value(ctx)?.value.into(),
        vh_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    let tuple = TupleValue::new(ctx, &[u.value, s.value, vh.value], None)?;
    Ok(tuple.value.into())
}

/// Invokes the `np_linalg_inv` linalg function
pub fn call_np_linalg_inv<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_inv";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out = NDArrayType::create(ctx, ctx.f64.into(), 2).construct(ctx, None)?;
    out.copy_shape_from(ctx, &x1)?;
    out.create_data(ctx)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let out_c = out.make_contiguous_ndarray(ctx)?;
    extern_fns::call_np_linalg_inv(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        out_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    Ok(out.value.into())
}

/// Invokes the `np_linalg_pinv` linalg function
pub fn call_np_linalg_pinv<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_pinv";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape(ctx)?;
    let d0 = x1_shape.inner_value(ctx, None)?.get_unchecked(ctx, &ctx.size_t.const_zero(), None)?;
    let d1 = x1_shape.inner_value(ctx, None)?.get_unchecked(
        ctx,
        &ctx.size_t.const_int(1, false),
        None,
    )?;

    let out = NDArrayType::create(ctx, ctx.f64.into(), 2).with_shape(ctx, &[d0, d1], None)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let out_c = out.make_contiguous_ndarray(ctx)?;
    extern_fns::call_np_linalg_pinv(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        out_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    Ok(out.value.into())
}

/// Invokes the `sp_linalg_lu` linalg function
pub fn call_sp_linalg_lu<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "sp_linalg_lu";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let x1_shape = x1.shape(ctx)?;
    let d0 = x1_shape.inner_value(ctx, None)?.get_unchecked(ctx, &ctx.size_t.const_zero(), None)?;
    let d1 = x1_shape.inner_value(ctx, None)?.get_unchecked(
        ctx,
        &ctx.size_t.const_int(1, false),
        None,
    )?;
    let dk = llvm_intrinsics::call_int_smin(ctx, d0, d1, None)?;

    let out_ndarray_ty = NDArrayType::create(ctx, ctx.f64.into(), 2);

    let l = out_ndarray_ty.with_shape(ctx, &[d0, dk], None)?;
    let u = out_ndarray_ty.with_shape(ctx, &[dk, d1], None)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let l_c = l.make_contiguous_ndarray(ctx)?;
    let u_c = u.make_contiguous_ndarray(ctx)?;
    extern_fns::call_sp_linalg_lu(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        l_c.inner_value(ctx)?.value.into(),
        u_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    let tuple = TupleValue::new(ctx, &[l.value, u.value], None)?;
    Ok(tuple.value.into())
}

/// Invokes the `np_linalg_matrix_power` linalg function
pub fn call_np_linalg_matrix_power<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
    (x2_ty, x2): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_matrix_power";

    let BasicValueEnum::PointerValue(x1) = x1 else {
        unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
    };

    let (elem_ty, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, x1_ty);
    let ndims = extract_ndims(&ctx.unifier, ndims);
    let x1_elem_ty = ctx.get_llvm_type(elem_ty);
    let x1 = NDArrayType::create(ctx, x1_elem_ty, ndims).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    // x2 is a float, but we are promoting this to a 1D ndarray (.shape == [1]) for uniformity in function call.
    let x2 = call_float(ctx, (x2_ty, x2))?;
    let BasicValueEnum::FloatValue(x2) = x2 else {
        unsupported_type(ctx, FN_NAME, &[x1_ty, x2_ty])
    };

    let x2 = RawNDArrayValue::new_scalar(ctx, x2.into(), None)?;
    let x2 = x2.atleast_nd(ctx, 1)?; // x2.shape == [1]

    let out = NDArrayType::create(ctx, ctx.f64.into(), 2).construct(ctx, None)?;
    out.copy_shape_from(ctx, &x1)?;
    out.create_data(ctx)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let x2_c = x2.make_contiguous_ndarray(ctx)?;
    let out_c = out.make_contiguous_ndarray(ctx)?;

    extern_fns::call_np_linalg_matrix_power(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        x2_c.inner_value(ctx)?.value.into(),
        out_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    Ok(out.value.into())
}

/// Invokes the `np_linalg_det` linalg function
pub fn call_np_linalg_det<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "np_linalg_matrix_power";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    // The output is a float64, but we are using an ndarray (shape == [1]) for uniformity in function call.
    let shape = ctx.size_t.const_int(1, false);
    let det = NDArrayType::create(ctx, ctx.f64.into(), 1).with_shape(ctx, &[shape], None)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let out_c = det.make_contiguous_ndarray(ctx)?;
    extern_fns::call_np_linalg_det(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        out_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    // Get the determinant out of `out`
    let det = det.first_element(ctx)?;
    Ok(det)
}

/// Invokes the `sp_linalg_schur` linalg function
pub fn call_sp_linalg_schur<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "sp_linalg_schur";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);
    assert_eq!(x1.ty.object.ndims, 2);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out_ndarray_ty = NDArrayType::create(ctx, ctx.f64.into(), 2);

    let t = out_ndarray_ty.construct(ctx, None)?;
    t.copy_shape_from(ctx, &x1)?;
    t.create_data(ctx)?;

    let z = out_ndarray_ty.construct(ctx, None)?;
    z.copy_shape_from(ctx, &x1)?;
    z.create_data(ctx)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let t_c = t.make_contiguous_ndarray(ctx)?;
    let z_c = z.make_contiguous_ndarray(ctx)?;
    extern_fns::call_sp_linalg_schur(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        t_c.inner_value(ctx)?.value.into(),
        z_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    let tuple = TupleValue::new(ctx, &[t.value, z.value], None)?;
    Ok(tuple.value.into())
}

/// Invokes the `sp_linalg_hessenberg` linalg function
pub fn call_sp_linalg_hessenberg<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (x1_ty, x1): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    const FN_NAME: &str = "sp_linalg_hessenberg";

    let BasicValueEnum::PointerValue(x1) = x1 else { unsupported_type(ctx, FN_NAME, &[x1_ty]) };

    let x1 = NDArrayType::from_unifier_type(ctx, x1_ty).map_value(x1, None);
    assert_eq!(x1.ty.object.ndims, 2);

    if !x1.ty.object.dtype.is_float_type() {
        unsupported_type(ctx, FN_NAME, &[x1_ty]);
    }

    let out_ndarray_ty = NDArrayType::create(ctx, ctx.f64.into(), 2);

    let h = out_ndarray_ty.construct(ctx, None)?;
    h.copy_shape_from(ctx, &x1)?;
    h.create_data(ctx)?;

    let q = out_ndarray_ty.construct(ctx, None)?;
    q.copy_shape_from(ctx, &x1)?;
    q.create_data(ctx)?;

    let x1_c = x1.make_contiguous_ndarray(ctx)?;
    let h_c = h.make_contiguous_ndarray(ctx)?;
    let q_c = q.make_contiguous_ndarray(ctx)?;
    extern_fns::call_sp_linalg_hessenberg(
        ctx,
        x1_c.inner_value(ctx)?.value.into(),
        h_c.inner_value(ctx)?.value.into(),
        q_c.inner_value(ctx)?.value.into(),
        None,
    )?;

    let tuple = TupleValue::new(ctx, &[h.value, q.value], None)?;
    Ok(tuple.value.into())
}
