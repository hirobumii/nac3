use inkwell::{
    attributes::{Attribute, AttributeLoc},
    values::{BasicValueEnum, FloatValue},
};

use super::{expr::infer_and_call_function, CodeGenContext};

/// Macro to generate extern function
/// Both function return type and function parameter type are `FloatValue`
///
/// Arguments:
/// * `unary/binary`: Whether the extern function requires one (unary) or two (binary) operands
/// * `$fn_name:ident`: The identifier of the rust function to be generated
/// * `$extern_fn:literal`: Name of underlying extern function
///
/// Optional Arguments:
/// * `$(,$attributes:literal)*)`: Attributes linked with the extern function.
///   The default attributes are "mustprogress", "nofree", "nounwind", "willreturn", and "writeonly".
///   These will be used unless other attributes are specified
/// * `$(,$args:ident)*`: Operands of the extern function
///   The data type of these operands will be set to `FloatValue`
///  
macro_rules! generate_extern_fn {
    ("unary", $fn_name:ident, $extern_fn:literal) => {
        generate_extern_fn!($fn_name, $extern_fn, arg, "mustprogress", "nofree", "nounwind", "willreturn", "writeonly");
    };
    ("unary", $fn_name:ident, $extern_fn:literal $(,$attributes:literal)*) => {
        generate_extern_fn!($fn_name, $extern_fn, arg $(,$attributes)*);
    };
    ("binary", $fn_name:ident, $extern_fn:literal) => {
        generate_extern_fn!($fn_name, $extern_fn, arg1, arg2, "mustprogress", "nofree", "nounwind", "willreturn", "writeonly");
    };
    ("binary", $fn_name:ident, $extern_fn:literal $(,$attributes:literal)*) => {
        generate_extern_fn!($fn_name, $extern_fn, arg1, arg2 $(,$attributes)*);
    };
    ($fn_name:ident, $extern_fn:literal $(,$args:ident)* $(,$attributes:literal)*) => {
        #[doc = concat!("Invokes the [`", stringify!($extern_fn), "`](https://en.cppreference.com/w/c/numeric/math/", stringify!($llvm_name), ") function." )]
        pub fn $fn_name<'ctx>(
            ctx: &CodeGenContext<'ctx, '_>,
            $($args: FloatValue<'ctx>,)*
            name: Option<&str>,
        ) -> FloatValue<'ctx> {
            const FN_NAME: &str = $extern_fn;

            let llvm_f64 = ctx.ctx.f64_type();
            $(debug_assert_eq!($args.get_type(), llvm_f64);)*

            infer_and_call_function(
                ctx,
                FN_NAME,
                Some(llvm_f64.into()),
                &[$($args.into()),*],
                name,
                Some(&|func| {
                   for attr in [$($attributes),*] {
                        func.add_attribute(
                            AttributeLoc::Function,
                            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
                        );
                    }
                })
            )
            .map(BasicValueEnum::into_float_value)
            .unwrap()
        }
    };
}

generate_extern_fn!("unary", call_j1, "j1", "nounwind");

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
            const FN_NAME: &str = stringify!($extern_fn);

            infer_and_call_function(
                ctx,
                FN_NAME,
                None,
                &[$($input_matrix.into(),)*],
                name,
                Some(&|func| {
                    func.add_attribute(
                        AttributeLoc::Function,
                        ctx.ctx.create_enum_attribute(
                            Attribute::get_named_enum_kind_id("nounwind"),
                            0,
                        ),
                    )
                }),
            );
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
