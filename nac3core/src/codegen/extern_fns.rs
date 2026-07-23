use inkwell::values::{BasicValueEnum, FloatValue};
#[cfg(all(feature = "malloc", not(feature = "ctrc")))]
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    builder::Builder,
    values::{IntValue, PointerValue},
};

use crate::codegen::{CodeGenContext, expr::call_extern};

/// Invokes the [`malloc`](https://en.cppreference.com/w/c/memory/malloc) function, allocating
/// `size` bytes.
///
/// Inkwell's `build_malloc`/`build_array_malloc` wrap LLVM's C API, which hardcodes an `i32` size
/// argument regardless of the target's pointer width - so on a 64-bit target they emit
/// `malloc(i32 ...)` and any allocation of 4 GiB or more silently wraps. Emitting the call directly
/// keeps the size in `size_t`.
///
/// Note: This function manually builds the call to `malloc` because `call_extern!` requires
/// `&mut CodeGenContext`, which cannot be satifisied when the `CodeGenContext` is already borrowed
/// by the `Builder`.
#[cfg(all(feature = "malloc", not(feature = "ctrc")))]
pub fn call_malloc<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    builder: &Builder<'ctx>,
    size: IntValue<'ctx>,
    name: &str,
) -> anyhow::Result<PointerValue<'ctx>> {
    const FUNC_NAME: &str = "malloc";

    let f = ctx.module.get_function(FUNC_NAME).unwrap_or_else(|| {
        ctx.module.add_function(FUNC_NAME, ctx.ptr.fn_type(&[ctx.size_t.into()], false), None)
    });
    // Apply the `noalias` attribute to the return value of `malloc` on every resolution as IRRT
    // may already have declared `malloc` when this module was set up. Reapplying the attribute is
    // idempotent as `noalias` is a set-attribute.
    f.add_attribute(
        AttributeLoc::Return,
        ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("noalias"), 0),
    );
    Ok(builder
        .build_call(f, &[size.into()], name)?
        .try_as_basic_value()
        .basic()
        .map(BasicValueEnum::into_pointer_value)
        .unwrap())
}

/// Invokes the [`j1`](https://en.cppreference.com/w/c/numeric/math/j1) function.
pub fn call_j1<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> anyhow::Result<FloatValue<'ctx>> {
    let llvm_f64 = ctx.f64;
    debug_assert_eq!(arg.get_type(), llvm_f64);
    call_extern!(ctx: llvm_f64 name? = ["nounwind"] "j1"(arg))
}

/// Macro to generate `np_linalg` and `sp_linalg` functions
/// The function takes as input `NDArray` and returns ()
///
/// Arguments:
/// * `$fn_name:ident`: The identifier of the rust function to be generated
/// * `$extern_fn:ident`: Name of underlying extern function
/// * (2/3/4): Number of `NDArray` that function takes as input
///
/// Note:
/// The operands and resulting `NDArray` are both passed as input to the funcion
/// It is the responsibility of caller to ensure that output `NDArray` is properly allocated on stack
/// The function changes the content of the output `NDArray` in-place
macro_rules! generate_linalg_extern_fn {
    ($fn_name:ident, $extern_fn:ident, 2) => {
        generate_linalg_extern_fn!($fn_name, $extern_fn, mat1, mat2);
    };
    ($fn_name:ident, $extern_fn:ident, 3) => {
        generate_linalg_extern_fn!($fn_name, $extern_fn, mat1, mat2, mat3);
    };
    ($fn_name:ident, $extern_fn:ident, 4) => {
        generate_linalg_extern_fn!($fn_name, $extern_fn, mat1, mat2, mat3, mat4);
    };
    ($fn_name:ident, $extern_fn:ident $(,$input_matrix:ident)*) => {
        #[doc = concat!("Invokes the linalg `", stringify!($extern_fn), "` function." )]
        pub fn $fn_name<'ctx>(
            ctx: &mut CodeGenContext<'ctx, '_>,
            $($input_matrix: BasicValueEnum<'ctx>,)*
            name: Option<&str>,
        ) -> anyhow::Result<()> {
            call_extern!(ctx: void name? = ["nounwind"] (stringify!($extern_fn))($($input_matrix),*))
        }
    };
}

generate_linalg_extern_fn!(call_np_linalg_cholesky, np_linalg_cholesky, 2);
generate_linalg_extern_fn!(call_np_linalg_qr, np_linalg_qr, 3);
generate_linalg_extern_fn!(call_np_linalg_svd, np_linalg_svd, 4);
generate_linalg_extern_fn!(call_np_linalg_inv, np_linalg_inv, 2);
generate_linalg_extern_fn!(call_np_linalg_pinv, np_linalg_pinv, 2);
generate_linalg_extern_fn!(call_np_linalg_matrix_power, np_linalg_matrix_power, 3);
generate_linalg_extern_fn!(call_np_linalg_det, np_linalg_det, 2);
generate_linalg_extern_fn!(call_sp_linalg_lu, sp_linalg_lu, 3);
generate_linalg_extern_fn!(call_sp_linalg_schur, sp_linalg_schur, 3);
generate_linalg_extern_fn!(call_sp_linalg_hessenberg, sp_linalg_hessenberg, 3);
