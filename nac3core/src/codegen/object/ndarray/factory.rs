use inkwell::{values::BasicValueEnum, IntPredicate};

use crate::{
    codegen::{
        irrt::call_nac3_ndarray_util_assert_shape_no_negative, model::*, CodeGenContext,
        CodeGenerator,
    },
    typecheck::typedef::Type,
};

use super::NDArrayObject;

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

impl<'ctx> NDArrayObject<'ctx> {
    /// Create an ndarray like `np.empty`.
    pub fn make_np_empty<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        ndims: u64,
        shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> Self {
        // Validate `shape`
        let ndims_llvm = Int(SizeT).const_int(generator, ctx.ctx, ndims);
        call_nac3_ndarray_util_assert_shape_no_negative(generator, ctx, ndims_llvm, shape);

        let ndarray = NDArrayObject::alloca(generator, ctx, dtype, ndims);
        ndarray.copy_shape_from_array(generator, ctx, shape);
        ndarray.create_data(generator, ctx);

        ndarray
    }

    /// Create an ndarray like `np.full`.
    pub fn make_np_full<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        ndims: u64,
        shape: Instance<'ctx, Ptr<Int<SizeT>>>,
        fill_value: BasicValueEnum<'ctx>,
    ) -> Self {
        let ndarray = NDArrayObject::make_np_empty(generator, ctx, dtype, ndims, shape);
        ndarray.fill(generator, ctx, fill_value);
        ndarray
    }

    /// Create an ndarray like `np.zero`.
    pub fn make_np_zeros<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        ndims: u64,
        shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> Self {
        let fill_value = ndarray_zero_value(generator, ctx, dtype);
        NDArrayObject::make_np_full(generator, ctx, dtype, ndims, shape, fill_value)
    }

    /// Create an ndarray like `np.ones`.
    pub fn make_np_ones<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        ndims: u64,
        shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> Self {
        let fill_value = ndarray_one_value(generator, ctx, dtype);
        NDArrayObject::make_np_full(generator, ctx, dtype, ndims, shape, fill_value)
    }

    /// Create an ndarray like `np.eye`.
    pub fn make_np_eye<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        nrows: Instance<'ctx, Int<SizeT>>,
        ncols: Instance<'ctx, Int<SizeT>>,
        offset: Instance<'ctx, Int<SizeT>>,
    ) -> Self {
        let ndzero = ndarray_zero_value(generator, ctx, dtype);
        let ndone = ndarray_one_value(generator, ctx, dtype);

        let ndarray = NDArrayObject::alloca_dynamic_shape(generator, ctx, dtype, &[nrows, ncols]);

        // Create data and make the matrix like look np.eye()
        ndarray.create_data(generator, ctx);
        ndarray
            .foreach(generator, ctx, |generator, ctx, _hooks, nditer| {
                // NOTE: rows and cols can never be zero here, since this ndarray's `np.size` would be zero
                // and this loop would not execute.

                // Load up `row_i` and `col_i` from indices.
                let row_i = nditer.get_indices().get_index_const(generator, ctx, 0);
                let col_i = nditer.get_indices().get_index_const(generator, ctx, 1);

                let be_one = row_i.add(ctx, offset).compare(ctx, IntPredicate::EQ, col_i);
                let value = ctx.builder.build_select(be_one.value, ndone, ndzero, "value").unwrap();

                let p = nditer.get_pointer(generator, ctx);
                ctx.builder.build_store(p, value).unwrap();

                Ok(())
            })
            .unwrap();

        ndarray
    }

    /// Create an ndarray like `np.identity`.
    pub fn make_np_identity<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        size: Instance<'ctx, Int<SizeT>>,
    ) -> Self {
        // Convenient implementation
        let offset = Int(SizeT).const_0(generator, ctx.ctx);
        NDArrayObject::make_np_eye(generator, ctx, dtype, size, size, offset)
    }
}
