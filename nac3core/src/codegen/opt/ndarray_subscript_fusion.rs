use inkwell::{
    IntPredicate,
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3parser::ast::{Expr, ExprKind};

use crate::{
    codegen::{
        CodeGenContext, CodeGenerator,
        allocator::AllocationScope,
        bool_to_i8,
        expr::call_extern,
        types::{ArrayLikeIndexer, NDArrayType, ProxyTypeBase},
    },
    symbol_resolver::ValueEnum,
    toplevel::{
        helper::{PrimDef, extract_ndims},
        numpy::unpack_ndarray_var_tys,
    },
    typecheck::typedef::{Type, Unifier},
};

/// A list of flattened index expressions paired with the axis they appear in within their own
/// bracket, in source order (innermost bracket first).
type ChainIndices<'a> = Vec<(&'a Expr<Option<Type>>, u64)>;

/// A fused chain of integer subscripts on an ndarray that reduces to a scalar element access.
pub struct FusableChain<'a> {
    /// The base ndarray expression the chain indexes into.
    base: &'a Expr<Option<Type>>,
    /// The flattened `(index expr, display axis)` list, in source order.
    indices: ChainIndices<'a>,
}

/// Checks whether `expr` has a fusable chain of integer subscripts on an ndarray.
///
/// Returns a `Some(FusableChain)` if the expression can be fused into a single scalar access, which
/// can be passed into [`gen_fused_scalar_getitem`] or [`gen_fused_scalar_setitem`].
pub fn try_fuse_scalar_chain<'a>(
    ctx: &mut CodeGenContext<'_, '_>,
    expr: &'a Expr<Option<Type>>,
) -> Option<FusableChain<'a>> {
    if ctx.registry.codegen_options.opt_level == "0" {
        return None;
    }
    collect_scalar_ndarray_chain(ctx, expr)
}

/// Lowers a fused scala read of `chain`, returning the loaded element with type `result_ty`.
pub fn gen_fused_scalar_getitem<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    chain: &FusableChain<'_>,
    result_ty: Type,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    let elem_ptr = gen_chain_element_ptr(generator, ctx, chain.base, &chain.indices)?;
    let dtype = ctx.get_llvm_type(result_ty);
    Ok(ctx.builder.build_load(dtype, elem_ptr, "ndarray_elem")?)
}

/// Lowers a fused scala write of `value` with type `value_ty` into `chain`.
pub fn gen_fused_scalar_setitem<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    chain: &FusableChain<'_>,
    value: &ValueEnum<'ctx>,
    value_ty: Type,
) -> anyhow::Result<()> {
    let elem_ptr = gen_chain_element_ptr(generator, ctx, chain.base, &chain.indices)?;
    let value = value.to_basic_value_enum(ctx, value_ty)?;
    let value = if ctx.unifier.unioned(value_ty, ctx.primitives.bool) {
        bool_to_i8(ctx, value.into_int_value())?.into()
    } else {
        value
    };
    ctx.builder.build_store(elem_ptr, value)?;
    Ok(())
}

/// If `expr` is a chain of two or more integer subscripts that indexes an ndarray entirely down to
/// a scalar (`a[i][j]...`), returns the base ndarray expression and the flat list of
/// `(index expr, display axis)` in source order. The display axis is the index's position within
/// its own `[]` bracket, matching the axis the unfused chain reports on an out-of-bounds access.
///
/// Tries to collect a chain of integer subscripts on an `ndarray` that indexes down to a scalar.
///
/// If such a chain is found, returns the [`FusableChain`] containing the base expression and the
/// flattened list of index.
fn collect_scalar_ndarray_chain<'a>(
    ctx: &mut CodeGenContext<'_, '_>,
    expr: &'a Expr<Option<Type>>,
) -> Option<FusableChain<'a>> {
    let is_ndarray = |ty: Option<Type>, unifier: &Unifier| {
        ty.and_then(|ty| ty.obj_id(unifier)) == Some(PrimDef::NDArray.id())
    };

    // Descend through the chain of ndarray subscripts, collecting each bracket's slice expression
    // (outermost bracket first).
    let mut bracket_slices: Vec<&'a Expr<Option<Type>>> = Vec::new();
    let mut cur = expr;
    while let ExprKind::Subscript { value, slice, .. } = &cur.node {
        if !is_ndarray(value.custom, &ctx.unifier) {
            break;
        }
        bracket_slices.push(slice.as_ref());
        cur = value.as_ref();
    }
    let base = cur;

    // Fusing chains with only a singular index is a no-op, and index chains on non-ndarrays cannot
    // be fused
    if bracket_slices.len() < 2 || !is_ndarray(base.custom, &ctx.unifier) {
        return None;
    }

    // Flatten the brackets in source order (innermost bracket first).
    let mut indices: ChainIndices<'a> = Vec::new();
    for slice in bracket_slices.iter().rev() {
        let bracket: Vec<&'a Expr<Option<Type>>> = match &slice.node {
            ExprKind::Tuple { elts, .. } => elts.iter().collect(),
            _ => vec![*slice],
        };
        for (display_axis, index) in bracket.into_iter().enumerate() {
            indices.push((index, display_axis as u64));
        }
    }

    // Chains that contain non-integer indices cannot be fully fused - Ignore for now
    let int32 = ctx.primitives.int32;
    for (index, _) in &indices {
        if matches!(index.node, ExprKind::Slice { .. })
            || !index.custom.is_some_and(|ty| ctx.unifier.unioned(ty, int32))
        {
            return None;
        }
    }

    // Chains that do not index the base ndarray to a scalar cannot be fused
    let (_, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, base.custom.unwrap());
    if indices.len() as u64 != extract_ndims(&ctx.unifier, ndims) {
        return None;
    }

    Some(FusableChain { base, indices })
}

/// Generates a pointer to the scalar element selected by a fusable chain, equivalent to
/// `base[indices...]`.
///
/// This function performs the same operation as `__nac3_ndarray_get_pelement_by_indices`, but
/// reconstructs the correct error message for out-of-bounds access as-if the chain is unfused.
fn gen_chain_element_ptr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    base: &Expr<Option<Type>>,
    indices: &[(&Expr<Option<Type>>, u64)],
) -> anyhow::Result<PointerValue<'ctx>> {
    let base_ty = base.custom.unwrap();
    let base_val = generator.gen_expr(ctx, base)?.to_basic_value_enum(ctx)?.into_pointer_value();
    let base = NDArrayType::from_unifier_type(ctx, base_ty).map_value(base_val, None);

    let size_t = ctx.size_t;
    let indices_ty = size_t.array_type(u32::try_from(indices.len()).unwrap());
    let indices_ptr =
        ctx.build_allocate(AllocationScope::StackStartOfFunc, indices_ty, Some("indices"))?;
    let shape = base.inner_value(ctx)?.shape(ctx)?.inner_value(ctx, None)?;

    for (real_axis, (index_ast, display_axis)) in indices.iter().enumerate() {
        let index = generator.gen_expr(ctx, index_ast)?.to_basic_value_enum(ctx)?.into_int_value();
        let index = ctx.builder.build_int_s_extend_or_bit_cast(index, size_t, "")?;

        let axis_idx = size_t.const_int(real_axis as u64, false);
        let dim = shape.get_unchecked::<IntValue<'ctx>>(ctx, &axis_idx, None)?;

        let is_negative =
            ctx.builder.build_int_compare(IntPredicate::SLT, index, size_t.const_zero(), "")?;
        let added = ctx.builder.build_int_add(index, dim, "")?;
        let resolved = ctx.builder.build_select(is_negative, added, index, "")?.into_int_value();
        let in_bounds = ctx.builder.build_int_compare(IntPredicate::ULT, resolved, dim, "")?;
        ctx.make_assert(
            in_bounds,
            "0:IndexError",
            "index {0} is out of bounds for axis {1} with size {2}",
            [Some(index), Some(size_t.const_int(*display_axis, false)), Some(dim)],
        )?;

        let slot = unsafe {
            ctx.builder.build_in_bounds_gep(
                indices_ty,
                indices_ptr,
                &[ctx.i32.const_zero(), ctx.i32.const_int(real_axis as u64, false)],
                "",
            )?
        };
        ctx.builder.build_store(slot, resolved)?;
    }

    call_extern!(ctx: (ctx.ptr) "pelement" = "__nac3_ndarray_get_pelement_by_indices"(base.value, indices_ptr))
}
