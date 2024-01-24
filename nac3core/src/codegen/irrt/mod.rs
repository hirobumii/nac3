use crate::typecheck::typedef::Type;

use super::{assert_is_list, assert_is_ndarray, CodeGenContext, CodeGenerator};
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    context::Context,
    memory_buffer::MemoryBuffer,
    module::Module,
    types::BasicTypeEnum,
    values::{FloatValue, IntValue, PointerValue},
    AddressSpace, IntPredicate,
};
use nac3parser::ast::Expr;

#[must_use]
pub fn load_irrt(ctx: &Context) -> Module {
    let bitcode_buf = MemoryBuffer::create_from_memory_range(
        include_bytes!(concat!(env!("OUT_DIR"), "/irrt.bc")),
        "irrt_bitcode_buffer",
    );
    let irrt_mod = Module::parse_bitcode_from_buffer(&bitcode_buf, ctx).unwrap();
    let inline_attr = Attribute::get_named_enum_kind_id("alwaysinline");
    for symbol in &[
        "__nac3_int_exp_int32_t",
        "__nac3_int_exp_int64_t",
        "__nac3_range_slice_len",
        "__nac3_slice_index_bound",
    ] {
        let function = irrt_mod.get_function(symbol).unwrap();
        function.add_attribute(AttributeLoc::Function, ctx.create_enum_attribute(inline_attr, 0));
    }
    irrt_mod
}

// repeated squaring method adapted from GNU Scientific Library:
// https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
pub fn integer_power<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    base: IntValue<'ctx>,
    exp: IntValue<'ctx>,
    signed: bool,
) -> IntValue<'ctx> {
    let symbol = match (base.get_type().get_bit_width(), exp.get_type().get_bit_width(), signed) {
        (32, 32, true) => "__nac3_int_exp_int32_t",
        (64, 64, true) => "__nac3_int_exp_int64_t",
        (32, 32, false) => "__nac3_int_exp_uint32_t",
        (64, 64, false) => "__nac3_int_exp_uint64_t",
        _ => unreachable!(),
    };
    let base_type = base.get_type();
    let pow_fun = ctx.module.get_function(symbol).unwrap_or_else(|| {
        let fn_type = base_type.fn_type(&[base_type.into(), base_type.into()], false);
        ctx.module.add_function(symbol, fn_type, None)
    });
    // throw exception when exp < 0
    let ge_zero = ctx.builder.build_int_compare(
        IntPredicate::SGE,
        exp,
        exp.get_type().const_zero(),
        "assert_int_pow_ge_0",
    );
    ctx.make_assert(
        generator,
        ge_zero,
        "0:ValueError",
        "integer power must be positive or zero",
        [None, None, None],
        ctx.current_loc,
    );
    ctx.builder
        .build_call(pow_fun, &[base.into(), exp.into()], "call_int_pow")
        .try_as_basic_value()
        .unwrap_left()
        .into_int_value()
}

pub fn calculate_len_for_slice_range<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    start: IntValue<'ctx>,
    end: IntValue<'ctx>,
    step: IntValue<'ctx>,
) -> IntValue<'ctx> {
    const SYMBOL: &str = "__nac3_range_slice_len";
    let len_func = ctx.module.get_function(SYMBOL).unwrap_or_else(|| {
        let i32_t = ctx.ctx.i32_type();
        let fn_t = i32_t.fn_type(&[i32_t.into(), i32_t.into(), i32_t.into()], false);
        ctx.module.add_function(SYMBOL, fn_t, None)
    });

    // assert step != 0, throw exception if not
    let not_zero = ctx.builder.build_int_compare(
        IntPredicate::NE,
        step,
        step.get_type().const_zero(),
        "range_step_ne",
    );
    ctx.make_assert(
        generator,
        not_zero,
        "0:ValueError",
        "step must not be zero",
        [None, None, None],
        ctx.current_loc,
    );
    ctx.builder
        .build_call(len_func, &[start.into(), end.into(), step.into()], "calc_len")
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value()
}

/// NOTE: the output value of the end index of this function should be compared ***inclusively***,
/// because python allows `a[2::-1]`, whose semantic is `[a[2], a[1], a[0]]`, which is equivalent to
/// NO numeric slice in python.
///
/// equivalent code:
/// ```pseudo_code
/// match (start, end, step):
///     case (s, e, None | Some(step)) if step > 0:
///         return (
///             match s:
///                 case None:
///                     0
///                 case Some(s):
///                     handle_in_bound(s)
///             ,match e:
///                 case None:
///                     length - 1
///                 case Some(e):
///                     handle_in_bound(e) - 1
///             ,step == None ? 1 : step
///         )
///     case (s, e, Some(step)) if step < 0:
///         return (
///             match s:
///                 case None:
///                     length - 1
///                 case Some(s):
///                     s = handle_in_bound(s)
///                     if s == length:
///                         s - 1
///                     else:
///                         s
///             ,match e:
///                 case None:
///                     0
///                 case Some(e):
///                     handle_in_bound(e) + 1
///             ,step
///         )
/// ```
pub fn handle_slice_indices<'ctx, G: CodeGenerator>(
    start: &Option<Box<Expr<Option<Type>>>>,
    end: &Option<Box<Expr<Option<Type>>>>,
    step: &Option<Box<Expr<Option<Type>>>>,
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut G,
    list: PointerValue<'ctx>,
) -> Result<Option<(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)>, String> {
    let int32 = ctx.ctx.i32_type();
    let zero = int32.const_zero();
    let one = int32.const_int(1, false);
    let length = ctx.build_gep_and_load(list, &[zero, one], Some("length")).into_int_value();
    let length = ctx.builder.build_int_truncate_or_bit_cast(length, int32, "leni32");
    Ok(Some(match (start, end, step) {
        (s, e, None) => (
            if let Some(s) = s.as_ref() {
                match handle_slice_index_bound(s, ctx, generator, length)? {
                    Some(v) => v,
                    None => return Ok(None),
                }
            } else {
                int32.const_zero()
            },
            {
                let e = if let Some(s) = e.as_ref() {
                    match handle_slice_index_bound(s, ctx, generator, length)? {
                        Some(v) => v,
                        None => return Ok(None),
                    }
                } else {
                    length
                };
                ctx.builder.build_int_sub(e, one, "final_end")
            },
            one,
        ),
        (s, e, Some(step)) => {
            let step = if let Some(v) = generator.gen_expr(ctx, step)? {
                v.to_basic_value_enum(ctx, generator, ctx.primitives.int32)?.into_int_value()
            } else {
                return Ok(None)
            };
            // assert step != 0, throw exception if not
            let not_zero = ctx.builder.build_int_compare(
                IntPredicate::NE,
                step,
                step.get_type().const_zero(),
                "range_step_ne",
            );
            ctx.make_assert(
                generator,
                not_zero,
                "0:ValueError",
                "slice step cannot be zero",
                [None, None, None],
                ctx.current_loc,
            );
            let len_id = ctx.builder.build_int_sub(length, one, "lenmin1");
            let neg = ctx.builder.build_int_compare(IntPredicate::SLT, step, zero, "step_is_neg");
            (
                match s {
                    Some(s) => {
                        let Some(s) = handle_slice_index_bound(s, ctx, generator, length)? else {
                            return Ok(None)
                        };
                        ctx.builder
                            .build_select(
                                ctx.builder.build_and(
                                    ctx.builder.build_int_compare(
                                        IntPredicate::EQ,
                                        s,
                                        length,
                                        "s_eq_len",
                                    ),
                                    neg,
                                    "should_minus_one",
                                ),
                                ctx.builder.build_int_sub(s, one, "s_min"),
                                s,
                                "final_start",
                            )
                            .into_int_value()
                    }
                    None => ctx.builder.build_select(neg, len_id, zero, "stt").into_int_value(),
                },
                match e {
                    Some(e) => {
                        let Some(e) = handle_slice_index_bound(e, ctx, generator, length)? else {
                            return Ok(None)
                        };
                        ctx.builder
                            .build_select(
                                neg,
                                ctx.builder.build_int_add(e, one, "end_add_one"),
                                ctx.builder.build_int_sub(e, one, "end_sub_one"),
                                "final_end",
                            )
                            .into_int_value()
                    }
                    None => ctx.builder.build_select(neg, zero, len_id, "end").into_int_value(),
                },
                step,
            )
        }
    }))
}

/// this function allows index out of range, since python
/// allows index out of range in slice (`a = [1,2,3]; a[1:10] == [2,3]`).
pub fn handle_slice_index_bound<'ctx, G: CodeGenerator>(
    i: &Expr<Option<Type>>,
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut G,
    length: IntValue<'ctx>,
) -> Result<Option<IntValue<'ctx>>, String> {
    const SYMBOL: &str = "__nac3_slice_index_bound";
    let func = ctx.module.get_function(SYMBOL).unwrap_or_else(|| {
        let i32_t = ctx.ctx.i32_type();
        let fn_t = i32_t.fn_type(&[i32_t.into(), i32_t.into()], false);
        ctx.module.add_function(SYMBOL, fn_t, None)
    });

    let i = if let Some(v) = generator.gen_expr(ctx, i)? {
        v.to_basic_value_enum(ctx, generator, i.custom.unwrap())?
    } else {
        return Ok(None)
    };
    Ok(Some(ctx
        .builder
        .build_call(func, &[i.into(), length.into()], "bounded_ind")
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value()))
}

/// This function handles 'end' **inclusively**.
/// Order of tuples `assign_idx` and `value_idx` is ('start', 'end', 'step').
/// Negative index should be handled before entering this function
pub fn list_slice_assignment<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: BasicTypeEnum<'ctx>,
    dest_arr: PointerValue<'ctx>,
    dest_idx: (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
    src_arr: PointerValue<'ctx>,
    src_idx: (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
) {
    let size_ty = generator.get_size_type(ctx.ctx);
    let int8_ptr = ctx.ctx.i8_type().ptr_type(AddressSpace::default());
    let int32 = ctx.ctx.i32_type();
    let (fun_symbol, elem_ptr_type) = ("__nac3_list_slice_assign_var_size", int8_ptr);
    let slice_assign_fun = {
        let ty_vec = vec![
            int32.into(),         // dest start idx
            int32.into(),         // dest end idx
            int32.into(),         // dest step
            elem_ptr_type.into(), // dest arr ptr
            int32.into(),         // dest arr len
            int32.into(),         // src start idx
            int32.into(),         // src end idx
            int32.into(),         // src step
            elem_ptr_type.into(), // src arr ptr
            int32.into(),         // src arr len
            int32.into(),         // size
        ];
        ctx.module.get_function(fun_symbol).unwrap_or_else(|| {
            let fn_t = int32.fn_type(ty_vec.as_slice(), false);
            ctx.module.add_function(fun_symbol, fn_t, None)
        })
    };

    let zero = int32.const_zero();
    let one = int32.const_int(1, false);
    let dest_arr_ptr = ctx.build_gep_and_load(dest_arr, &[zero, zero], Some("dest.addr"));
    let dest_arr_ptr = ctx.builder.build_pointer_cast(
        dest_arr_ptr.into_pointer_value(),
        elem_ptr_type,
        "dest_arr_ptr_cast",
    );
    let dest_len = ctx.build_gep_and_load(dest_arr, &[zero, one], Some("dest.len")).into_int_value();
    let dest_len = ctx.builder.build_int_truncate_or_bit_cast(dest_len, int32, "srclen32");
    let src_arr_ptr = ctx.build_gep_and_load(src_arr, &[zero, zero], Some("src.addr"));
    let src_arr_ptr = ctx.builder.build_pointer_cast(
        src_arr_ptr.into_pointer_value(),
        elem_ptr_type,
        "src_arr_ptr_cast",
    );
    let src_len = ctx.build_gep_and_load(src_arr, &[zero, one], Some("src.len")).into_int_value();
    let src_len = ctx.builder.build_int_truncate_or_bit_cast(src_len, int32, "srclen32");

    // index in bound and positive should be done
    // assert if dest.step == 1 then len(src) <= len(dest) else len(src) == len(dest), and
    // throw exception if not satisfied
    let src_end = ctx.builder
        .build_select(
            ctx.builder.build_int_compare(
                IntPredicate::SLT,
                src_idx.2,
                zero,
                "is_neg",
            ),
            ctx.builder.build_int_sub(src_idx.1, one, "e_min_one"),
            ctx.builder.build_int_add(src_idx.1, one, "e_add_one"),
            "final_e",
        )
        .into_int_value();
    let dest_end = ctx.builder
        .build_select(
            ctx.builder.build_int_compare(
                IntPredicate::SLT,
                dest_idx.2,
                zero,
                "is_neg",
            ),
            ctx.builder.build_int_sub(dest_idx.1, one, "e_min_one"),
            ctx.builder.build_int_add(dest_idx.1, one, "e_add_one"),
            "final_e",
        )
        .into_int_value();
    let src_slice_len =
        calculate_len_for_slice_range(generator, ctx, src_idx.0, src_end, src_idx.2);
    let dest_slice_len =
        calculate_len_for_slice_range(generator, ctx, dest_idx.0, dest_end, dest_idx.2);
    let src_eq_dest = ctx.builder.build_int_compare(
        IntPredicate::EQ,
        src_slice_len,
        dest_slice_len,
        "slice_src_eq_dest",
    );
    let src_slt_dest = ctx.builder.build_int_compare(
        IntPredicate::SLT,
        src_slice_len,
        dest_slice_len,
        "slice_src_slt_dest",
    );
    let dest_step_eq_one = ctx.builder.build_int_compare(
        IntPredicate::EQ,
        dest_idx.2,
        dest_idx.2.get_type().const_int(1, false),
        "slice_dest_step_eq_one",
    );
    let cond_1 = ctx.builder.build_and(dest_step_eq_one, src_slt_dest, "slice_cond_1");
    let cond = ctx.builder.build_or(src_eq_dest, cond_1, "slice_cond");
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
                    _ => unreachable!(),
                };
                ctx.builder.build_int_truncate_or_bit_cast(s, int32, "size")
            }
            .into(),
        ];
        ctx.builder
            .build_call(slice_assign_fun, args.as_slice(), "slice_assign")
            .try_as_basic_value()
            .unwrap_left()
            .into_int_value()
    };
    // update length
    let need_update =
        ctx.builder.build_int_compare(IntPredicate::NE, new_len, dest_len, "need_update");
    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
    let update_bb = ctx.ctx.append_basic_block(current, "update");
    let cont_bb = ctx.ctx.append_basic_block(current, "cont");
    ctx.builder.build_conditional_branch(need_update, update_bb, cont_bb);
    ctx.builder.position_at_end(update_bb);
    let dest_len_ptr = unsafe { ctx.builder.build_gep(dest_arr, &[zero, one], "dest_len_ptr") };
    let new_len = ctx.builder.build_int_z_extend_or_bit_cast(new_len, size_ty, "new_len");
    ctx.builder.build_store(dest_len_ptr, new_len);
    ctx.builder.build_unconditional_branch(cont_bb);
    ctx.builder.position_at_end(cont_bb);
}

/// Generates a call to `isinf` in IR. Returns an `i1` representing the result.
pub fn call_isinf<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> IntValue<'ctx> {
    let intrinsic_fn = ctx.module.get_function("__nac3_isinf").unwrap_or_else(|| {
        let fn_type = ctx.ctx.i32_type().fn_type(&[ctx.ctx.f64_type().into()], false);
        ctx.module.add_function("__nac3_isinf", fn_type, None)
    });

    let ret = ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "isinf")
        .try_as_basic_value()
        .unwrap_left()
        .into_int_value();

    generator.bool_to_i1(ctx, ret)
}

/// Generates a call to `isnan` in IR. Returns an `i1` representing the result.
pub fn call_isnan<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> IntValue<'ctx> {
    let intrinsic_fn = ctx.module.get_function("__nac3_isnan").unwrap_or_else(|| {
        let fn_type = ctx.ctx.i32_type().fn_type(&[ctx.ctx.f64_type().into()], false);
        ctx.module.add_function("__nac3_isnan", fn_type, None)
    });

    let ret = ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "isnan")
        .try_as_basic_value()
        .unwrap_left()
        .into_int_value();

    generator.bool_to_i1(ctx, ret)
}

/// Generates a call to `gamma` in IR. Returns an `f64` representing the result.
pub fn call_gamma<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let intrinsic_fn = ctx.module.get_function("__nac3_gamma").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_gamma", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "gamma")
        .try_as_basic_value()
        .unwrap_left()
        .into_float_value()
}

/// Generates a call to `gammaln` in IR. Returns an `f64` representing the result.
pub fn call_gammaln<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let intrinsic_fn = ctx.module.get_function("__nac3_gammaln").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_gammaln", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "gammaln")
        .try_as_basic_value()
        .unwrap_left()
        .into_float_value()
}

/// Generates a call to `j0` in IR. Returns an `f64` representing the result.
pub fn call_j0<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let intrinsic_fn = ctx.module.get_function("__nac3_j0").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_j0", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "j0")
        .try_as_basic_value()
        .unwrap_left()
        .into_float_value()
}

/// Generates a call to `__nac3_ndarray_calc_size`. Returns an [IntValue] representing the
/// calculated total size.
///
/// * `num_dims` - An [IntValue] containing the number of dimensions.
/// * `dims` - A [PointerValue] to an array containing the size of each dimensions.
pub fn call_ndarray_calc_size<'ctx>(
    generator: &dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    num_dims: IntValue<'ctx>,
    dims: PointerValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i64 = ctx.ctx.i64_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pi64 = llvm_i64.ptr_type(AddressSpace::default());

    let ndarray_calc_size_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_size",
        64 => "__nac3_ndarray_calc_size64",
        bw => unreachable!("Unsupported size type bit width: {}", bw)
    };
    let ndarray_calc_size_fn_t = llvm_usize.fn_type(
        &[
            llvm_pi64.into(),
            llvm_usize.into(),
        ],
        false,
    );
    let ndarray_calc_size_fn = ctx.module.get_function(ndarray_calc_size_fn_name)
        .unwrap_or_else(|| {
            ctx.module.add_function(ndarray_calc_size_fn_name, ndarray_calc_size_fn_t, None)
        });

    ctx.builder
        .build_call(
            ndarray_calc_size_fn,
            &[
                dims.into(),
                num_dims.into(),
            ],
            "",
        )
        .try_as_basic_value()
        .unwrap_left()
        .into_int_value()
}

/// Generates a call to `__nac3_ndarray_init_dims`.
///
/// * `ndarray` - LLVM pointer to the NDArray. This value must be the LLVM representation of an
/// `NDArray`.
/// * `shape` - LLVM pointer to the `shape` of the NDArray. This value must be the LLVM
/// representation of a `list`.
pub fn call_ndarray_init_dims<'ctx>(
    generator: &dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ndarray: PointerValue<'ctx>,
    shape: PointerValue<'ctx>,
) {
    assert_is_ndarray(ndarray);
    assert_is_list(shape);

    let llvm_void = ctx.ctx.void_type();
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pi32 = llvm_i32.ptr_type(AddressSpace::default());
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_init_dims_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_init_dims",
        64 => "__nac3_ndarray_init_dims64",
        bw => unreachable!("Unsupported size type bit width: {}", bw)
    };
    let ndarray_init_dims_fn = ctx.module.get_function(ndarray_init_dims_fn_name).unwrap_or_else(|| {
        let fn_type = llvm_void.fn_type(
            &[
                llvm_pusize.into(),
                llvm_pi32.into(),
                llvm_usize.into(),
            ],
            false,
        );

        ctx.module.add_function(ndarray_init_dims_fn_name, fn_type, None)
    });

    let ndarray_dims = ctx.build_gep_and_load(
        ndarray,
        &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
        None,
    );
    let shape_data = ctx.build_gep_and_load(
        shape,
        &[llvm_i32.const_zero(), llvm_i32.const_zero()],
        None
    );
    let ndarray_num_dims = ctx.build_gep_and_load(
        ndarray,
        &[llvm_i32.const_zero(), llvm_i32.const_zero()],
        None,
    ).into_int_value();

    ctx.builder.build_call(
        ndarray_init_dims_fn,
        &[
            ndarray_dims.into(),
            shape_data.into(),
            ndarray_num_dims.into(),
        ],
        "",
    );
}

/// Generates a call to `__nac3_ndarray_calc_nd_indices`.
///
/// * `index` - The index to compute the multidimensional index for.
/// * `ndarray` - LLVM pointer to the NDArray. This value must be the LLVM representation of an
/// `NDArray`.
pub fn call_ndarray_calc_nd_indices<'ctx>(
    generator: &dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    index: IntValue<'ctx>,
    ndarray: PointerValue<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    assert_is_ndarray(ndarray);

    let llvm_void = ctx.ctx.void_type();
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_nd_indices_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_nd_indices",
        64 => "__nac3_ndarray_calc_nd_indices64",
        bw => unreachable!("Unsupported size type bit width: {}", bw)
    };
    let ndarray_calc_nd_indices_fn = ctx.module.get_function(ndarray_calc_nd_indices_fn_name).unwrap_or_else(|| {
        let fn_type = llvm_void.fn_type(
            &[
                llvm_usize.into(),
                llvm_pusize.into(),
                llvm_usize.into(),
                llvm_pusize.into(),
            ],
            false,
        );

        ctx.module.add_function(ndarray_calc_nd_indices_fn_name, fn_type, None)
    });

    let ndarray_num_dims = ctx.build_gep_and_load(
        ndarray,
        &[llvm_i32.const_zero(), llvm_i32.const_zero()],
        None,
    ).into_int_value();
    let ndarray_dims = ctx.build_gep_and_load(
        ndarray,
        &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
        None,
    ).into_pointer_value();

    let indices = ctx.builder.build_array_alloca(
        llvm_usize,
        ndarray_num_dims,
        "",
    );

    ctx.builder.build_call(
        ndarray_calc_nd_indices_fn,
        &[
            index.into(),
            ndarray_dims.into(),
            ndarray_num_dims.into(),
            indices.into(),
        ],
        "",
    );

    Ok(indices)
}