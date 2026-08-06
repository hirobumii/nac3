use std::{collections::HashMap, iter::once};

use anyhow::{anyhow, bail};
use inkwell::{
    IntPredicate,
    basic_block::BasicBlock,
    builder::Builder,
    module::Linkage,
    types::{BasicMetadataTypeEnum, BasicType},
    values::{BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue},
};
use itertools::{Itertools as _, izip};
use nac3parser::ast::{ExcepthandlerKind, Expr, ExprKind, Stmt, StmtKind, StrRef};

#[cfg(feature = "ctrc")]
use crate::codegen::expr::call_extern;
use crate::{
    codegen::{
        CodeGenContext, CodeGenerator, ModuleContext, VarValue,
        allocator::AllocationScope,
        bool_to_i1, bool_to_i8, builder_is_terminated,
        expr::{destructure_range, gen_binop_expr},
        gen_in_range_check,
        irrt::{calculate_len_for_slice_range, handle_slice_indices, list_slice_assignment},
        llvm_fns::FunctionDecl,
        llvm_intrinsics,
        macros::codegen_unreachable,
        opt,
        types::{
            ArrayLikeIndexer, ClassType, EnumerateType, ExceptionType, ExceptionValue, ListType,
            ListValue, NDArrayType, OpaqueRefCountedType, ProxyTypeBase, RangeType, RawListType,
            RefCountedValue, RustNDIndex, ScalarOrNDArray, StringType, TupleType, TupleValue,
            TypedRefCountedType, broadcast, field, is_refcounted_type,
        },
    },
    symbol_resolver::{SymbolValue, ValueEnum},
    toplevel::{
        DefinitionId, TopLevelContext, TopLevelDef,
        helper::{PrimDef, arraylike_flatten_element_type, extract_ndims},
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
    },
    typecheck::{
        magic_methods::Binop,
        typedef::{FunSignature, Type, TypeEnum, iter_type_vars},
    },
};

pub(crate) fn get_personality<'ctx>(
    top_level: &TopLevelContext,
    ctx: &ModuleContext<'ctx>,
) -> Option<FunctionValue<'ctx>> {
    let sym = top_level.personality_symbol.as_ref()?;
    // The personality is the only symbol where we do not use our external function ABI handling.
    Some(
        ctx.module
            .get_function(sym)
            .unwrap_or_else(|| ctx.module.add_function(sym, ctx.i32.fn_type(&[], true), None)),
    )
}

/// See [`CodeGenerator::gen_store_target`].
pub fn gen_store_target<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    pattern: &Expr<Option<Type>>,
    name: Option<&str>,
) -> anyhow::Result<Option<PointerValue<'ctx>>> {
    // very similar to gen_expr, but we don't do an extra load at the end
    // and we flatten nested tuples
    Ok(Some(match &pattern.node {
        ExprKind::Name { id, .. } => match ctx.var_assignment.get(id) {
            None => {
                let ptr_ty = ctx.get_llvm_type(pattern.custom.unwrap());
                // Variable allocas are always stack-allocated at the function start.
                // For refcounted types, ptr_ty is a pointer (holding a reference to the
                // heap object); for value types like tuples, ptr_ty is the struct itself.
                let ptr = ctx.alloc_at(AllocationScope::StackStartOfFunc, ptr_ty, name)?;
                ctx.var_assignment.insert(*id, VarValue::new(ptr, pattern.custom.unwrap()));
                ptr
            }
            Some(v) => {
                let VarValue { ptr, ty, counter, .. } = *v;
                ctx.var_assignment.insert(*id, VarValue { ptr, ty, static_value: None, counter });
                ptr
            }
        },
        ExprKind::Attribute { value, attr, .. } => {
            let (index, _) = ctx.get_attr_index(value.custom.unwrap(), *attr);
            let val = generator.gen_expr(ctx, value)?.to_basic_value_enum(ctx)?;

            let BasicValueEnum::PointerValue(ptr) = val else {
                codegen_unreachable!(ctx);
            };
            let is_refcounted = is_refcounted_type(&mut ctx.unifier, value.custom.unwrap());
            if is_refcounted {
                let class_val =
                    ClassType::from_unifier_type(ctx, value.custom.unwrap()).map_value(ptr, None);
                class_val.inner_value(ctx)?.gep_field(ctx, index as u32)?
            } else {
                let alloca_ty = ctx.get_alloca_type(value.custom.unwrap());
                unsafe {
                    ctx.builder.build_in_bounds_gep(
                        alloca_ty,
                        ptr,
                        &[ctx.i32.const_zero(), ctx.i32.const_int(index as u64, false)],
                        name.unwrap_or(""),
                    )?
                }
            }
        }
        ExprKind::Tuple { elts, .. } => {
            let elts = elts
                .iter()
                .map(|e| {
                    generator
                        .gen_store_target(ctx, e, name)
                        .and_then(|v| v.ok_or_else(|| anyhow!("failed to generate store target")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let struct_ty =
                ctx.ctx.struct_type(&elts.iter().map(|p| p.get_type().into()).collect_vec(), false);
            let struct_ptr = ctx.alloc(struct_ty, name)?;
            for (i, elt) in elts.iter().enumerate() {
                ctx.builder.build_store(
                    unsafe {
                        ctx.builder.build_in_bounds_gep(
                            struct_ty,
                            struct_ptr,
                            &[ctx.i32.const_zero(), ctx.i32.const_int(i as u64, false)],
                            "",
                        )?
                    },
                    *elt,
                )?;
            }
            struct_ptr
        }
        _ => codegen_unreachable!(ctx),
    }))
}

/// See [`CodeGenerator::gen_assign`].
pub fn gen_assign<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    target: &Expr<Option<Type>>,
    value: &ValueEnum<'ctx>,
    value_ty: Type,
) -> anyhow::Result<()> {
    // See https://docs.python.org/3/reference/simple_stmts.html#assignment-statements.
    match &target.node {
        ExprKind::Subscript { value: sub_target, slice: key, .. } => {
            use opt::ndarray_subscript_fusion::{gen_fused_scalar_setitem, try_fuse_scalar_chain};

            // Handle "slicing" or "subscription"
            // Fuse chained integer subscripts (`a[i][j] = v`) into a single element store
            // (`a[i, j] = v`) where possible
            if let Some(chain) = try_fuse_scalar_chain(ctx, target) {
                gen_fused_scalar_setitem(generator, ctx, &chain, value, value_ty)?;
            } else {
                generator.gen_setitem(ctx, sub_target, key, value, value_ty)?;
            }
        }
        ExprKind::Tuple { elts, .. } | ExprKind::List { elts, .. } => {
            // Fold on `"[" [target_list] "]"` or `"(" [target_list] ")`
            generator.gen_assign_target_list(ctx, elts, value, value_ty)?;
        }
        _ => {
            // Handle attribute and direct variable assignments.
            let name = if let ExprKind::Name { id, .. } = &target.node {
                format!("{id}.addr")
            } else {
                String::from("target.addr")
            };
            let Some(ptr) = generator.gen_store_target(ctx, target, Some(name.as_str()))? else {
                return Ok(());
            };

            if let ExprKind::Name { id, .. } = &target.node {
                let VarValue { static_value, counter, .. } =
                    ctx.var_assignment.get_mut(id).unwrap();
                *counter += 1;
                if let ValueEnum::Static(s) = &value {
                    *static_value = Some(s.clone());
                }
            }
            let val = value.to_basic_value_enum(ctx, target.custom.unwrap())?;

            // Perform i1 <-> i8 conversion as needed
            let val = if ctx.unifier.unioned(target.custom.unwrap(), ctx.primitives.bool) {
                bool_to_i8(ctx, val.into_int_value())?.into()
            } else {
                val
            };

            // Handle reference counting:
            // The order of operations is roughly:
            //   - Store a pointer to the old value
            //   - Increment the refcount of the new value
            //   - Perform the assignment
            //   - Decrement the refcount of the old value
            // This order ensures that self-assignments work correctly.

            let target = if is_refcounted_type(&mut ctx.unifier, value_ty) {
                let non_null = ctx.builder.build_is_not_null(ptr, "")?;
                Some(ctx.build_ternary(
                    "assign.old_value",
                    non_null,
                    |ctx| {
                        let target_ty = ctx.get_llvm_type(target.custom.unwrap());
                        Ok(ctx.builder.build_load(target_ty, ptr, "")?.into_pointer_value())
                    },
                    |ctx| Ok(ctx.ptr.const_null()),
                )?)
            } else {
                None
            };

            if let BasicValueEnum::PointerValue(val) = val
                && is_refcounted_type(&mut ctx.unifier, value_ty)
            {
                OpaqueRefCountedType::new(ctx)
                    .map_value(val, None)
                    .header(ctx)
                    .safe_increment_refcount(ctx)?;
            }
            ctx.builder.build_store(ptr, val)?;
            if let Some(target) = target {
                OpaqueRefCountedType::new(ctx)
                    .map_value(target, None)
                    .header(ctx)
                    .safe_decrement_refcount(ctx)?;
            }
        }
    }
    Ok(())
}

/// See [`CodeGenerator::gen_assign_target_list`].
pub fn gen_assign_target_list<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    targets: &[Expr<Option<Type>>],
    value: &ValueEnum<'ctx>,
    value_ty: Type,
) -> anyhow::Result<()> {
    let do_assign = |generator: &mut G, ctx: &mut _, targets, values: &[_]| {
        izip!(targets, values).try_for_each(|(target, &(val_ty, val))| {
            generator.gen_assign(&mut *ctx, target, &ValueEnum::Dynamic(val), val_ty)
        })
    };
    let do_assign_list = |generator: &mut G, ctx: &mut _, target, list: &ListValue<'ctx>| {
        let ptr = generator.gen_store_target(ctx, target, Some("starred_target.addr"))?.unwrap();
        ctx.builder.build_store(ptr, list.value)?;
        anyhow::Ok(())
    };

    // Find the starred target if it exists.
    // Index of the "starred" target. If it exists, there may only be one.
    let mut starred_target_index = None;
    for (i, target) in targets.iter().enumerate() {
        if let ExprKind::Starred { value: mid, .. } = &target.node {
            let (head, tail) = (&targets[..i], &targets[i + 1..]);
            // Ensured by typechecker
            assert!(starred_target_index.replace((head, &**mid, tail)).is_none());
        }
    }

    match &*ctx.unifier.get_ty(value_ty) {
        TypeEnum::TTuple { ty: tuple_tys, .. } => {
            // Deconstruct the tuple `value`
            let tuple = value.to_basic_value_enum(ctx, value_ty)?.into_struct_value();
            let tuple = TupleType::from_unifier_type(ctx, value_ty).map_value(tuple, None);
            let tuple = tuple_tys
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    let elem = tuple.extract(ctx, i as u32)?;
                    anyhow::Ok((*ty, elem))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            // Unlike in python we require the starred target to consume one or more items when the
            // RHS is a tuple. Otherwise, it would be impossible type the starred target.
            if tuple.len() < targets.len() {
                bail!(
                    "Tuple unpacking requires at least as many values as targets including the starred target, but got {} values and {} targets (at {})",
                    tuple_tys.len(),
                    targets.len(),
                    targets[0].location
                );
            }

            let Some((head, mid, tail)) = starred_target_index else {
                // No starred target, simple assignment
                return do_assign(generator, ctx, targets, &tuple);
            };

            let (tup_head, tup_rest) = tuple.split_at(head.len());
            let (tup_mid, tup_tail) = tup_rest.split_at(tup_rest.len() - tail.len());
            // Handle assignments up to starred target
            do_assign(generator, ctx, head, tup_head)?;

            let tup_mid_ty = tup_mid[0].0;
            // nac3 lists can only contain one type, hence starred targets can only contain one type
            // This is previously checked by the typechecker
            debug_assert!(
                tup_mid.iter().all(|t| ctx.unifier.unioned(t.0, tup_mid_ty)),
                "Starred target must have same type for all items"
            );
            let tup_mid_len = ctx.size_t.const_int(tup_mid.len() as u64, false);
            let starred_list = ListType::create(ctx, tup_mid_ty).construct(
                ctx,
                tup_mid_len,
                Some("starred_list"),
            )?;
            let starred_list_data = starred_list.inner_value(ctx)?.data(ctx)?;
            for (i, &(_, val)) in tup_mid.iter().enumerate() {
                // Use set_unchecked: the array's internal length tracks refcounted
                // element count (0 for non-pointer elements), not the actual capacity.
                starred_list_data.inner_value(ctx, Some(tup_mid_len))?.set_unchecked(
                    ctx,
                    &ctx.size_t.const_int(i as u64, false),
                    val,
                    None,
                )?;
                // Increment refcount: existing reference copied into new list
                if let BasicValueEnum::PointerValue(p) = val
                    && is_refcounted_type(&mut ctx.unifier, tup_mid_ty)
                {
                    OpaqueRefCountedType::new(ctx)
                        .map_value(p, None)
                        .header(ctx)
                        .safe_increment_refcount(ctx)?;
                }
            }
            do_assign_list(generator, ctx, mid, &starred_list)?;

            do_assign(generator, ctx, tail, tup_tail)?;
        }
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::List.id() => {
            let list = value.to_basic_value_enum(ctx, value_ty)?.into_pointer_value();
            let list_ty = ListType::from_unifier_type(ctx, value_ty);
            let list = list_ty.map_value(list, None);
            let list_data = list.inner_value(ctx)?.data(ctx)?;
            let elem_ty = list.ty.object.item_ty;
            let rhs_size = list.inner_value(ctx)?.load(ctx, field!(len))?;

            let do_read = |ctx: &mut CodeGenContext<'ctx, '_>, at: _| {
                let elem: BasicValueEnum<'ctx> =
                    list_data.inner_value(ctx, Some(rhs_size))?.get_unchecked(ctx, &at, None)?;
                Ok((elem_ty, elem))
            };
            let read_fixed = |ctx: &mut CodeGenContext<'ctx, '_>, to: usize| {
                (0..to)
                    .map(|i| {
                        let i = ctx.size_t.const_int(i as u64, false);
                        do_read(ctx, i)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()
            };

            let Some((head, mid, tail)) = starred_target_index else {
                // No starred target, simple assignment

                {
                    let lhs_size = ctx.size_t.const_int(targets.len() as u64, false);
                    let rhs_size_zext =
                        ctx.builder.build_int_z_extend_or_bit_cast(rhs_size, ctx.i64, "")?;
                    let lhs_size_zext =
                        ctx.builder.build_int_z_extend_or_bit_cast(lhs_size, ctx.i64, "")?;
                    ctx.make_assert(
                        ctx.builder.build_int_compare(
                            IntPredicate::EQ,
                            rhs_size,
                            lhs_size,
                            "list_size_check",
                        )?,
                        "0:ValueError",
                        "incorrect number of values to unpack (expected {1})",
                        [Some(rhs_size_zext), Some(lhs_size_zext), None],
                    )?;
                }

                let values = read_fixed(ctx, targets.len())?;
                return do_assign(generator, ctx, targets, &values);
            };

            // All non-starred targets must be assigned exactly one value.
            let min_size = targets.len() - 1;
            let min_size_ = ctx.size_t.const_int(min_size as u64, false);
            {
                let min_size_zext =
                    ctx.builder.build_int_z_extend_or_bit_cast(min_size_, ctx.i64, "")?;
                let rhs_size_zext =
                    ctx.builder.build_int_z_extend_or_bit_cast(rhs_size, ctx.i64, "")?;
                ctx.make_assert(
                    ctx.builder.build_int_compare(
                        IntPredicate::ULE,
                        min_size_,
                        rhs_size,
                        "list_size_check",
                    )?,
                    "0:ValueError",
                    "too few values to unpack (expected at least {0}, got {1})",
                    [Some(min_size_zext), Some(rhs_size_zext), None],
                )?;
            }

            let head_values = read_fixed(ctx, head.len())?;
            do_assign(generator, ctx, head, &head_values)?;

            let head_len = ctx.size_t.const_int(head.len() as u64, false);
            let mid_len = ctx.builder.build_int_sub(rhs_size, min_size_, "mid_len")?;
            let mid_begin = list_data.inner_value(ctx, Some(rhs_size))?.ptr_offset_unchecked(
                ctx,
                &head_len,
                Some("mid_begin"),
            )?;
            let tail_len = ctx.size_t.const_int(tail.len() as u64, false);
            let tail_begin = ctx.builder.build_int_sub(rhs_size, tail_len, "tail_begin")?;

            let mid_list =
                ListType::create(ctx, elem_ty).construct(ctx, mid_len, Some("mid_list"))?;
            let mid_list_data = mid_list.inner_value(ctx)?.data(ctx)?;
            let llvm_list_elem_ty = ctx.get_llvm_type(elem_ty);
            let sizeof_elem = ctx.builder.build_int_truncate_or_bit_cast(
                llvm_list_elem_ty.size_of().unwrap(),
                ctx.size_t,
                "",
            )?;
            llvm_intrinsics::call_memcpy(
                ctx,
                mid_list_data.inner_value(ctx, Some(mid_len))?.value.0,
                mid_begin,
                ctx.builder.build_int_mul(mid_len, sizeof_elem, "")?,
            )?;
            // Increment refcount for each copied element in the new mid_list
            if is_refcounted_type(&mut ctx.unifier, elem_ty) {
                let mid_list_data_inner = mid_list_data.inner_value(ctx, Some(mid_len))?;
                ctx.build_repeat("list.assign.incref", mid_len, |ctx, _, i| {
                    let elem: PointerValue<'ctx> =
                        mid_list_data_inner.get_unchecked(ctx, &i, None)?;
                    OpaqueRefCountedType::new(ctx)
                        .map_value(elem, None)
                        .header(ctx)
                        .safe_increment_refcount(ctx)?;
                    Ok(())
                })?;
            }
            do_assign_list(generator, ctx, mid, &mid_list)?;

            let list_tail = (0..tail.len())
                .map(|i| {
                    let i = ctx.size_t.const_int(i as u64, false);
                    let idx = ctx.builder.build_int_add(tail_begin, i, "tail_index")?;
                    do_read(ctx, idx)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            do_assign(generator, ctx, tail, &list_tail)?;
        }
        // The typechecker ensures this
        _ => codegen_unreachable!(ctx),
    }
    Ok(())
}

/// See [`CodeGenerator::gen_setitem`].
pub fn gen_setitem<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    target: &Expr<Option<Type>>,
    key: &Expr<Option<Type>>,
    value: &ValueEnum<'ctx>,
    value_ty: Type,
) -> anyhow::Result<()> {
    let target_ty = target.custom.unwrap();

    match &*ctx.unifier.get_ty(target_ty) {
        TypeEnum::TObj { obj_id, params: list_params, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            // Handle list item assignment
            let target_item_ty = iter_type_vars(list_params).next().unwrap().ty;

            let target = generator.gen_expr(ctx, target)?;
            let target_val = target.to_basic_value_enum(ctx)?.into_pointer_value();
            let target = {
                let list_ty = RawListType::from_unifier_type(ctx, target_ty);
                TypedRefCountedType::new(ctx, list_ty).map_value(target_val, None)
            };

            if let ExprKind::Slice { .. } = &key.node {
                // Handle assigning to a slice
                let ExprKind::Slice { lower, upper, step } = &key.node else {
                    codegen_unreachable!(ctx)
                };
                let target_size = target.inner_value(ctx)?.load(ctx, field!(len))?;
                let Some((start, end, step)) =
                    handle_slice_indices(lower, upper, step, ctx, generator, target_size)?
                else {
                    return Ok(());
                };

                let value_val = value.to_basic_value_enum(ctx, value_ty)?.into_pointer_value();
                let value = {
                    let list_ty = RawListType::from_unifier_type(ctx, target_ty);
                    TypedRefCountedType::new(ctx, list_ty).map_value(value_val, None)
                };

                let target_item_llvm_ty = ctx.get_llvm_type(target_item_ty);
                let size = value.inner_value(ctx)?.load(ctx, field!(len))?;
                let Some(src_ind) =
                    handle_slice_indices(&None, &None, &None, ctx, generator, size)?
                else {
                    return Ok(());
                };

                // Decrement refcounts of destination elements being overwritten
                if is_refcounted_type(&mut ctx.unifier, target_item_ty) {
                    let dest_data =
                        target.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(target_size))?;
                    let llvm_i32 = ctx.i32;
                    let one = llvm_i32.const_int(1, false);
                    let zero = llvm_i32.const_zero();
                    let dest_end = ctx
                        .builder
                        .build_select(
                            ctx.builder.build_int_compare(
                                IntPredicate::SLT,
                                step,
                                zero,
                                "is_neg",
                            )?,
                            ctx.builder.build_int_sub(end, one, "")?,
                            ctx.builder.build_int_add(end, one, "")?,
                            "",
                        )?
                        .into_int_value();
                    let dest_slice_len = calculate_len_for_slice_range(ctx, start, dest_end, step)?;
                    let dest_slice_len = ctx.builder.build_int_z_extend_or_bit_cast(
                        dest_slice_len,
                        ctx.size_t,
                        "",
                    )?;
                    ctx.build_repeat("list.setitem.decref", dest_slice_len, |ctx, _, i| {
                        let actual_idx = {
                            let step_ext =
                                ctx.builder.build_int_s_extend_or_bit_cast(step, ctx.size_t, "")?;
                            let start_ext = ctx
                                .builder
                                .build_int_s_extend_or_bit_cast(start, ctx.size_t, "")?;
                            let offset = ctx.builder.build_int_mul(i, step_ext, "")?;
                            ctx.builder.build_int_add(start_ext, offset, "")?
                        };
                        let elem: PointerValue<'ctx> =
                            dest_data.get_unchecked(ctx, &actual_idx, None)?;
                        OpaqueRefCountedType::new(ctx)
                            .map_value(elem, None)
                            .header(ctx)
                            .safe_decrement_refcount(ctx)?;
                        Ok(())
                    })?;
                }

                list_slice_assignment(
                    ctx,
                    target_item_llvm_ty,
                    target,
                    (start, end, step),
                    value,
                    src_ind,
                )?;

                // Increment refcounts of source elements that were copied into dest
                if is_refcounted_type(&mut ctx.unifier, target_item_ty) {
                    let src_data =
                        value.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(size))?;
                    let llvm_i32 = ctx.i32;
                    let one = llvm_i32.const_int(1, false);
                    let zero = llvm_i32.const_zero();
                    let src_end = ctx
                        .builder
                        .build_select(
                            ctx.builder.build_int_compare(
                                IntPredicate::SLT,
                                src_ind.2,
                                zero,
                                "is_neg",
                            )?,
                            ctx.builder.build_int_sub(src_ind.1, one, "")?,
                            ctx.builder.build_int_add(src_ind.1, one, "")?,
                            "",
                        )?
                        .into_int_value();
                    let src_slice_len =
                        calculate_len_for_slice_range(ctx, src_ind.0, src_end, src_ind.2)?;
                    let src_slice_len = ctx.builder.build_int_z_extend_or_bit_cast(
                        src_slice_len,
                        ctx.size_t,
                        "",
                    )?;
                    ctx.build_repeat("list.setitem.incref", src_slice_len, |ctx, _, i| {
                        let actual_idx = {
                            let step_ext = ctx
                                .builder
                                .build_int_s_extend_or_bit_cast(src_ind.2, ctx.size_t, "")?;
                            let start_ext = ctx
                                .builder
                                .build_int_s_extend_or_bit_cast(src_ind.0, ctx.size_t, "")?;
                            let offset = ctx.builder.build_int_mul(i, step_ext, "")?;
                            ctx.builder.build_int_add(start_ext, offset, "")?
                        };
                        let elem: PointerValue<'ctx> =
                            src_data.get_unchecked(ctx, &actual_idx, None)?;
                        OpaqueRefCountedType::new(ctx)
                            .map_value(elem, None)
                            .header(ctx)
                            .safe_increment_refcount(ctx)?;
                        Ok(())
                    })?;
                }
            } else {
                // Handle assigning to an index
                let len = target.inner_value(ctx)?.load(ctx, field!(len))?;

                let index =
                    generator.gen_expr(ctx, key)?.to_basic_value_enum(ctx)?.into_int_value();
                let index = ctx.builder.build_int_s_extend(index, ctx.size_t, "sext")?;

                // handle negative index
                let is_negative = ctx.builder.build_int_compare(
                    IntPredicate::SLT,
                    index,
                    ctx.size_t.const_zero(),
                    "is_neg",
                )?;
                let adjusted = ctx.builder.build_int_add(index, len, "adjusted")?;
                let index = ctx
                    .builder
                    .build_select(is_negative, adjusted, index, "index")?
                    .into_int_value();

                // unsigned less than is enough, because negative index after adjustment is
                // bigger than the length (for unsigned cmp)
                {
                    let bound_check =
                        ctx.builder.build_int_compare(IntPredicate::ULT, index, len, "inbound")?;
                    let index_sext =
                        ctx.builder.build_int_s_extend_or_bit_cast(index, ctx.i64, "")?;
                    let len_zext = ctx.builder.build_int_z_extend_or_bit_cast(len, ctx.i64, "")?;
                    ctx.make_assert(
                        bound_check,
                        "0:IndexError",
                        "index {0} out of bounds 0:{1}",
                        [Some(index_sext), Some(len_zext), None],
                    )?;
                }

                // Write value to index on list
                let value = value.to_basic_value_enum(ctx, value_ty)?;
                let list_data = target.inner_value(ctx)?.data(ctx)?;
                let list_data_inner = list_data.inner_value(ctx, Some(len))?;

                if is_refcounted_type(&mut ctx.unifier, target_item_ty) {
                    // Load old element and increment new value before store
                    let old_elem: PointerValue<'ctx> =
                        list_data_inner.get_unchecked(ctx, &index, None)?;
                    OpaqueRefCountedType::new(ctx)
                        .map_value(value.into_pointer_value(), None)
                        .header(ctx)
                        .safe_increment_refcount(ctx)?;
                    // Bounds already verified above; array metadata tracks refcount
                    // element count (0 for non-pointer elements), not actual capacity.
                    list_data_inner.set_unchecked(ctx, &index, value, Some("list_item"))?;
                    OpaqueRefCountedType::new(ctx)
                        .map_value(old_elem, None)
                        .header(ctx)
                        .safe_decrement_refcount(ctx)?;
                } else {
                    list_data_inner.set_unchecked(ctx, &index, value, Some("list_item"))?;
                }
            }
        }
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
        {
            // Handle NDArray item assignment
            // Process target
            let target = generator.gen_expr(ctx, target)?.to_basic_value_enum(ctx)?;

            // Process key
            let key = RustNDIndex::from_subscript_expr(generator, ctx, key)?;

            // Process value
            let value = value.to_basic_value_enum(ctx, value_ty)?;

            let target = NDArrayType::from_unifier_type(ctx, target_ty)
                .map_value(target.into_pointer_value(), None);

            // Fast path: assigning a scalar to a single element selected by basic integer indexing
            // (exactly one integer index per axis) stores directly to the computed element pointer
            let ndarray_obj_id = ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap();
            let value_is_scalar = !matches!(
                &*ctx.unifier.get_ty(value_ty),
                TypeEnum::TObj { obj_id, .. } if *obj_id == ndarray_obj_id
            );
            let ndims = target.inner_value(ctx)?.ty.ndims;
            let single_indices = key
                .iter()
                .map(|idx| match idx {
                    RustNDIndex::SingleElement(v) => Some(*v),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>();

            if let Some(single_indices) = single_indices
                && value_is_scalar
                && u64::try_from(single_indices.len()).unwrap() == ndims
            {
                let elem_ptr = target.get_scalar_pelement(ctx, &single_indices)?;
                let value = if ctx.unifier.unioned(value_ty, ctx.primitives.bool) {
                    bool_to_i8(ctx, value.into_int_value())?.into()
                } else {
                    value
                };
                ctx.builder.build_store(elem_ptr, value)?;
            } else {
                // Reference code:
                // ```python
                // target = target[key]
                // value = np.asarray(value)
                //
                // shape = np.broadcast_shape((target, value))
                //
                // target = np.broadcast_to(target, shape)
                // value = np.broadcast_to(value, shape)
                //
                // # ...and finally copy 1-1 from value to target.
                // ```

                let target = target.index(ctx, &key)?;

                let value = ScalarOrNDArray::from_value(ctx, (value_ty, value)).to_ndarray(ctx)?;

                let broadcast_result = broadcast(ctx, &[target, value])?;

                let target = broadcast_result.ndarrays[0];
                let value = broadcast_result.ndarrays[1];

                target.copy_data_from(ctx, &value)?;
            }
        }
        _ => {
            panic!("encountered unknown target type: {}", ctx.unifier.stringify(target_ty));
        }
    }
    Ok(())
}

fn restore_var_assignment(
    ctx: &mut CodeGenContext<'_, '_>,
    var_assignment: &HashMap<StrRef, VarValue<'_>>,
) {
    for (k, VarValue { counter, .. }) in var_assignment {
        let VarValue { static_value: static_val, counter: counter2, .. } =
            ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }
}

impl<'ctx> CodeGenContext<'ctx, '_> {
    /// Generates a loop.
    ///
    /// Behaves like the following code:
    ///
    /// ```txt
    /// for (;; <update>)
    ///     <body>
    /// ```
    ///
    /// Note that a `continue` within the body would proceed to execute `update`.
    ///
    /// This is an infinite loop by default; to exit on a condition, use
    /// [`CodeGenContext::branch`] followed by [`BreakContinueHooks::build_break`].
    pub fn build_loop<T, U, BodyFn, UpdateFn>(
        &mut self,
        label: &str,
        body: BodyFn,
        update: UpdateFn,
    ) -> anyhow::Result<U>
    where
        BodyFn: FnOnce(&mut Self, BreakContinueHooks<'ctx>) -> anyhow::Result<T>,
        UpdateFn: FnOnce(&mut Self, T) -> anyhow::Result<U>,
    {
        fn init_bbs<'ctx>(
            ctx: &CodeGenContext<'ctx, '_>,
            label: &str,
        ) -> anyhow::Result<([BasicBlock<'ctx>; 3], BreakContinueHooks<'ctx>)> {
            let mut bb = ctx.builder.get_insert_block().unwrap();
            // - `cond`: Loop header, contains the exit condition.
            // - `update`: Loop update block, executed after each iteration of the body.
            // - `end`: Loop exit block, executed after the loop terminates.
            let bbs = ["cond", "update", "end"].map(|name| {
                bb = ctx.ctx.insert_basic_block_after(bb, &format!("{label}.{name}"));
                bb
            });
            ctx.builder.build_unconditional_branch(bbs[0])?;
            ctx.builder.position_at_end(bbs[0]);
            Ok((bbs, BreakContinueHooks { latch_bb: bbs[1], exit_bb: bbs[2] }))
        }

        let ([cond_bb, update_bb, end_bb], hooks) = init_bbs(self, label)?;
        // store loop bb information and restore it later
        let loop_bb = self.loop_target.replace((update_bb, end_bb));
        // var_assignment static values may be changed in another branch
        // if so, remove the static value as it may not be correct in this branch
        let var_assignment = self.var_assignment.clone();

        let result = body(self, hooks)?;
        self.jump_if_not_terminated(update_bb)?;

        self.builder.position_at_end(update_bb);
        let result = update(self, result)?;
        restore_var_assignment(self, &var_assignment);
        self.jump_if_not_terminated(cond_bb)?;

        self.builder.position_at_end(end_bb);
        self.loop_target = loop_bb;
        Ok(result)
    }

    /// Repeats the `body` `n` times. The index (in `[0, n)`) is provided to the closure.
    ///
    /// Internally generates a C-style for loop.
    pub fn build_repeat<T, BodyFn>(
        &mut self,
        label: &str,
        n: IntValue<'ctx>,
        body: BodyFn,
    ) -> anyhow::Result<T>
    where
        BodyFn: FnOnce(&mut Self, BreakContinueHooks<'ctx>, IntValue<'ctx>) -> anyhow::Result<T>,
    {
        fn do_cmp<'ctx>(
            ctx: &mut CodeGenContext<'ctx, '_>,
            label: &str,
            hooks: BreakContinueHooks<'ctx>,
            i_addr: PointerValue<'ctx>,
            n: IntValue<'ctx>,
        ) -> anyhow::Result<IntValue<'ctx>> {
            let i = ctx.builder.build_load(n.get_type(), i_addr, "")?.into_int_value();
            let cmp = ctx.builder.build_int_compare(IntPredicate::ULT, i, n, "")?;
            let finish = ctx.branch(label, cmp)?;
            ctx.in_block(finish, |ctx| hooks.build_break(&ctx.builder))?;
            Ok(i)
        }

        let ty = n.get_type();
        let one = ty.const_int(1, false);
        let i_addr = self.alloc(ty, Some("iter_counter"))?;
        self.builder.build_store(i_addr, ty.const_zero())?;

        self.build_loop(
            label,
            |ctx, hooks| {
                let i = do_cmp(ctx, label, hooks, i_addr, n)?;
                Ok((body(ctx, hooks, i)?, i))
            },
            |ctx, (result, i)| {
                let i = ctx.builder.build_int_add(i, one, "")?;
                ctx.builder.build_store(i_addr, i)?;
                Ok(result)
            },
        )
    }

    /// Generates a two-armed branch on `cond`.
    ///
    /// Returns each arm's result paired with the basic block that arm terminated in - i.e., the
    /// incoming edges a `phi` needs. The builder will be positioned at the merge block (`cont`)
    /// when this function returns.
    ///
    /// The generated basic blocks will be named `<name>.{then,else,end}` for the `then` branch,
    /// the `else` branch, and the merge block respectively.
    ///
    /// Note that the return value `T` of `then_fn` is passed into `else_fn` - This is so that a
    /// caller can move a borrow from `then_fn` to `else_fn` without capturing it in both closures.
    fn build_branching<A, B, T, ThenFn, ElseFn>(
        &mut self,
        name: &str,
        cond: IntValue<'ctx>,
        then_fn: ThenFn,
        else_fn: ElseFn,
    ) -> anyhow::Result<((A, BasicBlock<'ctx>), (B, BasicBlock<'ctx>))>
    where
        ThenFn: FnOnce(&mut Self) -> anyhow::Result<(A, T)>,
        ElseFn: FnOnce(&mut Self, T) -> anyhow::Result<B>,
    {
        let var_assignment = self.var_assignment.clone();

        let else_bb = self.branch(name, cond)?;
        let end_bb = self.ctx.insert_basic_block_after(else_bb, &format!("{name}.end"));

        let (then_val, arg) = then_fn(self)?;
        let then_end_bb = self.builder.get_insert_block().unwrap();
        self.jump_if_not_terminated(end_bb)?;
        restore_var_assignment(self, &var_assignment);

        self.builder.position_at_end(else_bb);
        let else_val = else_fn(self, arg)?;
        let else_end_bb = self.builder.get_insert_block().unwrap();
        self.jump_if_not_terminated(end_bb)?;
        restore_var_assignment(self, &var_assignment);

        self.builder.position_at_end(end_bb);
        Ok(((then_val, then_end_bb), (else_val, else_end_bb)))
    }

    /// Generates a C-style ternary operation, similar to the following C code:
    ///
    /// ```txt
    /// T val = <cond> ? <then_fn> : <else_fn>
    /// ```
    ///
    /// Both arms must produce the same LLVM type; the results are merged with a `phi`.
    pub fn build_ternary<V, ThenFn, ElseFn>(
        &mut self,
        name: &str,
        cond: IntValue<'ctx>,
        then_fn: ThenFn,
        else_fn: ElseFn,
    ) -> anyhow::Result<V>
    where
        ThenFn: FnOnce(&mut Self) -> anyhow::Result<V>,
        ElseFn: FnOnce(&mut Self) -> anyhow::Result<V>,
        V: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug>,
    {
        let ((then_val, then_bb), (else_val, else_bb)) = self.build_branching(
            name,
            cond,
            |ctx| Ok((then_fn(ctx)?, ())),
            |ctx, ()| else_fn(ctx),
        )?;

        let val_ty = then_val.as_basic_value_enum().get_type();
        assert_eq!(val_ty, else_val.as_basic_value_enum().get_type());

        let phi = self.builder.build_phi(val_ty, name)?;
        phi.add_incoming(&[(&then_val, then_bb), (&else_val, else_bb)]);

        Ok(phi.as_basic_value().try_into().unwrap())
    }

    /// Generates a C-style `if-else`, similar to the following C code:
    ///
    /// ```txt
    /// if <cond> { <then_fn> } else { <else_fn> }
    /// ```
    ///
    /// Note that the return value `T` of `then_fn` is passed into `else_fn` - See
    /// [`Self::build_branching`] for the purpose of this.
    pub fn build_if_else<T, U, ThenFn, ElseFn>(
        &mut self,
        name: &str,
        cond: IntValue<'ctx>,
        then_fn: ThenFn,
        else_fn: ElseFn,
    ) -> anyhow::Result<U>
    where
        ThenFn: FnOnce(&mut Self) -> anyhow::Result<T>,
        ElseFn: FnOnce(&mut Self, T) -> anyhow::Result<U>,
    {
        self.build_branching(name, cond, |ctx| Ok(((), then_fn(ctx)?)), else_fn)
            .map(|(((), _), (val, _))| val)
    }

    /// Generates a C-style `if`, similar to the following C code:
    ///
    /// ```txt
    /// if <cond> { <then_fn> }
    /// ```
    pub fn build_if<T, ThenFn>(
        &mut self,
        name: &str,
        cond: IntValue<'ctx>,
        then_fn: ThenFn,
    ) -> anyhow::Result<T>
    where
        ThenFn: FnOnce(&mut Self) -> anyhow::Result<T>,
    {
        self.build_if_else(name, cond, then_fn, |_, val| Ok(val))
    }
}

fn build_tuple_elem_switch<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    tuple: TupleValue<'ctx>,
    tuple_len: u64,
    element_ty: Option<Type>,
    start: IntValue<'ctx>,
    next_i: IntValue<'ctx>,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    let int32 = ctx.i32;
    let update_bb = ctx.builder.get_insert_block().unwrap();
    let merge_bb = ctx.ctx.insert_basic_block_after(update_bb, "tuple.end");
    let default_element_ty = ctx.get_llvm_type(element_ty.unwrap_or(ctx.primitives.int32));

    let mut tmp_bb = update_bb;
    let mut cases = Vec::new();
    for idx in 0..tuple_len {
        let case_bb = ctx.ctx.insert_basic_block_after(tmp_bb, &format!("tuple.case.{idx}"));
        cases.push((int32.const_int(idx, false), case_bb));
        tmp_bb = case_bb;
    }

    ctx.builder.build_switch(ctx.builder.build_int_sub(next_i, start, "sub")?, merge_bb, &cases)?;

    ctx.builder.position_at_end(merge_bb);
    let phi = ctx.builder.build_phi(default_element_ty, "tuple.elem.phi")?;

    for (idx, (_, case_bb)) in cases.iter().take(tuple_len as usize).enumerate() {
        ctx.builder.position_at_end(*case_bb);
        let elem_val = tuple.extract(ctx, idx as u32)?;
        ctx.builder.build_unconditional_branch(merge_bb)?;
        phi.add_incoming(&[(&elem_val, *case_bb)]);
    }

    ctx.builder.position_at_end(merge_bb);
    let default_value = default_element_ty.const_zero();
    phi.add_incoming(&[(&default_value, update_bb)]);
    Ok(phi.as_basic_value())
}

/// Generates a `for` statement with `enumerate(iterable)` as its iterable object.
///
/// * `element_ty` - The type of the iterable elements, if known.
/// * `length` - The length of the iterable.
/// * `start` - The starting index for enumeration.
/// * `target_expr` - The target expression to store the current element and/or the current index.
/// * `target_i` - The pointer to store the current index.
/// * `get_first_elem` - A closure that returns the first element of the iterable.
/// * `get_next_elem` - A closure that returns the next element given the next index.
/// * `body` - The body of the loop.
/// * `orelse` - The `else` block of the loop.
#[allow(clippy::too_many_arguments)]
fn gen_for_enumerate<'ctx, G: CodeGenerator, U>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    element_ty: Option<Type>,
    length: IntValue<'ctx>,
    start: IntValue<'ctx>,
    target_expr: &ExprKind<U>,
    target_i: PointerValue<'ctx>,
    get_first_elem: &dyn Fn(&mut CodeGenContext<'ctx, '_>) -> anyhow::Result<BasicValueEnum<'ctx>>,
    get_next_elem: &dyn Fn(
        &mut CodeGenContext<'ctx, '_>,
        IntValue<'ctx>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>>,
    body: &[Stmt<Option<Type>>],
    orelse: &[Stmt<Option<Type>>],
) -> anyhow::Result<()> {
    let int32 = ctx.i32;
    let default_element_ty = ctx.get_llvm_type(element_ty.unwrap_or(ctx.primitives.int32));

    let element_struct = ctx.ctx.struct_type(&[int32.into(), default_element_ty], false);
    let iv_pair = ctx.alloc(element_struct, Some("for.v.addr"))?;
    let i = ctx.builder.build_struct_gep(element_struct, iv_pair, 0, "i")?;
    ctx.builder.build_store(i, start)?;
    if element_ty.is_some() {
        let first_v = get_first_elem(ctx)?;
        let v = ctx.builder.build_struct_gep(element_struct, iv_pair, 1, "v")?;
        ctx.builder.build_store(v, first_v)?;
    }

    ctx.build_loop(
        "for.enumerate",
        |ctx, hooks| {
            let element_struct = ctx.ctx.struct_type(&[int32.into(), default_element_ty], false);
            let i = ctx.builder.build_struct_gep(element_struct, iv_pair, 0, "i")?;
            let i_val = ctx.builder.build_load(int32, i, "i_val")?.into_int_value();
            let in_range = gen_in_range_check(
                ctx,
                ctx.builder.build_int_sub(i_val, start, "sub")?,
                length,
                int32.const_int(1, false),
            )?;
            let finish = ctx.branch("for.enumerate", in_range)?;
            let element_struct = ctx.ctx.struct_type(&[int32.into(), default_element_ty], false);
            let target_struct_ty = ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.ptr.into()], false);
            match target_expr {
                ExprKind::Tuple { elts, .. } if elts.len() == 2 => {
                    let i = ctx.builder.build_struct_gep(element_struct, iv_pair, 0, "i")?;
                    let i_val = ctx.builder.build_load(int32, i, "i_val")?.into_int_value();
                    let ptr_1 =
                        ctx.builder.build_struct_gep(target_struct_ty, target_i, 0, "tuple.0")?;
                    let addr_1 = ctx
                        .builder
                        .build_load(ctx.ptr, ptr_1, "tuple.0.addr")?
                        .into_pointer_value();
                    ctx.builder.build_store(addr_1, i_val)?;
                    let v = ctx.builder.build_struct_gep(element_struct, iv_pair, 1, "v")?;
                    let v_val = ctx.builder.build_load(default_element_ty, v, "")?;
                    let ptr_2 =
                        ctx.builder.build_struct_gep(target_struct_ty, target_i, 1, "tuple.1")?;
                    let addr_2 = ctx
                        .builder
                        .build_load(ctx.ptr, ptr_2, "tuple.1.addr")?
                        .into_pointer_value();
                    ctx.builder.build_store(addr_2, v_val)?;
                }
                ExprKind::Name { .. } => {
                    // Load i and v from the internal iv_pair struct
                    let i = ctx.builder.build_struct_gep(element_struct, iv_pair, 0, "i")?;
                    let i_val = ctx.builder.build_load(int32, i, "i_val")?;
                    let v = ctx.builder.build_struct_gep(element_struct, iv_pair, 1, "v")?;
                    let v_val = ctx.builder.build_load(default_element_ty, v, "v_val")?;
                    // Construct a proper tuple (with ObjectHeader) from the values
                    let tuple_val = TupleValue::new(ctx, &[i_val, v_val], Some("iv"))?;
                    ctx.builder.build_store(target_i, tuple_val.value)?;
                }
                _ => codegen_unreachable!(
                    ctx,
                    "expected target expression of for enumerate to be a Name or a Tuple"
                ),
            }
            generator.gen_block(ctx, body.iter())?;
            ctx.in_block(finish, |ctx| {
                generator.gen_block(ctx, orelse.iter())?;
                hooks.build_break(&ctx.builder)?;
                anyhow::Ok(())
            })
        },
        |ctx, ()| {
            let element_struct = ctx.ctx.struct_type(&[int32.into(), default_element_ty], false);
            let i = ctx.builder.build_struct_gep(element_struct, iv_pair, 0, "i")?;
            let i_val = ctx.builder.build_load(int32, i, "i_val")?.into_int_value();
            let next_i = ctx.builder.build_int_add(i_val, int32.const_int(1, false), "inc")?;
            ctx.builder.build_store(i, next_i)?;
            if element_ty.is_some() {
                let next_v = get_next_elem(ctx, next_i)?;
                let v = ctx.builder.build_struct_gep(element_struct, iv_pair, 1, "v")?;
                ctx.builder.build_store(v, next_v)?;
            }
            Ok(())
        },
    )
}

/// See [`CodeGenerator::gen_for`].
pub fn gen_for<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::For { iter, target, body, orelse, .. } = &stmt.node else {
        codegen_unreachable!(ctx)
    };

    let int32 = ctx.i32;
    let size_t = ctx.size_t;

    let iter_ty = iter.custom.unwrap();
    let iter_val = generator.gen_expr(ctx, iter)?.to_basic_value_enum(ctx)?;

    match &*ctx.unifier.get_ty(iter_ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.range.obj_id(&ctx.unifier).unwrap() =>
        {
            let iter_val =
                RangeType::new(ctx).map_value(iter_val.into_pointer_value(), Some("range"));

            let (start, stop, step) = destructure_range(ctx, iter_val)?;

            // Check "If step is zero, ValueError is raised."
            let rangenez =
                ctx.builder.build_int_compare(IntPredicate::NE, step, int32.const_zero(), "")?;
            ctx.make_assert(
                rangenez,
                "0:ValueError",
                "range() arg 3 must not be zero",
                [None, None, None],
            )?;

            // Internal variable for loop; Cannot be assigned
            let i = ctx.alloc(int32, Some("for.i.addr"))?;
            // Variable declared in "target" expression of the loop; Can be reassigned *or* shadowed
            let Some(target_i) =
                generator.gen_store_target(ctx, target, Some("for.target.addr"))?
            else {
                codegen_unreachable!(ctx)
            };
            ctx.builder.build_store(i, start)?;

            ctx.build_loop(
                "for.range",
                |ctx, hooks| {
                    let in_range = gen_in_range_check(
                        ctx,
                        ctx.builder.build_load(int32, i, "")?.into_int_value(),
                        stop,
                        step,
                    )?;
                    let finish = ctx.branch("for.range", in_range)?;
                    ctx.builder.build_store(
                        target_i,
                        ctx.builder.build_load(int32, i, "")?.into_int_value(),
                    )?;
                    generator.gen_block(ctx, body.iter())?;

                    ctx.in_block(finish, |ctx| {
                        generator.gen_block(ctx, orelse.iter())?;
                        hooks.build_break(&ctx.builder)?;
                        anyhow::Ok(())
                    })
                },
                |ctx, ()| {
                    let next_i = ctx.builder.build_int_add(
                        ctx.builder.build_load(int32, i, "")?.into_int_value(),
                        step,
                        "inc",
                    )?;
                    ctx.builder.build_store(i, next_i)?;

                    Ok(())
                },
            )?;
        }
        TypeEnum::TObj { obj_id, params, .. }
            if *obj_id == ctx.primitives.enumerate.obj_id(&ctx.unifier).unwrap() =>
        {
            let enumerate =
                EnumerateType::new(ctx).map_value(iter_val.into_pointer_value(), Some("enumerate"));
            let start = enumerate.load(ctx, field!(start))?;
            let Some(target_i) =
                generator.gen_store_target(ctx, target, Some("for.target.addr"))?
            else {
                codegen_unreachable!(ctx)
            };
            let (iterable_ty, iterable_val) = if let ExprKind::Call { args, .. } = &iter.node {
                let ag = generator.gen_expr(ctx, &args[0])?.to_basic_value_enum(ctx)?;
                let iterable_ty = args[0].custom.unwrap();
                (iterable_ty, ag)
            } else {
                let iterable_ptr = enumerate.load(ctx, field!(iterable))?;
                let iterable_struct_ty =
                    ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.size_t.into()], false);
                let iterable_struct_val = ctx
                    .builder
                    .build_load(iterable_struct_ty, iterable_ptr, "iterable_struct")?
                    .into_struct_value();
                let iterable_data_i8ptr =
                    ctx.builder.build_extract_value(iterable_struct_val, 0, "iterable_data")?;
                let iterable_ty = iter_type_vars(params).nth(1).unwrap().ty;
                let iterable_llvm_ty = ctx.get_llvm_type(iterable_ty);
                let ag = ctx.builder.build_load(
                    iterable_llvm_ty,
                    iterable_data_i8ptr.into_pointer_value(),
                    "iterable_struct",
                )?;
                (iterable_ty, ag)
            };
            match &*ctx.unifier.get_ty(iterable_ty) {
                TypeEnum::TObj { obj_id, .. }
                    if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
                {
                    let list_ty = RawListType::from_unifier_type(ctx, iterable_ty);
                    let iterable = TypedRefCountedType::new(ctx, list_ty)
                        .map_value(iterable_val.into_pointer_value(), Some("list"));
                    let length_sizet = iterable.inner_value(ctx)?.load(ctx, field!(len))?;
                    let length = ctx.builder.build_int_truncate(length_sizet, int32, "length")?;
                    let val = arraylike_flatten_element_type(&mut ctx.unifier, iterable_ty);
                    let element_ty =
                        if ctx.unifier.is_concrete(val, &[]) { Some(val) } else { None };
                    gen_for_enumerate(
                        generator,
                        ctx,
                        element_ty,
                        length,
                        start,
                        &target.node,
                        target_i,
                        &|ctx| {
                            iterable
                                .inner_value(ctx)?
                                .data(ctx)?
                                .inner_value(ctx, Some(length_sizet))?
                                .get_unchecked(ctx, &int32.const_int(0, false), Some("first_v"))
                        },
                        &|ctx, next_i| {
                            iterable
                                .inner_value(ctx)?
                                .data(ctx)?
                                .inner_value(ctx, Some(length_sizet))?
                                .get_unchecked(
                                    ctx,
                                    &ctx.builder.build_int_sub(next_i, start, "sub")?,
                                    Some("next_v"),
                                )
                        },
                        body,
                        orelse,
                    )?;
                }

                TypeEnum::TTuple { ty: tuple_tys, .. } => {
                    let iterable = TupleType::from_unifier_type(ctx, iterable_ty)
                        .map_value(iterable_val.into_struct_value(), Some("tuple"));
                    let element_ty = if tuple_tys.is_empty() { None } else { Some(tuple_tys[0]) };
                    let length = int32.const_int(tuple_tys.len() as u64, false);
                    let tuple_len = tuple_tys.len() as u64;
                    gen_for_enumerate(
                        generator,
                        ctx,
                        element_ty,
                        length,
                        start,
                        &target.node,
                        target_i,
                        &|ctx| iterable.extract(ctx, 0),
                        &|ctx, next_i| {
                            build_tuple_elem_switch(
                                ctx, iterable, tuple_len, element_ty, start, next_i,
                            )
                        },
                        body,
                        orelse,
                    )?;
                }
                _ => {
                    bail!(
                        "enumerate() with unsupported iterable type: {:?} (at {})",
                        ctx.unifier.get_ty(iterable_ty),
                        iter.location
                    );
                }
            }
        }
        TypeEnum::TObj { obj_id, params: list_params, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            let list_elem_ty = iter_type_vars(list_params).next().unwrap().ty;
            let list_ty = RawListType::new(ctx, list_elem_ty);
            let iter_val = TypedRefCountedType::new(ctx, list_ty)
                .map_value(iter_val.into_pointer_value(), Some("list"));

            let len = iter_val.inner_value(ctx)?.load(ctx, field!(len))?;

            let index_addr = ctx.alloc(size_t, Some("for.index.addr"))?;
            ctx.builder.build_store(index_addr, size_t.const_zero())?;

            ctx.build_loop(
                "for.list",
                |ctx, hooks| {
                    let index =
                        ctx.builder.build_load(size_t, index_addr, "for.index")?.into_int_value();
                    let cmp =
                        ctx.builder.build_int_compare(IntPredicate::SLT, index, len, "cond")?;
                    let finish = ctx.branch("for.list", cmp)?;
                    let index =
                        ctx.builder.build_load(size_t, index_addr, "for.index")?.into_int_value();
                    let val: BasicValueEnum = iter_val
                        .inner_value(ctx)?
                        .data(ctx)?
                        .inner_value(ctx, Some(len))?
                        .get_unchecked(ctx, &index, Some("val"))?;
                    let val_ty = iter_type_vars(list_params).next().unwrap().ty;
                    generator.gen_assign(ctx, target, &val.into(), val_ty)?;
                    generator.gen_block(ctx, body.iter())?;

                    ctx.in_block(finish, |ctx| {
                        generator.gen_block(ctx, orelse.iter())?;
                        hooks.build_break(&ctx.builder)?;
                        anyhow::Ok(())
                    })?;
                    Ok(index)
                },
                |ctx, index| {
                    let inc = ctx.builder.build_int_add(index, size_t.const_int(1, true), "inc")?;
                    ctx.builder.build_store(index_addr, inc)?;

                    Ok(())
                },
            )?;
        }
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
        {
            let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, iter_ty);
            let ndims = extract_ndims(&ctx.unifier, ndims);
            let ndarray = NDArrayType::from_unifier_type(ctx, iter_ty)
                .map_value(iter_val.into_pointer_value(), None);

            let shape_dim0 = ndarray.len(ctx)?;
            let index_addr = ctx.alloc(size_t, Some("for.index.addr"))?;
            ctx.builder.build_store(index_addr, size_t.const_zero())?;

            ctx.build_loop(
                "for.ndarray",
                |ctx, hooks| {
                    let index =
                        ctx.builder.build_load(size_t, index_addr, "for.index")?.into_int_value();
                    let cmp = ctx.builder.build_int_compare(
                        IntPredicate::SLT,
                        index,
                        shape_dim0,
                        "cond",
                    )?;
                    let finish = ctx.branch("for.ndarray", cmp)?;

                    let val = ndarray
                        .index(
                            ctx,
                            &[RustNDIndex::SingleElement(
                                ctx.builder.build_int_truncate_or_bit_cast(index, int32, "")?,
                            )],
                        )?
                        .split_unsized(ctx)?
                        .to_basic_value_enum();

                    let val_ty = if ndims == 1 {
                        dtype
                    } else {
                        let new_ndims =
                            ctx.unifier.get_fresh_literal(vec![SymbolValue::U64(ndims - 1)], None);
                        make_ndarray_ty(
                            &mut ctx.unifier,
                            &ctx.primitives,
                            Some(dtype),
                            Some(new_ndims),
                        )
                    };

                    generator.gen_assign(ctx, target, &val.into(), val_ty)?;
                    generator.gen_block(ctx, body.iter())?;

                    ctx.in_block(finish, |ctx| {
                        generator.gen_block(ctx, orelse.iter())?;
                        hooks.build_break(&ctx.builder)?;
                        anyhow::Ok(())
                    })
                },
                |ctx, ()| {
                    let index = ctx.builder.build_load(size_t, index_addr, "")?.into_int_value();
                    let inc =
                        ctx.builder.build_int_add(index, size_t.const_int(1, false), "inc")?;
                    ctx.builder.build_store(index_addr, inc)?;
                    Ok(())
                },
            )?;
        }
        _ => {
            panic!("unsupported for loop iterator type: {}", ctx.unifier.stringify(iter_ty));
        }
    }

    Ok(())
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub struct BreakContinueHooks<'ctx> {
    /// The [exit block][`BasicBlock`] to branch to when `break`-ing out of a loop.
    exit_bb: BasicBlock<'ctx>,

    /// The [latch basic block][`BasicBlock`] to branch to for `continue`-ing to the next iteration
    /// of the loop.
    latch_bb: BasicBlock<'ctx>,
}

impl<'ctx> BreakContinueHooks<'ctx> {
    /// Creates a [`br` instruction][Builder::build_unconditional_branch] to the exit
    /// [`BasicBlock`], as if by calling `break`.
    ///
    /// If the block is already terminated, this is a no-op.
    pub fn build_break(&self, builder: &Builder<'ctx>) -> anyhow::Result<()> {
        if !builder_is_terminated(builder) {
            builder.build_unconditional_branch(self.exit_bb)?;
        }
        Ok(())
    }

    /// Creates a [`br` instruction][Builder::build_unconditional_branch] to the latch
    /// [`BasicBlock`], as if by calling `continue`.
    ///
    /// If the block is already terminated, this is a no-op.
    pub fn build_continue(&self, builder: &Builder<'ctx>) -> anyhow::Result<()> {
        if !builder_is_terminated(builder) {
            builder.build_unconditional_branch(self.latch_bb)?;
        }
        Ok(())
    }
}

/// See [`CodeGenerator::gen_while`].
pub fn gen_while<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::While { test, body, orelse, .. } = &stmt.node else { codegen_unreachable!(ctx) };

    ctx.build_loop(
        "while",
        |ctx, hooks| {
            let cond = generator.gen_expr(ctx, test)?.to_i1(ctx)?;
            let finish = ctx.branch("while", cond)?;
            generator.gen_block(ctx, body.iter())?;
            ctx.in_block(finish, |ctx| {
                generator.gen_block(ctx, orelse.iter())?;
                hooks.build_break(&ctx.builder)?;
                anyhow::Ok(())
            })
        },
        |_, ()| Ok(()),
    )?;

    Ok(())
}

/// See [`CodeGenerator::gen_if`].
pub fn gen_if<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::If { test, body, orelse, .. } = &stmt.node else { codegen_unreachable!(ctx) };

    let test = generator.gen_expr(ctx, test)?.to_i1(ctx)?;
    ctx.build_if_else(
        "if",
        test,
        |ctx| generator.gen_block(ctx, body.iter()).map(|()| generator),
        |ctx, generator| generator.gen_block(ctx, orelse.iter()),
    )
}

pub fn final_proxy<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    target: BasicBlock<'ctx>,
    block: BasicBlock<'ctx>,
    final_data: &mut (PointerValue, Vec<BasicBlock<'ctx>>, Vec<BasicBlock<'ctx>>),
) -> anyhow::Result<()> {
    let (final_state, final_targets, final_paths) = final_data;
    let prev = ctx.builder.get_insert_block().unwrap();
    ctx.builder.position_at_end(block);
    unsafe {
        ctx.builder.build_store(*final_state, target.get_address().unwrap())?;
    }
    ctx.builder.position_at_end(prev);
    final_targets.push(target);
    final_paths.push(block);
    Ok(())
}

/// Inserts the declaration of the builtin function with the specified `symbol` name, and returns
/// the function.
pub fn get_builtins<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, symbol: &str) -> FunctionDecl<'ctx> {
    let raise_arg = [ctx.get_llvm_type(ctx.primitives.exception)];
    let noreturn = ["noreturn"];
    ctx.declare_external(
        symbol,
        None,
        match symbol {
            "__nac3_raise" => &raise_arg,
            "__nac3_resume" | "__nac3_end_catch" => &[],
            _ => unimplemented!(),
        },
        false,
        match symbol {
            "__nac3_raise" | "__nac3_resume" => &noreturn,
            _ => &[],
        },
    )
}

pub fn exn_constructor<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    _fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
) -> anyhow::Result<Option<BasicValueEnum<'ctx>>> {
    let (zelf_ty, zelf) = obj.unwrap();
    let zelf = zelf.to_basic_value_enum(ctx, zelf_ty)?.into_pointer_value();
    let zelf_id = if let TypeEnum::TObj { obj_id, .. } = &*ctx.unifier.get_ty(zelf_ty) {
        obj_id.0
    } else {
        codegen_unreachable!(ctx)
    };
    let defs = &ctx.top_level.definitions;
    let TopLevelDef::Class { name: zelf_name, .. } = &*defs[zelf_id].read() else {
        codegen_unreachable!(ctx)
    };

    let mut args = args.into_iter();

    let zelf = ExceptionType::new(ctx).map_value(zelf, Some("exn"));
    let exception_name = format!("{}:{}", ctx.resolver.get_exception_id(zelf_id), zelf_name);

    let id = ctx.resolver.get_string_id(&exception_name);
    zelf.store(ctx, field!(name), ctx.i32.const_int(id as u64, false))?;

    let empty_string = StringType::new(ctx).constant(ctx, "", None)?.value;

    let msg = match args.next() {
        Some((_, v)) => v.to_basic_value_enum(ctx, ctx.primitives.str)?.try_into().unwrap(),
        None => empty_string,
    };
    zelf.store(ctx, field!(message), msg)?;

    let [param0, param1, param2] = std::array::from_fn(|_| {
        anyhow::Ok(match args.next() {
            Some((_, v)) => v.to_basic_value_enum(ctx, ctx.primitives.int64)?.try_into().unwrap(),
            None => ctx.i64.const_zero(),
        })
    });
    let [param0, param1, param2] = [param0?, param1?, param2?];

    zelf.store(ctx, field!(param0), param0)?;
    zelf.store(ctx, field!(param1), param1)?;
    zelf.store(ctx, field!(param2), param2)?;

    zelf.store(ctx, field!(file), empty_string)?;
    zelf.store(ctx, field!(func), empty_string)?;

    zelf.store(ctx, field!(line), ctx.i32.const_zero())?;
    zelf.store(ctx, field!(col), ctx.i32.const_zero())?;

    Ok(Some(zelf.value.into()))
}

/// Generates IR for a `raise` statement.
///
/// * `exception` - The exception thrown by the `raise` statement.
/// * `loc` - The location where the exception is raised from.
pub fn gen_raise<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    exception: Option<&ExceptionValue<'ctx>>,
) -> anyhow::Result<()> {
    let loc = ctx.current_loc;
    if let Some(exception) = exception {
        // exception.store(ctx, field!(location), loc);
        let file = ctx.gen_string(loc.file.0)?;
        let row = ctx.i32.const_int(loc.row as u64, false);
        let col = ctx.i32.const_int(loc.column as u64, false);
        exception.store(ctx, field!(file), file)?;
        exception.store(ctx, field!(line), row)?;
        exception.store(ctx, field!(col), col)?;

        let current_fun = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
        let fun_name = ctx.gen_string(current_fun.get_name().to_str().unwrap())?;
        exception.store(ctx, field!(func), fun_name)?;

        let raise = get_builtins(ctx, "__nac3_raise");
        ctx.build_call_or_invoke(&raise, &[exception.value.into()], "raise")?;
    } else {
        let resume = get_builtins(ctx, "__nac3_resume");
        ctx.build_call_or_invoke(&resume, &[], "resume")?;
    }
    ctx.builder.build_unreachable()?;
    Ok(())
}

/// Generates IR for a `try` statement.
pub fn gen_try<'ctx, 'a, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    target: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::Try { body, handlers, orelse, finalbody, .. } = &target.node else {
        codegen_unreachable!(ctx)
    };

    // if we need to generate anything related to exception, we must have personality defined
    let personality = get_personality(ctx.top_level, ctx).unwrap();
    let exception_type = ctx.get_llvm_type(ctx.primitives.exception);
    let ptr_type = ctx.ptr;
    let current_block = ctx.builder.get_insert_block().unwrap();
    let current_fun = current_block.get_parent().unwrap();
    let landingpad = ctx.ctx.append_basic_block(current_fun, "try.landingpad");
    let dispatcher = ctx.ctx.append_basic_block(current_fun, "try.dispatch");
    let mut dispatcher_end = dispatcher;
    ctx.builder.position_at_end(dispatcher);
    let exn = ctx.builder.build_phi(exception_type, "exn")?;
    ctx.builder.position_at_end(current_block);

    let mut cleanup = None;
    let mut old_loop_target = None;
    let mut old_return = None;
    let mut final_data = None;
    let has_cleanup = !finalbody.is_empty();
    if has_cleanup {
        let final_state = ctx.alloc(ptr_type, Some("try.final_state.addr"))?;
        final_data = Some((final_state, Vec::new(), Vec::new()));
        if let Some((continue_target, break_target)) = ctx.loop_target {
            let break_proxy = ctx.ctx.append_basic_block(current_fun, "try.break");
            let continue_proxy = ctx.ctx.append_basic_block(current_fun, "try.continue");
            final_proxy(ctx, break_target, break_proxy, final_data.as_mut().unwrap())?;
            final_proxy(ctx, continue_target, continue_proxy, final_data.as_mut().unwrap())?;
            old_loop_target = ctx.loop_target.replace((continue_proxy, break_proxy));
        }
        let return_proxy = ctx.ctx.append_basic_block(current_fun, "try.return");
        let return_target = ctx.return_target.unwrap();
        final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap())?;
        old_return = ctx.return_target.replace(return_proxy);
        cleanup = Some(ctx.ctx.append_basic_block(current_fun, "try.cleanup"));
    }

    let mut clauses = Vec::new();
    let mut found_catch_all = false;
    for handler_node in handlers {
        let ExcepthandlerKind::ExceptHandler { type_, .. } = &handler_node.node;
        // none or Exception
        if type_.is_none()
            || ctx
                .unifier
                .unioned(type_.as_ref().and_then(|t| t.custom).unwrap(), ctx.primitives.exception)
        {
            clauses.push(None);
            found_catch_all = true;
            break;
        }

        let type_ = type_.as_ref().unwrap();
        let exn_name = ctx.resolver.get_type_name(
            &ctx.top_level.definitions,
            &mut ctx.unifier,
            type_.custom.unwrap(),
        );
        let obj_id =
            if let TypeEnum::TObj { obj_id, .. } = &*ctx.unifier.get_ty(type_.custom.unwrap()) {
                *obj_id
            } else {
                codegen_unreachable!(ctx)
            };
        let exception_name = format!("{}:{}", ctx.resolver.get_exception_id(obj_id.0), exn_name);
        let exn_id = ctx.resolver.get_string_id(&exception_name);
        let exn_id_global = ctx.module.add_global(ctx.i32, None, &format!("exn.{exn_id}"));
        exn_id_global.set_linkage(Linkage::WeakAny);
        exn_id_global.set_initializer(&ctx.i32.const_int(exn_id as u64, false));
        clauses.push(Some(exn_id_global.as_pointer_value().as_basic_value_enum()));
    }
    let mut all_clauses = clauses.clone();
    if let Some(old_clauses) = &ctx.outer_catch_clauses
        && !found_catch_all
    {
        all_clauses.extend_from_slice(&old_clauses.0);
    }
    let old_clauses = ctx.outer_catch_clauses.replace((all_clauses, dispatcher, exn));
    let old_unwind = ctx.unwind_target.replace(landingpad);
    generator.gen_block(ctx, body.iter())?;
    if ctx.builder.get_insert_block().unwrap().get_terminator().is_none() {
        generator.gen_block(ctx, orelse.iter())?;
    }
    let body = ctx.builder.get_insert_block().unwrap();
    // reset old_clauses and old_unwind
    let (all_clauses, _, _) = ctx.outer_catch_clauses.take().unwrap();
    ctx.outer_catch_clauses = old_clauses;
    ctx.unwind_target = old_unwind;
    if has_cleanup {
        ctx.return_target = old_return;
    }
    ctx.loop_target = old_loop_target.or(ctx.loop_target);

    let old_unwind = if finalbody.is_empty() {
        old_unwind
    } else {
        let final_landingpad = ctx.ctx.append_basic_block(current_fun, "try.catch.final");
        ctx.builder.position_at_end(final_landingpad);
        ctx.builder.build_landing_pad(
            ctx.ctx.struct_type(&[ptr_type.into(), exception_type], false),
            personality,
            &[],
            true,
            "try.catch.final",
        )?;
        ctx.builder.build_unconditional_branch(cleanup.unwrap())?;
        ctx.builder.position_at_end(body);
        ctx.unwind_target.replace(final_landingpad)
    };

    // run end_catch before continue/break/return
    let mut final_proxy_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, target: BasicBlock<'ctx>, block: BasicBlock<'ctx>| {
            final_proxy(ctx, target, block, final_data.as_mut().unwrap())
        };
    let mut redirect_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, target: BasicBlock<'ctx>, block: BasicBlock<'ctx>| {
            ctx.builder.position_at_end(block);
            ctx.builder.build_unconditional_branch(target)?;
            ctx.builder.position_at_end(body);
            Ok(())
        };
    let redirect = if has_cleanup {
        &mut final_proxy_lambda
            as &mut dyn FnMut(
                &mut CodeGenContext<'ctx, 'a>,
                BasicBlock<'ctx>,
                BasicBlock<'ctx>,
            ) -> anyhow::Result<()>
    } else {
        &mut redirect_lambda
            as &mut dyn FnMut(
                &mut CodeGenContext<'ctx, 'a>,
                BasicBlock<'ctx>,
                BasicBlock<'ctx>,
            ) -> anyhow::Result<()>
    };
    let resume = get_builtins(ctx, "__nac3_resume");
    let end_catch = get_builtins(ctx, "__nac3_end_catch");
    if let Some((continue_target, break_target)) = ctx.loop_target.take() {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "try.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "try.continue");
        ctx.builder.position_at_end(break_proxy);
        ctx.build_call(&end_catch, &[], "end_catch")?;
        ctx.builder.position_at_end(continue_proxy);
        ctx.build_call(&end_catch, &[], "end_catch")?;
        ctx.builder.position_at_end(body);
        redirect(ctx, break_target, break_proxy)?;
        redirect(ctx, continue_target, continue_proxy)?;
        ctx.loop_target = Some((continue_proxy, break_proxy));
        old_loop_target = Some((continue_target, break_target));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "try.return");
    ctx.builder.position_at_end(return_proxy);
    ctx.build_call(&end_catch, &[], "end_catch")?;
    let return_target = ctx.return_target.take().unwrap();
    redirect(ctx, return_target, return_proxy)?;
    ctx.return_target = Some(return_proxy);
    old_return = Some(return_target);

    let mut post_handlers = Vec::new();

    let exnid = if handlers.is_empty() {
        None
    } else {
        ctx.builder.position_at_end(dispatcher);
        let exn = exn.as_basic_value().into_pointer_value();
        let exn = ExceptionType::new(ctx).map_value(exn, Some("exn"));
        Some(exn.load(ctx, field!(name))?)
    };

    for (handler_node, exn_type) in handlers.iter().zip(clauses.iter()) {
        let ExcepthandlerKind::ExceptHandler { type_, name, body } = &handler_node.node;
        let handler_bb = ctx.ctx.append_basic_block(current_fun, "try.handler");
        ctx.builder.position_at_end(handler_bb);
        if let Some(name) = name {
            let exn_ty = ctx.get_llvm_type(type_.as_ref().unwrap().custom.unwrap());
            let exn_store = ctx.alloc(exn_ty, Some("try.exn_store.addr"))?;
            ctx.var_assignment
                .insert(*name, VarValue::new(exn_store, type_.as_ref().unwrap().custom.unwrap()));
            ctx.builder.build_store(exn_store, exn.as_basic_value())?;
        }
        generator.gen_block(ctx, body.iter())?;
        let current = ctx.builder.get_insert_block().unwrap();
        // only need to call end catch if not terminated
        // otherwise, we already handled in return/break/continue/raise
        if current.get_terminator().is_none() {
            ctx.build_call(&end_catch, &[], "end_catch")?;
        }
        post_handlers.push(current);
        ctx.builder.position_at_end(dispatcher_end);
        if let Some(exn_type) = exn_type {
            let dispatcher_cont = ctx.ctx.append_basic_block(current_fun, "try.dispatch.cont");
            let actual_id = exnid.unwrap();
            let expected_id = ctx
                .builder
                .build_load(ctx.i32, exn_type.into_pointer_value(), "expected_id")?
                .into_int_value();
            let result = ctx.builder.build_int_compare(
                IntPredicate::EQ,
                actual_id,
                expected_id,
                "exncheck",
            )?;
            ctx.builder.build_conditional_branch(result, handler_bb, dispatcher_cont)?;
            dispatcher_end = dispatcher_cont;
        } else {
            ctx.builder.build_unconditional_branch(handler_bb)?;
            break;
        }
    }

    ctx.unwind_target = old_unwind;
    ctx.loop_target = old_loop_target.or(ctx.loop_target);
    ctx.return_target = old_return;

    ctx.builder.position_at_end(landingpad);
    let clauses: Vec<_> = if finalbody.is_empty() { &all_clauses } else { &clauses }
        .iter()
        .map(|v| v.unwrap_or(ptr_type.const_zero().into()))
        .collect();
    let landingpad_value = ctx
        .builder
        .build_landing_pad(
            ctx.ctx.struct_type(&[ptr_type.into(), exception_type], false),
            personality,
            &clauses,
            has_cleanup,
            "try.landingpad",
        )?
        .into_struct_value();
    let exn_val = ctx.builder.build_extract_value(landingpad_value, 1, "exn")?;
    ctx.builder.build_unconditional_branch(dispatcher)?;
    exn.add_incoming(&[(&exn_val, landingpad)]);

    if dispatcher_end.get_terminator().is_none() {
        ctx.builder.position_at_end(dispatcher_end);
        if let Some(cleanup) = cleanup {
            ctx.builder.build_unconditional_branch(cleanup)?;
        } else if let Some((_, outer_dispatcher, phi)) = ctx.outer_catch_clauses {
            phi.add_incoming(&[(&exn_val, dispatcher_end)]);
            ctx.builder.build_unconditional_branch(outer_dispatcher)?;
        } else {
            ctx.build_call_or_invoke(&resume, &[], "resume")?;
            ctx.builder.build_unreachable()?;
        }
    }

    if finalbody.is_empty() {
        let tail = ctx.ctx.append_basic_block(current_fun, "try.tail");
        if body.get_terminator().is_none() {
            ctx.builder.position_at_end(body);
            ctx.builder.build_unconditional_branch(tail)?;
        }
        if matches!(cleanup, Some(cleanup) if cleanup.get_terminator().is_none()) {
            ctx.builder.position_at_end(cleanup.unwrap());
            ctx.builder.build_unconditional_branch(tail)?;
        }
        for post_handler in post_handlers {
            if post_handler.get_terminator().is_none() {
                ctx.builder.position_at_end(post_handler);
                ctx.builder.build_unconditional_branch(tail)?;
            }
        }
        ctx.builder.position_at_end(tail);
    } else {
        // exception path
        let cleanup = cleanup.unwrap();
        ctx.builder.position_at_end(cleanup);
        generator.gen_block(ctx, finalbody.iter())?;
        if !ctx.is_terminated() {
            ctx.build_call_or_invoke(&resume, &[], "resume")?;
            ctx.builder.build_unreachable()?;
        }

        // normal path
        let (final_state, mut final_targets, final_paths) = final_data.unwrap();
        let tail = ctx.ctx.append_basic_block(current_fun, "try.tail");
        final_targets.push(tail);
        let finalizer = ctx.ctx.append_basic_block(current_fun, "try.finally");
        ctx.builder.position_at_end(finalizer);
        generator.gen_block(ctx, finalbody.iter())?;
        if !ctx.is_terminated() {
            let dest = ctx.builder.build_load(ptr_type, final_state, "final_dest")?;
            ctx.builder.build_indirect_branch(dest, &final_targets)?;
        }
        for block in &final_paths {
            if block.get_terminator().is_none() {
                ctx.builder.position_at_end(*block);
                ctx.builder.build_unconditional_branch(finalizer)?;
            }
        }
        for block in once(&body).chain(post_handlers.iter()) {
            if block.get_terminator().is_none() {
                ctx.builder.position_at_end(*block);
                unsafe {
                    ctx.builder.build_store(final_state, tail.get_address().unwrap())?;
                }
                ctx.builder.build_unconditional_branch(finalizer)?;
            }
        }
        ctx.builder.position_at_end(tail);
    }

    Ok(())
}

/// The method information for a `with` statement's `__enter__` method call.
///
/// This is used to generate the call to `__enter__` and the optional `as` binding.
struct WithEnterMethodInfo<'ctx> {
    obj_ty: Type,
    obj: ValueEnum<'ctx>,
    signature: FunSignature,
    fun_id: DefinitionId,
    optional_vars: Option<Box<Expr<Option<Type>>>>,
}

/// The method information for a `with` statement's `__exit__` method call.
///
/// This is used to generate the call to `__exit__`.
struct WithExitMethodInfo<'ctx> {
    obj_ty: Type,
    obj: ValueEnum<'ctx>,
    signature: FunSignature,
    fun_id: DefinitionId,
}

/// The context manager of a `with` statement, which may be either a method call or a critical
/// region.
enum CtxManager<'ctx> {
    /// A context manager object with `__enter__` and `__exit__` methods.
    Method { enter: WithEnterMethodInfo<'ctx>, exit: WithExitMethodInfo<'ctx> },

    /// A `with critical(...):` region, which reserves a number of free pages for the duration of
    /// the `with` block.
    #[cfg(feature = "ctrc")]
    Critical { num_free_pages: IntValue<'ctx> },
}

/// Evaluates the page count of a `with critical(...):` item, or materializes
/// [`CTRC_DEFAULT_RESERVED_PAGES`] if the argument is omitted.
///
/// The context expression is deliberately *not* evaluated to avoid a heap allocation of the
/// `critical` object - only the argument subexpression is generated.
///
/// Note that this function must be called **before** entering the body of the `with` statement.
/// Otherwise, if the page count assertion fires, this causes the `landingpad` (and thus
/// `__nac3_ctrc_exit`) to be invoked, causing a mismatched CTRC depth.
///
/// Raises a `ValueError` if the page count is negative.
#[cfg(feature = "ctrc")]
fn gen_critical_num_free_pages<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    context_expr: &Expr<Option<Type>>,
) -> anyhow::Result<IntValue<'ctx>> {
    use crate::codegen::allocator::CTRC_DEFAULT_RESERVED_PAGES;

    let ExprKind::Call { args, keywords, .. } = &context_expr.node else {
        // `critical` is a class, this must be a constructor call
        codegen_unreachable!(ctx)
    };

    let num_free_pages_expr = args.first().or_else(|| {
        keywords
            .iter()
            .find(|kw| kw.node.arg == Some("num_free_pages".into()))
            .map(|kw| &*kw.node.value)
    });

    let num_free_pages = match num_free_pages_expr {
        Some(expr) => generator.gen_expr(ctx, expr)?.to_basic_value_enum(ctx)?.into_int_value(),
        None => {
            // since `critical` bypasses the default-argument machinery, we materialize the default
            // value here
            ctx.i32.const_int(CTRC_DEFAULT_RESERVED_PAGES as u64, true)
        }
    };

    let is_non_negative = ctx.builder.build_int_compare(
        IntPredicate::SGE,
        num_free_pages,
        num_free_pages.get_type().const_zero(),
        "critical.num_free_pages.sge",
    )?;
    let num_free_pages_sext =
        ctx.builder.build_int_s_extend_or_bit_cast(num_free_pages, ctx.i64, "")?;
    ctx.make_assert(
        is_non_negative,
        "0:ValueError",
        "critical() expects a non-negative page count, got {0}",
        [Some(num_free_pages_sext), None, None],
    )?;

    Ok(num_free_pages)
}

/// See [`CodeGenerator::gen_with`].
pub fn gen_with<'ctx, 'a, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::With { items, body, .. } = &stmt.node else { codegen_unreachable!(ctx) };
    let mut ctx_mgrs = Vec::new();

    // prepare enters and exits
    for item in items {
        let expr_ty = item.context_expr.custom.unwrap();

        // `critical` is matched by definition ID *before* the context expression is evaluated:
        // constructing a `critical` object would be a heap allocation at the boundary of the region
        // whose allocation behavior is being changed. The type inferencer has already rejected the
        // multi-item and `as`-bound forms, so only the size argument remains to be generated.
        #[cfg(feature = "ctrc")]
        if matches!(&*ctx.unifier.get_ty(expr_ty), TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Critical.id())
        {
            let num_free_pages = gen_critical_num_free_pages(generator, ctx, &item.context_expr)?;

            ctx_mgrs.push(CtxManager::Critical { num_free_pages });
            continue;
        }

        // evaluate the expression first
        let expr = generator.gen_expr(ctx, &item.context_expr)?.val.unwrap();

        // get the __enter__ method signature and ID
        let TypeEnum::TObj { obj_id, fields, .. } = &*ctx.unifier.get_ty(expr_ty) else {
            codegen_unreachable!(ctx)
        };
        let top_level_defs = &ctx.top_level.definitions;
        let TopLevelDef::Class { methods, .. } = &*top_level_defs[obj_id.0].read() else {
            codegen_unreachable!(ctx)
        };
        let enter_fun_id = methods
            .iter()
            .find(|method| method.0 == "__enter__".into())
            .map(|method| method.2)
            .unwrap();
        let enter = fields[&"__enter__".into()];
        let TypeEnum::TFunc(enter_signature) = &*ctx.unifier.get_ty(enter.0) else {
            codegen_unreachable!(ctx)
        };

        // save __exit__() data to be called later in final stage
        let exit_fun_id = methods
            .iter()
            .find(|method| method.0 == "__exit__".into())
            .map(|method| method.2)
            .unwrap();
        let exit = fields[&"__exit__".into()];
        let TypeEnum::TFunc(exit_signature) = &*ctx.unifier.get_ty(exit.0) else {
            codegen_unreachable!(ctx)
        };

        // stack the exits as the exit order is opposite of enter
        // would be best to reuse try...finally but re-building Stmt vec seems infeasible
        ctx_mgrs.push(CtxManager::Method {
            enter: WithEnterMethodInfo {
                obj_ty: expr_ty,
                obj: expr.clone(),
                signature: enter_signature.clone(),
                fun_id: enter_fun_id,
                optional_vars: item.optional_vars.clone(),
            },
            exit: WithExitMethodInfo {
                obj_ty: expr_ty,
                obj: expr,
                signature: exit_signature.clone(),
                fun_id: exit_fun_id,
            },
        });
    }

    let body_gen_lambda = |ctx: &mut CodeGenContext<'ctx, 'a>, generator: &mut G| {
        for ctx_mgr in &ctx_mgrs {
            match ctx_mgr {
                CtxManager::Method {
                    enter: WithEnterMethodInfo { obj_ty, obj, signature, fun_id, optional_vars },
                    ..
                } => {
                    // call __enter__()
                    let enter_ret = generator.gen_call(
                        ctx,
                        Some((*obj_ty, obj.clone())),
                        (signature, *fun_id),
                        Vec::default(),
                    )?;

                    // deal with assignments (`as`)
                    if let Some(optional_vars) = optional_vars {
                        generator.gen_assign(
                            ctx,
                            optional_vars,
                            &enter_ret.unwrap().into(),
                            signature.ret,
                        )?;
                    }
                }

                #[cfg(feature = "ctrc")]
                CtxManager::Critical { num_free_pages } => {
                    let num_free_pages = ctx.builder.build_int_z_extend_or_bit_cast(
                        *num_free_pages,
                        ctx.size_t,
                        "",
                    )?;
                    // Note: The failure to reserve is not an error, since there may still be
                    // available cells for allocation. Actual allocation errors will be caught and
                    // reported at the point of allocation.
                    // `__nac3_ctrc_enter` must not raise either, since this would leave the CTRC
                    // mode depth unbalanced.
                    call_extern!(ctx: void _ = "__nac3_ctrc_enter"(num_free_pages))?;
                }
            }
        }

        // generate the `with` body
        generator.gen_block(ctx, body.iter())
    };

    let exit_gen_lambda = |ctx: &mut CodeGenContext<'ctx, 'a>, generator: &mut G| {
        // call __exit__()s in the reverse order
        for ctx_mgr in ctx_mgrs.iter().rev() {
            match ctx_mgr {
                CtxManager::Method {
                    exit: WithExitMethodInfo { obj_ty, obj, signature, fun_id },
                    ..
                } => {
                    generator.gen_call(
                        ctx,
                        Some((*obj_ty, obj.clone())),
                        (signature, *fun_id),
                        Vec::default(),
                    )?;
                }

                #[cfg(feature = "ctrc")]
                CtxManager::Critical { .. } => {
                    call_extern!(ctx: void _ = "__nac3_ctrc_exit"())?;
                }
            }
        }
        anyhow::Ok(())
    };

    // copied and trimmed from gen_try, to cover try (setup, enter)..finally (exit)
    let personality = get_personality(ctx.top_level, ctx).unwrap();
    let exception_type = ctx.get_llvm_type(ctx.primitives.exception);
    let ptr_type = ctx.ptr;
    let current_block = ctx.builder.get_insert_block().unwrap();
    let current_fun = current_block.get_parent().unwrap();
    let landingpad = ctx.ctx.append_basic_block(current_fun, "with.landingpad");
    let dispatcher = ctx.ctx.append_basic_block(current_fun, "with.dispatch");
    let dispatcher_end = dispatcher;
    ctx.builder.position_at_end(dispatcher);
    let exn = ctx.builder.build_phi(exception_type, "exn")?;
    ctx.builder.position_at_end(current_block);

    let mut old_loop_target = None;
    let final_state = ctx.alloc(ptr_type, Some("with.final_state.addr"))?;
    let mut final_data = Some((final_state, Vec::new(), Vec::new()));
    if let Some((continue_target, break_target)) = ctx.loop_target {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "with.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "with.continue");
        final_proxy(ctx, break_target, break_proxy, final_data.as_mut().unwrap())?;
        final_proxy(ctx, continue_target, continue_proxy, final_data.as_mut().unwrap())?;
        old_loop_target = ctx.loop_target.replace((continue_proxy, break_proxy));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "with.return");
    let return_target = ctx.return_target.unwrap();
    final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap())?;
    let old_return = ctx.return_target.replace(return_proxy);
    let cleanup = ctx.ctx.append_basic_block(current_fun, "with.cleanup");

    // replace unwind target, clauses stay the same
    let old_unwind = ctx.unwind_target.replace(landingpad);
    body_gen_lambda(ctx, generator)?;
    let body = ctx.builder.get_insert_block().unwrap();
    // reset old_unwind
    ctx.unwind_target = old_unwind;
    ctx.return_target = old_return;
    ctx.loop_target = old_loop_target.or(ctx.loop_target);

    let final_landingpad = ctx.ctx.append_basic_block(current_fun, "with.catch.final");
    ctx.builder.position_at_end(final_landingpad);
    ctx.builder.build_landing_pad(
        ctx.ctx.struct_type(&[ptr_type.into(), exception_type], false),
        personality,
        &[],
        true,
        "with.catch.final",
    )?;
    ctx.builder.build_unconditional_branch(cleanup)?;
    ctx.builder.position_at_end(body);
    let old_unwind = ctx.unwind_target.replace(final_landingpad);

    let mut final_proxy_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, target: BasicBlock<'ctx>, block: BasicBlock<'ctx>| {
            final_proxy(ctx, target, block, final_data.as_mut().unwrap())
        };
    let redirect = &mut final_proxy_lambda
        as &mut dyn FnMut(
            &mut CodeGenContext<'ctx, 'a>,
            BasicBlock<'ctx>,
            BasicBlock<'ctx>,
        ) -> anyhow::Result<()>;

    let resume = get_builtins(ctx, "__nac3_resume");
    let end_catch = get_builtins(ctx, "__nac3_end_catch");
    if let Some((continue_target, break_target)) = ctx.loop_target.take() {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "with.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "with.continue");
        ctx.builder.position_at_end(break_proxy);
        ctx.build_call(&end_catch, &[], "end_catch")?;
        ctx.builder.position_at_end(continue_proxy);
        ctx.build_call(&end_catch, &[], "end_catch")?;
        ctx.builder.position_at_end(body);
        redirect(ctx, break_target, break_proxy)?;
        redirect(ctx, continue_target, continue_proxy)?;
        ctx.loop_target = Some((continue_proxy, break_proxy));
        old_loop_target = Some((continue_target, break_target));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "with.return");
    ctx.builder.position_at_end(return_proxy);
    ctx.build_call(&end_catch, &[], "end_catch")?;
    let return_target = ctx.return_target.take().unwrap();
    redirect(ctx, return_target, return_proxy)?;
    ctx.return_target = Some(return_proxy);
    let old_return = Some(return_target);

    ctx.unwind_target = old_unwind;
    ctx.loop_target = old_loop_target.or(ctx.loop_target);
    ctx.return_target = old_return;

    ctx.builder.position_at_end(landingpad);

    let landingpad_value = ctx
        .builder
        .build_landing_pad(
            ctx.ctx.struct_type(&[ptr_type.into(), exception_type], false),
            personality,
            &Vec::new(),
            true,
            "try.landingpad",
        )?
        .into_struct_value();
    let exn_val = ctx.builder.build_extract_value(landingpad_value, 1, "exn")?;
    ctx.builder.build_unconditional_branch(dispatcher)?;
    exn.add_incoming(&[(&exn_val, landingpad)]);

    if dispatcher_end.get_terminator().is_none() {
        ctx.builder.position_at_end(dispatcher_end);
        ctx.builder.build_unconditional_branch(cleanup)?;
    }

    // exception path
    ctx.builder.position_at_end(cleanup);
    exit_gen_lambda(ctx, generator)?;
    ctx.build_call_or_invoke(&resume, &[], "resume")?;
    ctx.builder.build_unreachable()?;

    // normal path
    let (final_state, mut final_targets, final_paths) = final_data.unwrap();
    let tail = ctx.ctx.append_basic_block(current_fun, "with.tail");
    final_targets.push(tail);
    let finalizer = ctx.ctx.append_basic_block(current_fun, "with.finally");
    ctx.builder.position_at_end(finalizer);
    exit_gen_lambda(ctx, generator)?;
    let dest = ctx.builder.build_load(ptr_type, final_state, "final_dest")?;
    ctx.builder.build_indirect_branch(dest, &final_targets)?;
    for block in &final_paths {
        if block.get_terminator().is_none() {
            ctx.builder.position_at_end(*block);
            ctx.builder.build_unconditional_branch(finalizer)?;
        }
    }
    for block in &[body] {
        if block.get_terminator().is_none() {
            ctx.builder.position_at_end(*block);
            unsafe {
                ctx.builder.build_store(final_state, tail.get_address().unwrap())?;
            }
            ctx.builder.build_unconditional_branch(finalizer)?;
        }
    }

    ctx.builder.position_at_end(tail);

    Ok(())
}

/// Generates IR for a `return` statement.
pub fn gen_return<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    value: &Option<Box<Expr<Option<Type>>>>,
) -> anyhow::Result<()> {
    let func = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
    let value = if let Some(v_expr) = value.as_ref() {
        generator
            .gen_expr(ctx, v_expr)?
            .val
            .map(|v| v.to_basic_value_enum(ctx, v_expr.custom.unwrap()))
            .transpose()?
    } else {
        None
    };

    // Remap boolean return type into i1
    let value = value.map(|ret_val| {
        // The "return type" of a sret function is in the first parameter
        let expected_ty = func.get_type().get_return_type().unwrap().into();

        anyhow::Ok(if matches!(expected_ty, BasicMetadataTypeEnum::IntType(ty) if ty.get_bit_width() == 1) {
            bool_to_i1(ctx, ret_val.into_int_value())?.into()
        } else {
            ret_val
        })
    }).transpose()?;

    let return_target = ctx.return_target.unwrap();
    if let Some(value) = value {
        ctx.builder.build_store(ctx.return_buffer.unwrap(), value)?;
    }
    ctx.builder.build_unconditional_branch(return_target)?;
    Ok(())
}

/// See [`CodeGenerator::gen_stmt`].
pub fn gen_stmt<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    ctx.with_loc(stmt.location, |ctx| {
        match &stmt.node {
            StmtKind::Pass { .. } => {}
            StmtKind::Expr { value, .. } => {
                generator.gen_expr(ctx, value)?;
            }
            StmtKind::Return { value, .. } => {
                gen_return(generator, ctx, value)?;
            }
            StmtKind::AnnAssign { target, value, .. } => {
                if let Some(value) = value {
                    let value_enum = generator.gen_expr(ctx, value)?.val.unwrap();
                    generator.gen_assign(ctx, target, &value_enum, value.custom.unwrap())?;
                }
            }
            StmtKind::Assign { targets, value, .. } => {
                let value_enum = generator.gen_expr(ctx, value)?.val.unwrap();
                for target in targets {
                    generator.gen_assign(
                        ctx,
                        target,
                        &value_enum.clone(),
                        value.custom.unwrap(),
                    )?;
                }
            }
            StmtKind::Continue { .. } => {
                ctx.builder.build_unconditional_branch(ctx.loop_target.unwrap().0)?;
            }
            StmtKind::Break { .. } => {
                ctx.builder.build_unconditional_branch(ctx.loop_target.unwrap().1)?;
            }
            StmtKind::If { .. } => generator.gen_if(ctx, stmt)?,
            StmtKind::While { .. } => generator.gen_while(ctx, stmt)?,
            StmtKind::For { .. } => generator.gen_for(ctx, stmt)?,
            StmtKind::With { .. } => generator.gen_with(ctx, stmt)?,
            StmtKind::AugAssign { target, op, value, .. } => {
                let result_ty = target.custom.unwrap();
                let value_enum = gen_binop_expr(
                    generator,
                    ctx,
                    target,
                    Binop::aug_assign(*op),
                    value,
                    result_ty,
                )?
                .val
                .unwrap();
                generator.gen_assign(ctx, target, &value_enum, value.custom.unwrap())?;
            }
            StmtKind::Try { .. } => gen_try(generator, ctx, stmt)?,
            StmtKind::Raise { exc, .. } => {
                if let Some(exc) = exc {
                    let exn = if let ExprKind::Name { id, .. } = &exc.node {
                        // Handle "raise Exception" short form
                        let def_id = ctx
                            .resolver
                            .get_identifier_def(*id)
                            .map_err(|e| anyhow!("{} (at {})", e.first().unwrap(), exc.location))?;
                        let def = &ctx.top_level.definitions;
                        let TopLevelDef::Class { constructor, .. } = *def[def_id.0].read() else {
                            bail!("Failed to resolve symbol {id} (at {})", exc.location);
                        };

                        let TypeEnum::TFunc(signature) =
                            ctx.unifier.get_ty(constructor.unwrap()).as_ref().clone()
                        else {
                            bail!("Failed to resolve symbol {id} (at {})", exc.location);
                        };

                        generator
                            .gen_call(ctx, None, (&signature, def_id), Vec::default())?
                            .map(ValueEnum::Dynamic)
                    } else {
                        generator.gen_expr(ctx, exc)?.val
                    };

                    let exc = if let Some(v) = exn {
                        v.to_basic_value_enum(ctx, exc.custom.unwrap())?
                    } else {
                        return Ok(());
                    };
                    let exc = ExceptionType::new(ctx).map_value(exc.into_pointer_value(), None);
                    gen_raise(ctx, Some(&exc))?;
                } else {
                    gen_raise(ctx, None)?;
                }
            }
            StmtKind::Assert { test, msg, .. } => {
                let test = generator.gen_expr(ctx, test)?.to_basic_value_enum(ctx)?;

                let err_msg = match msg {
                    Some(msg) => generator.gen_expr(ctx, msg)?.to_basic_value_enum(ctx)?,
                    None => ctx.gen_string("")?.into(),
                };
                ctx.make_assert_impl(
                    bool_to_i1(ctx, test.into_int_value())?,
                    "0:AssertionError",
                    err_msg,
                    [None, None, None],
                )?;
            }
            _ => unimplemented!(),
        }
        Ok(())
    })
}

/// Generates IR for a block statement contains `stmts`.
pub fn gen_block<'a, G: CodeGenerator, I: Iterator<Item = &'a Stmt<Option<Type>>>>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmts: I,
) -> anyhow::Result<()> {
    for stmt in stmts {
        generator.gen_stmt(ctx, stmt)?;
        if ctx.is_terminated() {
            break;
        }
    }
    Ok(())
}
