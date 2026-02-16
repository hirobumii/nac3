use std::iter::{once, repeat_n};

use inkwell::values::PointerValue;
use itertools::Itertools as _;

use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    stmt::gen_if_callback,
    types::{
        array::ArraySliceValue,
        field,
        ndarray::{NDArrayType, NDArrayValue, indexing::RustNDIndex},
    },
};

impl<'ctx> NDArrayValue<'ctx> {
    /// Make sure the ndarray is at least `ndmin`-dimensional.
    ///
    /// If this ndarray's `ndims` is less than `ndmin`, a view is created on this with 1s prepended
    /// to the shape. Otherwise, this function does nothing and return this ndarray.
    pub fn atleast_nd(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndmin: u64,
    ) -> anyhow::Result<Self> {
        let ndims = self.ty.ndims;

        if ndims < ndmin {
            // Extend the dimensions with np.newaxis.
            let indices = repeat_n(RustNDIndex::NewAxis, (ndmin - ndims) as usize)
                .chain(once(RustNDIndex::Ellipsis))
                .collect_vec();
            self.index(ctx, &indices)
        } else {
            Ok(*self)
        }
    }

    /// Create a reshaped view on this ndarray like
    /// [`np.reshape()`](https://numpy.org/doc/stable/reference/generated/numpy.reshape.html).
    ///
    /// If there is a `-1` in `new_shape`, it will be resolved; `new_shape` would **NOT** be
    /// modified as a result.
    ///
    /// If reshape without copying is impossible, this function will allocate a new ndarray and copy
    /// contents.
    ///
    /// * `new_ndims` - The number of dimensions of `new_shape` as a [`Type`].
    /// * `new_shape` - The target shape to do `np.reshape()`.
    pub fn reshape_or_copy(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        new_ndims: u64,
        new_shape: ArraySliceValue<'ctx>,
    ) -> anyhow::Result<Self> {
        assert_eq!(new_shape.ty.item_ty, ctx.size_t.into());

        // TODO: The current criterion for whether to do a full copy or not is by checking
        //       `is_c_contiguous`, but this is not optimal - there are cases when the ndarray is
        //       not contiguous but could be reshaped without copying data. Look into how numpy does
        //       it.

        let dst_ndarray = NDArrayType::new(ctx, self.ty.dtype, new_ndims).construct(ctx, None)?;
        dst_ndarray.shape(ctx)?.memcpy_from(ctx, new_shape.value.0)?;

        // Resolve negative indices
        let size = self.size(ctx)?;
        let (dst_shape, dst_ndims) = dst_ndarray.shape(ctx)?.value;
        let name = get_usize_dependent_function_name(
            ctx,
            "__nac3_ndarray_reshape_resolve_and_check_new_shape",
        );
        call_extern!(ctx: void _ = name(size, dst_ndims, dst_shape))?;

        gen_if_callback(
            &mut (),
            ctx,
            |(), ctx| self.is_c_contiguous(ctx),
            |(), ctx| {
                // Reshape is possible without copying
                dst_ndarray.set_strides_contiguous(ctx)?;
                let data = self.load(ctx, field!(data))?;
                dst_ndarray.store(ctx, field!(data), data)?;
                Ok(())
            },
            |(), ctx| {
                // Reshape is impossible without copying
                dst_ndarray.create_data(ctx)?;
                dst_ndarray.copy_data_from(ctx, self)?;
                Ok(())
            },
        )?;

        Ok(dst_ndarray)
    }

    /// Create a transposed view on this ndarray like
    /// [`np.transpose(<ndarray>, <axes> = None)`](https://numpy.org/doc/stable/reference/generated/numpy.transpose.html).
    ///
    /// * `axes` - If specified, should be an array of the permutation (negative indices are
    ///   **allowed**).
    pub fn transpose(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        axes: Option<PointerValue<'ctx>>,
    ) -> anyhow::Result<Self> {
        // Define models
        let transposed_ndarray = self.ty.construct(ctx, None)?;

        let (axes, num_axes) = match axes {
            Some(axes) => (axes, self.ty.ndims_val(ctx)),
            None => (ctx.ptr.const_null(), ctx.size_t.const_zero()),
        };

        let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_transpose");
        call_extern!(ctx: void _ = name(
            self.value,
            transposed_ndarray.value,
            num_axes, axes,
        ))?;

        Ok(transposed_ndarray)
    }
}
