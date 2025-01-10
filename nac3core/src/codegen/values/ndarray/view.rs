use std::iter::{once, repeat_n};

use inkwell::values::{IntValue, PointerValue};
use itertools::Itertools;

use crate::codegen::{
    irrt,
    stmt::gen_if_callback,
    types::ndarray::NDArrayType,
    values::{
        ndarray::{NDArrayValue, RustNDIndex},
        ArrayLikeValue, ArraySliceValue, ProxyValue, TypedArrayLikeAccessor, TypedArrayLikeAdapter,
    },
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
        let ndims = self.ndims;

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
    #[must_use]
    pub fn reshape_or_copy<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        new_ndims: u64,
        new_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    ) -> Self {
        assert_eq!(new_shape.element_type(ctx, generator), self.llvm_usize.into());

        // TODO: The current criterion for whether to do a full copy or not is by checking
        //       `is_c_contiguous`, but this is not optimal - there are cases when the ndarray is
        //       not contiguous but could be reshaped without copying data. Look into how numpy does
        //       it.

        let dst_ndarray = NDArrayType::new(generator, ctx.ctx, self.dtype, new_ndims)
            .construct_uninitialized(generator, ctx, None);
        dst_ndarray.copy_shape_from_array(generator, ctx, new_shape.base_ptr(ctx, generator));

        // Resolve negative indices
        let size = self.size(generator, ctx);
        let dst_ndims = self.llvm_usize.const_int(dst_ndarray.get_type().ndims(), false);
        let dst_shape = dst_ndarray.shape();
        irrt::ndarray::call_nac3_ndarray_reshape_resolve_and_check_new_shape(
            generator,
            ctx,
            size,
            dst_ndims,
            dst_shape.as_slice_value(ctx, generator),
        );

        gen_if_callback(
            generator,
            ctx,
            |generator, ctx| Ok(self.is_c_contiguous(generator, ctx)),
            |generator, ctx| {
                // Reshape is possible without copying
                dst_ndarray.set_strides_contiguous(generator, ctx);
                dst_ndarray.store_data(ctx, self.data().base_ptr(ctx, generator));

                Ok(())
            },
            |generator, ctx| {
                // Reshape is impossible without copying
                unsafe {
                    dst_ndarray.create_data(generator, ctx);
                }
                dst_ndarray.copy_data_from(generator, ctx, *self);

                Ok(())
            },
        )
        .unwrap();

        dst_ndarray
    }

    /// Create a transposed view on this ndarray like
    /// [`np.transpose(<ndarray>, <axes> = None)`](https://numpy.org/doc/stable/reference/generated/numpy.transpose.html).
    ///
    /// * `axes` - If specified, should be an array of the permutation (negative indices are
    ///   **allowed**).
    #[must_use]
    pub fn transpose<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        axes: Option<PointerValue<'ctx>>,
    ) -> Self {
        assert!(
            axes.is_none_or(|axes| axes.get_type().get_element_type() == self.llvm_usize.into())
        );

        // Define models
        let transposed_ndarray = self.get_type().construct_uninitialized(generator, ctx, None);

        let axes = if let Some(axes) = axes {
            let num_axes = self.llvm_usize.const_int(self.ndims, false);

            // `axes = nullptr` if `axes` is unspecified.
            let axes = ArraySliceValue::from_ptr_val(axes, num_axes, None);

            Some(TypedArrayLikeAdapter::from(
                axes,
                |_, _, val| val.into_int_value(),
                |_, _, val| val.into(),
            ))
        } else {
            None
        };

        irrt::ndarray::call_nac3_ndarray_transpose(
            generator,
            ctx,
            *self,
            transposed_ndarray,
            axes.as_ref(),
        );

        transposed_ndarray
    }
}
