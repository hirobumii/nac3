use std::iter::once;

use anyhow::{anyhow, bail};
use inkwell::{
    AddressSpace, IntPredicate,
    basic_block::BasicBlock,
    builder::Builder,
    types::{AnyTypeEnum, BasicMetadataTypeEnum, BasicType},
    values::{BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue},
};
use itertools::{Itertools as _, izip};
use nac3parser::ast::{ExcepthandlerKind, Expr, ExprKind, Location, Stmt, StmtKind, StrRef};

use crate::{
    codegen::{
        CodeGenContext, CodeGenerator, ModuleContext, VarValue,
        allocator::AllocationScope,
        bool_to_i1, bool_to_i8,
        expr::{destructure_range, gen_binop_expr},
        gen_in_range_check,
        irrt::{calculate_len_for_slice_range, handle_slice_indices, list_slice_assignment},
        llvm_fns::FunctionDecl,
        llvm_intrinsics,
        macros::codegen_unreachable,
        typed_load, typed_store,
        types::{
            ArrayLikeIndexer, ArraySliceValue, EnumerateType, ExceptionType,
            ExceptionValue, ListType, ListValue, NDArrayType, OpaqueRefCountedType,
            ProxyTypeBase, RangeType, RawClassType, RawListType, RefCountedValue, RustNDIndex,
            ScalarOrNDArray, StringType, TupleType, TupleValue, TypedRefCountedType, broadcast,
            field, is_refcounted_type,
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

/// Allocates an LLVM stack variable for temporary storage. The variable is stored
/// at the beginning of the function.
///
/// You must ensure that the memory allocated here is supposed to be reused across
/// loops and branches. If you are possibly within a scope where the number of allocations
/// performed may depend on control flow (e.g. allocating objects within a comprehension),
/// use [`gen_dyn_var`] instead.
#[deprecated = "Use `CodeGenContext::build_allocate` instead"]
pub fn gen_var<'ctx, T: BasicType<'ctx> + Copy>(
    ctx: &CodeGenContext<'ctx, '_>,
    ty: T,
    name: Option<&str>,
) -> anyhow::Result<PointerValue<'ctx>> {
    ctx.build_allocate(AllocationScope::StackStartOfFunc, ty, name)
}

/// Allocates an LLVM stack variable for temporary storage. The alloca is inserted
/// at the current insertion point.
#[deprecated = "Use `CodeGenContext::build_allocate` instead"]
pub fn gen_dyn_var<'ctx, T: BasicType<'ctx> + Copy>(
    ctx: &CodeGenContext<'ctx, '_>,
    ty: T,
    name: Option<&str>,
) -> anyhow::Result<PointerValue<'ctx>> {
    ctx.build_allocate(AllocationScope::StackCurrentLoc, ty, name)
}

/// Allocates an LLVM stack array for temporary storage. The variable is stored
/// at the beginning of the function.
///
/// This function takes a fixed (compile-time) size for the array, because if the
/// size is dynamic and not known at the beginning of the function, it should be
/// allocated at the current insertion point instead.
#[deprecated = "Use `CodeGenContext::build_array_allocate` instead"]
pub fn gen_array_var<'ctx, 'a, T: BasicType<'ctx> + Copy>(
    ctx: &CodeGenContext<'ctx, 'a>,
    ty: T,
    size: u64,
    name: Option<&'ctx str>,
) -> anyhow::Result<ArraySliceValue<'ctx>> {
    ctx.build_array_allocate(AllocationScope::StackStartOfFunc, ty, size, name)
}

/// Allocates an LLVM stack array for temporary storage.
///
/// This happens at the current insertion point instead of the function's entry block,
/// which allows for dynamically sized arrays.
#[deprecated = "Use `CodeGenContext::build_dyn_array_allocate` instead"]
pub fn gen_dyn_array_var<'ctx, 'a, T: BasicType<'ctx> + Copy>(
    ctx: &CodeGenContext<'ctx, 'a>,
    ty: T,
    size: IntValue<'ctx>,
    name: Option<&'ctx str>,
) -> anyhow::Result<ArraySliceValue<'ctx>> {
    ctx.build_dyn_array_allocate(AllocationScope::StackCurrentLoc, ty, size, name)
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
                let ptr =
                    ctx.build_allocate(AllocationScope::StackStartOfFunc, ptr_ty, name)?;
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

            // For refcounted classes, use inner_ptr to skip past the ObjectHeader,
            // then GEP into the inner struct with the original field index.
            let is_refcounted = is_refcounted_type(&mut ctx.unifier, value.custom.unwrap());
            let ptr = if is_refcounted {
                let rc = OpaqueRefCountedType::new(ctx).map_value(ptr, None);
                let inner_ptr = rc.inner_ptr(ctx)?;
                let raw_class = RawClassType::from_unifier_type(ctx, value.custom.unwrap());
                ctx.builder.build_pointer_cast(
                    inner_ptr,
                    raw_class.inner_type().ptr_type(AddressSpace::default()),
                    "",
                )?
            } else {
                let alloca_ty = ctx.get_alloca_type(value.custom.unwrap());
                ctx.builder.build_pointer_cast(
                    ptr,
                    alloca_ty.ptr_type(AddressSpace::default()),
                    "",
                )?
            };
            unsafe {
                ctx.builder.build_in_bounds_gep(
                    ptr,
                    &[ctx.i32.const_zero(), ctx.i32.const_int(index as u64, false)],
                    name.unwrap_or(""),
                )?
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
            let struct_ptr = ctx.build_allocate(AllocationScope::Default, struct_ty, name)?;
            for (i, elt) in elts.iter().enumerate() {
                ctx.builder.build_store(
                    unsafe {
                        ctx.builder.build_in_bounds_gep(
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
        ExprKind::Subscript { value: target, slice: key, .. } => {
            // Handle "slicing" or "subscription"
            generator.gen_setitem(ctx, target, key, value, value_ty)?;
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

            let target = if let AnyTypeEnum::PointerType(ptr_ty) = ptr.get_type().get_element_type()
            {
                gen_if_else_expr_callback(
                    generator,
                    ctx,
                    |_, ctx| Ok(ctx.builder.build_is_not_null(ptr, "")?),
                    |_, ctx| {
                        let target_ty = ctx.get_llvm_type(target.custom.unwrap());
                        Ok(Some(typed_load(ctx.builder, ptr, target_ty, "")?.into_pointer_value()))
                    },
                    |_, _| Ok(Some(ptr_ty.const_null())),
                )?
            } else {
                None
            };

            if let BasicValueEnum::PointerValue(val) = val
                && is_refcounted_type(&mut ctx.unifier, value_ty)
            {
                let value_llvm_ty = ctx.get_llvm_type(value_ty).into_pointer_type();
                let val = ctx.builder.build_pointer_cast(val, value_llvm_ty, "")?;
                OpaqueRefCountedType::new(ctx)
                    .map_value(val, None)
                    .header(ctx)
                    .safe_increment_refcount(ctx)?;
            }
            typed_store(ctx.builder, ptr, val)?;
            if let Some(target) = target
                && is_refcounted_type(&mut ctx.unifier, value_ty)
            {
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
        typed_store(ctx.builder, ptr, list.value)?;
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
                    "Tuple unpacking requires at least as many values as targets including the starred target, but got {} values and {} targets",
                    tuple_tys.len(),
                    targets.len()
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

                let lhs_size = ctx.size_t.const_int(targets.len() as u64, false);
                ctx.make_assert(
                    ctx.builder.build_int_compare(
                        IntPredicate::EQ,
                        rhs_size,
                        lhs_size,
                        "list_size_check",
                    )?,
                    "ValueError",
                    "incorrect number of values to unpack (expected {1})",
                    [Some(rhs_size), Some(lhs_size), None],
                    Location::default(),
                )?;

                let values = read_fixed(ctx, targets.len())?;
                return do_assign(generator, ctx, targets, &values);
            };

            // All non-starred targets must be assigned exactly one value.
            let min_size = targets.len() - 1;
            let min_size_ = ctx.size_t.const_int(min_size as u64, false);
            ctx.make_assert(
                ctx.builder.build_int_compare(
                    IntPredicate::ULE,
                    min_size_,
                    rhs_size,
                    "list_size_check",
                )?,
                "ValueError",
                "too few values to unpack (expected at least {0}, got {1})",
                [Some(min_size_), Some(rhs_size), None],
                targets[0].location,
            )?;

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
            llvm_intrinsics::call_memcpy(
                ctx,
                mid_list_data.inner_value(ctx, Some(mid_len))?.value.0,
                mid_begin,
                ctx.builder.build_int_mul(
                    mid_len,
                    llvm_list_elem_ty
                        .size_of()
                        .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                        .unwrap(),
                    "",
                )?,
            )?;
            // Increment refcount for each copied element in the new mid_list
            if is_refcounted_type(&mut ctx.unifier, elem_ty) {
                let mid_list_data_inner = mid_list_data.inner_value(ctx, Some(mid_len))?;
                gen_for_callback_incrementing(
                    &mut (),
                    ctx,
                    None,
                    ctx.size_t.const_zero(),
                    (mid_len, false),
                    |(), ctx, _, i| {
                        let elem: PointerValue<'ctx> =
                            mid_list_data_inner.get_unchecked(ctx, &i, None)?;
                        OpaqueRefCountedType::new(ctx)
                            .map_value(elem, None)
                            .header(ctx)
                            .safe_increment_refcount(ctx)?;
                        Ok(())
                    },
                    ctx.size_t.const_int(1, false),
                    |(), _| Ok(()),
                )?;
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
                    let dest_data = target.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(target_size))?;
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
                    let dest_slice_len =
                        calculate_len_for_slice_range(ctx, start, dest_end, step)?;
                    let dest_slice_len = ctx.builder.build_int_z_extend_or_bit_cast(
                        dest_slice_len,
                        ctx.size_t,
                        "",
                    )?;
                    gen_for_callback_incrementing(
                        &mut (),
                        ctx,
                        None,
                        ctx.size_t.const_zero(),
                        (dest_slice_len, false),
                        |(), ctx, _, i| {
                            let actual_idx = {
                                let step_ext = ctx.builder.build_int_s_extend_or_bit_cast(
                                    step,
                                    ctx.size_t,
                                    "",
                                )?;
                                let start_ext = ctx.builder.build_int_s_extend_or_bit_cast(
                                    start,
                                    ctx.size_t,
                                    "",
                                )?;
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
                        },
                        ctx.size_t.const_int(1, false),
                        |(), _| Ok(()),
                    )?;
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
                    let src_data = value.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(size))?;
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
                    gen_for_callback_incrementing(
                        &mut (),
                        ctx,
                        None,
                        ctx.size_t.const_zero(),
                        (src_slice_len, false),
                        |(), ctx, _, i| {
                            let actual_idx = {
                                let step_ext = ctx.builder.build_int_s_extend_or_bit_cast(
                                    src_ind.2,
                                    ctx.size_t,
                                    "",
                                )?;
                                let start_ext = ctx.builder.build_int_s_extend_or_bit_cast(
                                    src_ind.0,
                                    ctx.size_t,
                                    "",
                                )?;
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
                        },
                        ctx.size_t.const_int(1, false),
                        |(), _| Ok(()),
                    )?;
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
                    .build_select(is_negative, adjusted, index, "index")
                    .map(BasicValueEnum::into_int_value)?;

                // unsigned less than is enough, because negative index after adjustment is
                // bigger than the length (for unsigned cmp)
                let bound_check =
                    ctx.builder.build_int_compare(IntPredicate::ULT, index, len, "inbound")?;
                ctx.make_assert(
                    bound_check,
                    "0:IndexError",
                    "index {0} out of bounds 0:{1}",
                    [Some(index), Some(len), None],
                    key.location,
                )?;

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

            let target = NDArrayType::from_unifier_type(ctx, target_ty)
                .map_value(target.into_pointer_value(), None);
            let target = target.index(ctx, &key)?;

            let value = ScalarOrNDArray::from_value(ctx, (value_ty, value)).to_ndarray(ctx)?;

            let broadcast_result = broadcast(ctx, &[target, value])?;

            let target = broadcast_result.ndarrays[0];
            let value = broadcast_result.ndarrays[1];

            target.copy_data_from(ctx, &value)?;
        }
        _ => {
            panic!("encountered unknown target type: {}", ctx.unifier.stringify(target_ty));
        }
    }
    Ok(())
}

/// Generates a Python-style `for` construct using lambdas, similar to the following desugared Python code:
///
/// ```python
/// v = init()
/// while cond(v):
///     body(v)
///     update(v)
/// else:
///     orelse()
/// ```
///
/// Note that this function only provides the bare control flow structure necessary for a generic
/// Python-based for loop; it does not implement any specific iteration semantics. The caller is
/// responsible for implementing the desired iteration behavior based on the type of the `iterable`
/// object.
///
/// * `init` - A lambda containing IR statements declaring and initializing loop variables. The
///   return value is a [Clone] value which will be passed to the other lambdas.
/// * `cond` - A lambda containing IR statements checking whether the loop should continue
///   executing. The result value must be an `i1` indicating if the loop should continue.
/// * `body` - A lambda containing IR statements within the loop body.
/// * `update` - A lambda containing IR statements updating loop variables.
/// * `orelse` - A lambda containing IR statements to execute if the `for` loop completes without
///   `break`.
#[allow(clippy::too_many_arguments)]
pub fn gen_for_callback<'ctx, 'a, G, I, InitFn, CondFn, BodyFn, UpdateFn, OrElseFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    init: InitFn,
    cond: CondFn,
    body: BodyFn,
    update: UpdateFn,
    orelse: OrElseFn,
) -> anyhow::Result<()>
where
    G: ?Sized,
    I: Clone,
    InitFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<I>,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, I) -> anyhow::Result<IntValue<'ctx>>,
    BodyFn: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        I,
    ) -> anyhow::Result<()>,
    UpdateFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, I) -> anyhow::Result<()>,
    OrElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
{
    let label = label.unwrap_or("for");

    let current_bb = ctx.builder.get_insert_block().unwrap();
    let init_bb = ctx.ctx.insert_basic_block_after(current_bb, &format!("{label}.init"));
    // The BB containing the loop condition check
    let cond_bb = ctx.ctx.insert_basic_block_after(init_bb, &format!("{label}.cond"));
    let body_bb = ctx.ctx.insert_basic_block_after(cond_bb, &format!("{label}.body"));
    // The BB containing the increment expression
    let update_bb = ctx.ctx.insert_basic_block_after(body_bb, &format!("{label}.update"));
    let orelse_bb = ctx.ctx.insert_basic_block_after(update_bb, &format!("{label}.orelse"));
    let cont_bb = ctx.ctx.insert_basic_block_after(orelse_bb, &format!("{label}.end"));
    // store loop bb information and restore it later
    let loop_bb = ctx.loop_target.replace((update_bb, cont_bb));

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();
    let restore_var_assignment = |ctx: &mut CodeGenContext<'ctx, 'a>| {
        for (k, VarValue { counter, .. }) in &var_assignment {
            let VarValue { static_value: static_val, counter: counter2, .. } =
                ctx.var_assignment.get_mut(k).unwrap();
            if counter != counter2 {
                *static_val = None;
            }
        }
    };

    ctx.builder.build_unconditional_branch(init_bb)?;

    ctx.builder.position_at_end(init_bb);
    let loop_var = init(generator, ctx)?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(cond_bb)?;
    }

    ctx.builder.position_at_end(cond_bb);
    let cond = cond(generator, ctx, loop_var.clone())?;
    assert_eq!(cond.get_type().get_bit_width(), ctx.i1.get_bit_width());
    if !ctx.is_terminated() {
        ctx.builder.build_conditional_branch(cond, body_bb, orelse_bb)?;
    }

    ctx.builder.position_at_end(body_bb);
    let hooks = BreakContinueHooks { exit_bb: cont_bb, latch_bb: update_bb };
    body(generator, ctx, hooks, loop_var.clone())?;
    restore_var_assignment(ctx);
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(update_bb)?;
    }

    ctx.builder.position_at_end(update_bb);
    update(generator, ctx, loop_var)?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(cond_bb)?;
    }

    ctx.builder.position_at_end(orelse_bb);
    orelse(generator, ctx)?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(cont_bb)?;
    }

    restore_var_assignment(ctx);
    ctx.builder.position_at_end(cont_bb);
    ctx.loop_target = loop_bb;

    Ok(())
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
    let merge_bb = ctx.ctx.insert_basic_block_after(update_bb, "tuple.merge");
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
fn gen_for_enumerate<'ctx, G, GetFirst, GetNext, U>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    element_ty: Option<Type>,
    length: IntValue<'ctx>,
    start: IntValue<'ctx>,
    target_expr: &ExprKind<U>,
    target_i: PointerValue<'ctx>,
    get_first_elem: GetFirst,
    get_next_elem: GetNext,
    body: &[Stmt<Option<Type>>],
    orelse: &[Stmt<Option<Type>>],
) -> anyhow::Result<()>
where
    G: CodeGenerator,
    GetFirst: Fn(&mut CodeGenContext<'ctx, '_>) -> anyhow::Result<BasicValueEnum<'ctx>>,
    GetNext:
        Fn(&mut CodeGenContext<'ctx, '_>, IntValue<'ctx>) -> anyhow::Result<BasicValueEnum<'ctx>>,
{
    let int32 = ctx.i32;
    let default_element_ty = ctx.get_llvm_type(element_ty.unwrap_or(ctx.primitives.int32));
    gen_for_callback(
        generator,
        ctx,
        None,
        |_, ctx| {
            let element_struct = ctx.ctx.struct_type(&[int32.into(), default_element_ty], false);
            let iv_pair =
                ctx.build_allocate(AllocationScope::Default, element_struct, Some("for.v.addr"))?;
            let i = ctx.builder.build_struct_gep(iv_pair, 0, "i")?;
            ctx.builder.build_store(i, start)?;
            if element_ty.is_some() {
                let first_v = get_first_elem(ctx)?;
                let v = ctx.builder.build_struct_gep(iv_pair, 1, "v")?;
                ctx.builder.build_store(v, first_v)?;
            }
            Ok(iv_pair)
        },
        |_, ctx, iv_pair| {
            let i = ctx.builder.build_struct_gep(iv_pair, 0, "i")?;
            let i_val = ctx.builder.build_load(i, "i_val").map(BasicValueEnum::into_int_value)?;
            gen_in_range_check(
                ctx,
                ctx.builder.build_int_sub(i_val, start, "sub")?,
                length,
                int32.const_int(1, false),
            )
        },
        |generator, ctx, _, iv_pair| {
            match target_expr {
                ExprKind::Tuple { elts, .. } if elts.len() == 2 => {
                    let i = ctx.builder.build_struct_gep(iv_pair, 0, "i")?;
                    let i_val =
                        ctx.builder.build_load(i, "i_val").map(BasicValueEnum::into_int_value)?;
                    let ptr_1 = ctx.builder.build_struct_gep(target_i, 0, "tuple.0")?;
                    let addr_1 =
                        ctx.builder.build_load(ptr_1, "tuple.0.addr")?.into_pointer_value();
                    ctx.builder.build_store(addr_1, i_val)?;
                    let v = ctx.builder.build_struct_gep(iv_pair, 1, "v")?;
                    let v_val = ctx.builder.build_load(v, "")?;
                    let ptr_2 = ctx.builder.build_struct_gep(target_i, 1, "tuple.1")?;
                    let addr_2 =
                        ctx.builder.build_load(ptr_2, "tuple.1.addr")?.into_pointer_value();
                    ctx.builder.build_store(addr_2, v_val)?;
                }
                ExprKind::Name { .. } => {
                    // Load i and v from the internal iv_pair struct
                    let i = ctx.builder.build_struct_gep(iv_pair, 0, "i")?;
                    let i_val = ctx.builder.build_load(i, "i_val")?;
                    let v = ctx.builder.build_struct_gep(iv_pair, 1, "v")?;
                    let v_val = ctx.builder.build_load(v, "v_val")?;
                    // Construct a proper tuple (with ObjectHeader) from the values
                    let tuple_val = TupleValue::new(ctx, &[i_val, v_val], Some("iv"))?;
                    typed_store(ctx.builder, target_i, tuple_val.value)?;
                }
                _ => codegen_unreachable!(
                    ctx,
                    "expected target expression of for enumerate to be a Name or a Tuple"
                ),
            }
            generator.gen_block(ctx, body.iter())?;
            Ok(())
        },
        |_, ctx, iv_pair| {
            let i = ctx.builder.build_struct_gep(iv_pair, 0, "i")?;
            let i_val = ctx.builder.build_load(i, "i_val").map(BasicValueEnum::into_int_value)?;
            let next_i = ctx.builder.build_int_add(i_val, int32.const_int(1, false), "inc")?;
            ctx.builder.build_store(i, next_i)?;
            if element_ty.is_some() {
                let next_v = get_next_elem(ctx, next_i)?;
                let v = ctx.builder.build_struct_gep(iv_pair, 1, "v")?;
                ctx.builder.build_store(v, next_v)?;
            }
            Ok(())
        },
        |generator, ctx| generator.gen_block(ctx, orelse.iter()),
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
                "ValueError",
                "range() arg 3 must not be zero",
                [None, None, None],
                ctx.current_loc,
            )?;

            gen_for_callback(
                generator,
                ctx,
                None,
                |generator, ctx| {
                    // Internal variable for loop; Cannot be assigned
                    let i =
                        ctx.build_allocate(AllocationScope::Default, int32, Some("for.i.addr"))?;
                    // Variable declared in "target" expression of the loop; Can be reassigned *or* shadowed
                    let Some(target_i) =
                        generator.gen_store_target(ctx, target, Some("for.target.addr"))?
                    else {
                        codegen_unreachable!(ctx)
                    };

                    typed_store(ctx.builder, i, start)?;

                    Ok((i, target_i))
                },
                |_, ctx, (i, _)| {
                    gen_in_range_check(
                        ctx,
                        ctx.builder.build_load(i, "").map(BasicValueEnum::into_int_value)?,
                        stop,
                        step,
                    )
                },
                |generator, ctx, _, (i, target_i)| {
                    typed_store(
                        ctx.builder,
                        target_i,
                        ctx.builder.build_load(i, "").map(BasicValueEnum::into_int_value)?,
                    )?;
                    generator.gen_block(ctx, body.iter())?;

                    Ok(())
                },
                |_, ctx, (i, _)| {
                    let next_i = ctx.builder.build_int_add(
                        ctx.builder.build_load(i, "").map(BasicValueEnum::into_int_value)?,
                        step,
                        "inc",
                    )?;
                    typed_store(ctx.builder, i, next_i)?;

                    Ok(())
                },
                |generator, ctx| generator.gen_block(ctx, orelse.iter()),
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
                let iterable_struct_val = typed_load(
                    ctx.builder,
                    iterable_ptr,
                    iterable_struct_ty.into(),
                    "iterable_struct",
                )?
                .into_struct_value();
                let iterable_data_i8ptr =
                    ctx.builder.build_extract_value(iterable_struct_val, 0, "iterable_data")?;
                let iterable_ty = iter_type_vars(params).nth(1).unwrap().ty;
                let iterable_llvm_ty = ctx.get_llvm_type(iterable_ty);
                let ag = typed_load(
                    ctx.builder,
                    iterable_data_i8ptr.into_pointer_value(),
                    iterable_llvm_ty,
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
                        |ctx| {
                            iterable.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(length_sizet))?.get_unchecked(
                                ctx,
                                &int32.const_int(0, false),
                                Some("first_v"),
                            )
                        },
                        |ctx, next_i| {
                            iterable.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(length_sizet))?.get_unchecked(
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
                        |ctx| iterable.extract(ctx, 0),
                        |ctx, next_i| {
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
                        "enumerate() with unsupported iterable type: {:?}",
                        ctx.unifier.get_ty(iterable_ty)
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

            gen_for_callback(
                generator,
                ctx,
                None,
                |_, ctx| {
                    let index_addr = ctx.build_allocate(
                        AllocationScope::Default,
                        size_t,
                        Some("for.index.addr"),
                    )?;
                    typed_store(ctx.builder, index_addr, size_t.const_zero())?;

                    Ok(index_addr)
                },
                |_, ctx, index_addr| {
                    let index = ctx
                        .builder
                        .build_load(index_addr, "for.index")
                        .map(BasicValueEnum::into_int_value)?;
                    let cmp =
                        ctx.builder.build_int_compare(IntPredicate::SLT, index, len, "cond")?;

                    Ok(cmp)
                },
                |generator, ctx, _, index_addr| {
                    let index = ctx
                        .builder
                        .build_load(index_addr, "for.index")
                        .map(BasicValueEnum::into_int_value)?;
                    let val: BasicValueEnum = iter_val
                        .inner_value(ctx)?
                        .data(ctx)?
                        .inner_value(ctx, Some(len))?
                        .get_unchecked(ctx, &index, Some("val"))?;
                    let val_ty = iter_type_vars(list_params).next().unwrap().ty;
                    generator.gen_assign(ctx, target, &val.into(), val_ty)?;
                    generator.gen_block(ctx, body.iter())?;

                    Ok(())
                },
                |_, ctx, index_addr| {
                    let index = ctx
                        .builder
                        .build_load(index_addr, "")
                        .map(BasicValueEnum::into_int_value)?;
                    let inc = ctx.builder.build_int_add(index, size_t.const_int(1, true), "inc")?;
                    typed_store(ctx.builder, index_addr, inc)?;

                    Ok(())
                },
                |generator, ctx| generator.gen_block(ctx, orelse.iter()),
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

            gen_for_callback(
                generator,
                ctx,
                None,
                |_, ctx| {
                    let index_addr = ctx.build_allocate(
                        AllocationScope::Default,
                        size_t,
                        Some("for.index.addr"),
                    )?;
                    typed_store(ctx.builder, index_addr, size_t.const_zero())?;

                    Ok(index_addr)
                },
                |_, ctx, index_addr| {
                    let index = ctx
                        .builder
                        .build_load(index_addr, "for.index")
                        .map(BasicValueEnum::into_int_value)?;
                    let cmp = ctx.builder.build_int_compare(
                        IntPredicate::SLT,
                        index,
                        shape_dim0,
                        "cond",
                    )?;

                    Ok(cmp)
                },
                |generator, ctx, _, index_addr| {
                    let index = ctx
                        .builder
                        .build_load(index_addr, "for.index")
                        .map(BasicValueEnum::into_int_value)?;

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

                    Ok(())
                },
                |_, ctx, index_addr| {
                    let index = ctx
                        .builder
                        .build_load(index_addr, "")
                        .map(BasicValueEnum::into_int_value)?;
                    let inc =
                        ctx.builder.build_int_add(index, size_t.const_int(1, false), "inc")?;
                    typed_store(ctx.builder, index_addr, inc)?;

                    Ok(())
                },
                |generator, ctx| generator.gen_block(ctx, orelse.iter()),
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
    pub fn build_break_branch(&self, builder: &Builder<'ctx>) -> anyhow::Result<()> {
        builder.build_unconditional_branch(self.exit_bb)?;
        Ok(())
    }

    /// Creates a [`br` instruction][Builder::build_unconditional_branch] to the latch
    /// [`BasicBlock`], as if by calling `continue`.
    pub fn build_continue_branch(&self, builder: &Builder<'ctx>) -> anyhow::Result<()> {
        builder.build_unconditional_branch(self.latch_bb)?;
        Ok(())
    }
}

/// Generates a C-style monotonically-increasing `for` construct using lambdas, similar to the
/// following C code:
///
/// ```c
/// for (int x = init_val; x /* < or <= ; see `max_val` */ max_val; x += incr_val) {
///     body(x);
/// }
/// ```
///
/// * `init_val` - The initial value of the loop variable. The type of this value will also be used
///   as the type of the loop variable.
/// * `max_val` - A tuple containing the maximum value of the loop variable, and whether the maximum
///   value should be treated as inclusive (as opposed to exclusive).
/// * `body` - A lambda containing IR statements within the loop body.
/// * `incr_val` - The value to increment the loop variable on each iteration.
/// * `orelse` - A lambda containing IR statements to execute if the `for` loop completes without
///   `break`.
#[allow(clippy::too_many_arguments)]
pub fn gen_for_callback_incrementing<'ctx, 'a, G, BodyFn, OrElseFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    init_val: IntValue<'ctx>,
    max_val: (IntValue<'ctx>, bool),
    body: BodyFn,
    incr_val: IntValue<'ctx>,
    orelse: OrElseFn,
) -> anyhow::Result<()>
where
    G: ?Sized,
    BodyFn: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        IntValue<'ctx>,
    ) -> anyhow::Result<()>,
    OrElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
{
    let init_val_t = init_val.get_type();

    gen_for_callback(
        generator,
        ctx,
        label,
        |_, ctx| {
            let i_addr = ctx.build_allocate(AllocationScope::Default, init_val_t, None)?;
            typed_store(ctx.builder, i_addr, init_val)?;

            Ok(i_addr)
        },
        |_, ctx, i_addr| {
            let cmp_op = if max_val.1 { IntPredicate::ULE } else { IntPredicate::ULT };

            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value)?;
            let max_val = ctx.builder.build_int_z_extend_or_bit_cast(max_val.0, init_val_t, "")?;

            Ok(ctx.builder.build_int_compare(cmp_op, i, max_val, "")?)
        },
        |generator, ctx, hooks, i_addr| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value)?;

            body(generator, ctx, hooks, i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value)?;
            let incr_val = ctx.builder.build_int_z_extend_or_bit_cast(incr_val, init_val_t, "")?;
            let i = ctx.builder.build_int_add(i, incr_val, "")?;
            typed_store(ctx.builder, i_addr, i)?;

            Ok(())
        },
        orelse,
    )
}

/// Generates a `for` construct over a `range`-like iterable using lambdas, similar to the following
/// C code:
///
/// ```c
/// bool incr = start_fn() <= end_fn();
/// for (int i = start_fn(); i /* < or > */ end_fn(); i += step_fn()) {
///     body_fn(i);
/// }
/// ```
///
/// - `is_unsigned`: Whether to treat the values of the `range` as unsigned.
/// - `start_fn`: A lambda of IR statements that retrieves the `start` value of the `range`-like
///   iterable.
/// - `stop_fn`: A lambda of IR statements that retrieves the `stop` value of the `range`-like
///   iterable. This value will be extended to the size of `start`.
/// - `stop_inclusive`: Whether the stop value should be treated as inclusive.
/// - `step_fn`: A lambda of IR statements that retrieves the `step` value of the  `range`-like
///   iterable. This value will be extended to the size of `start`.
/// - `body_fn`: A lambda of IR statements within the loop body.
/// * `orelse` - A lambda containing IR statements to execute if the `for` loop completes without
///   `break`.
#[allow(clippy::too_many_arguments)]
pub fn gen_for_range_callback<'ctx, 'a, G, StartFn, StopFn, StepFn, BodyFn, OrElseFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    is_unsigned: bool,
    start_fn: StartFn,
    (stop_fn, stop_inclusive): (StopFn, bool),
    step_fn: StepFn,
    body_fn: BodyFn,
    orelse: OrElseFn,
) -> anyhow::Result<()>
where
    G: ?Sized,
    StartFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<IntValue<'ctx>>,
    StopFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<IntValue<'ctx>>,
    StepFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<IntValue<'ctx>>,
    BodyFn: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        IntValue<'ctx>,
    ) -> anyhow::Result<()>,
    OrElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
{
    let init_val_t = start_fn(generator, ctx)?.get_type();

    gen_for_callback(
        generator,
        ctx,
        label,
        |generator, ctx| {
            let i_addr = ctx.build_allocate(AllocationScope::Default, init_val_t, None)?;

            let start = start_fn(generator, ctx)?;
            typed_store(ctx.builder, i_addr, start)?;

            let start = start_fn(generator, ctx)?;
            let stop = stop_fn(generator, ctx)?;
            let stop = if stop.get_type().get_bit_width() == start.get_type().get_bit_width() {
                stop
            } else if is_unsigned {
                ctx.builder.build_int_z_extend(stop, start.get_type(), "")?
            } else {
                ctx.builder.build_int_s_extend(stop, start.get_type(), "")?
            };

            let incr = ctx.builder.build_int_compare(
                if is_unsigned { IntPredicate::ULE } else { IntPredicate::SLE },
                start,
                stop,
                "",
            )?;

            Ok((i_addr, incr))
        },
        |generator, ctx, (i_addr, incr)| {
            let (lt_cmp_op, gt_cmp_op) = match (is_unsigned, stop_inclusive) {
                (true, true) => (IntPredicate::ULE, IntPredicate::UGE),
                (true, false) => (IntPredicate::ULT, IntPredicate::UGT),
                (false, true) => (IntPredicate::SLE, IntPredicate::SGE),
                (false, false) => (IntPredicate::SLT, IntPredicate::SGT),
            };

            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value)?;
            let stop = stop_fn(generator, ctx)?;
            let stop = if stop.get_type().get_bit_width() == i.get_type().get_bit_width() {
                stop
            } else if is_unsigned {
                ctx.builder.build_int_z_extend(stop, i.get_type(), "")?
            } else {
                ctx.builder.build_int_s_extend(stop, i.get_type(), "")?
            };

            let i_lt_end = ctx.builder.build_int_compare(lt_cmp_op, i, stop, "")?;
            let i_gt_end = ctx.builder.build_int_compare(gt_cmp_op, i, stop, "")?;

            let cond = ctx
                .builder
                .build_select(incr, i_lt_end, i_gt_end, "")
                .map(BasicValueEnum::into_int_value)?;

            Ok(cond)
        },
        |generator, ctx, hooks, (i_addr, _)| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value)?;

            body_fn(generator, ctx, hooks, i)
        },
        |generator, ctx, (i_addr, _)| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value)?;

            let incr_val = step_fn(generator, ctx)?;
            let incr_val = if incr_val.get_type().get_bit_width() == i.get_type().get_bit_width() {
                incr_val
            } else if is_unsigned {
                ctx.builder.build_int_z_extend(incr_val, i.get_type(), "")?
            } else {
                ctx.builder.build_int_s_extend(incr_val, i.get_type(), "")?
            };

            let i = ctx.builder.build_int_add(i, incr_val, "")?;
            typed_store(ctx.builder, i_addr, i)?;

            Ok(())
        },
        orelse,
    )
}

/// Generates a Python-style `while` construct using lambdas, similar to the following Python code:
/// ```python
/// while cond():
///     body()
/// else:
///     orelse()
/// ```
///
/// * `cond` - A lambda containing IR statements checking whether the loop should continue
///   executing. The result value must be an `i1` indicating if the loop should continue.
/// * `body` - A lambda containing IR statements within the loop body.
/// * `orelse` - A lambda containing IR statements to execute if the `while` loop completes without
///   `break`.
pub fn gen_while_callback<'ctx, 'a, G, CondFn, BodyFn, OrElseFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    cond: CondFn,
    body: BodyFn,
    orelse: OrElseFn,
) -> anyhow::Result<()>
where
    G: CodeGenerator + ?Sized,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<IntValue<'ctx>>,
    BodyFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
    OrElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
{
    let label = label.unwrap_or("while");

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();
    let restore_var_assignment = |ctx: &mut CodeGenContext<'ctx, 'a>| {
        for (k, VarValue { counter, .. }) in &var_assignment {
            let VarValue { static_value: static_val, counter: counter2, .. } =
                ctx.var_assignment.get_mut(k).unwrap();
            if counter != counter2 {
                *static_val = None;
            }
        }
    };

    let current_bb = ctx.builder.get_insert_block().unwrap();
    let test_bb = ctx.ctx.insert_basic_block_after(current_bb, &format!("{label}.test"));
    let body_bb = ctx.ctx.insert_basic_block_after(test_bb, &format!("{label}.body"));
    let orelse_bb = ctx.ctx.insert_basic_block_after(body_bb, &format!("{label}.orelse"));
    let cont_bb = ctx.ctx.insert_basic_block_after(orelse_bb, &format!("{label}.cont"));

    // store loop bb information and restore it later
    let loop_bb = ctx.loop_target.replace((test_bb, cont_bb));

    ctx.builder.build_unconditional_branch(test_bb)?;

    ctx.builder.position_at_end(test_bb);
    let test = cond(generator, ctx)?;
    if !ctx.is_terminated() {
        ctx.builder.build_conditional_branch(bool_to_i1(ctx, test)?, body_bb, orelse_bb)?;
    }

    ctx.builder.position_at_end(body_bb);
    body(generator, ctx)?;
    restore_var_assignment(ctx);
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(test_bb)?;
    }

    ctx.builder.position_at_end(orelse_bb);
    orelse(generator, ctx)?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(cont_bb)?;
    }

    restore_var_assignment(ctx);
    ctx.builder.position_at_end(cont_bb);
    ctx.loop_target = loop_bb;

    Ok(())
}

/// See [`CodeGenerator::gen_while`].
pub fn gen_while<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::While { test, body, orelse, .. } = &stmt.node else { codegen_unreachable!(ctx) };

    gen_while_callback(
        generator,
        ctx,
        None,
        |generator, ctx| {
            generator
                .gen_expr(ctx, test)?
                .to_basic_value_enum(ctx)
                .map(BasicValueEnum::into_int_value)
        },
        |generator, ctx| {
            generator.gen_block(ctx, body.iter())?;

            Ok(())
        },
        |generator, ctx| generator.gen_block(ctx, orelse.iter()),
    )?;

    Ok(())
}

/// Generates a C-style chained-`if` construct using lambdas, similar to the following C code:
///
/// ```c
/// T val;
/// if (cond_fn()) {
///   val = then_fn();
/// } else {
///   val = else_fn();
/// }
/// ```
pub fn gen_if_else_expr_callback<'ctx, 'a, G, CondFn, ThenFn, ElseFn, R>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    cond_fn: CondFn,
    then_fn: ThenFn,
    else_fn: ElseFn,
) -> anyhow::Result<Option<R>>
where
    G: ?Sized,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<IntValue<'ctx>>,
    ThenFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<Option<R>>,
    ElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<Option<R>>,
    R: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug>,
{
    let current_bb = ctx.builder.get_insert_block().unwrap();

    let then_bb = ctx.ctx.insert_basic_block_after(current_bb, "if.then");
    let else_bb = ctx.ctx.insert_basic_block_after(then_bb, "if.else");
    let end_bb = ctx.ctx.insert_basic_block_after(else_bb, "if.end");

    let cond = cond_fn(generator, ctx)?;
    assert_eq!(cond.get_type().get_bit_width(), ctx.i1.get_bit_width());
    ctx.builder.build_conditional_branch(cond, then_bb, else_bb)?;

    ctx.builder.position_at_end(then_bb);
    let then_val = then_fn(generator, ctx)?;
    let then_end_bb = ctx.builder.get_insert_block().unwrap();
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(end_bb)?;
    }

    ctx.builder.position_at_end(else_bb);
    let else_val = else_fn(generator, ctx)?;
    let else_end_bb = ctx.builder.get_insert_block().unwrap();
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(end_bb)?;
    }

    ctx.builder.position_at_end(end_bb);
    let phi = match (then_val, else_val) {
        (Some(tv), Some(ev)) => {
            let tv_ty = tv.as_basic_value_enum().get_type();
            assert_eq!(tv_ty, ev.as_basic_value_enum().get_type());

            let phi = ctx.builder.build_phi(tv_ty, "")?;
            phi.add_incoming(&[(&tv, then_end_bb), (&ev, else_end_bb)]);

            Some(phi.as_basic_value().try_into().unwrap())
        }
        (Some(tv), None) => Some(tv),
        (None, Some(ev)) => Some(ev),
        (None, None) => None,
    };

    Ok(phi)
}

/// Generates a C-style chained-`if` construct using lambdas, similar to the following C code:
///
/// ```c
/// if (cond_fn()) {
///   then_fn();
/// } else {
///   else_fn();
/// }
/// ```
pub fn gen_if_callback<'ctx, 'a, G, CondFn, ThenFn, ElseFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    cond_fn: CondFn,
    then_fn: ThenFn,
    else_fn: ElseFn,
) -> anyhow::Result<()>
where
    G: ?Sized,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<IntValue<'ctx>>,
    ThenFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
    ElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> anyhow::Result<()>,
{
    gen_if_else_expr_callback(
        generator,
        ctx,
        cond_fn,
        |generator, ctx| {
            then_fn(generator, ctx)?;
            Ok(None::<BasicValueEnum<'ctx>>)
        },
        |generator, ctx| {
            else_fn(generator, ctx)?;
            Ok(None)
        },
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

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();

    let current = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
    let test_bb = ctx.ctx.append_basic_block(current, "if.test");
    let body_bb = ctx.ctx.append_basic_block(current, "if.body");
    let mut cont_bb = None;
    // if there is no orelse, we just go to cont_bb
    let orelse_bb = if orelse.is_empty() {
        cont_bb = Some(ctx.ctx.append_basic_block(current, "if.cont"));
        cont_bb.unwrap()
    } else {
        ctx.ctx.append_basic_block(current, "if.orelse")
    };
    ctx.builder.build_unconditional_branch(test_bb)?;
    ctx.builder.position_at_end(test_bb);
    let test = generator
        .gen_expr(ctx, test)?
        .val
        .map(|val| val.to_basic_value_enum(ctx, test.custom.unwrap()))
        .transpose()?;
    if let Some(BasicValueEnum::IntValue(test)) = test {
        ctx.builder.build_conditional_branch(bool_to_i1(ctx, test)?, body_bb, orelse_bb)?;
    }
    ctx.builder.position_at_end(body_bb);
    generator.gen_block(ctx, body.iter())?;
    for (k, VarValue { counter, .. }) in &var_assignment {
        let VarValue { static_value: static_val, counter: counter2, .. } =
            ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }

    if !ctx.is_terminated() {
        if cont_bb.is_none() {
            cont_bb = Some(ctx.ctx.append_basic_block(current, "cont"));
        }
        ctx.builder.build_unconditional_branch(cont_bb.unwrap())?;
    }
    if !orelse.is_empty() {
        ctx.builder.position_at_end(orelse_bb);
        generator.gen_block(ctx, orelse.iter())?;
        if !ctx.is_terminated() {
            if cont_bb.is_none() {
                cont_bb = Some(ctx.ctx.append_basic_block(current, "cont"));
            }
            ctx.builder.build_unconditional_branch(cont_bb.unwrap())?;
        }
    }
    if let Some(cont_bb) = cont_bb {
        ctx.builder.position_at_end(cont_bb);
    }
    for (k, VarValue { counter, .. }) in &var_assignment {
        let VarValue { static_value: static_val, counter: counter2, .. } =
            ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }

    Ok(())
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
        typed_store(ctx.builder, *final_state, target.get_address().unwrap())?;
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
    let defs = ctx.top_level.definitions.read();
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
    loc: Location,
) -> anyhow::Result<()> {
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
        let exception = ctx.builder.build_pointer_cast(exception.value, ctx.ptr, "")?;
        ctx.build_call_or_invoke(&raise, &[exception.into()], "raise")?;
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
        let final_state =
            ctx.build_allocate(AllocationScope::Default, ptr_type, Some("try.final_state.addr"))?;
        final_data = Some((final_state, Vec::new(), Vec::new()));
        if let Some((continue_target, break_target)) = ctx.loop_target {
            let break_proxy = ctx.ctx.append_basic_block(current_fun, "try.break");
            let continue_proxy = ctx.ctx.append_basic_block(current_fun, "try.continue");
            final_proxy(ctx, break_target, break_proxy, final_data.as_mut().unwrap())?;
            final_proxy(ctx, continue_target, continue_proxy, final_data.as_mut().unwrap())?;
            old_loop_target = ctx.loop_target.replace((continue_proxy, break_proxy));
        }
        let return_proxy = ctx.ctx.append_basic_block(current_fun, "try.return");
        if let Some(return_target) = ctx.return_target {
            final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap())?;
        } else {
            let return_target = ctx.ctx.append_basic_block(current_fun, "try.return_target");
            ctx.builder.position_at_end(return_target);
            let return_value = ctx
                .return_buffer
                .map(|v| anyhow::Ok(ctx.builder.build_load(v, "$ret")?))
                .transpose()?;
            ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue))?;
            ctx.builder.position_at_end(current_block);
            final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap())?;
        }
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
            &ctx.top_level.definitions.read(),
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
    ctx.return_target = old_return;
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
    let return_target = ctx.return_target.take().map_or_else(
        || {
            let doreturn = ctx.ctx.append_basic_block(current_fun, "try.doreturn");
            ctx.builder.position_at_end(doreturn);
            let return_value = ctx
                .return_buffer
                .map(|v| anyhow::Ok(ctx.builder.build_load(v, "$ret")?))
                .transpose()?;
            ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue))?;
            anyhow::Ok(doreturn)
        },
        Ok,
    )?;
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
            let exn_store =
                ctx.build_allocate(AllocationScope::Default, exn_ty, Some("try.exn_store.addr"))?;
            ctx.var_assignment
                .insert(*name, VarValue::new(exn_store, type_.as_ref().unwrap().custom.unwrap()));
            typed_store(ctx.builder, exn_store, exn.as_basic_value())?;
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
            let dispatcher_cont = ctx.ctx.append_basic_block(current_fun, "try.dispatcher_cont");
            let actual_id = exnid.unwrap();
            let expected_id = ctx
                .builder
                .build_load(exn_type.into_pointer_value(), "expected_id")?
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
            let dest = ctx.builder.build_load(final_state, "final_dest")?;
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
                    typed_store(ctx.builder, final_state, tail.get_address().unwrap())?;
                }
                ctx.builder.build_unconditional_branch(finalizer)?;
            }
        }
        ctx.builder.position_at_end(tail);
    }

    Ok(())
}

/// See [`CodeGenerator::gen_with`].
pub fn gen_with<'ctx, 'a, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    let StmtKind::With { items, body, .. } = &stmt.node else { codegen_unreachable!(ctx) };
    let mut exits = vec![];
    let mut enters = vec![];

    // prepare enters and exits
    for item in items {
        // evaluate the expression first
        let expr_ty = item.context_expr.custom.unwrap();
        let expr = generator.gen_expr(ctx, &item.context_expr)?.val.unwrap();

        // get the __enter__ method signature and ID
        let TypeEnum::TObj { obj_id, fields, .. } = &*ctx.unifier.get_ty(expr_ty) else {
            codegen_unreachable!(ctx)
        };
        let top_level_defs = ctx.top_level.definitions.read();
        let TopLevelDef::Class { methods, .. } = &*top_level_defs[obj_id.0].read() else {
            codegen_unreachable!(ctx)
        };
        let enter_fun_id = methods
            .iter()
            .find(|method| method.0 == "__enter__".into())
            .map(|method| method.2)
            .unwrap();
        let enter = fields.get(&"__enter__".into()).copied().unwrap();
        let TypeEnum::TFunc(enter_signature) = &*ctx.unifier.get_ty(enter.0) else {
            codegen_unreachable!(ctx)
        };

        enters.push((
            expr_ty,
            expr.clone(),
            enter_signature.clone(),
            enter_fun_id,
            item.optional_vars.clone(),
        ));

        // save __exit__() data to be called later in final stage
        let exit_fun_id = methods
            .iter()
            .find(|method| method.0 == "__exit__".into())
            .map(|method| method.2)
            .unwrap();
        let exit = fields.get(&"__exit__".into()).copied().unwrap();
        let TypeEnum::TFunc(exit_signature) = &*ctx.unifier.get_ty(exit.0) else {
            codegen_unreachable!(ctx)
        };
        // stack the exits as the exit order is opposite of enter
        // would be best to reuse try...finally but re-building Stmt vec seems infeasible
        exits.push((expr_ty, expr, exit_signature.clone(), exit_fun_id));
    }

    let body_gen_lambda = |ctx: &mut CodeGenContext<'ctx, 'a>, generator: &mut G| {
        for enter in &enters {
            // call __enter__()
            let enter_ret = generator.gen_call(
                ctx,
                Some((enter.0, enter.1.clone())),
                (&enter.2, enter.3),
                Vec::default(),
            )?;

            // deal with assignments (`as`)
            if let Some(optional_vars) = &enter.4 {
                generator.gen_assign(
                    ctx,
                    optional_vars,
                    &enter_ret.unwrap().into(),
                    enter.2.ret,
                )?;
            }
        }

        // generate the `with` body
        generator.gen_block(ctx, body.iter())
    };

    let exit_gen_lambda = |ctx: &mut CodeGenContext<'ctx, 'a>, generator: &mut G| {
        // call __exit__()s in the reverse order
        for exit in exits.iter().rev() {
            generator.gen_call(
                ctx,
                Some((exit.0, exit.1.clone())),
                (&exit.2, exit.3),
                Vec::default(),
            )?;
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
    let final_state =
        ctx.build_allocate(AllocationScope::Default, ptr_type, Some("with.final_state.addr"))?;
    let mut final_data = Some((final_state, Vec::new(), Vec::new()));
    if let Some((continue_target, break_target)) = ctx.loop_target {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "with.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "with.continue");
        final_proxy(ctx, break_target, break_proxy, final_data.as_mut().unwrap())?;
        final_proxy(ctx, continue_target, continue_proxy, final_data.as_mut().unwrap())?;
        old_loop_target = ctx.loop_target.replace((continue_proxy, break_proxy));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "with.return");
    if let Some(return_target) = ctx.return_target {
        final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap())?;
    } else {
        let return_target = ctx.ctx.append_basic_block(current_fun, "with.return_target");
        ctx.builder.position_at_end(return_target);
        let return_value = ctx
            .return_buffer
            .map(|v| anyhow::Ok(ctx.builder.build_load(v, "$ret")?))
            .transpose()?;
        ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue))?;
        ctx.builder.position_at_end(current_block);
        final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap())?;
    }
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
    let return_target = ctx.return_target.take().map_or_else(
        || {
            let doreturn = ctx.ctx.append_basic_block(current_fun, "with.doreturn");
            ctx.builder.position_at_end(doreturn);
            let return_value = ctx
                .return_buffer
                .map(|v| anyhow::Ok(ctx.builder.build_load(v, "$ret")?))
                .transpose()?;
            ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue))?;
            anyhow::Ok(doreturn)
        },
        Ok,
    )?;
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
    let finalizer = ctx.ctx.append_basic_block(current_fun, "with.exits");
    ctx.builder.position_at_end(finalizer);
    exit_gen_lambda(ctx, generator)?;
    let dest = ctx.builder.build_load(final_state, "final_dest")?;
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
                typed_store(ctx.builder, final_state, tail.get_address().unwrap())?;
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

    if let Some(return_target) = ctx.return_target {
        if let Some(value) = value {
            typed_store(ctx.builder, ctx.return_buffer.unwrap(), value)?;
        }
        ctx.builder.build_unconditional_branch(return_target)?;
    } else {
        // TODO(Derppening): Remove once all LLVM pointers are migrated to opaque pointers
        let value = value
            .map(|v| {
                anyhow::Ok(if v.is_pointer_value() && v.get_type() != ctx.ptr.into() {
                    ctx.builder
                        .build_pointer_cast(v.into_pointer_value(), ctx.ptr, "cast_ret")?
                        .as_basic_value_enum()
                } else {
                    v
                })
            })
            .transpose()?;
        let value = value.as_ref().map(|v| v as &dyn BasicValue);
        ctx.builder.build_return(value)?;
    }
    Ok(())
}

/// See [`CodeGenerator::gen_stmt`].
pub fn gen_stmt<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> anyhow::Result<()> {
    ctx.current_loc = stmt.location;

    let loc = ctx.debug_info.0.create_debug_location(
        ctx.ctx,
        ctx.current_loc.row as u32,
        ctx.current_loc.column as u32,
        ctx.debug_info.2,
        None,
    );
    ctx.builder.set_current_debug_location(loc);

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
                generator.gen_assign(ctx, target, &value_enum.clone(), value.custom.unwrap())?;
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
                stmt.location,
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
                    let def = ctx.top_level.definitions.read();
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
                gen_raise(ctx, Some(&exc), stmt.location)?;
            } else {
                gen_raise(ctx, None, stmt.location)?;
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
                stmt.location,
            )?;
        }
        _ => unimplemented!(),
    }
    Ok(())
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
