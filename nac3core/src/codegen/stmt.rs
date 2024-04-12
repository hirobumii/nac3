use super::{
    super::symbol_resolver::ValueEnum,
    expr::destructure_range,
    irrt::{handle_slice_indices, list_slice_assignment},
    CodeGenContext, CodeGenerator,
};
use crate::{
    codegen::{
        classes::{ArrayLikeIndexer, ArraySliceValue, ListValue, RangeValue},
        expr::gen_binop_expr,
        gen_in_range_check,
    },
    toplevel::{
        DefinitionId,
        helper::PRIMITIVE_DEF_IDS,
        numpy::unpack_ndarray_var_tys,
        TopLevelDef,
    },
    typecheck::typedef::{FunSignature, Type, TypeEnum},
};
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    basic_block::BasicBlock,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue},
    IntPredicate,
};
use nac3parser::ast::{
    Constant, ExcepthandlerKind, Expr, ExprKind, Location, Stmt, StmtKind, StrRef,
};
use std::convert::TryFrom;

/// See [`CodeGenerator::gen_var_alloc`].
pub fn gen_var<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
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
    let llvm_usize = generator.get_size_type(ctx.ctx);

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
        }
        ExprKind::Attribute { value, attr, .. } => {
            let index = ctx.get_attr_index(value.custom.unwrap(), *attr);
            let val = if let Some(v) = generator.gen_expr(ctx, value)? {
                v.to_basic_value_enum(ctx, generator, value.custom.unwrap())?
            } else {
                return Ok(None)
            };
            let BasicValueEnum::PointerValue(ptr) = val else {
                unreachable!();
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
            }.unwrap()
        }
        ExprKind::Subscript { value, slice, .. } => {
            match ctx.unifier.get_ty_immutable(value.custom.unwrap()).as_ref() {
                TypeEnum::TList { .. } => {
                    let v = generator
                        .gen_expr(ctx, value)?
                        .unwrap()
                        .to_basic_value_enum(ctx, generator, value.custom.unwrap())?
                        .into_pointer_value();
                    let v = ListValue::from_ptr_val(v, llvm_usize, None);
                    let len = v.load_size(ctx, Some("len"));
                    let raw_index = generator
                        .gen_expr(ctx, slice)?
                        .unwrap()
                        .to_basic_value_enum(ctx, generator, slice.custom.unwrap())?
                        .into_int_value();
                    let raw_index = ctx.builder
                        .build_int_s_extend(raw_index, generator.get_size_type(ctx.ctx), "sext")
                        .unwrap();
                    // handle negative index
                    let is_negative = ctx.builder
                        .build_int_compare(
                            IntPredicate::SLT,
                            raw_index,
                            generator.get_size_type(ctx.ctx).const_zero(),
                            "is_neg",
                        )
                        .unwrap();
                    let adjusted = ctx.builder.build_int_add(raw_index, len, "adjusted").unwrap();
                    let index = ctx
                        .builder
                        .build_select(is_negative, adjusted, raw_index, "index")
                        .map(BasicValueEnum::into_int_value)
                        .unwrap();
                    // unsigned less than is enough, because negative index after adjustment is
                    // bigger than the length (for unsigned cmp)
                    let bound_check = ctx.builder
                        .build_int_compare(
                            IntPredicate::ULT,
                            index,
                            len,
                            "inbound",
                        )
                        .unwrap();
                    ctx.make_assert(
                        generator,
                        bound_check,
                        "0:IndexError",
                        "index {0} out of bounds 0:{1}",
                        [Some(raw_index), Some(len), None],
                        slice.location,
                    );
                    v.data().ptr_offset(ctx, generator, index, name)
                }

                TypeEnum::TObj { obj_id, .. } if *obj_id == PRIMITIVE_DEF_IDS.ndarray => {
                    todo!()
                }

                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }))
}

/// See [`CodeGenerator::gen_assign`].
pub fn gen_assign<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    target: &Expr<Option<Type>>,
    value: ValueEnum<'ctx>,
) -> Result<(), String> {
    let llvm_usize = generator.get_size_type(ctx.ctx);

    match &target.node {
        ExprKind::Tuple { elts, .. } => {
            let BasicValueEnum::StructValue(v) =
                value.to_basic_value_enum(ctx, generator, target.custom.unwrap())? else {
                unreachable!()
            };

            for (i, elt) in elts.iter().enumerate() {
                let v = ctx
                    .builder
                    .build_extract_value(v, u32::try_from(i).unwrap(), "struct_elem")
                    .unwrap();
                generator.gen_assign(ctx, elt, v.into())?;
            }
        }
        ExprKind::Subscript { value: ls, slice, .. }
            if matches!(&slice.node, ExprKind::Slice { .. }) =>
        {
            let ExprKind::Slice { lower, upper, step } = &slice.node else {
                unreachable!()
            };

            let ls = generator
                .gen_expr(ctx, ls)?
                .unwrap()
                .to_basic_value_enum(ctx, generator, ls.custom.unwrap())?
                .into_pointer_value();
            let ls = ListValue::from_ptr_val(ls, llvm_usize, None);
            let Some((start, end, step)) =
                handle_slice_indices(lower, upper, step, ctx, generator, ls)? else {
                return Ok(())
            };
            let value = value
                .to_basic_value_enum(ctx, generator, target.custom.unwrap())?
                .into_pointer_value();
            let value = ListValue::from_ptr_val(value, llvm_usize, None);
            let ty = match &*ctx.unifier.get_ty_immutable(target.custom.unwrap()) {
                TypeEnum::TList { ty } => *ty,
                TypeEnum::TObj { obj_id, .. } if *obj_id == PRIMITIVE_DEF_IDS.ndarray => {
                    unpack_ndarray_var_tys(&mut ctx.unifier, target.custom.unwrap()).0
                }
                _ => unreachable!(),
            };

            let ty = ctx.get_llvm_type(generator, ty);
            let Some(src_ind) = handle_slice_indices(&None, &None, &None, ctx, generator, value)? else {
                return Ok(())
            };
            list_slice_assignment(generator, ctx, ty, ls, (start, end, step), value, src_ind);
        }
        _ => {
            let name = if let ExprKind::Name { id, .. } = &target.node {
                format!("{id}.addr")
            } else {
                String::from("target.addr")
            };
            let Some(ptr) = generator.gen_store_target(ctx, target, Some(name.as_str()))? else {
                return Ok(())
            };

            if let ExprKind::Name { id, .. } = &target.node {
                let (_, static_value, counter) = ctx.var_assignment.get_mut(id).unwrap();
                *counter += 1;
                if let ValueEnum::Static(s) = &value {
                    *static_value = Some(s.clone());
                }
            }
            let val = value.to_basic_value_enum(ctx, generator, target.custom.unwrap())?;
            ctx.builder.build_store(ptr, val).unwrap();
        }
    };
    Ok(())
}

/// See [`CodeGenerator::gen_for`].
pub fn gen_for<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> Result<(), String> {
    let StmtKind::For { iter, target, body, orelse, .. } = &stmt.node else {
        unreachable!()
    };

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();

    let int32 = ctx.ctx.i32_type();
    let size_t = generator.get_size_type(ctx.ctx);
    let zero = int32.const_zero();
    let current = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
    let body_bb = ctx.ctx.append_basic_block(current, "for.body");
    let cont_bb = ctx.ctx.append_basic_block(current, "for.end");
    // if there is no orelse, we just go to cont_bb
    let orelse_bb = if orelse.is_empty() {
        cont_bb
    } else {
        ctx.ctx.append_basic_block(current, "for.orelse")
    };

    // Whether the iterable is a range() expression
    let is_iterable_range_expr = ctx.unifier.unioned(iter.custom.unwrap(), ctx.primitives.range);

    // The BB containing the increment expression
    let incr_bb = ctx.ctx.append_basic_block(current, "for.incr");
    // The BB containing the loop condition check
    let cond_bb = ctx.ctx.append_basic_block(current, "for.cond");

    // store loop bb information and restore it later
    let loop_bb = ctx.loop_target.replace((incr_bb, cont_bb));

    let iter_val = if let Some(v) = generator.gen_expr(ctx, iter)? {
        v.to_basic_value_enum(
            ctx,
            generator,
            iter.custom.unwrap(),
        )?
    } else {
        return Ok(())
    };
    if is_iterable_range_expr {
        let iter_val = RangeValue::from_ptr_val(iter_val.into_pointer_value(), Some("range"));
        // Internal variable for loop; Cannot be assigned
        let i = generator.gen_var_alloc(ctx, int32.into(), Some("for.i.addr"))?;
        // Variable declared in "target" expression of the loop; Can be reassigned *or* shadowed
        let Some(target_i) = generator.gen_store_target(ctx, target, Some("for.target.addr"))? else {
            unreachable!()
        };
        let (start, stop, step) = destructure_range(ctx, iter_val);

        ctx.builder.build_store(i, start).unwrap();

        // Check "If step is zero, ValueError is raised."
        let rangenez = ctx.builder
            .build_int_compare(IntPredicate::NE, step, int32.const_zero(), "")
            .unwrap();
        ctx.make_assert(
            generator,
            rangenez,
            "ValueError",
            "range() arg 3 must not be zero",
            [None, None, None],
            ctx.current_loc
        );
        ctx.builder.build_unconditional_branch(cond_bb).unwrap();

        {
            ctx.builder.position_at_end(cond_bb);
            ctx.builder
                .build_conditional_branch(
                    gen_in_range_check(
                        ctx,
                        ctx.builder.build_load(i, "").map(BasicValueEnum::into_int_value).unwrap(),
                        stop,
                        step,
                    ),
                    body_bb,
                    orelse_bb,
                )
                .unwrap();
        }

        ctx.builder.position_at_end(incr_bb);
        let next_i = ctx.builder
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
    } else {
        let index_addr = generator.gen_var_alloc(ctx, size_t.into(), Some("for.index.addr"))?;
        ctx.builder.build_store(index_addr, size_t.const_zero()).unwrap();
        let len = ctx
            .build_gep_and_load(
                iter_val.into_pointer_value(),
                &[zero, int32.const_int(1, false)],
                Some("len")
            )
            .into_int_value();
        ctx.builder.build_unconditional_branch(cond_bb).unwrap();

        ctx.builder.position_at_end(cond_bb);
        let index = ctx.builder
            .build_load(index_addr, "for.index")
            .map(BasicValueEnum::into_int_value)
            .unwrap();
        let cmp = ctx.builder.build_int_compare(IntPredicate::SLT, index, len, "cond").unwrap();
        ctx.builder.build_conditional_branch(cmp, body_bb, orelse_bb).unwrap();

        ctx.builder.position_at_end(incr_bb);
        let index = ctx.builder.build_load(index_addr, "").map(BasicValueEnum::into_int_value).unwrap();
        let inc = ctx.builder.build_int_add(index, size_t.const_int(1, true), "inc").unwrap();
        ctx.builder.build_store(index_addr, inc).unwrap();
        ctx.builder.build_unconditional_branch(cond_bb).unwrap();

        ctx.builder.position_at_end(body_bb);
        let arr_ptr = ctx
            .build_gep_and_load(iter_val.into_pointer_value(), &[zero, zero], Some("arr.addr"))
            .into_pointer_value();
        let index = ctx.builder
            .build_load(index_addr, "for.index")
            .map(BasicValueEnum::into_int_value)
            .unwrap();
        let val = ctx.build_gep_and_load(arr_ptr, &[index], Some("val"));
        generator.gen_assign(ctx, target, val.into())?;
        generator.gen_block(ctx, body.iter())?;
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

/// Generates a C-style `for` construct using lambdas, similar to the following C code:
///
/// ```c
/// for (x... = init(); cond(x...); update(x...)) {
///     body(x...);
/// }
/// ```
///
/// * `init` - A lambda containing IR statements declaring and initializing loop variables. The
/// return value is a [Clone] value which will be passed to the other lambdas.
/// * `cond` - A lambda containing IR statements checking whether the loop should continue
/// executing. The result value must be an `i1` indicating if the loop should continue.
/// * `body` - A lambda containing IR statements within the loop body.
/// * `update` - A lambda containing IR statements updating loop variables.
pub fn gen_for_callback<'ctx, 'a, G, I, InitFn, CondFn, BodyFn, UpdateFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
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
        BodyFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, I) -> Result<(), String>,
        UpdateFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, I) -> Result<(), String>,
{
    let current = ctx.builder.get_insert_block().and_then(BasicBlock::get_parent).unwrap();
    let init_bb = ctx.ctx.append_basic_block(current, "for.init");
    // The BB containing the loop condition check
    let cond_bb = ctx.ctx.append_basic_block(current, "for.cond");
    let body_bb = ctx.ctx.append_basic_block(current, "for.body");
    // The BB containing the increment expression
    let update_bb = ctx.ctx.append_basic_block(current, "for.update");
    let cont_bb = ctx.ctx.append_basic_block(current, "for.end");

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
        ctx.builder
            .build_conditional_branch(cond, body_bb, cont_bb)
            .unwrap();
    }

    ctx.builder.position_at_end(body_bb);
    body(generator, ctx, loop_var.clone())?;
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
/// as the type of the loop variable.
/// * `max_val` - A tuple containing the maximum value of the loop variable, and whether the maximum
/// value should be treated as inclusive (as opposed to exclusive).
/// * `body` - A lambda containing IR statements within the loop body.
/// * `incr_val` - The value to increment the loop variable on each iteration.
pub fn gen_for_callback_incrementing<'ctx, 'a, G, BodyFn>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    init_val: IntValue<'ctx>,
    max_val: (IntValue<'ctx>, bool),
    body: BodyFn,
    incr_val: IntValue<'ctx>,
) -> Result<(), String>
    where
        G: CodeGenerator + ?Sized,
        BodyFn: FnOnce(&mut G, &mut CodeGenContext<'ctx, 'a>, IntValue<'ctx>) -> Result<(), String>,
{
    let init_val_t = init_val.get_type();

    gen_for_callback(
        generator,
        ctx,
        |generator, ctx| {
            let i_addr = generator.gen_var_alloc(ctx, init_val_t.into(), None)?;
            ctx.builder.build_store(i_addr, init_val).unwrap();

            Ok(i_addr)
        },
        |_, ctx, i_addr| {
            let cmp_op = if max_val.1 {
                IntPredicate::ULE
            } else {
                IntPredicate::ULT
            };

            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let max_val = ctx.builder
                .build_int_z_extend_or_bit_cast(max_val.0, init_val_t, "")
                .unwrap();

            Ok(ctx.builder.build_int_compare(cmp_op, i, max_val, "").unwrap())
        },
        |generator, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            body(generator, ctx, i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let incr_val = ctx.builder
                .build_int_z_extend_or_bit_cast(incr_val, init_val_t, "")
                .unwrap();
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
    let StmtKind::While { test, body, orelse, .. } = &stmt.node else {
        unreachable!()
    };

    // var_assignment static values may be changed in another branch
    // if so, remove the static value as it may not be correct in this branch
    let var_assignment = ctx.var_assignment.clone();

    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
    let test_bb = ctx.ctx.append_basic_block(current, "while.test");
    let body_bb = ctx.ctx.append_basic_block(current, "while.body");
    let cont_bb = ctx.ctx.append_basic_block(current, "while.cont");
    // if there is no orelse, we just go to cont_bb
    let orelse_bb =
        if orelse.is_empty() { cont_bb } else { ctx.ctx.append_basic_block(current, "while.orelse") };
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

        return Ok(())
    };
    let BasicValueEnum::IntValue(test) = test else {
        unreachable!()
    };

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

/// See [`CodeGenerator::gen_if`].
pub fn gen_if<G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> Result<(), String> {
    let StmtKind::If { test, body, orelse, .. } = &stmt.node else {
        unreachable!()
    };

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
    let test = generator
        .gen_expr(ctx, test)
        .and_then(|v| v.map(|v| v.to_basic_value_enum(ctx, generator, test.custom.unwrap())).transpose())?;
    if let Some(BasicValueEnum::IntValue(test)) = test {
        ctx.builder
            .build_conditional_branch(generator.bool_to_i1(ctx, test), body_bb, orelse_bb)
            .unwrap();
    };
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
) -> FunctionValue<'ctx> {
    ctx.module.get_function(symbol).unwrap_or_else(|| {
        let ty = match symbol {
            "__nac3_raise" => ctx
                .ctx
                .void_type()
                .fn_type(&[ctx.get_llvm_type(generator, ctx.primitives.exception).into()], false),
            "__nac3_resume" | "__nac3_end_catch" => ctx.ctx.void_type().fn_type(&[], false),
            _ => unimplemented!(),
        };
        let fun = ctx.module.add_function(symbol, ty, None);
        if symbol == "__nac3_raise" || symbol == "__nac3_resume" {
            fun.add_attribute(
                AttributeLoc::Function,
                ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
            );
        }
        fun
    })
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
        unreachable!()
    };
    let defs = ctx.top_level.definitions.read();
    let def = defs[zelf_id].read();
    let TopLevelDef::Class { name: zelf_name, .. } = &*def else {
        unreachable!()
    };
    let exception_name = format!("{}:{}", ctx.resolver.get_exception_id(zelf_id), zelf_name);
    unsafe {
        let id_ptr = ctx.builder.build_in_bounds_gep(zelf, &[zero, zero], "exn.id").unwrap();
        let id = ctx.resolver.get_string_id(&exception_name);
        ctx.builder.build_store(id_ptr, int32.const_int(id as u64, false)).unwrap();
        let empty_string = ctx.gen_const(generator, &Constant::Str(String::new()), ctx.primitives.str);
        let ptr = ctx.builder
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
            let ptr = ctx.builder
                .build_in_bounds_gep(zelf, &[zero, int32.const_int(*i, false)], "exn.param")
                .unwrap();
            ctx.builder.build_store(ptr, value).unwrap();
        }
        // set file, func to empty string
        for i in &[1, 4] {
            let ptr = ctx.builder
                .build_in_bounds_gep(zelf, &[zero, int32.const_int(*i, false)], "exn.str")
                .unwrap();
            ctx.builder.build_store(ptr, empty_string.unwrap()).unwrap();
        }
        // set ints to zero
        for i in &[2, 3] {
            let ptr = ctx.builder
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
    exception: Option<&BasicValueEnum<'ctx>>,
    loc: Location,
) {
    if let Some(exception) = exception {
        unsafe {
            let int32 = ctx.ctx.i32_type();
            let zero = int32.const_zero();
            let exception = exception.into_pointer_value();
            let file_ptr = ctx.builder
                .build_in_bounds_gep(exception, &[zero, int32.const_int(1, false)], "file_ptr")
                .unwrap();
            let filename = ctx.gen_string(generator, loc.file.0);
            ctx.builder.build_store(file_ptr, filename).unwrap();
            let row_ptr = ctx.builder
                .build_in_bounds_gep(exception, &[zero, int32.const_int(2, false)], "row_ptr")
                .unwrap();
            ctx.builder.build_store(row_ptr, int32.const_int(loc.row as u64, false)).unwrap();
            let col_ptr = ctx.builder
                .build_in_bounds_gep(exception, &[zero, int32.const_int(3, false)], "col_ptr")
                .unwrap();
            ctx.builder.build_store(col_ptr, int32.const_int(loc.column as u64, false)).unwrap();

            let current_fun = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
            let fun_name = ctx.gen_string(generator, current_fun.get_name().to_str().unwrap());
            let name_ptr = ctx.builder
                .build_in_bounds_gep(exception, &[zero, int32.const_int(4, false)], "name_ptr")
                .unwrap();
            ctx.builder.build_store(name_ptr, fun_name).unwrap();
        }

        let raise = get_builtins(generator, ctx, "__nac3_raise");
        let exception = *exception;
        ctx.build_call_or_invoke(raise, &[exception], "raise");
    } else {
        let resume = get_builtins(generator, ctx, "__nac3_resume");
        ctx.build_call_or_invoke(resume, &[], "resume");
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
        unreachable!()
    };

    // if we need to generate anything related to exception, we must have personality defined
    let personality_symbol = ctx.top_level.personality_symbol.as_ref().unwrap();
    let personality = ctx.module.get_function(personality_symbol).unwrap_or_else(|| {
        let ty = ctx.ctx.i32_type().fn_type(&[], true);
        ctx.module.add_function(personality_symbol, ty, None)
    });
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
        let final_state = generator.gen_var_alloc(ctx, ptr_type.into(), Some("try.final_state.addr"))?;
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
            let return_value = ctx.return_buffer
                .map(|v| ctx.builder.build_load(v, "$ret").unwrap());
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
        let obj_id = if let TypeEnum::TObj { obj_id, .. } = &*ctx.unifier.get_ty(type_.custom.unwrap()) {
            *obj_id
        } else {
            unreachable!()
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
    ctx.loop_target = old_loop_target.or(ctx.loop_target).take();

    let old_unwind = if finalbody.is_empty() {
        None
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
        |ctx: &mut CodeGenContext<'ctx, 'a>,
         target: BasicBlock<'ctx>,
         block: BasicBlock<'ctx>| final_proxy(ctx, target, block, final_data.as_mut().unwrap());
    let mut redirect_lambda = |ctx: &mut CodeGenContext<'ctx, 'a>,
                               target: BasicBlock<'ctx>,
                               block: BasicBlock<'ctx>| {
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
        ctx.builder.build_call(end_catch, &[], "end_catch").unwrap();
        ctx.builder.position_at_end(continue_proxy);
        ctx.builder.build_call(end_catch, &[], "end_catch").unwrap();
        ctx.builder.position_at_end(body);
        redirect(ctx, break_target, break_proxy);
        redirect(ctx, continue_target, continue_proxy);
        ctx.loop_target = Some((continue_proxy, break_proxy));
        old_loop_target = Some((continue_target, break_target));
    }
    let return_proxy = ctx.ctx.append_basic_block(current_fun, "try.return");
    ctx.builder.position_at_end(return_proxy);
    ctx.builder.build_call(end_catch, &[], "end_catch").unwrap();
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
            let exnid_ptr = ctx.builder
                .build_gep(
                    exn.as_basic_value().into_pointer_value(),
                    &[zero, zero],
                    "exnidptr",
                )
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
            ctx.builder.build_call(end_catch, &[], "end_catch").unwrap();
        }
        post_handlers.push(current);
        ctx.builder.position_at_end(dispatcher_end);
        if let Some(exn_type) = exn_type {
            let dispatcher_cont =
                ctx.ctx.append_basic_block(current_fun, "try.dispatcher_cont");
            let actual_id = exnid.unwrap().into_int_value();
            let expected_id = ctx
                .builder
                .build_load(exn_type.into_pointer_value(), "expected_id")
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            let result = ctx.builder
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
    ctx.loop_target = old_loop_target.or(ctx.loop_target).take();
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
            ctx.build_call_or_invoke(resume, &[], "resume");
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
            ctx.build_call_or_invoke(resume, &[], "resume");
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
pub fn gen_with<G: CodeGenerator>(
    _: &mut G,
    _: &mut CodeGenContext<'_, '_>,
    stmt: &Stmt<Option<Type>>,
) -> Result<(), String> {
    // TODO: Implement with statement after finishing exceptions
    Err(format!("With statement with custom types is not yet supported (at {})", stmt.location))
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
            Some(
                v.and_then(|v| v.to_basic_value_enum(ctx, generator, v_expr.custom.unwrap()))?
            )
        } else {
            return Ok(())
        }
    } else {
        None
    };
    if let Some(return_target) = ctx.return_target {
        if let Some(value) = value {
            ctx.builder.build_store(ctx.return_buffer.unwrap(), value).unwrap();
        }
        ctx.builder.build_unconditional_branch(return_target).unwrap();
    } else if ctx.need_sret {
        // sret
        ctx.builder.build_store(ctx.return_buffer.unwrap(), value.unwrap()).unwrap();
        ctx.builder.build_return(None).unwrap();
    } else {
        // Remap boolean return type into i1
        let value = value.map(|v| {
            let expected_ty = func.get_type().get_return_type().unwrap();
            let ret_val = v.as_basic_value_enum();

            if expected_ty.is_int_type() && ret_val.is_int_value() {
                let ret_type = expected_ty.into_int_type();
                let ret_val = ret_val.into_int_value();

                if ret_type.get_bit_width() == 1 && ret_val.get_type().get_bit_width() != 1 {
                    generator.bool_to_i1(ctx, ret_val)
                } else {
                    ret_val
                }.into()
            } else {
                ret_val
            }
        });
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
                let Some(value) = generator.gen_expr(ctx, value)? else {
                    return Ok(())
                };
                generator.gen_assign(ctx, target, value)?;
            }
        }
        StmtKind::Assign { targets, value, .. } => {
            let Some(value) = generator.gen_expr(ctx, value)? else {
                return Ok(())
            };
            for target in targets {
                generator.gen_assign(ctx, target, value.clone())?;
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
            let value = gen_binop_expr(generator, ctx, target, op, value, stmt.location, true)?;
            generator.gen_assign(ctx, target, value.unwrap())?;
        }
        StmtKind::Try { .. } => gen_try(generator, ctx, stmt)?,
        StmtKind::Raise { exc, .. } => {
            if let Some(exc) = exc {
                let exc = if let Some(v) = generator.gen_expr(ctx, exc)? {
                    v.to_basic_value_enum(ctx, generator, exc.custom.unwrap())?
                } else {
                    return Ok(())
                };
                gen_raise(generator, ctx, Some(&exc), stmt.location);
            } else {
                gen_raise(generator, ctx, None, stmt.location);
            }
        }
        StmtKind::Assert { test, msg, .. } => {
            let test = if let Some(v) = generator.gen_expr(ctx, test)? {
                v.to_basic_value_enum(ctx, generator, test.custom.unwrap())?
            } else {
                return Ok(())
            };
            let err_msg = match msg {
                Some(msg) => if let Some(v) = generator.gen_expr(ctx, msg)? {
                    v.to_basic_value_enum(ctx, generator, msg.custom.unwrap())?
                } else {
                    return Ok(())
                },
                None => ctx.gen_string(generator, ""),
            };
            ctx.make_assert_impl(
                generator,
                test.into_int_value(),
                "0:AssertionError",
                err_msg,
                [None, None, None],
                stmt.location,
            );
        }
        _ => unimplemented!()
    };
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
