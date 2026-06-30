use inkwell::values::IntValue;
use nac3parser::ast::Operator;

use crate::{
    codegen::{
        CodeGenContext, expr::{call_extern, gen_prim_binop_expr}, stmt::gen_for_callback_incrementing, types::{
            NDArrayValue, RefCountedArrayValue, array::ArrayLikeIndexer, ndarray::{NDArrayOut, assert_ndarray_can_be_written_by_out, indexing::RustNDIndex},
        },
    }, toplevel::helper::arraylike_flatten_element_type, typecheck::{magic_methods::Binop, typedef::Type},
};

/// Perform `np.einsum("...ij,...jk->...ik", in_a, in_b)`.
///
/// `dst_dtype` defines the dtype of the returned ndarray.
fn matmul_at_least_2d<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    dst_dtype: Type,
    (in_a_ty, in_a): (Type, NDArrayValue<'ctx>),
    (in_b_ty, in_b): (Type, NDArrayValue<'ctx>),
) -> anyhow::Result<NDArrayValue<'ctx>> {
    assert!(in_a.ty.object.ndims >= 2, "in_a (which is {}) must be >= 2", in_a.ty.object.ndims);
    assert!(in_b.ty.object.ndims >= 2, "in_b (which is {}) must be >= 2", in_b.ty.object.ndims);

    let lhs_dtype = arraylike_flatten_element_type(&mut ctx.unifier, in_a_ty);
    let rhs_dtype = arraylike_flatten_element_type(&mut ctx.unifier, in_b_ty);

    let llvm_dst_dtype = ctx.get_llvm_type(dst_dtype);

    // Deduce ndims of the result of matmul.
    let ndims_int = in_a.ty.object.ndims.max(in_b.ty.object.ndims);
    let ndims = ctx.size_t.const_int(ndims_int, false);

    // Broadcasts `in_a.shape[:-2]` and `in_b.shape[:-2]` together and allocate the
    // destination ndarray to store the result of matmul.
    let (lhs, rhs, dst) = {
        let in_lhs_shape = in_a.inner_value(ctx)?.shape(ctx)?;
        let in_rhs_shape = in_b.inner_value(ctx)?.shape(ctx)?;
        let [lhs_shape, rhs_shape, dst_shape] = core::array::from_fn(|_| {
            RefCountedArrayValue::new(ctx, ctx.size_t, ndims_int as u32, None)
        });
        let [lhs_shape, rhs_shape, dst_shape] = [lhs_shape?, rhs_shape?, dst_shape?];

        let a_ndims = ctx.size_t.const_int(in_a.ty.object.ndims, false);
        let b_ndims = ctx.size_t.const_int(in_b.ty.object.ndims, false);
        call_extern!(ctx: void _ = "__nac3_ndarray_matmul_calculate_shapes"(
            a_ndims, in_lhs_shape.value,
            b_ndims, in_rhs_shape.value,
            ndims,
            lhs_shape.value,
            rhs_shape.value,
            dst_shape.value,
        ))?;

        let lhs = in_a.broadcast_to(ctx, ndims_int, lhs_shape)?;
        let rhs = in_b.broadcast_to(ctx, ndims_int, rhs_shape)?;
        let dst = NDArrayOut::NewNDArray { dtype: llvm_dst_dtype }.resolve(
            ctx,
            ndims_int,
            dst_shape.inner_value(ctx, None)?,
        )?;

        (lhs, rhs, dst)
    };

    let len = lhs.shape(ctx)?.inner_value(ctx, None)?.get_unchecked(
        ctx,
        &ctx.size_t.const_int(ndims_int - 1, false),
        None,
    )?;

    let [at_row, at_col] = [ndims_int - 2, ndims_int - 1].map(|x| ctx.size_t.const_int(x, true));

    let dst_dtype_llvm = ctx.get_llvm_type(dst_dtype);
    let dst_zero = dst_dtype_llvm.const_zero();

    dst.foreach(ctx, |ctx, _, hdl| {
        let pdst_ij = hdl.inner_value(ctx)?.curr_ptr(ctx)?;

        ctx.builder.build_store(pdst_ij, dst_zero)?;

        let indices = hdl.inner_value(ctx)?.indices(ctx)?;
        let i = indices.get_unchecked::<IntValue<'ctx>>(ctx, &at_row, None)?;
        let j = indices.get_unchecked::<IntValue<'ctx>>(ctx, &at_col, None)?;

        let num_0 = ctx.size_t.const_int(0, false);
        let num_1 = ctx.size_t.const_int(1, false);

        gen_for_callback_incrementing(
            &mut (),
            ctx,
            None,
            num_0,
            (len, false),
            |(), ctx, _, k| {
                // `indices` is modified to index into `a` and `b`, and restored.
                indices.set_unchecked(ctx, &at_row, i, None)?;
                indices.set_unchecked(ctx, &at_col, k, None)?;
                let a_ik = lhs.get_unchecked(ctx, &indices, None)?;

                indices.set_unchecked(ctx, &at_row, k, None)?;
                indices.set_unchecked(ctx, &at_col, j, None)?;
                let b_kj = rhs.get_unchecked(ctx, &indices, None)?;

                // Restore `indices`.
                indices.set_unchecked(ctx, &at_row, i, None)?;
                indices.set_unchecked(ctx, &at_col, j, None)?;

                // x = a_[...]ik * b_[...]kj
                let x = gen_prim_binop_expr(
                    ctx,
                    (&Some(lhs_dtype), a_ik),
                    Binop::normal(Operator::Mult),
                    (&Some(rhs_dtype), b_kj),
                )?
                .expect("matmul: ndarray should contain primtives only");

                // dst_[...]ij += x
                let dst_ij = ctx.builder.build_load(dst_dtype_llvm, pdst_ij, "")?;
                let dst_ij = gen_prim_binop_expr(
                    ctx,
                    (&Some(dst_dtype), dst_ij),
                    Binop::normal(Operator::Add),
                    (&Some(dst_dtype), x),
                )?
                .expect("matmul: ndarray should contain primtives only");
                ctx.builder.build_store(pdst_ij, dst_ij)?;

                Ok(())
            },
            num_1,
            |(), _| Ok(()),
        )
    })?;

    Ok(dst)
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Perform [`np.matmul`](https://numpy.org/doc/stable/reference/generated/numpy.matmul.html).
    ///
    /// This function always return an [`NDArrayValue`]. You may want to use
    /// [`NDArrayValue::split_unsized`] to handle when the output could be a scalar.
    ///
    /// `dst_dtype` defines the dtype of the returned ndarray.
    pub fn matmul(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        self_ty: Type,
        (other_ty, other): (Type, Self),
        (out_dtype, out): (Type, NDArrayOut<'ctx>),
    ) -> anyhow::Result<Self> {
        // Sanity check, but type inference should prevent this.
        assert!(
            self.ty.object.ndims > 0 && other.ty.object.ndims > 0,
            "np.matmul disallows scalar input"
        );

        // If both arguments are 2-D they are multiplied like conventional matrices.
        //
        // If either argument is N-D, N > 2, it is treated as a stack of matrices residing in the
        // last two indices and broadcast accordingly.
        //
        // If the first argument is 1-D, it is promoted to a matrix by prepending a 1 to its
        // dimensions. After matrix multiplication the prepended 1 is removed.
        //
        // If the second argument is 1-D, it is promoted to a matrix by appending a 1 to its
        // dimensions. After matrix multiplication the appended 1 is removed.

        let new_a = if self.ty.object.ndims == 1 {
            // Prepend 1 to its dimensions
            self.index(ctx, &[RustNDIndex::NewAxis, RustNDIndex::Ellipsis])
        } else {
            Ok(*self)
        }?;

        let new_b = if other.ty.object.ndims == 1 {
            // Append 1 to its dimensions
            other.index(ctx, &[RustNDIndex::Ellipsis, RustNDIndex::NewAxis])
        } else {
            Ok(other)
        }?;

        // NOTE: `result` will always be a newly allocated ndarray.
        // Current implementation cannot do in-place matrix muliplication.
        let mut result = matmul_at_least_2d(ctx, out_dtype, (self_ty, new_a), (other_ty, new_b))?;

        // Postprocessing on the result to remove prepended/appended axes.
        let mut postindices = vec![];
        let zero = ctx.i32.const_zero();

        if self.ty.object.ndims == 1 {
            // Remove the prepended 1
            postindices.push(RustNDIndex::SingleElement(zero));
        }

        if other.ty.object.ndims == 1 {
            // Remove the appended 1
            postindices.push(RustNDIndex::Ellipsis);
            postindices.push(RustNDIndex::SingleElement(zero));
        }

        if !postindices.is_empty() {
            result = result.index(ctx, &postindices)?;
        }

        Ok(match out {
            NDArrayOut::NewNDArray { .. } => result,
            NDArrayOut::WriteToNDArray { ndarray: out_ndarray } => {
                let result_shape = result.shape(ctx)?;
                let out_shape = out_ndarray.shape(ctx)?;
                assert_ndarray_can_be_written_by_out(
                    ctx,
                    result_shape.inner_value(ctx, None)?,
                    out_shape.inner_value(ctx, None)?,
                )?;

                out_ndarray.copy_data_from(ctx, &result)?;
                out_ndarray
            }
        })
    }
}
