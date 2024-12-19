use inkwell::{
    values::{BasicValueEnum, IntValue},
    IntPredicate,
};

use super::NDArrayType;
use crate::{
    codegen::{
        irrt, types::ProxyType, values::TypedArrayLikeAccessor, CodeGenContext, CodeGenerator,
    },
    typecheck::typedef::Type,
};

/// Get the zero value in `np.zeros()` of a `dtype`.
fn ndarray_zero_value<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    dtype: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        ctx.ctx.i32_type().const_zero().into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        ctx.ctx.i64_type().const_zero().into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.float) {
        ctx.ctx.f64_type().const_zero().into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.bool) {
        ctx.ctx.bool_type().const_zero().into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.str) {
        ctx.gen_string(generator, "").into()
    } else {
        panic!("unrecognized dtype: {}", ctx.unifier.stringify(dtype));
    }
}

/// Get the one value in `np.ones()` of a `dtype`.
fn ndarray_one_value<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    dtype: Type,
) -> BasicValueEnum<'ctx> {
    if [ctx.primitives.int32, ctx.primitives.uint32]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        let is_signed = ctx.unifier.unioned(dtype, ctx.primitives.int32);
        ctx.ctx.i32_type().const_int(1, is_signed).into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        let is_signed = ctx.unifier.unioned(dtype, ctx.primitives.int64);
        ctx.ctx.i64_type().const_int(1, is_signed).into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.float) {
        ctx.ctx.f64_type().const_float(1.0).into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.bool) {
        ctx.ctx.bool_type().const_int(1, false).into()
    } else if ctx.unifier.unioned(dtype, ctx.primitives.str) {
        ctx.gen_string(generator, "1").into()
    } else {
        panic!("unrecognized dtype: {}", ctx.unifier.stringify(dtype));
    }
}

impl<'ctx> NDArrayType<'ctx> {
    /// Create an ndarray like
    /// [`np.empty`](https://numpy.org/doc/stable/reference/generated/numpy.empty.html).
    pub fn construct_numpy_empty<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let ndarray = self.construct_uninitialized(generator, ctx, name);

        // Validate `shape`
        irrt::ndarray::call_nac3_ndarray_util_assert_shape_no_negative(generator, ctx, shape);

        ndarray.copy_shape_from_array(generator, ctx, shape.base_ptr(ctx, generator));
        unsafe { ndarray.create_data(generator, ctx) };

        ndarray
    }

    /// Create an ndarray like
    /// [`np.full`](https://numpy.org/doc/stable/reference/generated/numpy.full.html).
    pub fn construct_numpy_full<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
        fill_value: BasicValueEnum<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let ndarray = self.construct_numpy_empty(generator, ctx, shape, name);
        ndarray.fill(generator, ctx, fill_value);
        ndarray
    }

    /// Create an ndarray like
    /// [`np.zero`](https://numpy.org/doc/stable/reference/generated/numpy.zeros.html).
    pub fn construct_numpy_zeros<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(
            ctx.get_llvm_type(generator, dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(generator, dtype).print_to_string(),
        );

        let fill_value = ndarray_zero_value(generator, ctx, dtype);
        self.construct_numpy_full(generator, ctx, shape, fill_value, name)
    }

    /// Create an ndarray like
    /// [`np.ones`](https://numpy.org/doc/stable/reference/generated/numpy.ones.html).
    pub fn construct_numpy_ones<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(
            ctx.get_llvm_type(generator, dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(generator, dtype).print_to_string(),
        );

        let fill_value = ndarray_one_value(generator, ctx, dtype);
        self.construct_numpy_full(generator, ctx, shape, fill_value, name)
    }

    /// Create an ndarray like
    /// [`np.eye`](https://numpy.org/doc/stable/reference/generated/numpy.eye.html).
    #[allow(clippy::too_many_arguments)]
    pub fn construct_numpy_eye<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        nrows: IntValue<'ctx>,
        ncols: IntValue<'ctx>,
        offset: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(
            ctx.get_llvm_type(generator, dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(generator, dtype).print_to_string(),
        );
        assert_eq!(nrows.get_type(), self.llvm_usize);
        assert_eq!(ncols.get_type(), self.llvm_usize);
        assert_eq!(offset.get_type(), self.llvm_usize);

        let ndzero = ndarray_zero_value(generator, ctx, dtype);
        let ndone = ndarray_one_value(generator, ctx, dtype);

        let ndarray = self.construct_dyn_shape(generator, ctx, &[nrows, ncols], name);

        // Create data and make the matrix like look np.eye()
        unsafe {
            ndarray.create_data(generator, ctx);
        }
        ndarray
            .foreach(generator, ctx, |generator, ctx, _, nditer| {
                // NOTE: rows and cols can never be zero here, since this ndarray's `np.size` would be zero
                // and this loop would not execute.

                let indices = nditer.get_indices();

                let row_i = unsafe {
                    indices.get_typed_unchecked(ctx, generator, &self.llvm_usize.const_zero(), None)
                };
                let col_i = unsafe {
                    indices.get_typed_unchecked(
                        ctx,
                        generator,
                        &self.llvm_usize.const_int(1, false),
                        None,
                    )
                };

                let be_one = ctx
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        ctx.builder.build_int_add(row_i, offset, "").unwrap(),
                        col_i,
                        "",
                    )
                    .unwrap();
                let value = ctx.builder.build_select(be_one, ndone, ndzero, "value").unwrap();

                let p = nditer.get_pointer(ctx);
                ctx.builder.build_store(p, value).unwrap();

                Ok(())
            })
            .unwrap();

        ndarray
    }

    /// Create an ndarray like
    /// [`np.identity`](https://numpy.org/doc/stable/reference/generated/numpy.identity.html).
    pub fn construct_numpy_identity<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let offset = self.llvm_usize.const_zero();
        self.construct_numpy_eye(generator, ctx, dtype, size, size, offset, name)
    }
}
