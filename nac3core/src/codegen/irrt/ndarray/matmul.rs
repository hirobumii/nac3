use inkwell::values::IntValue;

use crate::codegen::{
    CodeGenContext, CodeGenerator, expr::call_extern, irrt::get_usize_dependent_function_name,
    values::TypedArrayLikeAccessor,
};

/// Generates a call to `__nac3_ndarray_matmul_calculate_shapes`.
///
/// Calculates the broadcasted shapes for `a`, `b`, and the `ndarray` holding the final values of
/// `a @ b`.
#[allow(clippy::too_many_arguments)]
pub fn call_nac3_ndarray_matmul_calculate_shapes<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    a_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    b_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    final_ndims: IntValue<'ctx>,
    new_a_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    new_b_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    dst_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.get_size_type();

    assert_eq!(a_shape.element_type(ctx, generator), llvm_usize.into());
    assert_eq!(b_shape.element_type(ctx, generator), llvm_usize.into());
    assert_eq!(final_ndims.get_type(), llvm_usize);
    assert_eq!(new_a_shape.element_type(ctx, generator), llvm_usize.into());
    assert_eq!(new_b_shape.element_type(ctx, generator), llvm_usize.into());
    assert_eq!(dst_shape.element_type(ctx, generator), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_matmul_calculate_shapes");
    call_extern!(ctx: void _ = name(
        a_shape.size(ctx, generator),
        a_shape.base_ptr(ctx, generator),
        b_shape.size(ctx, generator),
        b_shape.base_ptr(ctx, generator),
        final_ndims,
        new_a_shape.base_ptr(ctx, generator),
        new_b_shape.base_ptr(ctx, generator),
        dst_shape.base_ptr(ctx, generator),
    ));
}
