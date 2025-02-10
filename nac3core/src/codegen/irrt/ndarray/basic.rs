use inkwell::{
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace,
};

use crate::codegen::{
    expr::infer_and_call_function,
    irrt::get_usize_dependent_function_name,
    values::{ndarray::NDArrayValue, ProxyValue, TypedArrayLikeAccessor},
    CodeGenContext, CodeGenerator,
};

/// Generates a call to `__nac3_ndarray_util_assert_shape_no_negative`.
///
/// Assets that `shape` does not contain negative dimensions.
pub fn call_nac3_ndarray_util_assert_shape_no_negative<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.get_size_type();

    assert_eq!(shape.element_type(ctx, generator), llvm_usize.into());

    let name =
        get_usize_dependent_function_name(ctx, "__nac3_ndarray_util_assert_shape_no_negative");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[shape.size(ctx, generator).into(), shape.base_ptr(ctx, generator).into()],
        None,
        None,
    );
}

/// Generates a call to `__nac3_ndarray_util_assert_shape_output_shape_same`.
///
/// Asserts that `ndarray_shape` and `output_shape` are the same in the context of writing output to
/// an `ndarray`.
pub fn call_nac3_ndarray_util_assert_output_shape_same<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    output_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.get_size_type();

    assert_eq!(ndarray_shape.element_type(ctx, generator), llvm_usize.into());
    assert_eq!(output_shape.element_type(ctx, generator), llvm_usize.into());

    let name =
        get_usize_dependent_function_name(ctx, "__nac3_ndarray_util_assert_output_shape_same");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[
            ndarray_shape.size(ctx, generator).into(),
            ndarray_shape.base_ptr(ctx, generator).into(),
            output_shape.size(ctx, generator).into(),
            output_shape.base_ptr(ctx, generator).into(),
        ],
        None,
        None,
    );
}

/// Generates a call to `__nac3_ndarray_size`.
///
/// Returns a [`usize`][CodeGenerator::get_size_type] value of the number of elements of an
/// `ndarray`, corresponding to the value of `ndarray.size`.
pub fn call_nac3_ndarray_size<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = ctx.get_size_type();

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_size");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[ndarray.as_abi_value(ctx).into()],
        Some("size"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

/// Generates a call to `__nac3_ndarray_nbytes`.
///
/// Returns a [`usize`][CodeGenerator::get_size_type] value of the number of bytes consumed by the
/// data of the `ndarray`, corresponding to the value of `ndarray.nbytes`.
pub fn call_nac3_ndarray_nbytes<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = ctx.get_size_type();

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_nbytes");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[ndarray.as_abi_value(ctx).into()],
        Some("nbytes"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

/// Generates a call to `__nac3_ndarray_len`.
///
/// Returns a [`usize`][CodeGenerator::get_size_type] value of the size of the topmost dimension of
/// the `ndarray`, corresponding to the value of `ndarray.__len__`.
pub fn call_nac3_ndarray_len<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = ctx.get_size_type();

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_len");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[ndarray.as_abi_value(ctx).into()],
        Some("len"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

/// Generates a call to `__nac3_ndarray_is_c_contiguous`.
///
/// Returns an `i1` value indicating whether the `ndarray` is C-contiguous.
pub fn call_nac3_ndarray_is_c_contiguous<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_is_c_contiguous");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_i1.into()),
        &[ndarray.as_abi_value(ctx).into()],
        Some("is_c_contiguous"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

/// Generates a call to `__nac3_ndarray_get_nth_pelement`.
///
/// Returns a [`PointerValue`] to the `index`-th flattened element of the `ndarray`.
pub fn call_nac3_ndarray_get_nth_pelement<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    index: IntValue<'ctx>,
) -> PointerValue<'ctx> {
    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());
    let llvm_usize = ctx.get_size_type();

    assert_eq!(index.get_type(), llvm_usize);

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_get_nth_pelement");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_pi8.into()),
        &[ndarray.as_abi_value(ctx).into(), index.into()],
        Some("pelement"),
        None,
    )
    .map(BasicValueEnum::into_pointer_value)
    .unwrap()
}

/// Generates a call to `__nac3_ndarray_get_pelement_by_indices`.
///
/// `indices` must have the same number of elements as the number of dimensions in `ndarray`.
///
/// Returns a [`PointerValue`] to the element indexed by `indices`.
pub fn call_nac3_ndarray_get_pelement_by_indices<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    indices: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) -> PointerValue<'ctx> {
    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());
    let llvm_usize = ctx.get_size_type();

    assert_eq!(indices.element_type(ctx, generator), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_get_pelement_by_indices");

    infer_and_call_function(
        ctx,
        &name,
        Some(llvm_pi8.into()),
        &[ndarray.as_abi_value(ctx).into(), indices.base_ptr(ctx, generator).into()],
        Some("pelement"),
        None,
    )
    .map(BasicValueEnum::into_pointer_value)
    .unwrap()
}

/// Generates a call to `__nac3_ndarray_set_strides_by_shape`.
///
/// Sets `ndarray.strides` assuming that `ndarray.shape` is C-contiguous.
pub fn call_nac3_ndarray_set_strides_by_shape<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_set_strides_by_shape");

    infer_and_call_function(ctx, &name, None, &[ndarray.as_abi_value(ctx).into()], None, None);
}

/// Generates a call to `__nac3_ndarray_copy_data`.
///
/// Copies all elements from `src_ndarray` to `dst_ndarray` using their flattened views. The number
/// of elements in `src_ndarray` must be greater than or equal to the number of elements in
/// `dst_ndarray`.
pub fn call_nac3_ndarray_copy_data<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_copy_data");

    infer_and_call_function(
        ctx,
        &name,
        None,
        &[src_ndarray.as_abi_value(ctx).into(), dst_ndarray.as_abi_value(ctx).into()],
        None,
        None,
    );
}
