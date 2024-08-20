use inkwell::values::BasicValueEnum;
use itertools::Itertools;

use crate::{
    codegen::{
        object::ndarray::{AnyObject, NDArrayObject},
        stmt::gen_for_callback,
        CodeGenContext, CodeGenerator,
    },
    typecheck::typedef::Type,
};

use super::{nditer::NDIterHandle, NDArrayOut, ScalarOrNDArray};

impl<'ctx> NDArrayObject<'ctx> {
    /// Generate LLVM IR to broadcast `ndarray`s together, and starmap through them with `mapping` elementwise.
    ///
    /// `mapping` is an LLVM IR generator. The input of `mapping` is the list of elements when iterating through
    /// the input `ndarrays` after broadcasting. The output of `mapping` is the result of the elementwise operation.
    ///
    /// `out` specifies whether the result should be a new ndarray or to be written an existing ndarray.
    pub fn broadcast_starmap<'a, G, MappingFn>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        ndarrays: &[Self],
        out: NDArrayOut<'ctx>,
        mapping: MappingFn,
    ) -> Result<Self, String>
    where
        G: CodeGenerator + ?Sized,
        MappingFn: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            &[BasicValueEnum<'ctx>],
        ) -> Result<BasicValueEnum<'ctx>, String>,
    {
        // Broadcast inputs
        let broadcast_result = NDArrayObject::broadcast(generator, ctx, ndarrays);

        let out_ndarray = match out {
            NDArrayOut::NewNDArray { dtype } => {
                // Create a new ndarray based on the broadcast shape.
                let result_ndarray =
                    NDArrayObject::alloca(generator, ctx, dtype, broadcast_result.ndims);
                result_ndarray.copy_shape_from_array(generator, ctx, broadcast_result.shape);
                result_ndarray.create_data(generator, ctx);
                result_ndarray
            }
            NDArrayOut::WriteToNDArray { ndarray: result_ndarray } => {
                // Use an existing ndarray.

                // Check that its shape is compatible with the broadcast shape.
                result_ndarray.assert_can_be_written_by_out(
                    generator,
                    ctx,
                    broadcast_result.ndims,
                    broadcast_result.shape,
                );
                result_ndarray
            }
        };

        // Map element-wise and store results into `mapped_ndarray`.
        let nditer = NDIterHandle::new(generator, ctx, out_ndarray);
        gen_for_callback(
            generator,
            ctx,
            Some("broadcast_starmap"),
            |generator, ctx| {
                // Create NDIters for all broadcasted input ndarrays.
                let other_nditers = broadcast_result
                    .ndarrays
                    .iter()
                    .map(|ndarray| NDIterHandle::new(generator, ctx, *ndarray))
                    .collect_vec();
                Ok((nditer, other_nditers))
            },
            |generator, ctx, (out_nditer, _in_nditers)| {
                // We can simply use `out_nditer`'s `has_next()`.
                // `in_nditers`' `has_next()`s should return the same value.
                Ok(out_nditer.has_next(generator, ctx).value)
            },
            |generator, ctx, _hooks, (out_nditer, in_nditers)| {
                // Get all the scalars from the broadcasted input ndarrays, pass them to `mapping`,
                // and write to `out_ndarray`.

                let in_scalars = in_nditers
                    .iter()
                    .map(|nditer| nditer.get_scalar(generator, ctx).value)
                    .collect_vec();

                let result = mapping(generator, ctx, &in_scalars)?;

                let p = out_nditer.get_pointer(generator, ctx);
                ctx.builder.build_store(p, result).unwrap();

                Ok(())
            },
            |generator, ctx, (out_nditer, in_nditers)| {
                // Advance all iterators
                out_nditer.next(generator, ctx);
                in_nditers.iter().for_each(|nditer| nditer.next(generator, ctx));
                Ok(())
            },
        )?;

        Ok(out_ndarray)
    }

    /// Map through this ndarray with an elementwise function.
    pub fn map<'a, G, Mapping>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        out: NDArrayOut<'ctx>,
        mapping: Mapping,
    ) -> Result<Self, String>
    where
        G: CodeGenerator + ?Sized,
        Mapping: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            BasicValueEnum<'ctx>,
        ) -> Result<BasicValueEnum<'ctx>, String>,
    {
        NDArrayObject::broadcast_starmap(
            generator,
            ctx,
            &[*self],
            out,
            |generator, ctx, scalars| mapping(generator, ctx, scalars[0]),
        )
    }
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Starmap through a list of inputs using `mapping`, where an input could be an ndarray, a scalar.
    ///
    /// This function is very helpful when implementing NumPy functions that takes on either scalars or ndarrays or a mix of them
    /// as their inputs and produces either an ndarray with broadcast, or a scalar if all its inputs are all scalars.
    ///
    /// For example ,this function can be used to implement `np.add`, which has the following behaviors:
    /// - `np.add(3, 4) = 7` # (scalar, scalar) -> scalar
    /// - `np.add(3, np.array([4, 5, 6]))` # (scalar, ndarray) -> ndarray; the first `scalar` is converted into an ndarray and broadcasted.
    /// - `np.add(np.array([[1], [2], [3]]), np.array([[4, 5, 6]]))` # (ndarray, ndarray) -> ndarray; there is broadcasting.
    ///
    /// ## Details:
    ///
    /// If `inputs` are all [`ScalarOrNDArray::Scalar`], the output will be a [`ScalarOrNDArray::Scalar`] with type `ret_dtype`.
    ///
    /// Otherwise (if there are any [`ScalarOrNDArray::NDArray`] in `inputs`), all inputs will be 'as-ndarray'-ed into ndarrays,
    /// then all inputs (now all ndarrays) will be passed to [`NDArrayObject::broadcasting_starmap`] and **create** a new ndarray
    /// with dtype `ret_dtype`.
    pub fn broadcasting_starmap<'a, G, MappingFn>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        inputs: &[ScalarOrNDArray<'ctx>],
        ret_dtype: Type,
        mapping: MappingFn,
    ) -> Result<ScalarOrNDArray<'ctx>, String>
    where
        G: CodeGenerator + ?Sized,
        MappingFn: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            &[BasicValueEnum<'ctx>],
        ) -> Result<BasicValueEnum<'ctx>, String>,
    {
        // Check if all inputs are Scalars
        let all_scalars: Option<Vec<_>> = inputs.iter().map(AnyObject::try_from).try_collect().ok();

        if let Some(scalars) = all_scalars {
            let scalars = scalars.iter().map(|scalar| scalar.value).collect_vec();
            let value = mapping(generator, ctx, &scalars)?;

            Ok(ScalarOrNDArray::Scalar(AnyObject { ty: ret_dtype, value }))
        } else {
            // Promote all input to ndarrays and map through them.
            let inputs = inputs.iter().map(|input| input.to_ndarray(generator, ctx)).collect_vec();
            let ndarray = NDArrayObject::broadcast_starmap(
                generator,
                ctx,
                &inputs,
                NDArrayOut::NewNDArray { dtype: ret_dtype },
                mapping,
            )?;
            Ok(ScalarOrNDArray::NDArray(ndarray))
        }
    }

    /// Map through this [`ScalarOrNDArray`] with an elementwise function.
    ///
    /// If this is a scalar, `mapping` will directly act on the scalar. This function will return a [`ScalarOrNDArray::Scalar`] of that result.
    ///
    /// If this is an ndarray, `mapping` will be applied to the elements of the ndarray. A new ndarray of the results will be created and
    /// returned as a [`ScalarOrNDArray::NDArray`].
    pub fn map<'a, G, Mapping>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        ret_dtype: Type,
        mapping: Mapping,
    ) -> Result<ScalarOrNDArray<'ctx>, String>
    where
        G: CodeGenerator + ?Sized,
        Mapping: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            BasicValueEnum<'ctx>,
        ) -> Result<BasicValueEnum<'ctx>, String>,
    {
        ScalarOrNDArray::broadcasting_starmap(
            generator,
            ctx,
            &[*self],
            ret_dtype,
            |generator, ctx, scalars| mapping(generator, ctx, scalars[0]),
        )
    }
}
