use inkwell::{
    IntPredicate,
    values::{BasicValue, BasicValueEnum, IntValue},
};
use nac3parser::ast::Operator;

use super::NDArrayType;
use crate::{
    codegen::{
        CodeGenContext, CodeGenerator, irrt, llvm_intrinsics::call_float_ceil, types::ProxyType,
        values::TypedArrayLikeAccessor,
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
        ctx.i32.const_zero().into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        ctx.i64.const_zero().into()
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
        ctx.i32.const_int(1, is_signed).into()
    } else if [ctx.primitives.int64, ctx.primitives.uint64]
        .iter()
        .any(|ty| ctx.unifier.unioned(dtype, *ty))
    {
        let is_signed = ctx.unifier.unioned(dtype, ctx.primitives.int64);
        ctx.i64.const_int(1, is_signed).into()
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

/// Function written to generate basic binary operations on numeric types
/// Passing a Float and Int will cast the Int to a Float, unsigned Ints will be cast to signed Ints
/// This is not intended for general use and designed specifically for certain specific numpy operations
fn gen_numpy_scalar_binop<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: BasicValueEnum<'ctx>,
    left_ty: Type,
    right: BasicValueEnum<'ctx>,
    right_ty: Type,
    op: Operator,
) -> BasicValueEnum<'ctx> {
    let mut is_signed = |val: Type| {
        [ctx.primitives.int32, ctx.primitives.int64].contains(&ctx.unifier.get_representative(val))
    };
    let left_attrs = left.is_int_value().then(|| is_signed(left_ty));
    let right_attrs = right.is_int_value().then(|| is_signed(right_ty));

    // If both left and right are integers, we can use the integer operations, otherwise we cast
    // any integers to floats and use float operations
    match (left_attrs, right_attrs) {
        (Some(l), Some(r)) => {
            let left = ctx
                .builder
                .build_int_s_extend_or_bit_cast(left.into_int_value(), ctx.size_t, "")
                .unwrap()
                .as_basic_value_enum();
            let right = ctx
                .builder
                .build_int_s_extend_or_bit_cast(right.into_int_value(), ctx.size_t, "")
                .unwrap()
                .as_basic_value_enum();

            ctx.gen_int_ops(generator, op, left, right, l || r)
        }
        (l, r) => {
            let cast = |val: BasicValueEnum<'ctx>, signed: bool| {
                if signed {
                    ctx.builder
                        .build_signed_int_to_float(val.into_int_value(), ctx.ctx.f64_type(), "")
                        .unwrap()
                        .into()
                } else {
                    ctx.builder
                        .build_unsigned_int_to_float(val.into_int_value(), ctx.ctx.f64_type(), "")
                        .unwrap()
                        .into()
                }
            };
            // Cast is only performed if the value is an integer, floats are left as is
            let left_float = l.map_or_else(|| left, |signed| cast(left, signed));
            let right_float = r.map_or_else(|| right, |signed| cast(right, signed));

            ctx.gen_float_ops(op, left_float, right_float)
        }
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
            ctx.get_llvm_type(dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(dtype).print_to_string(),
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
            ctx.get_llvm_type(dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(dtype).print_to_string(),
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
            ctx.get_llvm_type(dtype),
            self.dtype,
            "Expected LLVM dtype={} but got {}",
            self.dtype.print_to_string(),
            ctx.get_llvm_type(dtype).print_to_string(),
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

    /// Create an ndarray like [`np.arange`](https://numpy.org/doc/stable/reference/generated/numpy.arange.html).
    #[allow(clippy::too_many_arguments)]
    pub fn construct_numpy_arange<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        start: BasicValueEnum<'ctx>,
        stop: BasicValueEnum<'ctx>,
        step: BasicValueEnum<'ctx>,
        start_ty: Type,
        stop_ty: Type,
        step_ty: Type,
        dtype: Type,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let start_ty = ctx.unifier.get_representative(start_ty);
        let stop_ty = ctx.unifier.get_representative(stop_ty);
        let step_ty = ctx.unifier.get_representative(step_ty);
        let dtype = ctx.unifier.get_representative(dtype);

        let diff =
            gen_numpy_scalar_binop(generator, ctx, stop, stop_ty, start, start_ty, Operator::Sub);
        let diff_ty = if stop_ty == ctx.primitives.float || start_ty == ctx.primitives.float {
            ctx.primitives.float
        } else {
            ctx.primitives.int64
        };

        // `gen_num_binops` calls either `gen_int_ops` or `gen_float_ops`, for `Operator::Div` both
        // generate a float div, so `length` will always be a float.
        let length =
            gen_numpy_scalar_binop(generator, ctx, diff, diff_ty, step, step_ty, Operator::Div);
        debug_assert!(length.is_float_value(), "Expected length to be a float value.",);

        let ceil = call_float_ceil(ctx, length.into_float_value(), Some("arange_length_ceil"));
        let length_int = ctx
            .builder
            .build_float_to_unsigned_int(ceil, ctx.ctx.i64_type(), "arange_length_int")
            .unwrap();

        let ndarray = self.construct_dyn_shape(generator, ctx, &[length_int], name);
        unsafe { ndarray.create_data(generator, ctx) };

        ndarray
            .foreach(generator, ctx, |generator, ctx, _, nditer| {
                // For each iteration we want to calculate dtype(start + step * i) where i is the index.
                // Up to casting to dtype we use the least general type possible - this can cause loss
                // of precision, however this mirrors the behavior of `np.arange()` in Python.
                let indices = nditer.get_indices();
                let i = unsafe {
                    indices.get_typed_unchecked(ctx, generator, &ctx.size_t.const_zero(), None)
                };

                let step_i = gen_numpy_scalar_binop(
                    generator,
                    ctx,
                    step,
                    step_ty,
                    i.into(),
                    ctx.primitives.usize(),
                    Operator::Mult,
                );
                // Ty is the least general type that can hold step * i
                let mut ty = if step_ty == ctx.primitives.float {
                    ctx.primitives.float
                } else if [ctx.primitives.int32, ctx.primitives.int64].contains(&step_ty) {
                    ctx.primitives.isize()
                } else {
                    ctx.primitives.usize()
                };

                let value = gen_numpy_scalar_binop(
                    generator,
                    ctx,
                    start,
                    start_ty,
                    step_i,
                    ty,
                    Operator::Add,
                );
                ty = if start_ty == ctx.primitives.float {
                    ctx.primitives.float
                } else if [ctx.primitives.int32, ctx.primitives.int64].contains(&start_ty) {
                    ctx.primitives.isize()
                } else {
                    ty
                };

                let mut is_signed = |val: Type| {
                    [ctx.primitives.int32, ctx.primitives.int64]
                        .contains(&ctx.unifier.get_representative(val))
                };

                let casted = if ty == ctx.primitives.float && self.dtype.is_int_type() {
                    if is_signed(dtype) {
                        ctx.builder.build_float_to_signed_int(
                            value.into_float_value(),
                            self.dtype.into_int_type(),
                            "arange_casted",
                        )
                    } else {
                        ctx.builder.build_float_to_unsigned_int(
                            value.into_float_value(),
                            self.dtype.into_int_type(),
                            "arange_casted",
                        )
                    }
                    .unwrap()
                    .as_basic_value_enum()
                } else if value.is_int_value() && self.dtype.is_float_type() {
                    if is_signed(ty) {
                        ctx.builder.build_signed_int_to_float(
                            value.into_int_value(),
                            self.dtype.into_float_type(),
                            "arange_casted",
                        )
                    } else {
                        ctx.builder.build_unsigned_int_to_float(
                            value.into_int_value(),
                            self.dtype.into_float_type(),
                            "arange_casted",
                        )
                    }
                    .unwrap()
                    .as_basic_value_enum()
                } else if value.is_int_value() && self.dtype.is_int_type() {
                    let int_value = value.into_int_value();
                    let int_type = int_value.get_type();

                    let width_diff = i64::from(self.dtype.into_int_type().get_bit_width())
                        - i64::from(int_type.get_bit_width());

                    if width_diff >= 0 {
                        if is_signed(dtype) {
                            ctx.builder.build_int_s_extend_or_bit_cast(
                                int_value,
                                self.dtype.into_int_type(),
                                "arange_casted",
                            )
                        } else {
                            ctx.builder.build_int_z_extend_or_bit_cast(
                                int_value,
                                self.dtype.into_int_type(),
                                "arange_casted",
                            )
                        }
                    } else {
                        ctx.builder.build_int_truncate(
                            int_value,
                            self.dtype.into_int_type(),
                            "arange_casted",
                        )
                    }
                    .unwrap()
                    .as_basic_value_enum()
                } else if ty == dtype {
                    value
                } else {
                    unreachable!(
                        "Unsupported dtype cast for arange: from {}, to {}",
                        ctx.unifier.stringify(ty),
                        ctx.unifier.stringify(dtype)
                    )
                };

                let p = nditer.get_pointer(ctx);
                ctx.builder.build_store(p, casted).unwrap();
                Ok(())
            })
            .unwrap();

        ndarray
    }
}
