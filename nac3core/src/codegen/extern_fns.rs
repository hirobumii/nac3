use inkwell::values::{BasicValueEnum, FloatValue};

use super::CodeGenContext;
use crate::codegen::expr::call_extern;

/// Invokes the [`j1`](https://en.cppreference.com/w/c/numeric/math/j1) function.
pub fn call_j1<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    arg: FloatValue<'ctx>,
    name: Option<&str>,
) -> FloatValue<'ctx> {
    let llvm_f64 = ctx.ctx.f64_type();
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
        ) {
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
