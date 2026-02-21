use inkwell::{
    IntPredicate,
    values::{BasicValueEnum, IntValue},
};

use crate::{
    codegen::{
        CodeGenContext,
        expr::call_extern,
        irrt::get_usize_dependent_function_name,
        typed_store,
        types::{
            array::{ArrayLikeIndexer, ArraySliceValue},
            ndarray::{NDArrayType, NDArrayValue},
        },
    },
    typecheck::typedef::Type,
};

/// Get the zero value in `np.zeros()` of a `dtype`.
fn ndarray_zero_value<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    dtype: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        ctx.i32.const_zero().into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        ctx.i64.const_zero().into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.float) {
        ctx.f64.const_zero().into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.bool) {
        ctx.i1.const_zero().into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.str) {
        ctx.gen_string("").into()
    } else {
        panic!("unrecognized dtype: {}", ctx.unifier.stringify(dtype));
    }
}

/// Get the one value in `np.ones()` of a `dtype`.
fn ndarray_one_value<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    dtype: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        let is_signed = ctx.unifier.unioned(dtype, ctx.primitives.int32);
        ctx.i32.const_int(1, is_signed).into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        let is_signed = ctx.unifier.unioned(dtype, ctx.primitives.int64);
        ctx.i64.const_int(1, is_signed).into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.float) {
        ctx.f64.const_float(1.0).into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.bool) {
        ctx.i1.const_int(1, false).into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.str) {
        ctx.gen_string("1").into()
    } else {
        panic!("unrecognized dtype: {}", ctx.unifier.stringify(dtype));
    }
}

impl<'ctx> NDArrayType<'ctx> {
    /// Create an ndarray like
    /// [`np.empty`](https://numpy.org/doc/stable/reference/generated/numpy.empty.html).
    pub fn construct_numpy_empty(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: ArraySliceValue<'ctx>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        let ndarray = self.construct(ctx, name);

        // Validate `shape`
        let (shape_ptr, shape_len) = shape.value;
        let name =
            get_usize_dependent_function_name(ctx, "__nac3_ndarray_util_assert_shape_no_negative");
        call_extern!(ctx: (ctx.size_t) _ = name(shape_len, shape_ptr));

        ndarray.shape(ctx).memcpy_from(ctx, shape_ptr);
        ndarray.create_data(ctx);
        ndarray
    }

    /// Create an ndarray like
    /// [`np.full`](https://numpy.org/doc/stable/reference/generated/numpy.full.html).
    pub fn construct_numpy_full(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: ArraySliceValue<'ctx>,
        fill_value: BasicValueEnum<'ctx>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        let ndarray = self.construct_numpy_empty(ctx, shape, name);
        ndarray.fill(ctx, fill_value);
        ndarray
    }

    fn assert_compatible_dtype(&self, ctx: &mut CodeGenContext<'ctx, '_>, dtype: Type) {
        assert_eq!(
            ctx.get_llvm_type(dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(dtype).print_to_string(),
        );
    }

    /// Create an ndarray like
    /// [`np.zero`](https://numpy.org/doc/stable/reference/generated/numpy.zeros.html).
    pub fn construct_numpy_zeros(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        shape: ArraySliceValue<'ctx>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        self.assert_compatible_dtype(ctx, dtype);
        let fill_value = ndarray_zero_value(ctx, dtype);
        self.construct_numpy_full(ctx, shape, fill_value, name)
    }

    /// Create an ndarray like
    /// [`np.ones`](https://numpy.org/doc/stable/reference/generated/numpy.ones.html).
    pub fn construct_numpy_ones(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        shape: ArraySliceValue<'ctx>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        self.assert_compatible_dtype(ctx, dtype);
        let fill_value = ndarray_one_value(ctx, dtype);
        self.construct_numpy_full(ctx, shape, fill_value, name)
    }

    /// Create an ndarray like
    /// [`np.eye`](https://numpy.org/doc/stable/reference/generated/numpy.eye.html).
    #[allow(clippy::too_many_arguments)]
    pub fn construct_numpy_eye(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        nrows: IntValue<'ctx>,
        ncols: IntValue<'ctx>,
        offset: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        self.assert_compatible_dtype(ctx, dtype);
        assert_eq!(nrows.get_type(), ctx.size_t);
        assert_eq!(ncols.get_type(), ctx.size_t);
        assert_eq!(offset.get_type(), ctx.size_t);

        let ndzero = ndarray_zero_value(ctx, dtype);
        let ndone = ndarray_one_value(ctx, dtype);

        let ndarray = self.with_shape(ctx, &[nrows, ncols], name);

        ndarray
            .foreach(ctx, |ctx, _, nditer| {
                // NOTE: rows and cols can never be zero here, since this ndarray's `np.size` would be zero
                // and this loop would not execute.

                let indices = nditer.indices(ctx);
                let row_i = indices.get_unchecked(ctx, &ctx.size_t.const_zero(), None);
                let col_i = indices.get_unchecked(ctx, &ctx.size_t.const_int(1, false), None);

                let with_offset = ctx.builder.build_int_add(row_i, offset, "").unwrap();
                let be_one = ctx
                    .builder
                    .build_int_compare(IntPredicate::EQ, with_offset, col_i, "")
                    .unwrap();
                let value = ctx.builder.build_select(be_one, ndone, ndzero, "value").unwrap();

                let p = nditer.curr_ptr(ctx);
                typed_store(ctx.builder, p, value);

                Ok(())
            })
            .unwrap();

        ndarray
    }

    /// Create an ndarray like
    /// [`np.identity`](https://numpy.org/doc/stable/reference/generated/numpy.identity.html).
    pub fn construct_numpy_identity(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        size: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        let offset = ctx.size_t.const_zero();
        self.construct_numpy_eye(ctx, dtype, size, size, offset, name)
    }
}
