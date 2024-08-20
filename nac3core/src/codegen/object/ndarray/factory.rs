use inkwell::values::BasicValueEnum;

use super::NDArrayObject;
use crate::{
    codegen::{
        irrt::call_nac3_ndarray_util_assert_shape_no_negative, model::*, CodeGenContext,
        CodeGenerator,
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
        let ndims_llvm = Int(SizeT).const_int(generator, ctx.ctx, ndims, false);
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
}
