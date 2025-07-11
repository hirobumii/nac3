use inkwell::{
    IntPredicate,
    basic_block::BasicBlock,
    builder::Builder,
    module::Module,
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue},
};
use itertools::{Itertools, izip};

use nac3parser::ast::{
    Constant, ExcepthandlerKind, Expr, ExprKind, Location, Stmt, StmtKind, StrRef,
};

use super::{
    CodeGenContext, CodeGenerator,
    expr::{destructure_range, gen_binop_expr},
    gen_in_range_check,
    irrt::{handle_slice_indices, list_slice_assignment},
    llvm_intrinsics::call_memcpy_generic_array,
    macros::codegen_unreachable,
    types::{ExceptionType, ListType, RangeType, ndarray::NDArrayType},
    values::{
        ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue, ExceptionValue, ListValue, ProxyValue,
        ndarray::{RustNDIndex, ScalarOrNDArray},
        structure::StructProxyValue,
    },
};
use crate::{
    codegen::llvm_fns::FunctionDecl,
    symbol_resolver::ValueEnum,
    toplevel::{DefinitionId, TopLevelContext, TopLevelDef},
    typecheck::{
        magic_methods::Binop,
        typedef::{FunSignature, Type, TypeEnum, iter_type_vars},
    },
};

pub(crate) fn get_personality<'ctx>(
    top_level: &TopLevelContext,
    module: &Module<'ctx>,
) -> Option<FunctionValue<'ctx>> {
    let sym = top_level.personality_symbol.as_ref()?;
    // The personality is the only symbol where we do not use our external function ABI handling.
    Some(module.get_function(sym).unwrap_or_else(|| {
        module.add_function(sym, module.get_context().i32_type().fn_type(&[], true), None)
    }))
}

/// See [`CodeGenerator::gen_var_alloc`].
pub fn gen_var<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ty: BasicTypeEnum<'ctx>,
    name: Option<&str>,
) -> Result<PointerValue<'ctx>, String> {
    // Restore debug location
    let di_loc = ctx.debug_info.0.create_debug_location(
        ctx.ctx,
        ctx.current_loc.row as u32,
        ctx.current_loc.column as u32,
        ctx.debug_info.2,
        None,
    );

    // put the alloca in init block
    let current = ctx.builder.get_insert_block().unwrap();

    // position before the last branching instruction...
    ctx.builder.position_before(&ctx.init_bb.get_last_instruction().unwrap());
    ctx.builder.set_current_debug_location(di_loc);

    let ptr = ctx.builder.build_alloca(ty, name.unwrap_or("")).unwrap();

    ctx.builder.position_at_end(current);
    ctx.builder.set_current_debug_location(di_loc);

    Ok(ptr)
}

/// See [`CodeGenerator::gen_array_var_alloc`].
pub fn gen_array_var<'ctx, 'a, T: BasicType<'ctx>>(
    ctx: &mut CodeGenContext<'ctx, 'a>,
    ty: T,
    size: IntValue<'ctx>,
    name: Option<&'ctx str>,
) -> Result<ArraySliceValue<'ctx>, String> {
    // Restore debug location
    let di_loc = ctx.debug_info.0.create_debug_location(
        ctx.ctx,
        ctx.current_loc.row as u32,
        ctx.current_loc.column as u32,
        ctx.debug_info.2,
        None,
    );

    // put the alloca in init block
    let current = ctx.builder.get_insert_block().unwrap();

    // position before the last branching instruction...
    ctx.builder.position_before(&ctx.init_bb.get_last_instruction().unwrap());
    ctx.builder.set_current_debug_location(di_loc);

    let ptr = ctx.builder.build_array_alloca(ty, size, name.unwrap_or("")).unwrap();
    let ptr = ArraySliceValue::from_ptr_val(ptr, size, name);

    ctx.builder.position_at_end(current);
    ctx.builder.set_current_debug_location(di_loc);

    Ok(ptr)
}

/// See [`CodeGenerator::gen_store_target`].
pub fn gen_store_target<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    pattern: &Expr<Option<Type>>,
    name: Option<&str>,
) -> Result<Option<PointerValue<'ctx>>, String> {
    // very similar to gen_expr, but we don't do an extra load at the end
    // and we flatten nested tuples
    Ok(Some(match &pattern.node {
        ExprKind::Name { id, .. } => match ctx.var_assignment.get(id) {
            None => {
                let ptr_ty = ctx.get_llvm_type(generator, pattern.custom.unwrap());
                let ptr = generator.gen_var_alloc(ctx, ptr_ty, name)?;
                ctx.var_assignment.insert(*id, (ptr, None, 0));
                ptr
            }
            Some(v) => {
                let (ptr, counter) = (v.0, v.2);
                ctx.var_assignment.insert(*id, (ptr, None, counter));
                ptr
            }
        },
        ExprKind::Attribute { value, attr, .. } => {
            let (index, _) = ctx.get_attr_index(value.custom.unwrap(), *attr);
            let val = if let Some(v) = generator.gen_expr(ctx, value)? {
                v.to_basic_value_enum(ctx, generator, value.custom.unwrap())?
            } else {
                return Ok(None);
            };
            let BasicValueEnum::PointerValue(ptr) = val else {
                codegen_unreachable!(ctx);
            };
            unsafe {
                ctx.builder.build_in_bounds_gep(
                    ptr,
                    &[
                        ctx.ctx.i32_type().const_zero(),
                        ctx.ctx.i32_type().const_int(index as u64, false),
                    ],
                    name.unwrap_or(""),
                )
            }
            .unwrap()
        }
        _ => codegen_unreachable!(ctx),
    }))
}

/// See [`CodeGenerator::gen_assign`].
pub fn gen_assign<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    target: &Expr<Option<Type>>,
    value: ValueEnum<'ctx>,
    value_ty: Type,
) -> Result<(), String> {
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
                let (_, static_value, counter) = ctx.var_assignment.get_mut(id).unwrap();
                *counter += 1;
                if let ValueEnum::Static(s) = &value {
                    *static_value = Some(s.clone());
                }
            }
            let val = value.to_basic_value_enum(ctx, generator, target.custom.unwrap())?;

            // Perform i1 <-> i8 conversion as needed
            let val = if ctx.unifier.unioned(target.custom.unwrap(), ctx.primitives.bool) {
                generator.bool_to_i8(ctx, val.into_int_value()).into()
            } else {
                val
            };

            ctx.builder.build_store(ptr, val).unwrap();
        }
    }
    Ok(())
}

/// See [`CodeGenerator::gen_assign_target_list`].
pub fn gen_assign_target_list<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    targets: &[Expr<Option<Type>>],
    value: ValueEnum<'ctx>,
    value_ty: Type,
) -> Result<(), String> {
    let llvm_usize = ctx.get_size_type();
    match &*ctx.unifier.get_ty(value_ty) {
        TypeEnum::TTuple { ty: tuple_tys, .. } => {
            // Deconstruct the tuple `value`
            let tuple = value.to_basic_value_enum(ctx, generator, value_ty)?.into_struct_value();

            assert_eq!(tuple.get_type().count_fields() as usize, tuple_tys.len());

            let tuple = (0..tuple.get_type().count_fields())
                .map(|i| ctx.builder.build_extract_value(tuple, i, "").unwrap())
                .collect_vec();

            // Find the starred target if it exists.
            // Index of the "starred" target. If it exists, there may only be one.
            let mut starred_target_index = None;
            for (i, target) in targets.iter().enumerate() {
                if matches!(target.node, ExprKind::Starred { .. }) {
                    // Ensured by typechecker
                    assert!(starred_target_index.replace(i).is_none());
                }
            }

            // Unlike in python we require the starred target to consume one or more items when the
            // RHS is a tuple. Otherwise, it would be impossible type the starred target.
            if tuple_tys.len() < targets.len() {
                return Err(format!(
                    "Tuple unpacking requires at least as many values as targets including the starred target, but got {} values and {} targets",
                    tuple_tys.len(),
                    targets.len()
                ));
            }

            // tuple[before_idx..after_idx] is assigned to the starred target
            let starred_target_index = starred_target_index.unwrap_or(tuple_tys.len());
            let before_idx = starred_target_index;
            let after_idx = (tuple_tys.len() + starred_target_index + 1) - targets.len();

            // Handle assignments up to starred target - if no starred target, this is all assignments
            for (target, val, val_ty) in izip!(
                &targets[..starred_target_index],
                &tuple[..before_idx],
                &tuple_tys[..before_idx]
            ) {
                generator.gen_assign(ctx, target, ValueEnum::Dynamic(*val), *val_ty)?;
            }

            // If there is a starred target we need to handle it, and then assignements after it.
            let Some(ExprKind::Starred { value: target, .. }) =
                &targets.get(starred_target_index).map(|v| &v.node)
            else {
                return Ok(());
            };

            let ty = tuple[before_idx].get_type();

            // nac3 lists can only contain one type, hence starred targets can only contain one type
            // This is previously checked by the typechecker
            debug_assert!(
                tuple_tys[before_idx..after_idx]
                    .iter()
                    .all(|t| ctx.unifier.unioned(*t, tuple_tys[before_idx])),
                "Starred target must have same type for all items"
            );

            let starred_list = ListType::new(ctx, &ty).construct(
                generator,
                ctx,
                llvm_usize.const_int((after_idx - before_idx) as u64, false),
                Some("starred_list"),
            );

            for (i, val) in tuple[before_idx..after_idx].iter().enumerate() {
                let ptr = starred_list.data().ptr_offset(
                    ctx,
                    generator,
                    &llvm_usize.const_int(i as u64, false),
                    None,
                );
                ctx.builder.build_store(ptr, *val).unwrap();
            }

            let target_ptr =
                generator.gen_store_target(ctx, target, Some("starred_target.addr"))?.unwrap();
            ctx.builder.build_store(target_ptr, starred_list.get_pointer_value(ctx)).unwrap();

            // Handle assignment after the starred target
            for (target, val, val_ty) in izip!(
                &targets[starred_target_index + 1..],
                &tuple[after_idx..],
                &tuple_tys[after_idx..]
            ) {
                generator.gen_assign(ctx, target, ValueEnum::Dynamic(*val), *val_ty)?;
            }
        }
        TypeEnum::TObj { params, .. } => {
            let BasicValueEnum::PointerValue(list_ptr) =
                value.to_basic_value_enum(ctx, generator, value_ty)?
            else {
                codegen_unreachable!(ctx);
            };

            let rhs_list =
                ListValue::from_pointer_value(list_ptr, ctx.get_size_type(), Some("rhs_list"));
            let rhs_size = rhs_list.load_size(ctx, Some("rhs_size"));
            let starred_idx =
                targets.iter().position(|t| matches!(t.node, ExprKind::Starred { .. }));

            // Check if the lhs targets contain a starred target
            if let Some(starred_idx) = starred_idx {
                // Get the targets before and after the starred target
                let before = targets.iter().enumerate().take(starred_idx);
                let after = targets.iter().skip(starred_idx + 1).rev();

                let unstarred_size =
                    llvm_usize.const_int(before.len() as u64 + after.len() as u64, false);
                ctx.make_assert(
                    generator,
                    ctx.builder
                        .build_int_compare(
                            IntPredicate::ULE,
                            unstarred_size,
                            rhs_size,
                            "list_size_check",
                        )
                        .unwrap(),
                    "ValueError",
                    "too few values to unpack (expected at least {0}, got {1})",
                    [Some(unstarred_size), Some(rhs_size), None],
                    targets[0].location,
                );

                // Assign the first n values to the n targets before the starred target
                for (i, target) in before {
                    let target_ptr =
                        generator.gen_store_target(ctx, target, Some("list_target.addr"))?.unwrap();

                    let item_ptr = rhs_list.data().ptr_offset(
                        ctx,
                        generator,
                        &llvm_usize.const_int(i as u64, false),
                        Some("item_ptr"),
                    );
                    let item_val = ctx.builder.build_load(item_ptr, "item_val").unwrap();
                    ctx.builder.build_store(target_ptr, item_val).unwrap();
                }

                // Iterate backwards to assign the last m values to the m targets after the starred target
                let mut idx = rhs_size;
                for target in after {
                    let target_ptr = generator
                        .gen_store_target(ctx, target, Some("list_target.addr2"))?
                        .unwrap();

                    // rhs_size isn't constant so we need to generate the pointer arithemetic
                    idx =
                        ctx.builder.build_int_sub(idx, llvm_usize.const_int(1, false), "").unwrap();

                    let item_ptr =
                        rhs_list.data().ptr_offset(ctx, generator, &idx, Some("item_ptr2"));
                    let item_val = ctx.builder.build_load(item_ptr, "item_val2").unwrap();
                    ctx.builder.build_store(target_ptr, item_val).unwrap();
                }

                // Get the type of the list items
                let Some((_, ty)) = params.first() else {
                    // Lists are always typed; the typechecker ensures this
                    codegen_unreachable!(ctx);
                };

                // Get the number of values to be stored in the starred target
                let starred_size = ctx
                    .builder
                    .build_int_sub(
                        idx,
                        llvm_usize.const_int(starred_idx as u64, false),
                        "starred_size",
                    )
                    .unwrap();

                // Allocate a new list for the starred target and copy the data from the rhs list into it
                let llvm_array_ty = ctx.get_llvm_type(generator, *ty);
                let new_list = ListType::new(ctx, &llvm_array_ty).construct(
                    generator,
                    ctx,
                    starred_size,
                    Some("new_list"),
                );

                let cur_bb = ctx.builder.get_insert_block().unwrap();
                let then_bb = ctx.ctx.insert_basic_block_after(cur_bb, "if.star");
                let after_bb = ctx.ctx.insert_basic_block_after(then_bb, "after.star");

                // Check if starred_size is not zero
                let cond = ctx
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        starred_size,
                        llvm_usize.const_zero(),
                        "is_starred_empty",
                    )
                    .unwrap();

                ctx.builder.build_conditional_branch(cond, after_bb, then_bb).unwrap();

                ctx.builder.position_at_end(then_bb);

                // Get the pointer for where the values to be stored in the starred target begin
                let rest_start_ptr = rhs_list.data().ptr_offset(
                    ctx,
                    generator,
                    &llvm_usize.const_int(starred_idx as u64, false),
                    Some("start_ptr"),
                );
                call_memcpy_generic_array(
                    ctx,
                    new_list.data().base_ptr(ctx, generator),
                    rest_start_ptr,
                    starred_size,
                );

                ctx.builder.build_unconditional_branch(after_bb).unwrap();

                ctx.builder.position_at_end(after_bb);

                // Store the new list in the starred target
                let ExprKind::Starred { value: target, .. } = &targets[starred_idx].node else {
                    codegen_unreachable!(ctx)
                };
                let target_ptr =
                    generator.gen_store_target(ctx, target, Some("list_target.addr3"))?.unwrap();
                ctx.builder.build_store(target_ptr, new_list.get_pointer_value(ctx)).unwrap();
            } else {
                // If no starred target, make sure the number of targets matches the number of items in the list
                let lhs_size = llvm_usize.const_int(targets.len() as u64, false);
                ctx.make_assert(
                    generator,
                    ctx.builder
                        .build_int_compare(IntPredicate::EQ, rhs_size, lhs_size, "list_size_check")
                        .unwrap(),
                    "ValueError",
                    "incorrect number of values to unpack (expected {1})",
                    [Some(rhs_size), Some(lhs_size), None],
                    Location::default(),
                );

                // Assign each item in the list to the corresponding target
                for (i, target) in targets.iter().enumerate() {
                    let target_ptr =
                        generator.gen_store_target(ctx, target, Some("list_target.addr"))?.unwrap();

                    let item_ptr = rhs_list.data().ptr_offset(
                        ctx,
                        generator,
                        &llvm_usize.const_int(i as u64, false),
                        Some("item_ptr"),
                    );
                    let item_val = ctx.builder.build_load(item_ptr, "item_val").unwrap();
                    ctx.builder.build_store(target_ptr, item_val).unwrap();
                }
            }
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
    value: ValueEnum<'ctx>,
    value_ty: Type,
) -> Result<(), String> {
    let target_ty = target.custom.unwrap();
    let key_ty = key.custom.unwrap();

    match &*ctx.unifier.get_ty(target_ty) {
        TypeEnum::TObj { obj_id, params: list_params, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            // Handle list item assignment
            let llvm_usize = ctx.get_size_type();
            let target_item_ty = iter_type_vars(list_params).next().unwrap().ty;

            let target = generator
                .gen_expr(ctx, target)?
                .unwrap()
                .to_basic_value_enum(ctx, generator, target_ty)?
                .into_pointer_value();
            let target = ListValue::from_pointer_value(target, llvm_usize, None);

            if let ExprKind::Slice { .. } = &key.node {
                // Handle assigning to a slice
                let ExprKind::Slice { lower, upper, step } = &key.node else {
                    codegen_unreachable!(ctx)
                };
                let size = target.load_size(ctx, None);
                let Some((start, end, step)) =
                    handle_slice_indices(lower, upper, step, ctx, generator, size)?
                else {
                    return Ok(());
                };

                let value =
                    value.to_basic_value_enum(ctx, generator, value_ty)?.into_pointer_value();
                let value = ListValue::from_pointer_value(value, llvm_usize, None);

                let target_item_ty = ctx.get_llvm_type(generator, target_item_ty);
                let size = value.load_size(ctx, None);
                let Some(src_ind) =
                    handle_slice_indices(&None, &None, &None, ctx, generator, size)?
                else {
                    return Ok(());
                };
                list_slice_assignment(
                    generator,
                    ctx,
                    target_item_ty,
                    target,
                    (start, end, step),
                    value,
                    src_ind,
                );
            } else {
                // Handle assigning to an index
                let len = target.load_size(ctx, Some("len"));

                let index = generator
                    .gen_expr(ctx, key)?
                    .unwrap()
                    .to_basic_value_enum(ctx, generator, key_ty)?
                    .into_int_value();
                let index =
                    ctx.builder.build_int_s_extend(index, ctx.get_size_type(), "sext").unwrap();

                // handle negative index
                let is_negative = ctx
                    .builder
                    .build_int_compare(
                        IntPredicate::SLT,
                        index,
                        ctx.get_size_type().const_zero(),
                        "is_neg",
                    )
                    .unwrap();
                let adjusted = ctx.builder.build_int_add(index, len, "adjusted").unwrap();
                let index = ctx
                    .builder
                    .build_select(is_negative, adjusted, index, "index")
                    .map(BasicValueEnum::into_int_value)
                    .unwrap();

                // unsigned less than is enough, because negative index after adjustment is
                // bigger than the length (for unsigned cmp)
                let bound_check = ctx
                    .builder
                    .build_int_compare(IntPredicate::ULT, index, len, "inbound")
                    .unwrap();
                ctx.make_assert(
                    generator,
                    bound_check,
                    "0:IndexError",
                    "index {0} out of bounds 0:{1}",
                    [Some(index), Some(len), None],
                    key.location,
                );

                // Write value to index on list
                let item_ptr =
                    target.data().ptr_offset(ctx, generator, &index, Some("list_item_ptr"));
                let value = value.to_basic_value_enum(ctx, generator, value_ty)?;
                ctx.builder.build_store(item_ptr, value).unwrap();
            }
        }
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
        {
            // Handle NDArray item assignment
            // Process target
            let target = generator
                .gen_expr(ctx, target)?
                .unwrap()
                .to_basic_value_enum(ctx, generator, target_ty)?;

            // Process key
            let key = RustNDIndex::from_subscript_expr(generator, ctx, key)?;

            // Process value
            let value = value.to_basic_value_enum(ctx, generator, value_ty)?;

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

            let target = NDArrayType::from_unifier_type(generator, ctx, target_ty)
                .map_pointer_value(target.into_pointer_value(), None);
            let target = target.index(generator, ctx, &key);

            let value = ScalarOrNDArray::from_value(generator, ctx, (value_ty, value))
                .to_ndarray(generator, ctx);

            let broadcast_ndims =
                [target.get_type().ndims(), value.get_type().ndims()].into_iter().max().unwrap();
            let broadcast_result = NDArrayType::new(
                ctx,
                value.get_type().element_type(),
                broadcast_ndims,
            )
            .broadcast(generator, ctx, &[target, value]);

            let target = broadcast_result.ndarrays[0];
            let value = broadcast_result.ndarrays[1];

            target.copy_data_from(ctx, value);
        }
        _ => {
            panic!("encountered unknown target type: {}", ctx.unifier.stringify(target_ty));
        }
    }
    Ok(())
}

/// See [`CodeGenerator::gen_for`].
pub fn gen_for<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> Result<(), String> {
    let StmtKind::For { iter, target, body, orelse, .. } = &stmt.node else {
        codegen_unreachable!(ctx)
    };

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();

    let int32 = ctx.ctx.i32_type();
    let size_t = ctx.get_size_type();
    let zero = int32.const_zero();
    let current = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
    let body_bb = ctx.ctx.append_basic_block(current, "for.body");
    let cont_bb = ctx.ctx.append_basic_block(current, "for.end");
    // if there is no orelse, we just go to cont_bb
    let orelse_bb =
        if orelse.is_empty() { cont_bb } else { ctx.ctx.append_basic_block(current, "for.orelse") };

    // The BB containing the increment expression
    let incr_bb = ctx.ctx.append_basic_block(current, "for.incr");
    // The BB containing the loop condition check
    let cond_bb = ctx.ctx.append_basic_block(current, "for.cond");

    // store loop bb information and restore it later
    let loop_bb = ctx.loop_target.replace((incr_bb, cont_bb));

    let iter_ty = iter.custom.unwrap();
    let iter_val = if let Some(v) = generator.gen_expr(ctx, iter)? {
        v.to_basic_value_enum(ctx, generator, iter_ty)?
    } else {
        return Ok(());
    };

    match &*ctx.unifier.get_ty(iter_ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.range.obj_id(&ctx.unifier).unwrap() =>
        {
            let iter_val =
                RangeType::new(ctx).map_pointer_value(iter_val.into_pointer_value(), Some("range"));
            // Internal variable for loop; Cannot be assigned
            let i = generator.gen_var_alloc(ctx, int32.into(), Some("for.i.addr"))?;
            // Variable declared in "target" expression of the loop; Can be reassigned *or* shadowed
            let Some(target_i) =
                generator.gen_store_target(ctx, target, Some("for.target.addr"))?
            else {
                codegen_unreachable!(ctx)
            };
            let (start, stop, step) = destructure_range(ctx, iter_val);

            ctx.builder.build_store(i, start).unwrap();

            // Check "If step is zero, ValueError is raised."
            let rangenez = ctx
                .builder
                .build_int_compare(IntPredicate::NE, step, int32.const_zero(), "")
                .unwrap();
            ctx.make_assert(
                generator,
                rangenez,
                "ValueError",
                "range() arg 3 must not be zero",
                [None, None, None],
                ctx.current_loc,
            );
            ctx.builder.build_unconditional_branch(cond_bb).unwrap();

            {
                ctx.builder.position_at_end(cond_bb);
                ctx.builder
                    .build_conditional_branch(
                        gen_in_range_check(
                            ctx,
                            ctx.builder
                                .build_load(i, "")
                                .map(BasicValueEnum::into_int_value)
                                .unwrap(),
                            stop,
                            step,
                        ),
                        body_bb,
                        orelse_bb,
                    )
                    .unwrap();
            }

            ctx.builder.position_at_end(incr_bb);
            let next_i = ctx
                .builder
                .build_int_add(
                    ctx.builder.build_load(i, "").map(BasicValueEnum::into_int_value).unwrap(),
                    step,
                    "inc",
                )
                .unwrap();
            ctx.builder.build_store(i, next_i).unwrap();
            ctx.builder.build_unconditional_branch(cond_bb).unwrap();

            ctx.builder.position_at_end(body_bb);
            ctx.builder
                .build_store(
                    target_i,
                    ctx.builder.build_load(i, "").map(BasicValueEnum::into_int_value).unwrap(),
                )
                .unwrap();
            generator.gen_block(ctx, body.iter())?;
        }
        TypeEnum::TObj { obj_id, params: list_params, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            let index_addr = generator.gen_var_alloc(ctx, size_t.into(), Some("for.index.addr"))?;
            ctx.builder.build_store(index_addr, size_t.const_zero()).unwrap();
            let len = ctx
                .build_gep_and_load(
                    iter_val.into_pointer_value(),
                    &[zero, int32.const_int(1, false)],
                    Some("len"),
                )
                .into_int_value();
            ctx.builder.build_unconditional_branch(cond_bb).unwrap();

            ctx.builder.position_at_end(cond_bb);
            let index = ctx
                .builder
                .build_load(index_addr, "for.index")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let cmp = ctx.builder.build_int_compare(IntPredicate::SLT, index, len, "cond").unwrap();
            ctx.builder.build_conditional_branch(cmp, body_bb, orelse_bb).unwrap();

            ctx.builder.position_at_end(incr_bb);
            let index =
                ctx.builder.build_load(index_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let inc = ctx.builder.build_int_add(index, size_t.const_int(1, true), "inc").unwrap();
            ctx.builder.build_store(index_addr, inc).unwrap();
            ctx.builder.build_unconditional_branch(cond_bb).unwrap();

            ctx.builder.position_at_end(body_bb);
            let arr_ptr = ctx
                .build_gep_and_load(iter_val.into_pointer_value(), &[zero, zero], Some("arr.addr"))
                .into_pointer_value();
            let index = ctx
                .builder
                .build_load(index_addr, "for.index")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let val = ctx.build_gep_and_load(arr_ptr, &[index], Some("val"));
            let val_ty = iter_type_vars(list_params).next().unwrap().ty;
            generator.gen_assign(ctx, target, val.into(), val_ty)?;
            generator.gen_block(ctx, body.iter())?;
        }
        _ => {
            panic!("unsupported for loop iterator type: {}", ctx.unifier.stringify(iter_ty));
        }
    }

    for (k, (_, _, counter)) in &var_assignment {
        let (_, static_val, counter2) = ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }

    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(incr_bb).unwrap();
    }

    if !orelse.is_empty() {
        ctx.builder.position_at_end(orelse_bb);
        generator.gen_block(ctx, orelse.iter())?;
        if !ctx.is_terminated() {
            ctx.builder.build_unconditional_branch(cont_bb).unwrap();
        }
    }

    for (k, (_, _, counter)) in &var_assignment {
        let (_, static_val, counter2) = ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }

    ctx.builder.position_at_end(cont_bb);
    ctx.loop_target = loop_bb;

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
    pub fn build_break_branch(&self, builder: &Builder<'ctx>) {
        builder.build_unconditional_branch(self.exit_bb).unwrap();
    }

    /// Creates a [`br` instruction][Builder::build_unconditional_branch] to the latch
    /// [`BasicBlock`], as if by calling `continue`.
    pub fn build_continue_branch(&self, builder: &Builder<'ctx>) {
        builder.build_unconditional_branch(self.latch_bb).unwrap();
    }
}

/// Generates a C-style `for` construct using lambdas, similar to the following C code:
///
/// ```c
/// for (x... = init(); cond(x...); update(x...)) {
///     body(x...);
/// }
/// ```
///
/// * `init` - A lambda containing IR statements declaring and initializing loop variables. The
///   return value is a [Clone] value which will be passed to the other lambdas.
/// * `cond` - A lambda containing IR statements checking whether the loop should continue
///   executing. The result value must be an `i1` indicating if the loop should continue.
/// * `body` - A lambda containing IR statements within the loop body.
/// * `update` - A lambda containing IR statements updating loop variables.
pub fn gen_for_callback<'ctx, 'a, G, I, InitFn, CondFn, BodyFn, UpdateFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    init: InitFn,
    cond: CondFn,
    body: BodyFn,
    update: UpdateFn,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    I: Clone,
    InitFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<I, String>,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, I) -> Result<IntValue<'ctx>, String>,
    BodyFn: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        I,
    ) -> Result<(), String>,
    UpdateFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, I) -> Result<(), String>,
{
    let label = label.unwrap_or("for");

    let current_bb = ctx.builder.get_insert_block().unwrap();
    let init_bb = ctx.ctx.insert_basic_block_after(current_bb, &format!("{label}.init"));
    // The BB containing the loop condition check
    let cond_bb = ctx.ctx.insert_basic_block_after(init_bb, &format!("{label}.cond"));
    let body_bb = ctx.ctx.insert_basic_block_after(cond_bb, &format!("{label}.body"));
    // The BB containing the increment expression
    let update_bb = ctx.ctx.insert_basic_block_after(body_bb, &format!("{label}.update"));
    let cont_bb = ctx.ctx.insert_basic_block_after(update_bb, &format!("{label}.end"));

    // store loop bb information and restore it later
    let loop_bb = ctx.loop_target.replace((update_bb, cont_bb));

    ctx.builder.build_unconditional_branch(init_bb).unwrap();

    ctx.builder.position_at_end(init_bb);
    let loop_var = init(generator, ctx)?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(cond_bb).unwrap();
    }

    ctx.builder.position_at_end(cond_bb);
    let cond = cond(generator, ctx, loop_var.clone())?;
    assert_eq!(cond.get_type().get_bit_width(), ctx.ctx.bool_type().get_bit_width());
    if !ctx.is_terminated() {
        ctx.builder.build_conditional_branch(cond, body_bb, cont_bb).unwrap();
    }

    ctx.builder.position_at_end(body_bb);
    let hooks = BreakContinueHooks { exit_bb: cont_bb, latch_bb: update_bb };
    body(generator, ctx, hooks, loop_var.clone())?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(update_bb).unwrap();
    }

    ctx.builder.position_at_end(update_bb);
    update(generator, ctx, loop_var)?;
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(cond_bb).unwrap();
    }

    ctx.builder.position_at_end(cont_bb);
    ctx.loop_target = loop_bb;

    Ok(())
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
pub fn gen_for_callback_incrementing<'ctx, 'a, G, BodyFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    init_val: IntValue<'ctx>,
    max_val: (IntValue<'ctx>, bool),
    body: BodyFn,
    incr_val: IntValue<'ctx>,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    BodyFn: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        IntValue<'ctx>,
    ) -> Result<(), String>,
{
    let init_val_t = init_val.get_type();

    gen_for_callback(
        generator,
        ctx,
        label,
        |generator, ctx| {
            let i_addr = generator.gen_var_alloc(ctx, init_val_t.into(), None)?;
            ctx.builder.build_store(i_addr, init_val).unwrap();

            Ok(i_addr)
        },
        |_, ctx, i_addr| {
            let cmp_op = if max_val.1 { IntPredicate::ULE } else { IntPredicate::ULT };

            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let max_val =
                ctx.builder.build_int_z_extend_or_bit_cast(max_val.0, init_val_t, "").unwrap();

            Ok(ctx.builder.build_int_compare(cmp_op, i, max_val, "").unwrap())
        },
        |generator, ctx, hooks, i_addr| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value).unwrap();

            body(generator, ctx, hooks, i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let incr_val =
                ctx.builder.build_int_z_extend_or_bit_cast(incr_val, init_val_t, "").unwrap();
            let i = ctx.builder.build_int_add(i, incr_val, "").unwrap();
            ctx.builder.build_store(i_addr, i).unwrap();

            Ok(())
        },
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
#[allow(clippy::too_many_arguments)]
pub fn gen_for_range_callback<'ctx, 'a, G, StartFn, StopFn, StepFn, BodyFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    label: Option<&str>,
    is_unsigned: bool,
    start_fn: StartFn,
    (stop_fn, stop_inclusive): (StopFn, bool),
    step_fn: StepFn,
    body_fn: BodyFn,
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    StartFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<IntValue<'ctx>, String>,
    StopFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<IntValue<'ctx>, String>,
    StepFn: Fn(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<IntValue<'ctx>, String>,
    BodyFn: FnOnce(
        &mut G,
        &mut CodeGenContext<'ctx, 'a>,
        BreakContinueHooks<'ctx>,
        IntValue<'ctx>,
    ) -> Result<(), String>,
{
    let init_val_t = start_fn(generator, ctx).map(IntValue::get_type).unwrap();

    gen_for_callback(
        generator,
        ctx,
        label,
        |generator, ctx| {
            let i_addr = generator.gen_var_alloc(ctx, init_val_t.into(), None)?;

            let start = start_fn(generator, ctx)?;
            ctx.builder.build_store(i_addr, start).unwrap();

            let start = start_fn(generator, ctx)?;
            let stop = stop_fn(generator, ctx)?;
            let stop = if stop.get_type().get_bit_width() == start.get_type().get_bit_width() {
                stop
            } else if is_unsigned {
                ctx.builder.build_int_z_extend(stop, start.get_type(), "").unwrap()
            } else {
                ctx.builder.build_int_s_extend(stop, start.get_type(), "").unwrap()
            };

            let incr = ctx
                .builder
                .build_int_compare(
                    if is_unsigned { IntPredicate::ULE } else { IntPredicate::SLE },
                    start,
                    stop,
                    "",
                )
                .unwrap();

            Ok((i_addr, incr))
        },
        |generator, ctx, (i_addr, incr)| {
            let (lt_cmp_op, gt_cmp_op) = match (is_unsigned, stop_inclusive) {
                (true, true) => (IntPredicate::ULE, IntPredicate::UGE),
                (true, false) => (IntPredicate::ULT, IntPredicate::UGT),
                (false, true) => (IntPredicate::SLE, IntPredicate::SGE),
                (false, false) => (IntPredicate::SLT, IntPredicate::SGT),
            };

            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value).unwrap();
            let stop = stop_fn(generator, ctx)?;
            let stop = if stop.get_type().get_bit_width() == i.get_type().get_bit_width() {
                stop
            } else if is_unsigned {
                ctx.builder.build_int_z_extend(stop, i.get_type(), "").unwrap()
            } else {
                ctx.builder.build_int_s_extend(stop, i.get_type(), "").unwrap()
            };

            let i_lt_end = ctx.builder.build_int_compare(lt_cmp_op, i, stop, "").unwrap();
            let i_gt_end = ctx.builder.build_int_compare(gt_cmp_op, i, stop, "").unwrap();

            let cond = ctx
                .builder
                .build_select(incr, i_lt_end, i_gt_end, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            Ok(cond)
        },
        |generator, ctx, hooks, (i_addr, _)| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value).unwrap();

            body_fn(generator, ctx, hooks, i)
        },
        |generator, ctx, (i_addr, _)| {
            let i = ctx.builder.build_load(i_addr, "").map(BasicValueEnum::into_int_value).unwrap();

            let incr_val = step_fn(generator, ctx)?;
            let incr_val = if incr_val.get_type().get_bit_width() == i.get_type().get_bit_width() {
                incr_val
            } else if is_unsigned {
                ctx.builder.build_int_z_extend(incr_val, i.get_type(), "").unwrap()
            } else {
                ctx.builder.build_int_s_extend(incr_val, i.get_type(), "").unwrap()
            };

            let i = ctx.builder.build_int_add(i, incr_val, "").unwrap();
            ctx.builder.build_store(i_addr, i).unwrap();

            Ok(())
        },
    )
}

/// See [`CodeGenerator::gen_while`].
pub fn gen_while<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> Result<(), String> {
    let StmtKind::While { test, body, orelse, .. } = &stmt.node else { codegen_unreachable!(ctx) };

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();

    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
    let test_bb = ctx.ctx.append_basic_block(current, "while.test");
    let body_bb = ctx.ctx.append_basic_block(current, "while.body");
    let cont_bb = ctx.ctx.append_basic_block(current, "while.cont");
    // if there is no orelse, we just go to cont_bb
    let orelse_bb = if orelse.is_empty() {
        cont_bb
    } else {
        ctx.ctx.append_basic_block(current, "while.orelse")
    };
    // store loop bb information and restore it later
    let loop_bb = ctx.loop_target.replace((test_bb, cont_bb));
    ctx.builder.build_unconditional_branch(test_bb).unwrap();
    ctx.builder.position_at_end(test_bb);
    let test = if let Some(v) = generator.gen_expr(ctx, test)? {
        v.to_basic_value_enum(ctx, generator, test.custom.unwrap())?
    } else {
        for bb in [body_bb, cont_bb] {
            ctx.builder.position_at_end(bb);
            ctx.builder.build_unreachable().unwrap();
        }

        return Ok(());
    };
    let BasicValueEnum::IntValue(test) = test else { codegen_unreachable!(ctx) };

    ctx.builder
        .build_conditional_branch(generator.bool_to_i1(ctx, test), body_bb, orelse_bb)
        .unwrap();

    ctx.builder.position_at_end(body_bb);
    generator.gen_block(ctx, body.iter())?;
    for (k, (_, _, counter)) in &var_assignment {
        let (_, static_val, counter2) = ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(test_bb).unwrap();
    }
    if !orelse.is_empty() {
        ctx.builder.position_at_end(orelse_bb);
        generator.gen_block(ctx, orelse.iter())?;
        if !ctx.is_terminated() {
            ctx.builder.build_unconditional_branch(cont_bb).unwrap();
        }
    }
    for (k, (_, _, counter)) in &var_assignment {
        let (_, static_val, counter2) = ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }
    ctx.builder.position_at_end(cont_bb);
    ctx.loop_target = loop_bb;

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
) -> Result<Option<BasicValueEnum<'ctx>>, String>
where
    G: CodeGenerator + ?Sized,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<IntValue<'ctx>, String>,
    ThenFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<Option<R>, String>,
    ElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<Option<R>, String>,
    R: BasicValue<'ctx>,
{
    let current_bb = ctx.builder.get_insert_block().unwrap();

    let then_bb = ctx.ctx.insert_basic_block_after(current_bb, "if.then");
    let else_bb = ctx.ctx.insert_basic_block_after(then_bb, "if.else");
    let end_bb = ctx.ctx.insert_basic_block_after(else_bb, "if.end");

    let cond = cond_fn(generator, ctx)?;
    assert_eq!(cond.get_type().get_bit_width(), ctx.ctx.bool_type().get_bit_width());
    ctx.builder.build_conditional_branch(cond, then_bb, else_bb).unwrap();

    ctx.builder.position_at_end(then_bb);
    let then_val = then_fn(generator, ctx)?;
    let then_end_bb = ctx.builder.get_insert_block().unwrap();
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(end_bb).unwrap();
    }

    ctx.builder.position_at_end(else_bb);
    let else_val = else_fn(generator, ctx)?;
    let else_end_bb = ctx.builder.get_insert_block().unwrap();
    if !ctx.is_terminated() {
        ctx.builder.build_unconditional_branch(end_bb).unwrap();
    }

    ctx.builder.position_at_end(end_bb);
    let phi = match (then_val, else_val) {
        (Some(tv), Some(ev)) => {
            let tv_ty = tv.as_basic_value_enum().get_type();
            assert_eq!(tv_ty, ev.as_basic_value_enum().get_type());

            let phi = ctx.builder.build_phi(tv_ty, "").unwrap();
            phi.add_incoming(&[(&tv, then_end_bb), (&ev, else_end_bb)]);

            Some(phi.as_basic_value())
        }
        (Some(tv), None) => Some(tv.as_basic_value_enum()),
        (None, Some(ev)) => Some(ev.as_basic_value_enum()),
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
) -> Result<(), String>
where
    G: CodeGenerator + ?Sized,
    CondFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<IntValue<'ctx>, String>,
    ThenFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<(), String>,
    ElseFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>) -> Result<(), String>,
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
) -> Result<(), String> {
    let StmtKind::If { test, body, orelse, .. } = &stmt.node else { codegen_unreachable!(ctx) };

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();

    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
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
    ctx.builder.build_unconditional_branch(test_bb).unwrap();
    ctx.builder.position_at_end(test_bb);
    let test = generator.gen_expr(ctx, test).and_then(|v| {
        v.map(|v| v.to_basic_value_enum(ctx, generator, test.custom.unwrap())).transpose()
    })?;
    if let Some(BasicValueEnum::IntValue(test)) = test {
        ctx.builder
            .build_conditional_branch(generator.bool_to_i1(ctx, test), body_bb, orelse_bb)
            .unwrap();
    }
    ctx.builder.position_at_end(body_bb);
    generator.gen_block(ctx, body.iter())?;
    for (k, (_, _, counter)) in &var_assignment {
        let (_, static_val, counter2) = ctx.var_assignment.get_mut(k).unwrap();
        if counter != counter2 {
            *static_val = None;
        }
    }

    if !ctx.is_terminated() {
        if cont_bb.is_none() {
            cont_bb = Some(ctx.ctx.append_basic_block(current, "cont"));
        }
        ctx.builder.build_unconditional_branch(cont_bb.unwrap()).unwrap();
    }
    if !orelse.is_empty() {
        ctx.builder.position_at_end(orelse_bb);
        generator.gen_block(ctx, orelse.iter())?;
        if !ctx.is_terminated() {
            if cont_bb.is_none() {
                cont_bb = Some(ctx.ctx.append_basic_block(current, "cont"));
            }
            ctx.builder.build_unconditional_branch(cont_bb.unwrap()).unwrap();
        }
    }
    if let Some(cont_bb) = cont_bb {
        ctx.builder.position_at_end(cont_bb);
    }
    for (k, (_, _, counter)) in &var_assignment {
        let (_, static_val, counter2) = ctx.var_assignment.get_mut(k).unwrap();
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
) {
    let (final_state, final_targets, final_paths) = final_data;
    let prev = ctx.builder.get_insert_block().unwrap();
    ctx.builder.position_at_end(block);
    unsafe {
        ctx.builder.build_store(*final_state, target.get_address().unwrap()).unwrap();
    }
    ctx.builder.position_at_end(prev);
    final_targets.push(target);
    final_paths.push(block);
}

/// Inserts the declaration of the builtin function with the specified `symbol` name, and returns
/// the function.
pub fn get_builtins<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    symbol: &str,
) -> FunctionDecl<'ctx> {
    let raise_arg = [ctx.get_llvm_type(generator, ctx.primitives.exception)];
    let noreturn = ["noreturn"];
    ctx.fn_store.declare_external(
        &ctx.module,
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
    mut args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let (zelf_ty, zelf) = obj.unwrap();
    let zelf = zelf.to_basic_value_enum(ctx, generator, zelf_ty)?.into_pointer_value();
    let int32 = ctx.ctx.i32_type();
    let zero = int32.const_zero();
    let zelf_id = if let TypeEnum::TObj { obj_id, .. } = &*ctx.unifier.get_ty(zelf_ty) {
        obj_id.0
    } else {
        codegen_unreachable!(ctx)
    };
    let defs = ctx.top_level.definitions.read();
    let def = defs[zelf_id].read();
    let TopLevelDef::Class { name: zelf_name, .. } = &*def else { codegen_unreachable!(ctx) };
    let exception_name = format!("{}:{}", ctx.resolver.get_exception_id(zelf_id), zelf_name);
    unsafe {
        let id_ptr = ctx.builder.build_in_bounds_gep(zelf, &[zero, zero], "exn.id").unwrap();
        let id = ctx.resolver.get_string_id(&exception_name);
        ctx.builder.build_store(id_ptr, int32.const_int(id as u64, false)).unwrap();
        let empty_string =
            ctx.gen_const(generator, &Constant::Str(String::new()), ctx.primitives.str);
        let ptr = ctx
            .builder
            .build_in_bounds_gep(zelf, &[zero, int32.const_int(5, false)], "exn.msg")
            .unwrap();
        let msg = if args.is_empty() {
            empty_string.unwrap()
        } else {
            args.remove(0).1.to_basic_value_enum(ctx, generator, ctx.primitives.str)?
        };
        ctx.builder.build_store(ptr, msg).unwrap();
        for i in &[6, 7, 8] {
            let value = if args.is_empty() {
                ctx.ctx.i64_type().const_zero().into()
            } else {
                args.remove(0).1.to_basic_value_enum(ctx, generator, ctx.primitives.int64)?
            };
            let ptr = ctx
                .builder
                .build_in_bounds_gep(zelf, &[zero, int32.const_int(*i, false)], "exn.param")
                .unwrap();
            ctx.builder.build_store(ptr, value).unwrap();
        }
        // set file, func to empty string
        for i in &[1, 4] {
            let ptr = ctx
                .builder
                .build_in_bounds_gep(zelf, &[zero, int32.const_int(*i, false)], "exn.str")
                .unwrap();
            ctx.builder.build_store(ptr, empty_string.unwrap()).unwrap();
        }
        // set ints to zero
        for i in &[2, 3] {
            let ptr = ctx
                .builder
                .build_in_bounds_gep(zelf, &[zero, int32.const_int(*i, false)], "exn.ints")
                .unwrap();
            ctx.builder.build_store(ptr, zero).unwrap();
        }
    }
    Ok(Some(zelf.into()))
}

/// Generates IR for a `raise` statement.
///
/// * `exception` - The exception thrown by the `raise` statement.
/// * `loc` - The location where the exception is raised from.
pub fn gen_raise<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    exception: Option<&ExceptionValue<'ctx>>,
    loc: Location,
) {
    if let Some(exception) = exception {
        exception.store_location(generator, ctx, loc);

        let current_fun = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
        let fun_name = ctx.gen_string(generator, current_fun.get_name().to_str().unwrap());
        exception.store_func(ctx, fun_name);

        let raise = get_builtins(generator, ctx, "__nac3_raise");
        let exception = *exception;
        ctx.build_call_or_invoke(&raise, &[exception.as_abi_value(ctx).into()], "raise");
    } else {
        let resume = get_builtins(generator, ctx, "__nac3_resume");
        ctx.build_call_or_invoke(&resume, &[], "resume");
    }
    ctx.builder.build_unreachable().unwrap();
}

/// Generates IR for a `try` statement.
pub fn gen_try<'ctx, 'a, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    target: &Stmt<Option<Type>>,
) -> Result<(), String> {
    let StmtKind::Try { body, handlers, orelse, finalbody, .. } = &target.node else {
        codegen_unreachable!(ctx)
    };

    // if we need to generate anything related to exception, we must have personality defined
    let personality = get_personality(ctx.top_level, &ctx.module).unwrap();
    let exception_type = ctx.get_llvm_type(generator, ctx.primitives.exception);
    let ptr_type = ctx.ctx.i8_type().ptr_type(inkwell::AddressSpace::default());
    let current_block = ctx.builder.get_insert_block().unwrap();
    let current_fun = current_block.get_parent().unwrap();
    let landingpad = ctx.ctx.append_basic_block(current_fun, "try.landingpad");
    let dispatcher = ctx.ctx.append_basic_block(current_fun, "try.dispatch");
    let mut dispatcher_end = dispatcher;
    ctx.builder.position_at_end(dispatcher);
    let exn = ctx.builder.build_phi(exception_type, "exn").unwrap();
    ctx.builder.position_at_end(current_block);

    let mut cleanup = None;
    let mut old_loop_target = None;
    let mut old_return = None;
    let mut final_data = None;
    let has_cleanup = !finalbody.is_empty();
    if has_cleanup {
        let final_state =
            generator.gen_var_alloc(ctx, ptr_type.into(), Some("try.final_state.addr"))?;
        final_data = Some((final_state, Vec::new(), Vec::new()));
        if let Some((continue_target, break_target)) = ctx.loop_target {
            let break_proxy = ctx.ctx.append_basic_block(current_fun, "try.break");
            let continue_proxy = ctx.ctx.append_basic_block(current_fun, "try.continue");
            final_proxy(ctx, break_target, break_proxy, final_data.as_mut().unwrap());
            final_proxy(ctx, continue_target, continue_proxy, final_data.as_mut().unwrap());
            old_loop_target = ctx.loop_target.replace((continue_proxy, break_proxy));
        }
        let return_proxy = ctx.ctx.append_basic_block(current_fun, "try.return");
        if let Some(return_target) = ctx.return_target {
            final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap());
        } else {
            let return_target = ctx.ctx.append_basic_block(current_fun, "try.return_target");
            ctx.builder.position_at_end(return_target);
            let return_value =
                ctx.return_buffer.map(|v| ctx.builder.build_load(v, "$ret").unwrap());
            ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue)).unwrap();
            ctx.builder.position_at_end(current_block);
            final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap());
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
                .unioned(type_.as_ref().unwrap().custom.unwrap(), ctx.primitives.exception)
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
        let exn_id_global =
            ctx.module.add_global(ctx.ctx.i32_type(), None, &format!("exn.{exn_id}"));
        exn_id_global.set_initializer(&ctx.ctx.i32_type().const_int(exn_id as u64, false));
        clauses.push(Some(exn_id_global.as_pointer_value().as_basic_value_enum()));
    }
    let mut all_clauses = clauses.clone();
    if let Some(old_clauses) = &ctx.outer_catch_clauses {
        if !found_catch_all {
            all_clauses.extend_from_slice(&old_clauses.0);
        }
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
        ctx.builder
            .build_landing_pad(
                ctx.ctx.struct_type(&[ptr_type.into(), exception_type], false),
                personality,
                &[],
                true,
                "try.catch.final",
            )
            .unwrap();
        ctx.builder.build_unconditional_branch(cleanup.unwrap()).unwrap();
        ctx.builder.position_at_end(body);
        ctx.unwind_target.replace(final_landingpad)
    };

    // run end_catch before continue/break/return
    let mut final_proxy_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, target: BasicBlock<'ctx>, block: BasicBlock<'ctx>| {
            final_proxy(ctx, target, block, final_data.as_mut().unwrap());
        };
    let mut redirect_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, target: BasicBlock<'ctx>, block: BasicBlock<'ctx>| {
            ctx.builder.position_at_end(block);
            ctx.builder.build_unconditional_branch(target).unwrap();
            ctx.builder.position_at_end(body);
        };
    let redirect = if has_cleanup {
        &mut final_proxy_lambda
            as &mut dyn FnMut(&mut CodeGenContext<'ctx, 'a>, BasicBlock<'ctx>, BasicBlock<'ctx>)
    } else {
        &mut redirect_lambda
            as &mut dyn FnMut(&mut CodeGenContext<'ctx, 'a>, BasicBlock<'ctx>, BasicBlock<'ctx>)
    };
    let resume = get_builtins(generator, ctx, "__nac3_resume");
    let end_catch = get_builtins(generator, ctx, "__nac3_end_catch");
    if let Some((continue_target, break_target)) = ctx.loop_target.take() {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "try.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "try.continue");
        ctx.builder.position_at_end(break_proxy);
        ctx.build_call(&end_catch, &[], "end_catch");
        ctx.builder.position_at_end(continue_proxy);
        ctx.build_call(&end_catch, &[], "end_catch");
        ctx.builder.position_at_end(body);
        redirect(ctx, break_target, break_proxy);
        redirect(ctx, continue_target, continue_proxy);
        ctx.loop_target = Some((continue_proxy, break_proxy));
        old_loop_target = Some((continue_target, break_target));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "try.return");
    ctx.builder.position_at_end(return_proxy);
    ctx.build_call(&end_catch, &[], "end_catch");
    let return_target = ctx.return_target.take().unwrap_or_else(|| {
        let doreturn = ctx.ctx.append_basic_block(current_fun, "try.doreturn");
        ctx.builder.position_at_end(doreturn);
        let return_value = ctx.return_buffer.map(|v| ctx.builder.build_load(v, "$ret").unwrap());
        ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue)).unwrap();
        doreturn
    });
    redirect(ctx, return_target, return_proxy);
    ctx.return_target = Some(return_proxy);
    old_return = Some(return_target);

    let mut post_handlers = Vec::new();

    let exnid = if handlers.is_empty() {
        None
    } else {
        ctx.builder.position_at_end(dispatcher);
        unsafe {
            let zero = ctx.ctx.i32_type().const_zero();
            let exnid_ptr = ctx
                .builder
                .build_gep(exn.as_basic_value().into_pointer_value(), &[zero, zero], "exnidptr")
                .unwrap();
            Some(ctx.builder.build_load(exnid_ptr, "exnid").unwrap())
        }
    };

    for (handler_node, exn_type) in handlers.iter().zip(clauses.iter()) {
        let ExcepthandlerKind::ExceptHandler { type_, name, body } = &handler_node.node;
        let handler_bb = ctx.ctx.append_basic_block(current_fun, "try.handler");
        ctx.builder.position_at_end(handler_bb);
        if let Some(name) = name {
            let exn_ty = ctx.get_llvm_type(generator, type_.as_ref().unwrap().custom.unwrap());
            let exn_store = generator.gen_var_alloc(ctx, exn_ty, Some("try.exn_store.addr"))?;
            ctx.var_assignment.insert(*name, (exn_store, None, 0));
            ctx.builder.build_store(exn_store, exn.as_basic_value()).unwrap();
        }
        generator.gen_block(ctx, body.iter())?;
        let current = ctx.builder.get_insert_block().unwrap();
        // only need to call end catch if not terminated
        // otherwise, we already handled in return/break/continue/raise
        if current.get_terminator().is_none() {
            ctx.build_call(&end_catch, &[], "end_catch");
        }
        post_handlers.push(current);
        ctx.builder.position_at_end(dispatcher_end);
        if let Some(exn_type) = exn_type {
            let dispatcher_cont = ctx.ctx.append_basic_block(current_fun, "try.dispatcher_cont");
            let actual_id = exnid.unwrap().into_int_value();
            let expected_id = ctx
                .builder
                .build_load(exn_type.into_pointer_value(), "expected_id")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let result = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, actual_id, expected_id, "exncheck")
                .unwrap();
            ctx.builder.build_conditional_branch(result, handler_bb, dispatcher_cont).unwrap();
            dispatcher_end = dispatcher_cont;
        } else {
            ctx.builder.build_unconditional_branch(handler_bb).unwrap();
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
        )
        .map(BasicValueEnum::into_struct_value)
        .unwrap();
    let exn_val = ctx.builder.build_extract_value(landingpad_value, 1, "exn").unwrap();
    ctx.builder.build_unconditional_branch(dispatcher).unwrap();
    exn.add_incoming(&[(&exn_val, landingpad)]);

    if dispatcher_end.get_terminator().is_none() {
        ctx.builder.position_at_end(dispatcher_end);
        if let Some(cleanup) = cleanup {
            ctx.builder.build_unconditional_branch(cleanup).unwrap();
        } else if let Some((_, outer_dispatcher, phi)) = ctx.outer_catch_clauses {
            phi.add_incoming(&[(&exn_val, dispatcher_end)]);
            ctx.builder.build_unconditional_branch(outer_dispatcher).unwrap();
        } else {
            ctx.build_call_or_invoke(&resume, &[], "resume");
            ctx.builder.build_unreachable().unwrap();
        }
    }

    if finalbody.is_empty() {
        let tail = ctx.ctx.append_basic_block(current_fun, "try.tail");
        if body.get_terminator().is_none() {
            ctx.builder.position_at_end(body);
            ctx.builder.build_unconditional_branch(tail).unwrap();
        }
        if matches!(cleanup, Some(cleanup) if cleanup.get_terminator().is_none()) {
            ctx.builder.position_at_end(cleanup.unwrap());
            ctx.builder.build_unconditional_branch(tail).unwrap();
        }
        for post_handler in post_handlers {
            if post_handler.get_terminator().is_none() {
                ctx.builder.position_at_end(post_handler);
                ctx.builder.build_unconditional_branch(tail).unwrap();
            }
        }
        ctx.builder.position_at_end(tail);
    } else {
        // exception path
        let cleanup = cleanup.unwrap();
        ctx.builder.position_at_end(cleanup);
        generator.gen_block(ctx, finalbody.iter())?;
        if !ctx.is_terminated() {
            ctx.build_call_or_invoke(&resume, &[], "resume");
            ctx.builder.build_unreachable().unwrap();
        }

        // normal path
        let (final_state, mut final_targets, final_paths) = final_data.unwrap();
        let tail = ctx.ctx.append_basic_block(current_fun, "try.tail");
        final_targets.push(tail);
        let finalizer = ctx.ctx.append_basic_block(current_fun, "try.finally");
        ctx.builder.position_at_end(finalizer);
        generator.gen_block(ctx, finalbody.iter())?;
        if !ctx.is_terminated() {
            let dest = ctx.builder.build_load(final_state, "final_dest").unwrap();
            ctx.builder.build_indirect_branch(dest, &final_targets).unwrap();
        }
        for block in &final_paths {
            if block.get_terminator().is_none() {
                ctx.builder.position_at_end(*block);
                ctx.builder.build_unconditional_branch(finalizer).unwrap();
            }
        }
        for block in [body].iter().chain(post_handlers.iter()) {
            if block.get_terminator().is_none() {
                ctx.builder.position_at_end(*block);
                unsafe {
                    ctx.builder.build_store(final_state, tail.get_address().unwrap()).unwrap();
                }
                ctx.builder.build_unconditional_branch(finalizer).unwrap();
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
) -> Result<(), String> {
    let StmtKind::With { items, body, .. } = &stmt.node else { codegen_unreachable!(ctx) };
    let mut exits = vec![];
    let mut enters = vec![];

    // prepare enters and exits
    for item in items {
        // evaluate the expression first
        let expr_ty = item.context_expr.custom.unwrap();
        let expr = generator.gen_expr(ctx, &item.context_expr)?.unwrap();

        // get the __enter__ method signature and ID
        let TypeEnum::TObj { obj_id, fields, .. } = &*ctx.unifier.get_ty(expr_ty) else {
            codegen_unreachable!(ctx)
        };
        let top_level_defs = ctx.top_level.definitions.read();
        let def = top_level_defs[obj_id.0].read();
        let TopLevelDef::Class { methods, .. } = &*def else { codegen_unreachable!(ctx) };
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

    let body_gen_lambda = |ctx: &mut CodeGenContext<'ctx, 'a>,
                           generator: &mut G|
     -> Result<(), String> {
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
                generator.gen_assign(ctx, optional_vars, enter_ret.unwrap().into(), enter.2.ret)?;
            }
        }

        // generate the `with` body
        generator.gen_block(ctx, body.iter())
    };

    let exit_gen_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, generator: &mut G| -> Result<(), String> {
            // call __exit__()s in the reverse order
            for exit in exits.iter().rev() {
                generator.gen_call(
                    ctx,
                    Some((exit.0, exit.1.clone())),
                    (&exit.2, exit.3),
                    Vec::default(),
                )?;
            }
            Ok(())
        };

    // copied and trimmed from gen_try, to cover try (setup, enter)..finally (exit)
    let personality = get_personality(ctx.top_level, &ctx.module).unwrap();
    let exception_type = ctx.get_llvm_type(generator, ctx.primitives.exception);
    let ptr_type = ctx.ctx.i8_type().ptr_type(inkwell::AddressSpace::default());
    let current_block = ctx.builder.get_insert_block().unwrap();
    let current_fun = current_block.get_parent().unwrap();
    let landingpad = ctx.ctx.append_basic_block(current_fun, "with.landingpad");
    let dispatcher = ctx.ctx.append_basic_block(current_fun, "with.dispatch");
    let dispatcher_end = dispatcher;
    ctx.builder.position_at_end(dispatcher);
    let exn = ctx.builder.build_phi(exception_type, "exn").unwrap();
    ctx.builder.position_at_end(current_block);

    let mut old_loop_target = None;
    let final_state =
        generator.gen_var_alloc(ctx, ptr_type.into(), Some("with.final_state.addr"))?;
    let mut final_data = Some((final_state, Vec::new(), Vec::new()));
    if let Some((continue_target, break_target)) = ctx.loop_target {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "with.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "with.continue");
        final_proxy(ctx, break_target, break_proxy, final_data.as_mut().unwrap());
        final_proxy(ctx, continue_target, continue_proxy, final_data.as_mut().unwrap());
        old_loop_target = ctx.loop_target.replace((continue_proxy, break_proxy));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "with.return");
    if let Some(return_target) = ctx.return_target {
        final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap());
    } else {
        let return_target = ctx.ctx.append_basic_block(current_fun, "with.return_target");
        ctx.builder.position_at_end(return_target);
        let return_value = ctx.return_buffer.map(|v| ctx.builder.build_load(v, "$ret").unwrap());
        ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue)).unwrap();
        ctx.builder.position_at_end(current_block);
        final_proxy(ctx, return_target, return_proxy, final_data.as_mut().unwrap());
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
    ctx.builder
        .build_landing_pad(
            ctx.ctx.struct_type(&[ptr_type.into(), exception_type], false),
            personality,
            &[],
            true,
            "with.catch.final",
        )
        .unwrap();
    ctx.builder.build_unconditional_branch(cleanup).unwrap();
    ctx.builder.position_at_end(body);
    let old_unwind = ctx.unwind_target.replace(final_landingpad);

    let mut final_proxy_lambda =
        |ctx: &mut CodeGenContext<'ctx, 'a>, target: BasicBlock<'ctx>, block: BasicBlock<'ctx>| {
            final_proxy(ctx, target, block, final_data.as_mut().unwrap());
        };
    let redirect = &mut final_proxy_lambda
        as &mut dyn FnMut(&mut CodeGenContext<'ctx, 'a>, BasicBlock<'ctx>, BasicBlock<'ctx>);

    let resume = get_builtins(generator, ctx, "__nac3_resume");
    let end_catch = get_builtins(generator, ctx, "__nac3_end_catch");
    if let Some((continue_target, break_target)) = ctx.loop_target.take() {
        let break_proxy = ctx.ctx.append_basic_block(current_fun, "with.break");
        let continue_proxy = ctx.ctx.append_basic_block(current_fun, "with.continue");
        ctx.builder.position_at_end(break_proxy);
        ctx.build_call(&end_catch, &[], "end_catch");
        ctx.builder.position_at_end(continue_proxy);
        ctx.build_call(&end_catch, &[], "end_catch");
        ctx.builder.position_at_end(body);
        redirect(ctx, break_target, break_proxy);
        redirect(ctx, continue_target, continue_proxy);
        ctx.loop_target = Some((continue_proxy, break_proxy));
        old_loop_target = Some((continue_target, break_target));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "with.return");
    ctx.builder.position_at_end(return_proxy);
    ctx.build_call(&end_catch, &[], "end_catch");
    let return_target = ctx.return_target.take().unwrap_or_else(|| {
        let doreturn = ctx.ctx.append_basic_block(current_fun, "with.doreturn");
        ctx.builder.position_at_end(doreturn);
        let return_value = ctx.return_buffer.map(|v| ctx.builder.build_load(v, "$ret").unwrap());
        ctx.builder.build_return(return_value.as_ref().map(|v| v as &dyn BasicValue)).unwrap();
        doreturn
    });
    redirect(ctx, return_target, return_proxy);
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
        )
        .map(BasicValueEnum::into_struct_value)
        .unwrap();
    let exn_val = ctx.builder.build_extract_value(landingpad_value, 1, "exn").unwrap();
    ctx.builder.build_unconditional_branch(dispatcher).unwrap();
    exn.add_incoming(&[(&exn_val, landingpad)]);

    if dispatcher_end.get_terminator().is_none() {
        ctx.builder.position_at_end(dispatcher_end);
        ctx.builder.build_unconditional_branch(cleanup).unwrap();
    }

    // exception path
    ctx.builder.position_at_end(cleanup);
    exit_gen_lambda(ctx, generator)?;
    ctx.build_call_or_invoke(&resume, &[], "resume");
    ctx.builder.build_unreachable().unwrap();

    // normal path
    let (final_state, mut final_targets, final_paths) = final_data.unwrap();
    let tail = ctx.ctx.append_basic_block(current_fun, "with.tail");
    final_targets.push(tail);
    let finalizer = ctx.ctx.append_basic_block(current_fun, "with.exits");
    ctx.builder.position_at_end(finalizer);
    exit_gen_lambda(ctx, generator)?;
    let dest = ctx.builder.build_load(final_state, "final_dest").unwrap();
    ctx.builder.build_indirect_branch(dest, &final_targets).unwrap();
    for block in &final_paths {
        if block.get_terminator().is_none() {
            ctx.builder.position_at_end(*block);
            ctx.builder.build_unconditional_branch(finalizer).unwrap();
        }
    }
    for block in &[body] {
        if block.get_terminator().is_none() {
            ctx.builder.position_at_end(*block);
            unsafe {
                ctx.builder.build_store(final_state, tail.get_address().unwrap()).unwrap();
            }
            ctx.builder.build_unconditional_branch(finalizer).unwrap();
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
) -> Result<(), String> {
    let func = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
    let value = if let Some(v_expr) = value.as_ref() {
        if let Some(v) = generator.gen_expr(ctx, v_expr).transpose() {
            Some(v.and_then(|v| v.to_basic_value_enum(ctx, generator, v_expr.custom.unwrap()))?)
        } else {
            return Ok(());
        }
    } else {
        None
    };

    // Remap boolean return type into i1
    let value = value.map(|ret_val| {
        // The "return type" of a sret function is in the first parameter
        let expected_ty = func.get_type().get_return_type().unwrap().into();

        if matches!(expected_ty, BasicMetadataTypeEnum::IntType(ty) if ty.get_bit_width() == 1) {
            generator.bool_to_i1(ctx, ret_val.into_int_value()).into()
        } else {
            ret_val
        }
    });

    if let Some(return_target) = ctx.return_target {
        if let Some(value) = value {
            ctx.builder.build_store(ctx.return_buffer.unwrap(), value).unwrap();
        }
        ctx.builder.build_unconditional_branch(return_target).unwrap();
    } else {
        let value = value.as_ref().map(|v| v as &dyn BasicValue);
        ctx.builder.build_return(value).unwrap();
    }
    Ok(())
}

/// See [`CodeGenerator::gen_stmt`].
pub fn gen_stmt<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> Result<(), String> {
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
                let Some(value_enum) = generator.gen_expr(ctx, value)? else { return Ok(()) };
                generator.gen_assign(ctx, target, value_enum, value.custom.unwrap())?;
            }
        }
        StmtKind::Assign { targets, value, .. } => {
            let Some(value_enum) = generator.gen_expr(ctx, value)? else { return Ok(()) };
            for target in targets {
                generator.gen_assign(ctx, target, value_enum.clone(), value.custom.unwrap())?;
            }
        }
        StmtKind::Continue { .. } => {
            ctx.builder.build_unconditional_branch(ctx.loop_target.unwrap().0).unwrap();
        }
        StmtKind::Break { .. } => {
            ctx.builder.build_unconditional_branch(ctx.loop_target.unwrap().1).unwrap();
        }
        StmtKind::If { .. } => generator.gen_if(ctx, stmt)?,
        StmtKind::While { .. } => generator.gen_while(ctx, stmt)?,
        StmtKind::For { .. } => generator.gen_for(ctx, stmt)?,
        StmtKind::With { .. } => generator.gen_with(ctx, stmt)?,
        StmtKind::AugAssign { target, op, value, .. } => {
            let value_enum = gen_binop_expr(
                generator,
                ctx,
                target,
                Binop::aug_assign(*op),
                value,
                stmt.location,
            )?
            .unwrap();
            generator.gen_assign(ctx, target, value_enum, value.custom.unwrap())?;
        }
        StmtKind::Try { .. } => gen_try(generator, ctx, stmt)?,
        StmtKind::Raise { exc, .. } => {
            if let Some(exc) = exc {
                let exn = if let ExprKind::Name { id, .. } = &exc.node {
                    // Handle "raise Exception" short form
                    let def_id = ctx.resolver.get_identifier_def(*id).map_err(|e| {
                        format!("{} (at {})", e.iter().next().unwrap(), exc.location)
                    })?;
                    let def = ctx.top_level.definitions.read();
                    let TopLevelDef::Class { constructor, .. } = *def[def_id.0].read() else {
                        return Err(format!("Failed to resolve symbol {id} (at {})", exc.location));
                    };

                    let TypeEnum::TFunc(signature) =
                        ctx.unifier.get_ty(constructor.unwrap()).as_ref().clone()
                    else {
                        return Err(format!("Failed to resolve symbol {id} (at {})", exc.location));
                    };

                    generator
                        .gen_call(ctx, None, (&signature, def_id), Vec::default())?
                        .map(Into::into)
                } else {
                    generator.gen_expr(ctx, exc)?
                };

                let exc = if let Some(v) = exn {
                    v.to_basic_value_enum(ctx, generator, exc.custom.unwrap())?
                } else {
                    return Ok(());
                };
                let exc = ExceptionType::get_instance(generator, ctx)
                    .map_pointer_value(exc.into_pointer_value(), None);
                gen_raise(generator, ctx, Some(&exc), stmt.location);
            } else {
                gen_raise(generator, ctx, None, stmt.location);
            }
        }
        StmtKind::Assert { test, msg, .. } => {
            let test = if let Some(v) = generator.gen_expr(ctx, test)? {
                v.to_basic_value_enum(ctx, generator, test.custom.unwrap())?
            } else {
                return Ok(());
            };
            let err_msg = match msg {
                Some(msg) => {
                    if let Some(v) = generator.gen_expr(ctx, msg)? {
                        v.to_basic_value_enum(ctx, generator, msg.custom.unwrap())?
                    } else {
                        return Ok(());
                    }
                }
                None => ctx.gen_string(generator, "").into(),
            };
            ctx.make_assert_impl(
                generator,
                generator.bool_to_i1(ctx, test.into_int_value()),
                "0:AssertionError",
                err_msg,
                [None, None, None],
                stmt.location,
            );
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
) -> Result<(), String> {
    for stmt in stmts {
        generator.gen_stmt(ctx, stmt)?;
        if ctx.is_terminated() {
            break;
        }
    }
    Ok(())
}
