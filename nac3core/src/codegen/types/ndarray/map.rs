use inkwell::{types::BasicTypeEnum, values::BasicValueEnum};
use itertools::Itertools;

use crate::codegen::{
    CodeGenContext,
    stmt::gen_for_callback,
    types::{
        ProxyType,
        ndarray::{NDArrayType, NDIterType},
    },
    values::{
        ArrayLikeValue, ProxyValue,
        ndarray::{NDArrayOut, NDArrayValue, ScalarOrNDArray},
    },
};

impl<'ctx> NDArrayType<'ctx> {
    /// Generate LLVM IR to broadcast `ndarray`s together, and starmap through them with `mapping`
    /// elementwise.
    ///
    /// `mapping` is an LLVM IR generator. The input of `mapping` is the list of elements when
    /// iterating through the input `ndarrays` after broadcasting. The output of `mapping` is the
    /// result of the elementwise operation.
    ///
    /// `out` specifies whether the result should be a new ndarray or to be written an existing
    /// ndarray.
    pub fn broadcast_starmap<'a, MappingFn>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        ndarrays: &[NDArrayValue<'ctx>],
        out: NDArrayOut<'ctx>,
        mapping: MappingFn,
    ) -> Result<<Self as ProxyType<'ctx>>::Value, String>
    where
        MappingFn: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            &[BasicValueEnum<'ctx>],
        ) -> Result<BasicValueEnum<'ctx>, String>,
    {
        // Broadcast inputs
        let broadcast_result = self.broadcast(ctx, ndarrays);

        let out_ndarray = match out {
            NDArrayOut::NewNDArray { dtype } => {
                // Create a new ndarray based on the broadcast shape.
                let result_ndarray = NDArrayType::new(ctx, dtype, broadcast_result.ndims)
                    .construct_uninitialized(ctx, None);
                result_ndarray.copy_shape_from_array(ctx, broadcast_result.shape.base_ptr(ctx));
                unsafe {
                    result_ndarray.create_data(ctx);
                }
                result_ndarray
            }

            NDArrayOut::WriteToNDArray { ndarray: result_ndarray } => {
                // Use an existing ndarray.

                // Check that its shape is compatible with the broadcast shape.
                result_ndarray.assert_can_be_written_by_out(ctx, broadcast_result.shape);
                result_ndarray
            }
        };

        // Map element-wise and store results into `mapped_ndarray`.
        let nditer = NDIterType::new(ctx).construct(ctx, out_ndarray);
        gen_for_callback(
            &mut (),
            ctx,
            Some("broadcast_starmap"),
            |(), ctx| {
                // Create NDIters for all broadcasted input ndarrays.
                let other_nditers = broadcast_result
                    .ndarrays
                    .iter()
                    .map(|ndarray| NDIterType::new(ctx).construct(ctx, *ndarray))
                    .collect_vec();
                Ok((nditer, other_nditers))
            },
            |(), ctx, (out_nditer, _in_nditers)| {
                // We can simply use `out_nditer`'s `has_element()`.
                // `in_nditers`' `has_element()`s should return the same value.
                Ok(out_nditer.has_element(ctx))
            },
            |(), ctx, _hooks, (out_nditer, in_nditers)| {
                // Get all the scalars from the broadcasted input ndarrays, pass them to `mapping`,
                // and write to `out_ndarray`.
                let in_scalars =
                    in_nditers.iter().map(|nditer| nditer.get_scalar(ctx)).collect_vec();

                let result = mapping(ctx, &in_scalars)?;

                let p = out_nditer.get_pointer(ctx);
                ctx.builder.build_store(p, result).unwrap();

                Ok(())
            },
            |(), ctx, (out_nditer, in_nditers)| {
                // Advance all iterators
                out_nditer.next(ctx);
                for nditer in &in_nditers {
                    nditer.next(ctx);
                }
                Ok(())
            },
            |(), _| Ok(()),
        )?;

        Ok(out_ndarray)
    }
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Starmap through a list of inputs using `mapping`, where an input could be an ndarray, a
    /// scalar.
    ///
    /// This function is very helpful when implementing NumPy functions that takes on either scalars
    /// or ndarrays or a mix of them as their inputs and produces either an ndarray with broadcast,
    /// or a scalar if all its inputs are all scalars.
    ///
    /// For example ,this function can be used to implement `np.add`, which has the following
    /// behaviors:
    ///
    /// - `np.add(3, 4) = 7` # (scalar, scalar) -> scalar
    /// - `np.add(3, np.array([4, 5, 6]))` # (scalar, ndarray) -> ndarray; the first `scalar` is
    ///   converted into an ndarray and broadcasted.
    /// - `np.add(np.array([[1], [2], [3]]), np.array([[4, 5, 6]]))` # (ndarray, ndarray) ->
    ///   ndarray; there is broadcasting.
    ///
    /// ## Details:
    ///
    /// If `inputs` are all [`ScalarOrNDArray::Scalar`], the output will be a
    /// [`ScalarOrNDArray::Scalar`] with type `ret_dtype`.
    ///
    /// Otherwise (if there are any [`ScalarOrNDArray::NDArray`] in `inputs`), all inputs will be
    /// 'as-ndarray'-ed into ndarrays, then all inputs (now all ndarrays) will be passed to
    /// [`NDArrayValue::broadcasting_starmap`] and **create** a new ndarray with dtype `ret_dtype`.
    pub fn broadcasting_starmap<'a, MappingFn>(
        ctx: &mut CodeGenContext<'ctx, 'a>,
        inputs: &[Self],
        ret_dtype: BasicTypeEnum<'ctx>,
        mapping: MappingFn,
    ) -> Result<Self, String>
    where
        MappingFn: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            &[BasicValueEnum<'ctx>],
        ) -> Result<BasicValueEnum<'ctx>, String>,
    {
        // Check if all inputs are Scalars
        let all_scalars: Option<Vec<_>> =
            inputs.iter().map(BasicValueEnum::<'ctx>::try_from).try_collect().ok();

        if let Some(scalars) = all_scalars {
            let scalars = scalars.iter().copied().collect_vec();
            let value = mapping(ctx, &scalars)?;

            Ok(ScalarOrNDArray::Scalar(value))
        } else {
            // Promote all input to ndarrays and map through them.
            let inputs = inputs.iter().map(|input| input.to_ndarray(ctx)).collect_vec();
            let ndarray = NDArrayType::new_broadcast(
                ctx,
                ret_dtype,
                &inputs.iter().map(NDArrayValue::get_type).collect_vec(),
            )
            .broadcast_starmap(
                ctx,
                &inputs,
                NDArrayOut::NewNDArray { dtype: ret_dtype },
                mapping,
            )?;
            Ok(ScalarOrNDArray::NDArray(ndarray))
        }
    }
}
