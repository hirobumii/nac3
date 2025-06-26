use inkwell::{types::BasicTypeEnum, values::IntValue};

use crate::codegen::{
    CodeGenContext, CodeGenerator,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    values::{ListValue, ProxyValue, TypedArrayLikeAccessor, ndarray::NDArrayValue},
};

/// Generates a call to `__nac3_ndarray_array_set_and_validate_list_shape`.
///
/// Deduces the target shape of the `ndarray` from the provided `list`, raising an exception if
/// there is any issue with the resultant `shape`.
///
/// `shape` must be pre-allocated by the caller of this function to `[usize; ndims]`, and must be
/// initialized to all `-1`s.
pub fn call_nac3_ndarray_array_set_and_validate_list_shape<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    list: ListValue<'ctx>,
    ndims: IntValue<'ctx>,
    shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.get_size_type();
    assert_eq!(list.get_type().element_type().unwrap(), ctx.ctx.i8_type().into());
    assert_eq!(ndims.get_type(), llvm_usize);
    assert_eq!(
        BasicTypeEnum::try_from(shape.element_type(ctx, generator)).unwrap(),
        llvm_usize.into()
    );

    let name =
        get_usize_dependent_function_name(ctx, "__nac3_ndarray_array_set_and_validate_list_shape");
    call_extern!(ctx: void _ = name(list.as_abi_value(ctx), ndims, shape.base_ptr(ctx, generator)));
}

/// Generates a call to `__nac3_ndarray_array_write_list_to_array`.
///
/// Copies the contents stored in `list` into `ndarray`.
///
/// The `ndarray` must fulfill the following preconditions:
///
/// - `ndarray.itemsize`: Must be initialized.
/// - `ndarray.ndims`: Must be initialized.
/// - `ndarray.shape`: Must be initialized.
/// - `ndarray.data`: Must be allocated and contiguous.
pub fn call_nac3_ndarray_array_write_list_to_array<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    list: ListValue<'ctx>,
    ndarray: NDArrayValue<'ctx>,
) {
    assert_eq!(list.get_type().element_type().unwrap(), ctx.ctx.i8_type().into());
    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_array_write_list_to_array");
    call_extern!(ctx: void _ = name(list.as_abi_value(ctx), ndarray.as_abi_value(ctx)));
}
