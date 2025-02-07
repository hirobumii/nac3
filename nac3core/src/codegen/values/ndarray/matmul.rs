use std::cmp::max;

use nac3parser::ast::Operator;

use super::{NDArrayOut, NDArrayValue, RustNDIndex};
use crate::{
    codegen::{
        expr::gen_binop_expr_with_values,
        irrt,
        stmt::gen_for_callback_incrementing,
        types::ndarray::NDArrayType,
        values::{
            ArrayLikeValue, ArraySliceValue, TypedArrayLikeAccessor, TypedArrayLikeAdapter,
            UntypedArrayLikeAccessor, UntypedArrayLikeMutator,
        },
        CodeGenContext, CodeGenerator,
    },
    toplevel::helper::arraylike_flatten_element_type,
    typecheck::{magic_methods::Binop, typedef::Type},
};

/// Perform `np.einsum("...ij,...jk->...ik", in_a, in_b)`.
///
/// `dst_dtype` defines the dtype of the returned ndarray.
fn matmul_at_least_2d<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    dst_dtype: Type,
    (in_a_ty, in_a): (Type, NDArrayValue<'ctx>),
    (in_b_ty, in_b): (Type, NDArrayValue<'ctx>),
) -> NDArrayValue<'ctx> {
    assert!(in_a.ndims >= 2, "in_a (which is {}) must be >= 2", in_a.ndims);
    assert!(in_b.ndims >= 2, "in_b (which is {}) must be >= 2", in_b.ndims);

    let lhs_dtype = arraylike_flatten_element_type(&mut ctx.unifier, in_a_ty);
    let rhs_dtype = arraylike_flatten_element_type(&mut ctx.unifier, in_b_ty);

    let llvm_usize = ctx.get_size_type();
    let llvm_dst_dtype = ctx.get_llvm_type(generator, dst_dtype);

    // Deduce ndims of the result of matmul.
    let ndims_int = max(in_a.ndims, in_b.ndims);
    let ndims = llvm_usize.const_int(ndims_int, false);

    // Broadcasts `in_a.shape[:-2]` and `in_b.shape[:-2]` together and allocate the
    // destination ndarray to store the result of matmul.
    let (lhs, rhs, dst) = {
        let in_lhs_ndims = llvm_usize.const_int(in_a.ndims, false);
        let in_lhs_shape = TypedArrayLikeAdapter::from(
            ArraySliceValue::from_ptr_val(
                in_a.shape().base_ptr(ctx, generator),
                in_lhs_ndims,
                None,
            ),
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );
        let in_rhs_ndims = llvm_usize.const_int(in_b.ndims, false);
        let in_rhs_shape = TypedArrayLikeAdapter::from(
            ArraySliceValue::from_ptr_val(
                in_b.shape().base_ptr(ctx, generator),
                in_rhs_ndims,
                None,
            ),
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );
        let lhs_shape = TypedArrayLikeAdapter::from(
            ArraySliceValue::from_ptr_val(
                ctx.builder.build_array_alloca(llvm_usize, ndims, "").unwrap(),
                ndims,
                None,
            ),
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );
        let rhs_shape = TypedArrayLikeAdapter::from(
            ArraySliceValue::from_ptr_val(
                ctx.builder.build_array_alloca(llvm_usize, ndims, "").unwrap(),
                ndims,
                None,
            ),
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );
        let dst_shape = TypedArrayLikeAdapter::from(
            ArraySliceValue::from_ptr_val(
                ctx.builder.build_array_alloca(llvm_usize, ndims, "").unwrap(),
                ndims,
                None,
            ),
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );

        // Matmul dimension compatibility is checked here.
        irrt::ndarray::call_nac3_ndarray_matmul_calculate_shapes(
            generator,
            ctx,
            &in_lhs_shape,
            &in_rhs_shape,
            ndims,
            &lhs_shape,
            &rhs_shape,
            &dst_shape,
        );

        let lhs = in_a.broadcast_to(generator, ctx, ndims_int, &lhs_shape);
        let rhs = in_b.broadcast_to(generator, ctx, ndims_int, &rhs_shape);

        let dst = NDArrayType::new(ctx, llvm_dst_dtype, ndims_int)
            .construct_uninitialized(generator, ctx, None);
        dst.copy_shape_from_array(generator, ctx, dst_shape.base_ptr(ctx, generator));
        unsafe {
            dst.create_data(generator, ctx);
        }

        (lhs, rhs, dst)
    };

    let len = unsafe {
        lhs.shape().get_typed_unchecked(
            ctx,
            generator,
            &llvm_usize.const_int(ndims_int - 1, false),
            None,
        )
    };

    let at_row = i64::try_from(ndims_int - 2).unwrap();
    let at_col = i64::try_from(ndims_int - 1).unwrap();

    let dst_dtype_llvm = ctx.get_llvm_type(generator, dst_dtype);
    let dst_zero = dst_dtype_llvm.const_zero();

    dst.foreach(generator, ctx, |generator, ctx, _, hdl| {
        let pdst_ij = hdl.get_pointer(ctx);

        ctx.builder.build_store(pdst_ij, dst_zero).unwrap();

        let indices = hdl.get_indices::<G>();
        let i = unsafe {
            indices.get_unchecked(ctx, generator, &llvm_usize.const_int(at_row as u64, true), None)
        };
        let j = unsafe {
            indices.get_unchecked(ctx, generator, &llvm_usize.const_int(at_col as u64, true), None)
        };

        let num_0 = llvm_usize.const_int(0, false);
        let num_1 = llvm_usize.const_int(1, false);

        gen_for_callback_incrementing(
            generator,
            ctx,
            None,
            num_0,
            (len, false),
            |generator, ctx, _, k| {
                // `indices` is modified to index into `a` and `b`, and restored.
                unsafe {
                    indices.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(at_row as u64, true),
                        i,
                    );
                    indices.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(at_col as u64, true),
                        k.into(),
                    );
                }
                let a_ik = unsafe { lhs.data().get_unchecked(ctx, generator, &indices, None) };

                unsafe {
                    indices.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(at_row as u64, true),
                        k.into(),
                    );
                    indices.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(at_col as u64, true),
                        j,
                    );
                }
                let b_kj = unsafe { rhs.data().get_unchecked(ctx, generator, &indices, None) };

                // Restore `indices`.
                unsafe {
                    indices.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(at_row as u64, true),
                        i,
                    );
                    indices.set_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(at_col as u64, true),
                        j,
                    );
                }

                // x = a_[...]ik * b_[...]kj
                let x = gen_binop_expr_with_values(
                    generator,
                    ctx,
                    (&Some(lhs_dtype), a_ik),
                    Binop::normal(Operator::Mult),
                    (&Some(rhs_dtype), b_kj),
                    ctx.current_loc,
                )?;

                // dst_[...]ij += x
                let dst_ij = ctx.builder.build_load(pdst_ij, "").unwrap();
                let dst_ij = gen_binop_expr_with_values(
                    generator,
                    ctx,
                    (&Some(dst_dtype), dst_ij),
                    Binop::normal(Operator::Add),
                    (&Some(dst_dtype), x),
                    ctx.current_loc,
                )?;
                ctx.builder.build_store(pdst_ij, dst_ij).unwrap();

                Ok(())
            },
            num_1,
        )
    })
    .unwrap();

    dst
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Perform [`np.matmul`](https://numpy.org/doc/stable/reference/generated/numpy.matmul.html).
    ///
    /// This function always return an [`NDArrayValue`]. You may want to use
    /// [`NDArrayValue::split_unsized`] to handle when the output could be a scalar.
    ///
    /// `dst_dtype` defines the dtype of the returned ndarray.
    #[must_use]
    pub fn matmul<G: CodeGenerator>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        self_ty: Type,
        (other_ty, other): (Type, Self),
        (out_dtype, out): (Type, NDArrayOut<'ctx>),
    ) -> Self {
        // Sanity check, but type inference should prevent this.
        assert!(self.ndims > 0 && other.ndims > 0, "np.matmul disallows scalar input");

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

        let new_a = if self.ndims == 1 {
            // Prepend 1 to its dimensions
            self.index(generator, ctx, &[RustNDIndex::NewAxis, RustNDIndex::Ellipsis])
        } else {
            *self
        };

        let new_b = if other.ndims == 1 {
            // Append 1 to its dimensions
            other.index(generator, ctx, &[RustNDIndex::Ellipsis, RustNDIndex::NewAxis])
        } else {
            other
        };

        // NOTE: `result` will always be a newly allocated ndarray.
        // Current implementation cannot do in-place matrix muliplication.
        let mut result =
            matmul_at_least_2d(generator, ctx, out_dtype, (self_ty, new_a), (other_ty, new_b));

        // Postprocessing on the result to remove prepended/appended axes.
        let mut postindices = vec![];
        let zero = ctx.ctx.i32_type().const_zero();

        if self.ndims == 1 {
            // Remove the prepended 1
            postindices.push(RustNDIndex::SingleElement(zero));
        }

        if other.ndims == 1 {
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
                let result_shape = result.shape();
                out_ndarray.assert_can_be_written_by_out(generator, ctx, result_shape);

                out_ndarray.copy_data_from(ctx, result);
                out_ndarray
            }
        }
    }
}
