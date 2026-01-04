use inkwell::{
    IntPredicate,
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue},
};

use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::calculate_len_for_slice_range,
    macros::codegen_unreachable,
    stmt::gen_if_callback,
    types::{ListValue, field},
};

/// This function handles 'end' **inclusively**.
/// Order of tuples `assign_idx` and `value_idx` is ('start', 'end', 'step').
/// Negative index should be handled before entering this function
pub fn list_slice_assignment<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: BasicTypeEnum<'ctx>,
    dest_arr: ListValue<'ctx>,
    dest_idx: (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
    src_arr: ListValue<'ctx>,
    src_idx: (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
) {
    let llvm_usize = ctx.size_t;
    let llvm_i32 = ctx.i32;

    assert_eq!(dest_idx.0.get_type(), llvm_i32);
    assert_eq!(dest_idx.1.get_type(), llvm_i32);
    assert_eq!(dest_idx.2.get_type(), llvm_i32);
    assert_eq!(src_idx.0.get_type(), llvm_i32);
    assert_eq!(src_idx.1.get_type(), llvm_i32);
    assert_eq!(src_idx.2.get_type(), llvm_i32);

    let fun_symbol = "__nac3_list_slice_assign_var_size";

    let zero = llvm_i32.const_zero();
    let one = llvm_i32.const_int(1, false);
    let (dest_ptr, dest_len) = dest_arr.data(ctx).value;
    let dest_len =
        ctx.builder.build_int_truncate_or_bit_cast(dest_len, llvm_i32, "srclen32").unwrap();
    let (src_ptr, src_len) = src_arr.data(ctx).value;
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
    let src_slice_len = calculate_len_for_slice_range(ctx, src_idx.0, src_end, src_idx.2);
    let dest_slice_len = calculate_len_for_slice_range(ctx, dest_idx.0, dest_end, dest_idx.2);
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
        cond,
        "0:ValueError",
        "attempt to assign sequence of size {0} to slice of size {1} with step size {2}",
        [Some(src_slice_len), Some(dest_slice_len), Some(dest_idx.2)],
        ctx.current_loc,
    );

    let new_len = call_extern!(ctx: llvm_i32 "slice_assign" = fun_symbol(
        dest_idx.0,   // dest start idx
        dest_idx.1,   // dest end idx
        dest_idx.2,   // dest step
        dest_ptr,     // dest arr ptr
        dest_len,     // dest arr len
        src_idx.0,    // src start idx
        src_idx.1,    // src end idx
        src_idx.2,    // src step
        src_ptr,      // src arr ptr
        src_len,      // src arr len
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
    ));

    // update length
    gen_if_callback(
        &mut (),
        ctx,
        |(), ctx| {
            Ok(ctx
                .builder
                .build_int_compare(IntPredicate::NE, new_len, dest_len, "need_update")
                .unwrap())
        },
        |(), ctx| {
            let new_len =
                ctx.builder.build_int_z_extend_or_bit_cast(new_len, llvm_usize, "new_len").unwrap();
            dest_arr.store(ctx, field!(len), new_len);
            Ok(())
        },
        |(), _| Ok(()),
    )
    .unwrap();
}
