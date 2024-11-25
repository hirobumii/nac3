use inkwell::{
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace,
};

use crate::codegen::{
    expr::{create_and_call_function, infer_and_call_function},
    irrt::get_usize_dependent_function_name,
    types::ProxyType,
    values::{ndarray::NDArrayValue, ProxyValue},
    CodeGenContext, CodeGenerator,
};

pub fn call_nac3_ndarray_util_assert_shape_no_negative<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndims: IntValue<'ctx>,
    shape: PointerValue<'ctx>,
) {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let name = get_usize_dependent_function_name(
        generator,
        ctx,
        "__nac3_ndarray_util_assert_shape_no_negative",
    );

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[(llvm_usize.into(), ndims.into()), (llvm_pusize.into(), shape.into())],
        None,
        None,
    );
}

pub fn call_nac3_ndarray_util_assert_output_shape_same<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray_ndims: IntValue<'ctx>,
    ndarray_shape: PointerValue<'ctx>,
    output_ndims: IntValue<'ctx>,
    output_shape: IntValue<'ctx>,
) {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let name = get_usize_dependent_function_name(
        generator,
        ctx,
        "__nac3_ndarray_util_assert_output_shape_same",
    );

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[
            (llvm_usize.into(), ndarray_ndims.into()),
            (llvm_pusize.into(), ndarray_shape.into()),
            (llvm_usize.into(), output_ndims.into()),
            (llvm_pusize.into(), output_shape.into()),
        ],
        None,
        None,
    );
}

pub fn call_nac3_ndarray_size<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_size");

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[(llvm_ndarray.into(), ndarray.as_base_value().into())],
        Some("size"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

pub fn call_nac3_ndarray_nbytes<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_nbytes");

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[(llvm_ndarray.into(), ndarray.as_base_value().into())],
        Some("nbytes"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

pub fn call_nac3_ndarray_len<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_len");

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_usize.into()),
        &[(llvm_ndarray.into(), ndarray.as_base_value().into())],
        Some("len"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

pub fn call_nac3_ndarray_is_c_contiguous<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) -> IntValue<'ctx> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_is_c_contiguous");

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_i1.into()),
        &[(llvm_ndarray.into(), ndarray.as_base_value().into())],
        Some("is_c_contiguous"),
        None,
    )
    .map(BasicValueEnum::into_int_value)
    .unwrap()
}

pub fn call_nac3_ndarray_get_nth_pelement<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    index: IntValue<'ctx>,
) -> PointerValue<'ctx> {
    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_get_nth_pelement");

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_pi8.into()),
        &[(llvm_ndarray.into(), ndarray.as_base_value().into()), (llvm_usize.into(), index.into())],
        Some("pelement"),
        None,
    )
    .map(BasicValueEnum::into_pointer_value)
    .unwrap()
}

pub fn call_nac3_ndarray_get_pelement_by_indices<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
    indices: PointerValue<'ctx>,
) -> PointerValue<'ctx> {
    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name =
        get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_get_pelement_by_indices");

    create_and_call_function(
        ctx,
        &name,
        Some(llvm_pi8.into()),
        &[
            (llvm_ndarray.into(), ndarray.as_base_value().into()),
            (llvm_pusize.into(), indices.into()),
        ],
        Some("pelement"),
        None,
    )
    .map(BasicValueEnum::into_pointer_value)
    .unwrap()
}

pub fn call_nac3_ndarray_set_strides_by_shape<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    ndarray: NDArrayValue<'ctx>,
) {
    let llvm_ndarray = ndarray.get_type().as_base_type();

    let name =
        get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_set_strides_by_shape");

    create_and_call_function(
        ctx,
        &name,
        None,
        &[(llvm_ndarray.into(), ndarray.as_base_value().into())],
        None,
        None,
    );
}

pub fn call_nac3_ndarray_copy_data<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &CodeGenContext<'ctx, '_>,
    src_ndarray: NDArrayValue<'ctx>,
    dst_ndarray: NDArrayValue<'ctx>,
) {
    let name = get_usize_dependent_function_name(generator, ctx, "__nac3_ndarray_copy_data");

    infer_and_call_function(
        ctx,
        &name,
        None,
        &[src_ndarray.as_base_value().into(), dst_ndarray.as_base_value().into()],
        None,
        None,
    );
}
