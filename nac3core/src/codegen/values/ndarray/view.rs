use std::iter::{once, repeat_n};

use itertools::Itertools;

use crate::codegen::{
    values::ndarray::{NDArrayValue, RustNDIndex},
    CodeGenContext, CodeGenerator,
};

impl<'ctx> NDArrayValue<'ctx> {
    /// Make sure the ndarray is at least `ndmin`-dimensional.
    ///
    /// If this ndarray's `ndims` is less than `ndmin`, a view is created on this with 1s prepended
    /// to the shape. Otherwise, this function does nothing and return this ndarray.
    #[must_use]
    pub fn atleast_nd<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndmin: u64,
    ) -> Self {
        assert!(self.ndims.is_some(), "NDArrayValue::atleast_nd is only supported for instances with compile-time known ndims (self.ndims = Some(...))");

        let ndims = self.ndims.unwrap();

        if ndims < ndmin {
            // Extend the dimensions with np.newaxis.
            let indices = repeat_n(RustNDIndex::NewAxis, (ndmin - ndims) as usize)
                .chain(once(RustNDIndex::Ellipsis))
                .collect_vec();
            self.index(generator, ctx, &indices)
        } else {
            *self
        }
    }
}
