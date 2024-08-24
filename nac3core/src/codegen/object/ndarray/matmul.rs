use std::cmp::max;

use nac3parser::ast::Operator;
use util::gen_for_model;

use crate::{
    codegen::{
        expr::gen_binop_expr_with_values, irrt::call_nac3_ndarray_matmul_calculate_shapes,
        model::*, object::ndarray::indexing::RustNDIndex, CodeGenContext, CodeGenerator,
    },
    typecheck::{magic_methods::Binop, typedef::Type},
};

use super::{NDArrayObject, NDArrayOut};

/// Perform `np.einsum("...ij,...jk->...ik", in_a, in_b)`.
///
/// `dst_dtype` defines the dtype of the returned ndarray.
fn matmul_at_least_2d<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    dst_dtype: Type,
    in_a: NDArrayObject<'ctx>,
    in_b: NDArrayObject<'ctx>,
) -> NDArrayObject<'ctx> {
    assert!(in_a.ndims >= 2);
    assert!(in_b.ndims >= 2);

    // Deduce ndims of the result of matmul.
    let ndims_int = max(in_a.ndims, in_b.ndims);
    let ndims = Int(SizeT).const_int(generator, ctx.ctx, ndims_int);

    let num_0 = Int(SizeT).const_int(generator, ctx.ctx, 0);
    let num_1 = Int(SizeT).const_int(generator, ctx.ctx, 1);

    // Broadcasts `in_a.shape[:-2]` and `in_b.shape[:-2]` together and allocate the
    // destination ndarray to store the result of matmul.
    let (lhs, rhs, dst) = {
        let in_lhs_ndims = in_a.ndims_llvm(generator, ctx.ctx);
        let in_lhs_shape = in_a.instance.get(generator, ctx, |f| f.shape);
        let in_rhs_ndims = in_b.ndims_llvm(generator, ctx.ctx);
        let in_rhs_shape = in_b.instance.get(generator, ctx, |f| f.shape);
        let lhs_shape = Int(SizeT).array_alloca(generator, ctx, ndims.value);
        let rhs_shape = Int(SizeT).array_alloca(generator, ctx, ndims.value);
        let dst_shape = Int(SizeT).array_alloca(generator, ctx, ndims.value);

        // Matmul dimension compatibility is checked here.
        call_nac3_ndarray_matmul_calculate_shapes(
            generator,
            ctx,
            in_lhs_ndims,
            in_lhs_shape,
            in_rhs_ndims,
            in_rhs_shape,
            ndims,
            lhs_shape,
            rhs_shape,
            dst_shape,
        );

        let lhs = in_a.broadcast_to(generator, ctx, ndims_int, lhs_shape);
        let rhs = in_b.broadcast_to(generator, ctx, ndims_int, rhs_shape);

        let dst = NDArrayObject::alloca(generator, ctx, dst_dtype, ndims_int);
        dst.copy_shape_from_array(generator, ctx, dst_shape);
        dst.create_data(generator, ctx);

        (lhs, rhs, dst)
    };

    let len = lhs.instance.get(generator, ctx, |f| f.shape).get_index_const(
        generator,
        ctx,
        ndims_int - 1,
    );

    let at_row = ndims_int - 2;
    let at_col = ndims_int - 1;

    let dst_dtype_llvm = ctx.get_llvm_type(generator, dst_dtype);
    let dst_zero = dst_dtype_llvm.const_zero();

    dst.foreach(generator, ctx, |generator, ctx, _, hdl| {
        let pdst_ij = hdl.get_pointer(generator, ctx);

        ctx.builder.build_store(pdst_ij, dst_zero).unwrap();

        let indices = hdl.get_indices();
        let i = indices.get_index_const(generator, ctx, at_row);
        let j = indices.get_index_const(generator, ctx, at_col);

        gen_for_model(generator, ctx, num_0, len, num_1, |generator, ctx, _, k| {
            // `indices` is modified to index into `a` and `b`, and restored.
            indices.set_index_const(ctx, at_row, i);
            indices.set_index_const(ctx, at_col, k);
            let a_ik = lhs.get_scalar_by_indices(generator, ctx, indices);

            indices.set_index_const(ctx, at_row, k);
            indices.set_index_const(ctx, at_col, j);
            let b_kj = rhs.get_scalar_by_indices(generator, ctx, indices);

            // Restore `indices`.
            indices.set_index_const(ctx, at_row, i);
            indices.set_index_const(ctx, at_col, j);

            // x = a_[...]ik * b_[...]kj
            let x = gen_binop_expr_with_values(
                generator,
                ctx,
                (&Some(lhs.dtype), a_ik.value),
                Binop::normal(Operator::Mult),
                (&Some(rhs.dtype), b_kj.value),
                ctx.current_loc,
            )?
            .unwrap()
            .to_basic_value_enum(ctx, generator, dst_dtype)?;

            // dst_[...]ij += x
            let dst_ij = ctx.builder.build_load(pdst_ij, "").unwrap();
            let dst_ij = gen_binop_expr_with_values(
                generator,
                ctx,
                (&Some(dst_dtype), dst_ij),
                Binop::normal(Operator::Add),
                (&Some(dst_dtype), x),
                ctx.current_loc,
            )?
            .unwrap()
            .to_basic_value_enum(ctx, generator, dst_dtype)?;
            ctx.builder.build_store(pdst_ij, dst_ij).unwrap();

            Ok(())
        })
    })
    .unwrap();

    dst
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Perform `np.matmul` according to the rules in
    /// <https://numpy.org/doc/stable/reference/generated/numpy.matmul.html>.
    ///
    /// This function always return an [`NDArrayObject`]. You may want to use [`NDArrayObject::split_unsized`]
    /// to handle when the output could be a scalar.
    ///
    /// `dst_dtype` defines the dtype of the returned ndarray.
    pub fn matmul<G: CodeGenerator>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        a: Self,
        b: Self,
        out: NDArrayOut<'ctx>,
    ) -> Self {
        // Sanity check, but type inference should prevent this.
        assert!(a.ndims > 0 && b.ndims > 0, "np.matmul disallows scalar input");

        /*
            If both arguments are 2-D they are multiplied like conventional matrices.
            If either argument is N-D, N > 2, it is treated as a stack of matrices residing in the last two indices and broadcast accordingly.
            If the first argument is 1-D, it is promoted to a matrix by prepending a 1 to its dimensions. After matrix multiplication the prepended 1 is removed.
            If the second argument is 1-D, it is promoted to a matrix by appending a 1 to its dimensions. After matrix multiplication the appended 1 is removed.
        */

        let new_a = if a.ndims == 1 {
            // Prepend 1 to its dimensions
            a.index(generator, ctx, &[RustNDIndex::NewAxis, RustNDIndex::Ellipsis])
        } else {
            a
        };

        let new_b = if b.ndims == 1 {
            // Append 1 to its dimensions
            b.index(generator, ctx, &[RustNDIndex::Ellipsis, RustNDIndex::NewAxis])
        } else {
            b
        };

        // NOTE: `result` will always be a newly allocated ndarray.
        // Current implementation cannot do in-place matrix muliplication.
        let mut result = matmul_at_least_2d(generator, ctx, out.get_dtype(), new_a, new_b);

        // Postprocessing on the result to remove prepended/appended axes.
        let mut postindices = vec![];
        let zero = Int(Int32).const_0(generator, ctx.ctx);

        if a.ndims == 1 {
            // Remove the prepended 1
            postindices.push(RustNDIndex::SingleElement(zero));
        }

        if b.ndims == 1 {
            // Remove the appended 1
            postindices.push(RustNDIndex::Ellipsis);
            postindices.push(RustNDIndex::SingleElement(zero));
        }

        if !postindices.is_empty() {
            result = result.index(generator, ctx, &postindices);
        }

        match out {
            NDArrayOut::NewNDArray { .. } => result,
            NDArrayOut::WriteToNDArray { ndarray: out_ndarray } => {
                let result_shape = result.instance.get(generator, ctx, |f| f.shape);
                out_ndarray.assert_can_be_written_by_out(
                    generator,
                    ctx,
                    result.ndims,
                    result_shape,
                );

                out_ndarray.copy_data_from(generator, ctx, result);
                out_ndarray
            }
        }
    }
}
