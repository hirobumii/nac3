use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, CallSiteValue, IntValue},
    AddressSpace, IntPredicate,
};
use itertools::Either;

use super::calculate_len_for_slice_range;
use crate::codegen::{
    macros::codegen_unreachable,
    values::{ArrayLikeValue, ListValue},
    CodeGenContext, CodeGenerator,
};

/// This function handles 'end' **inclusively**.
/// Order of tuples `assign_idx` and `value_idx` is ('start', 'end', 'step').
/// Negative index should be handled before entering this function
pub fn list_slice_assignment<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: BasicTypeEnum<'ctx>,
    dest_arr: ListValue<'ctx>,
    dest_idx: (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
    src_arr: ListValue<'ctx>,
    src_idx: (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
) {
    let llvm_usize = ctx.get_size_type();
    let llvm_pi8 = ctx.ctx.i8_type().ptr_type(AddressSpace::default());
    let llvm_i32 = ctx.ctx.i32_type();

    assert_eq!(dest_idx.0.get_type(), llvm_i32);
    assert_eq!(dest_idx.1.get_type(), llvm_i32);
    assert_eq!(dest_idx.2.get_type(), llvm_i32);
    assert_eq!(src_idx.0.get_type(), llvm_i32);
    assert_eq!(src_idx.1.get_type(), llvm_i32);
    assert_eq!(src_idx.2.get_type(), llvm_i32);

    let (fun_symbol, elem_ptr_type) = ("__nac3_list_slice_assign_var_size", llvm_pi8);
    let slice_assign_fun = {
        let ty_vec = vec![
            llvm_i32.into(),      // dest start idx
            llvm_i32.into(),      // dest end idx
            llvm_i32.into(),      // dest step
            elem_ptr_type.into(), // dest arr ptr
            llvm_i32.into(),      // dest arr len
            llvm_i32.into(),      // src start idx
            llvm_i32.into(),      // src end idx
            llvm_i32.into(),      // src step
            elem_ptr_type.into(), // src arr ptr
            llvm_i32.into(),      // src arr len
            llvm_i32.into(),      // size
        ];
        ctx.module.get_function(fun_symbol).unwrap_or_else(|| {
            let fn_t = llvm_i32.fn_type(ty_vec.as_slice(), false);
            ctx.module.add_function(fun_symbol, fn_t, None)
        })
    };

    let zero = llvm_i32.const_zero();
    let one = llvm_i32.const_int(1, false);
    let dest_arr_ptr = dest_arr.data().base_ptr(ctx, generator);
    let dest_arr_ptr =
        ctx.builder.build_pointer_cast(dest_arr_ptr, elem_ptr_type, "dest_arr_ptr_cast").unwrap();
    let dest_len = dest_arr.load_size(ctx, Some("dest.len"));
    let dest_len =
        ctx.builder.build_int_truncate_or_bit_cast(dest_len, llvm_i32, "srclen32").unwrap();
    let src_arr_ptr = src_arr.data().base_ptr(ctx, generator);
    let src_arr_ptr =
        ctx.builder.build_pointer_cast(src_arr_ptr, elem_ptr_type, "src_arr_ptr_cast").unwrap();
    let src_len = src_arr.load_size(ctx, Some("src.len"));
    let src_len =
        ctx.builder.build_int_truncate_or_bit_cast(src_len, llvm_i32, "srclen32").unwrap();

    // index in bound and positive should be done
    // assert if dest.step == 1 then len(src) <= len(dest) else len(src) == len(dest), and
    // throw exception if not satisfied
    let src_end = ctx
        .builder
        .build_select(
            ctx.builder.build_int_compare(IntPredicate::SLT, src_idx.2, zero, "is_neg").unwrap(),
            ctx.builder.build_int_sub(src_idx.1, one, "e_min_one").unwrap(),
            ctx.builder.build_int_add(src_idx.1, one, "e_add_one").unwrap(),
            "final_e",
        )
        .map(BasicValueEnum::into_int_value)
        .unwrap();
    let dest_end = ctx
        .builder
        .build_select(
            ctx.builder.build_int_compare(IntPredicate::SLT, dest_idx.2, zero, "is_neg").unwrap(),
            ctx.builder.build_int_sub(dest_idx.1, one, "e_min_one").unwrap(),
            ctx.builder.build_int_add(dest_idx.1, one, "e_add_one").unwrap(),
            "final_e",
        )
        .map(BasicValueEnum::into_int_value)
        .unwrap();
    let src_slice_len =
        calculate_len_for_slice_range(generator, ctx, src_idx.0, src_end, src_idx.2);
    let dest_slice_len =
        calculate_len_for_slice_range(generator, ctx, dest_idx.0, dest_end, dest_idx.2);
    let src_eq_dest = ctx
        .builder
        .build_int_compare(IntPredicate::EQ, src_slice_len, dest_slice_len, "slice_src_eq_dest")
        .unwrap();
    let src_slt_dest = ctx
        .builder
        .build_int_compare(IntPredicate::SLT, src_slice_len, dest_slice_len, "slice_src_slt_dest")
        .unwrap();
    let dest_step_eq_one = ctx
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            dest_idx.2,
            dest_idx.2.get_type().const_int(1, false),
            "slice_dest_step_eq_one",
        )
        .unwrap();
    let cond_1 = ctx.builder.build_and(dest_step_eq_one, src_slt_dest, "slice_cond_1").unwrap();
    let cond = ctx.builder.build_or(src_eq_dest, cond_1, "slice_cond").unwrap();
    ctx.make_assert(
        generator,
        cond,
        "0:ValueError",
        "attempt to assign sequence of size {0} to slice of size {1} with step size {2}",
        [Some(src_slice_len), Some(dest_slice_len), Some(dest_idx.2)],
        ctx.current_loc,
    );

    let new_len = {
        let args = vec![
            dest_idx.0.into(),   // dest start idx
            dest_idx.1.into(),   // dest end idx
            dest_idx.2.into(),   // dest step
            dest_arr_ptr.into(), // dest arr ptr
            dest_len.into(),     // dest arr len
            src_idx.0.into(),    // src start idx
            src_idx.1.into(),    // src end idx
            src_idx.2.into(),    // src step
            src_arr_ptr.into(),  // src arr ptr
            src_len.into(),      // src arr len
            {
                let s = match ty {
                    BasicTypeEnum::FloatType(t) => t.size_of(),
                    BasicTypeEnum::IntType(t) => t.size_of(),
                    BasicTypeEnum::PointerType(t) => t.size_of(),
                    BasicTypeEnum::StructType(t) => t.size_of().unwrap(),
                    _ => codegen_unreachable!(ctx),
                };
                ctx.builder.build_int_truncate_or_bit_cast(s, llvm_i32, "size").unwrap()
            }
            .into(),
        ];
        ctx.builder
            .build_call(slice_assign_fun, args.as_slice(), "slice_assign")
            .map(CallSiteValue::try_as_basic_value)
            .map(|v| v.map_left(BasicValueEnum::into_int_value))
            .map(Either::unwrap_left)
            .unwrap()
    };

    // update length
    let need_update =
        ctx.builder.build_int_compare(IntPredicate::NE, new_len, dest_len, "need_update").unwrap();
    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
    let update_bb = ctx.ctx.append_basic_block(current, "update");
    let cont_bb = ctx.ctx.append_basic_block(current, "cont");
    ctx.builder.build_conditional_branch(need_update, update_bb, cont_bb).unwrap();
    ctx.builder.position_at_end(update_bb);
    let new_len =
        ctx.builder.build_int_z_extend_or_bit_cast(new_len, llvm_usize, "new_len").unwrap();
    dest_arr.store_size(ctx, new_len);
    ctx.builder.build_unconditional_branch(cont_bb).unwrap();
    ctx.builder.position_at_end(cont_bb);
}
