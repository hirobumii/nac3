use super::{indexing::RustNDIndex, NDArrayObject};
use crate::codegen::{
    irrt::call_nac3_ndarray_reshape_resolve_and_check_new_shape, model::*, CodeGenContext,
    CodeGenerator,
};

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

    /// Create a reshaped view on this ndarray like `np.reshape()`.
    ///
    /// If there is a `-1` in `new_shape`, it will be resolved; `new_shape` would **NOT** be modified as a result.
    ///
    /// If reshape without copying is impossible, this function will allocate a new ndarray and copy contents.
    ///
    /// * `new_ndims` - The number of dimensions of `new_shape` as a [`Type`].
    /// * `new_shape` - The target shape to do `np.reshape()`.
    #[must_use]
    pub fn reshape_or_copy<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        new_ndims: u64,
        new_shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> Self {
        // TODO: The current criterion for whether to do a full copy or not is by checking `is_c_contiguous`,
        // but this is not optimal - there are cases when the ndarray is not contiguous but could be reshaped
        // without copying data. Look into how numpy does it.

        let current_bb = ctx.builder.get_insert_block().unwrap();
        let then_bb = ctx.ctx.insert_basic_block_after(current_bb, "then_bb");
        let else_bb = ctx.ctx.insert_basic_block_after(then_bb, "else_bb");
        let end_bb = ctx.ctx.insert_basic_block_after(else_bb, "end_bb");

        let dst_ndarray = NDArrayObject::alloca(generator, ctx, self.dtype, new_ndims);
        dst_ndarray.copy_shape_from_array(generator, ctx, new_shape);

        // Reolsve negative indices
        let size = self.size(generator, ctx);
        let dst_ndims = dst_ndarray.ndims_llvm(generator, ctx.ctx);
        let dst_shape = dst_ndarray.instance.get(generator, ctx, |f| f.shape);
        call_nac3_ndarray_reshape_resolve_and_check_new_shape(
            generator, ctx, size, dst_ndims, dst_shape,
        );

        let is_c_contiguous = self.is_c_contiguous(generator, ctx);
        ctx.builder.build_conditional_branch(is_c_contiguous.value, then_bb, else_bb).unwrap();

        // Inserting into then_bb: reshape is possible without copying
        ctx.builder.position_at_end(then_bb);
        dst_ndarray.set_strides_contiguous(generator, ctx);
        dst_ndarray.instance.set(ctx, |f| f.data, self.instance.get(generator, ctx, |f| f.data));
        ctx.builder.build_unconditional_branch(end_bb).unwrap();

        // Inserting into else_bb: reshape is impossible without copying
        ctx.builder.position_at_end(else_bb);
        dst_ndarray.create_data(generator, ctx);
        dst_ndarray.copy_data_from(generator, ctx, *self);
        ctx.builder.build_unconditional_branch(end_bb).unwrap();

        // Reposition for continuation
        ctx.builder.position_at_end(end_bb);

        dst_ndarray
    }
}
