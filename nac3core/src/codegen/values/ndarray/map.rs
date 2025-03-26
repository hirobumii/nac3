use inkwell::{types::BasicTypeEnum, values::BasicValueEnum};

use crate::codegen::{
    CodeGenContext, CodeGenerator,
    values::{
        ProxyValue,
        ndarray::{NDArrayOut, NDArrayValue, ScalarOrNDArray},
    },
};

impl<'ctx> NDArrayValue<'ctx> {
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
        self.get_type().broadcast_starmap(
            generator,
            ctx,
            &[*self],
            out,
            |generator, ctx, scalars| mapping(generator, ctx, scalars[0]),
        )
    }
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Map through this [`ScalarOrNDArray`] with an elementwise function.
    ///
    /// If this is a scalar, `mapping` will directly act on the scalar. This function will return a
    /// [`ScalarOrNDArray::Scalar`] of that result.
    ///
    /// If this is an ndarray, `mapping` will be applied to the elements of the ndarray. A new
    /// ndarray of the results will be created and returned as a [`ScalarOrNDArray::NDArray`].
    pub fn map<'a, G, Mapping>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        ret_dtype: BasicTypeEnum<'ctx>,
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
