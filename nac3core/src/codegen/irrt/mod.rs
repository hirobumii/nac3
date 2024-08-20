use crate::typecheck::typedef::Type;

use super::{
    classes::{
        ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue, ListValue, NDArrayValue,
        TypedArrayLikeAdapter, UntypedArrayLikeAccessor,
    },
    llvm_intrinsics, CodeGenContext, CodeGenerator,
};
use crate::codegen::classes::TypedArrayLikeAccessor;
use crate::codegen::stmt::gen_for_callback_incrementing;
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    context::Context,
    memory_buffer::MemoryBuffer,
    module::Module,
    types::{BasicTypeEnum, IntType},
    values::{BasicValueEnum, CallSiteValue, FloatValue, IntValue},
    AddressSpace, IntPredicate,
};
use itertools::Either;
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
pub fn integer_power<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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
    let ge_zero = ctx
        .builder
        .build_int_compare(
            IntPredicate::SGE,
            exp,
            exp.get_type().const_zero(),
            "assert_int_pow_ge_0",
        )
        .unwrap();
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
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

pub fn calculate_len_for_slice_range<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
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
    let not_zero = ctx
        .builder
        .build_int_compare(IntPredicate::NE, step, step.get_type().const_zero(), "range_step_ne")
        .unwrap();
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
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
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
    length: IntValue<'ctx>,
) -> Result<Option<(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>)>, String> {
    let int32 = ctx.ctx.i32_type();
    let zero = int32.const_zero();
    let one = int32.const_int(1, false);
    let length = ctx.builder.build_int_truncate_or_bit_cast(length, int32, "leni32").unwrap();
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
                ctx.builder.build_int_sub(e, one, "final_end").unwrap()
            },
            one,
        ),
        (s, e, Some(step)) => {
            let step = if let Some(v) = generator.gen_expr(ctx, step)? {
                v.to_basic_value_enum(ctx, generator, ctx.primitives.int32)?.into_int_value()
            } else {
                return Ok(None);
            };
            // assert step != 0, throw exception if not
            let not_zero = ctx
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    step,
                    step.get_type().const_zero(),
                    "range_step_ne",
                )
                .unwrap();
            ctx.make_assert(
                generator,
                not_zero,
                "0:ValueError",
                "slice step cannot be zero",
                [None, None, None],
                ctx.current_loc,
            );
            let len_id = ctx.builder.build_int_sub(length, one, "lenmin1").unwrap();
            let neg = ctx
                .builder
                .build_int_compare(IntPredicate::SLT, step, zero, "step_is_neg")
                .unwrap();
            (
                match s {
                    Some(s) => {
                        let Some(s) = handle_slice_index_bound(s, ctx, generator, length)? else {
                            return Ok(None);
                        };
                        ctx.builder
                            .build_select(
                                ctx.builder
                                    .build_and(
                                        ctx.builder
                                            .build_int_compare(
                                                IntPredicate::EQ,
                                                s,
                                                length,
                                                "s_eq_len",
                                            )
                                            .unwrap(),
                                        neg,
                                        "should_minus_one",
                                    )
                                    .unwrap(),
                                ctx.builder.build_int_sub(s, one, "s_min").unwrap(),
                                s,
                                "final_start",
                            )
                            .map(BasicValueEnum::into_int_value)
                            .unwrap()
                    }
                    None => ctx
                        .builder
                        .build_select(neg, len_id, zero, "stt")
                        .map(BasicValueEnum::into_int_value)
                        .unwrap(),
                },
                match e {
                    Some(e) => {
                        let Some(e) = handle_slice_index_bound(e, ctx, generator, length)? else {
                            return Ok(None);
                        };
                        ctx.builder
                            .build_select(
                                neg,
                                ctx.builder.build_int_add(e, one, "end_add_one").unwrap(),
                                ctx.builder.build_int_sub(e, one, "end_sub_one").unwrap(),
                                "final_end",
                            )
                            .map(BasicValueEnum::into_int_value)
                            .unwrap()
                    }
                    None => ctx
                        .builder
                        .build_select(neg, zero, len_id, "end")
                        .map(BasicValueEnum::into_int_value)
                        .unwrap(),
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
        return Ok(None);
    };
    Ok(Some(
        ctx.builder
            .build_call(func, &[i.into(), length.into()], "bounded_ind")
            .map(CallSiteValue::try_as_basic_value)
            .map(|v| v.map_left(BasicValueEnum::into_int_value))
            .map(Either::unwrap_left)
            .unwrap(),
    ))
}

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
    let dest_arr_ptr = dest_arr.data().base_ptr(ctx, generator);
    let dest_arr_ptr =
        ctx.builder.build_pointer_cast(dest_arr_ptr, elem_ptr_type, "dest_arr_ptr_cast").unwrap();
    let dest_len = dest_arr.load_size(ctx, Some("dest.len"));
    let dest_len = ctx.builder.build_int_truncate_or_bit_cast(dest_len, int32, "srclen32").unwrap();
    let src_arr_ptr = src_arr.data().base_ptr(ctx, generator);
    let src_arr_ptr =
        ctx.builder.build_pointer_cast(src_arr_ptr, elem_ptr_type, "src_arr_ptr_cast").unwrap();
    let src_len = src_arr.load_size(ctx, Some("src.len"));
    let src_len = ctx.builder.build_int_truncate_or_bit_cast(src_len, int32, "srclen32").unwrap();

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
                    _ => unreachable!(),
                };
                ctx.builder.build_int_truncate_or_bit_cast(s, int32, "size").unwrap()
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
    let new_len = ctx.builder.build_int_z_extend_or_bit_cast(new_len, size_ty, "new_len").unwrap();
    dest_arr.store_size(ctx, generator, new_len);
    ctx.builder.build_unconditional_branch(cont_bb).unwrap();
    ctx.builder.position_at_end(cont_bb);
}

/// Generates a call to `isinf` in IR. Returns an `i1` representing the result.
pub fn call_isinf<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> IntValue<'ctx> {
    let intrinsic_fn = ctx.module.get_function("__nac3_isinf").unwrap_or_else(|| {
        let fn_type = ctx.ctx.i32_type().fn_type(&[ctx.ctx.f64_type().into()], false);
        ctx.module.add_function("__nac3_isinf", fn_type, None)
    });

    let ret = ctx
        .builder
        .build_call(intrinsic_fn, &[v.into()], "isinf")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();

    generator.bool_to_i1(ctx, ret)
}

/// Generates a call to `isnan` in IR. Returns an `i1` representing the result.
pub fn call_isnan<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &CodeGenContext<'ctx, '_>,
    v: FloatValue<'ctx>,
) -> IntValue<'ctx> {
    let intrinsic_fn = ctx.module.get_function("__nac3_isnan").unwrap_or_else(|| {
        let fn_type = ctx.ctx.i32_type().fn_type(&[ctx.ctx.f64_type().into()], false);
        ctx.module.add_function("__nac3_isnan", fn_type, None)
    });

    let ret = ctx
        .builder
        .build_call(intrinsic_fn, &[v.into()], "isnan")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();

    generator.bool_to_i1(ctx, ret)
}

/// Generates a call to `gamma` in IR. Returns an `f64` representing the result.
pub fn call_gamma<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let intrinsic_fn = ctx.module.get_function("__nac3_gamma").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_gamma", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "gamma")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `gammaln` in IR. Returns an `f64` representing the result.
pub fn call_gammaln<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let intrinsic_fn = ctx.module.get_function("__nac3_gammaln").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_gammaln", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "gammaln")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `j0` in IR. Returns an `f64` representing the result.
pub fn call_j0<'ctx>(ctx: &CodeGenContext<'ctx, '_>, v: FloatValue<'ctx>) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();

    let intrinsic_fn = ctx.module.get_function("__nac3_j0").unwrap_or_else(|| {
        let fn_type = llvm_f64.fn_type(&[llvm_f64.into()], false);
        ctx.module.add_function("__nac3_j0", fn_type, None)
    });

    ctx.builder
        .build_call(intrinsic_fn, &[v.into()], "j0")
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_float_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `__nac3_ndarray_calc_size`. Returns an [`IntValue`] representing the
/// calculated total size.
///
/// * `dims` - An [`ArrayLikeIndexer`] containing the size of each dimension.
/// * `range` - The dimension index to begin and end (exclusively) calculating the dimensions for,
///   or [`None`] if starting from the first dimension and ending at the last dimension
///   respectively.
pub fn call_ndarray_calc_size<'ctx, G, Dims>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    dims: &Dims,
    (begin, end): (Option<IntValue<'ctx>>, Option<IntValue<'ctx>>),
) -> IntValue<'ctx>
where
    G: CodeGenerator + ?Sized,
    Dims: ArrayLikeIndexer<'ctx>,
{
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_size_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_size",
        64 => "__nac3_ndarray_calc_size64",
        bw => unreachable!("Unsupported size type bit width: {}", bw),
    };
    let ndarray_calc_size_fn_t = llvm_usize.fn_type(
        &[llvm_pusize.into(), llvm_usize.into(), llvm_usize.into(), llvm_usize.into()],
        false,
    );
    let ndarray_calc_size_fn =
        ctx.module.get_function(ndarray_calc_size_fn_name).unwrap_or_else(|| {
            ctx.module.add_function(ndarray_calc_size_fn_name, ndarray_calc_size_fn_t, None)
        });

    let begin = begin.unwrap_or_else(|| llvm_usize.const_zero());
    let end = end.unwrap_or_else(|| dims.size(ctx, generator));
    ctx.builder
        .build_call(
            ndarray_calc_size_fn,
            &[
                dims.base_ptr(ctx, generator).into(),
                dims.size(ctx, generator).into(),
                begin.into(),
                end.into(),
            ],
            "",
        )
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap()
}

/// Generates a call to `__nac3_ndarray_calc_nd_indices`. Returns a [`TypeArrayLikeAdpater`]
/// containing `i32` indices of the flattened index.
///
/// * `index` - The index to compute the multidimensional index for.
/// * `ndarray` - LLVM pointer to the `NDArray`. This value must be the LLVM representation of an
///   `NDArray`.
pub fn call_ndarray_calc_nd_indices<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    index: IntValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
) -> TypedArrayLikeAdapter<'ctx, IntValue<'ctx>> {
    let llvm_void = ctx.ctx.void_type();
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pi32 = llvm_i32.ptr_type(AddressSpace::default());
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_nd_indices_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_nd_indices",
        64 => "__nac3_ndarray_calc_nd_indices64",
        bw => unreachable!("Unsupported size type bit width: {}", bw),
    };
    let ndarray_calc_nd_indices_fn =
        ctx.module.get_function(ndarray_calc_nd_indices_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_void.fn_type(
                &[llvm_usize.into(), llvm_pusize.into(), llvm_usize.into(), llvm_pi32.into()],
                false,
            );

            ctx.module.add_function(ndarray_calc_nd_indices_fn_name, fn_type, None)
        });

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    let ndarray_dims = ndarray.dim_sizes();

    let indices = ctx.builder.build_array_alloca(llvm_i32, ndarray_num_dims, "").unwrap();

    ctx.builder
        .build_call(
            ndarray_calc_nd_indices_fn,
            &[
                index.into(),
                ndarray_dims.base_ptr(ctx, generator).into(),
                ndarray_num_dims.into(),
                indices.into(),
            ],
            "",
        )
        .unwrap();

    TypedArrayLikeAdapter::from(
        ArraySliceValue::from_ptr_val(indices, ndarray_num_dims, None),
        Box::new(|_, v| v.into_int_value()),
        Box::new(|_, v| v.into()),
    )
}

fn call_ndarray_flatten_index_impl<'ctx, G, Indices>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    indices: &Indices,
) -> IntValue<'ctx>
where
    G: CodeGenerator + ?Sized,
    Indices: ArrayLikeIndexer<'ctx>,
{
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pi32 = llvm_i32.ptr_type(AddressSpace::default());
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    debug_assert_eq!(
        IntType::try_from(indices.element_type(ctx, generator))
            .map(IntType::get_bit_width)
            .unwrap_or_default(),
        llvm_i32.get_bit_width(),
        "Expected i32 value for argument `indices` to `call_ndarray_flatten_index_impl`"
    );
    debug_assert_eq!(
        indices.size(ctx, generator).get_type().get_bit_width(),
        llvm_usize.get_bit_width(),
        "Expected usize integer value for argument `indices_size` to `call_ndarray_flatten_index_impl`"
    );

    let ndarray_flatten_index_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_flatten_index",
        64 => "__nac3_ndarray_flatten_index64",
        bw => unreachable!("Unsupported size type bit width: {}", bw),
    };
    let ndarray_flatten_index_fn =
        ctx.module.get_function(ndarray_flatten_index_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_usize.fn_type(
                &[llvm_pusize.into(), llvm_usize.into(), llvm_pi32.into(), llvm_usize.into()],
                false,
            );

            ctx.module.add_function(ndarray_flatten_index_fn_name, fn_type, None)
        });

    let ndarray_num_dims = ndarray.load_ndims(ctx);
    let ndarray_dims = ndarray.dim_sizes();

    let index = ctx
        .builder
        .build_call(
            ndarray_flatten_index_fn,
            &[
                ndarray_dims.base_ptr(ctx, generator).into(),
                ndarray_num_dims.into(),
                indices.base_ptr(ctx, generator).into(),
                indices.size(ctx, generator).into(),
            ],
            "",
        )
        .map(CallSiteValue::try_as_basic_value)
        .map(|v| v.map_left(BasicValueEnum::into_int_value))
        .map(Either::unwrap_left)
        .unwrap();

    index
}

/// Generates a call to `__nac3_ndarray_flatten_index`. Returns the flattened index for the
/// multidimensional index.
///
/// * `ndarray` - LLVM pointer to the `NDArray`. This value must be the LLVM representation of an
///   `NDArray`.
/// * `indices` - The multidimensional index to compute the flattened index for.
pub fn call_ndarray_flatten_index<'ctx, G, Index>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    indices: &Index,
) -> IntValue<'ctx>
where
    G: CodeGenerator + ?Sized,
    Index: ArrayLikeIndexer<'ctx>,
{
    call_ndarray_flatten_index_impl(generator, ctx, ndarray, indices)
}

/// Generates a call to `__nac3_ndarray_calc_broadcast`. Returns a tuple containing the number of
/// dimension and size of each dimension of the resultant `ndarray`.
pub fn call_ndarray_calc_broadcast<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    lhs: NDArrayValue<'ctx>,
    rhs: NDArrayValue<'ctx>,
) -> TypedArrayLikeAdapter<'ctx, IntValue<'ctx>> {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_broadcast_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_broadcast",
        64 => "__nac3_ndarray_calc_broadcast64",
        bw => unreachable!("Unsupported size type bit width: {}", bw),
    };
    let ndarray_calc_broadcast_fn =
        ctx.module.get_function(ndarray_calc_broadcast_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_usize.fn_type(
                &[
                    llvm_pusize.into(),
                    llvm_usize.into(),
                    llvm_pusize.into(),
                    llvm_usize.into(),
                    llvm_pusize.into(),
                ],
                false,
            );

            ctx.module.add_function(ndarray_calc_broadcast_fn_name, fn_type, None)
        });

    let lhs_ndims = lhs.load_ndims(ctx);
    let rhs_ndims = rhs.load_ndims(ctx);
    let min_ndims = llvm_intrinsics::call_int_umin(ctx, lhs_ndims, rhs_ndims, None);

    gen_for_callback_incrementing(
        generator,
        ctx,
        None,
        llvm_usize.const_zero(),
        (min_ndims, false),
        |generator, ctx, _, idx| {
            let idx = ctx.builder.build_int_sub(min_ndims, idx, "").unwrap();
            let (lhs_dim_sz, rhs_dim_sz) = unsafe {
                (
                    lhs.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None),
                    rhs.dim_sizes().get_typed_unchecked(ctx, generator, &idx, None),
                )
            };

            let llvm_usize_const_one = llvm_usize.const_int(1, false);
            let lhs_eqz = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, lhs_dim_sz, llvm_usize_const_one, "")
                .unwrap();
            let rhs_eqz = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, rhs_dim_sz, llvm_usize_const_one, "")
                .unwrap();
            let lhs_or_rhs_eqz = ctx.builder.build_or(lhs_eqz, rhs_eqz, "").unwrap();

            let lhs_eq_rhs = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, lhs_dim_sz, rhs_dim_sz, "")
                .unwrap();

            let is_compatible = ctx.builder.build_or(lhs_or_rhs_eqz, lhs_eq_rhs, "").unwrap();

            ctx.make_assert(
                generator,
                is_compatible,
                "0:ValueError",
                "operands could not be broadcast together",
                [None, None, None],
                ctx.current_loc,
            );

            Ok(())
        },
        llvm_usize.const_int(1, false),
    )
    .unwrap();

    let max_ndims = llvm_intrinsics::call_int_umax(ctx, lhs_ndims, rhs_ndims, None);
    let lhs_dims = lhs.dim_sizes().base_ptr(ctx, generator);
    let lhs_ndims = lhs.load_ndims(ctx);
    let rhs_dims = rhs.dim_sizes().base_ptr(ctx, generator);
    let rhs_ndims = rhs.load_ndims(ctx);
    let out_dims = ctx.builder.build_array_alloca(llvm_usize, max_ndims, "").unwrap();
    let out_dims = ArraySliceValue::from_ptr_val(out_dims, max_ndims, None);

    ctx.builder
        .build_call(
            ndarray_calc_broadcast_fn,
            &[
                lhs_dims.into(),
                lhs_ndims.into(),
                rhs_dims.into(),
                rhs_ndims.into(),
                out_dims.base_ptr(ctx, generator).into(),
            ],
            "",
        )
        .unwrap();

    TypedArrayLikeAdapter::from(
        out_dims,
        Box::new(|_, v| v.into_int_value()),
        Box::new(|_, v| v.into()),
    )
}

/// Generates a call to `__nac3_ndarray_calc_broadcast_idx`. Returns an [`ArrayAllocaValue`]
/// containing the indices used for accessing `array` corresponding to the index of the broadcasted
/// array `broadcast_idx`.
pub fn call_ndarray_calc_broadcast_index<
    'ctx,
    G: CodeGenerator + ?Sized,
    BroadcastIdx: UntypedArrayLikeAccessor<'ctx>,
>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    array: NDArrayValue<'ctx>,
    broadcast_idx: &BroadcastIdx,
) -> TypedArrayLikeAdapter<'ctx, IntValue<'ctx>> {
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pi32 = llvm_i32.ptr_type(AddressSpace::default());
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let ndarray_calc_broadcast_fn_name = match llvm_usize.get_bit_width() {
        32 => "__nac3_ndarray_calc_broadcast_idx",
        64 => "__nac3_ndarray_calc_broadcast_idx64",
        bw => unreachable!("Unsupported size type bit width: {}", bw),
    };
    let ndarray_calc_broadcast_fn =
        ctx.module.get_function(ndarray_calc_broadcast_fn_name).unwrap_or_else(|| {
            let fn_type = llvm_usize.fn_type(
                &[llvm_pusize.into(), llvm_usize.into(), llvm_pi32.into(), llvm_pi32.into()],
                false,
            );

            ctx.module.add_function(ndarray_calc_broadcast_fn_name, fn_type, None)
        });

    let broadcast_size = broadcast_idx.size(ctx, generator);
    let out_idx = ctx.builder.build_array_alloca(llvm_i32, broadcast_size, "").unwrap();

    let array_dims = array.dim_sizes().base_ptr(ctx, generator);
    let array_ndims = array.load_ndims(ctx);
    let broadcast_idx_ptr = unsafe {
        broadcast_idx.ptr_offset_unchecked(ctx, generator, &llvm_usize.const_zero(), None)
    };

    ctx.builder
        .build_call(
            ndarray_calc_broadcast_fn,
            &[array_dims.into(), array_ndims.into(), broadcast_idx_ptr.into(), out_idx.into()],
            "",
        )
        .unwrap();

    TypedArrayLikeAdapter::from(
        ArraySliceValue::from_ptr_val(out_idx, broadcast_size, None),
        Box::new(|_, v| v.into_int_value()),
        Box::new(|_, v| v.into()),
    )
}
