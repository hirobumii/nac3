use inkwell::values::IntValue;

use crate::codegen::{
    CodeGenContext, expr::call_extern, irrt::get_usize_dependent_function_name,
    values::TypedArrayLikeAccessor,
};

/// Generates a call to `__nac3_ndarray_matmul_calculate_shapes`.
///
/// Calculates the broadcasted shapes for `a`, `b`, and the `ndarray` holding the final values of
/// `a @ b`.
#[allow(clippy::too_many_arguments)]
pub fn call_nac3_ndarray_matmul_calculate_shapes<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    a_shape: &impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
    b_shape: &impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
    final_ndims: IntValue<'ctx>,
    new_a_shape: &impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
    new_b_shape: &impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
    dst_shape: &impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
) {
    let llvm_usize = ctx.size_t;

    assert_eq!(a_shape.element_type(ctx), llvm_usize.into());
    assert_eq!(b_shape.element_type(ctx), llvm_usize.into());
    assert_eq!(final_ndims.get_type(), llvm_usize);
    assert_eq!(new_a_shape.element_type(ctx), llvm_usize.into());
    assert_eq!(new_b_shape.element_type(ctx), llvm_usize.into());
    assert_eq!(dst_shape.element_type(ctx), llvm_usize.into());

    let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_matmul_calculate_shapes");
    call_extern!(ctx: void _ = name(
        a_shape.size(ctx),
        a_shape.base_ptr(ctx),
        b_shape.size(ctx),
        b_shape.base_ptr(ctx),
        final_ndims,
        new_a_shape.base_ptr(ctx),
        new_b_shape.base_ptr(ctx),
        dst_shape.base_ptr(ctx),
    ));
}
