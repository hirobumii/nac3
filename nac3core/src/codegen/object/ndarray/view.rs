use super::{indexing::RustNDIndex, NDArrayObject};
use crate::codegen::{CodeGenContext, CodeGenerator};

impl<'ctx> NDArrayObject<'ctx> {
    /// Make sure the ndarray is at least `ndmin`-dimensional.
    ///
    /// If this ndarray's `ndims` is less than `ndmin`, a view is created on this with 1s prepended to the shape.
    /// If this ndarray's `ndims` is not less than `ndmin`, this function does nothing and return this ndarray.
    #[must_use]
    pub fn atleast_nd<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndmin: u64,
    ) -> Self {
        if self.ndims < ndmin {
            // Extend the dimensions with np.newaxis.
            let mut indices = vec![];
            for _ in self.ndims..ndmin {
                indices.push(RustNDIndex::NewAxis);
            }
            indices.push(RustNDIndex::Ellipsis);
            self.index(generator, ctx, &indices)
        } else {
            *self
        }
    }
}
