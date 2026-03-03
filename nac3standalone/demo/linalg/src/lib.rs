//! Uses `nalgebra` crate to invoke `np_linalg` and `sp_linalg` functions
//! When converting between `nalgebra::Matrix` and `NDArray` following considerations are necessary
//!
//! * Both `nalgebra::Matrix` and `NDArray` require their content to be stored in row-major order
//! * `NDArray` data pointer can be directly read and converted to `nalgebra::Matrix` (row and column number must be known)
//! * `nalgebra::Matrix::as_slice` returns the content of matrix in column-major order and initial data needs to be transposed before storing it in `NDArray` data pointer

#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::slice;

use nalgebra::DMatrix;

fn report_error(
    error_name: &str,
    fn_name: &str,
    file_name: &str,
    line_num: u32,
    col_num: u32,
    err_msg: &str,
) -> ! {
    panic!(
        "Exception {error_name} from {fn_name} in {file_name}:{line_num}:{col_num}, message: {err_msg}"
    );
}

pub struct InputMatrix {
    pub ndims: usize,
    pub dims: *const usize,
    pub data: *mut f64,
    pub base: *mut u8,
    pub offset: isize,
}

impl InputMatrix {
    fn get_dims(&mut self) -> Vec<usize> {
        let dims = unsafe { slice::from_raw_parts(self.dims, self.ndims) };
        dims.to_vec()
    }
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_cholesky(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out = unsafe { out.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "np_linalg_cholesky", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();
    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        report_error("LinAlgError", "np_linalg_cholesky", file!(), line!(), column!(), &err_msg);
    }

    let outdim = out.get_dims();
    let out_slice = unsafe {
        slice::from_raw_parts_mut(out.data.byte_offset(out.offset), outdim[0] * outdim[1])
    };
    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };

    let matrix1 = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let result = matrix1.cholesky();
    match result {
        Some(res) => {
            out_slice.copy_from_slice(res.unpack().transpose().as_slice());
        }
        None => {
            report_error(
                "LinAlgError",
                "np_linalg_cholesky",
                file!(),
                line!(),
                column!(),
                "Matrix is not positive definite",
            );
        }
    }
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_qr(
    mat1: *mut InputMatrix,
    out_q: *mut InputMatrix,
    out_r: *mut InputMatrix,
) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out_q = unsafe { out_q.as_mut().unwrap() };
    let out_r = unsafe { out_r.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "np_linalg_qr", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let outq_dim = (*out_q).get_dims();
    let outr_dim = (*out_r).get_dims();

    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };
    let out_q_slice = unsafe {
        slice::from_raw_parts_mut(out_q.data.byte_offset(out_q.offset), outq_dim[0] * outq_dim[1])
    };
    let out_r_slice = unsafe {
        slice::from_raw_parts_mut(out_r.data.byte_offset(out_r.offset), outr_dim[0] * outr_dim[1])
    };

    // Refer to https://github.com/dimforge/nalgebra/issues/735
    let matrix1 = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);

    let res = matrix1.qr();
    let (q, r) = res.unpack();

    // Uses different algo need to match numpy
    out_q_slice.copy_from_slice(q.transpose().as_slice());
    out_r_slice.copy_from_slice(r.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_svd(
    mat1: *mut InputMatrix,
    outu: *mut InputMatrix,
    outs: *mut InputMatrix,
    outvh: *mut InputMatrix,
) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let outu = unsafe { outu.as_mut().unwrap() };
    let outs = unsafe { outs.as_mut().unwrap() };
    let outvh = unsafe { outvh.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "np_linalg_svd", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let outu_dim = (*outu).get_dims();
    let outs_dim = (*outs).get_dims();
    let outvh_dim = (*outvh).get_dims();

    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };
    let out_u_slice = unsafe {
        slice::from_raw_parts_mut(outu.data.byte_offset(outu.offset), outu_dim[0] * outu_dim[1])
    };
    let out_s_slice =
        unsafe { slice::from_raw_parts_mut(outs.data.byte_offset(outs.offset), outs_dim[0]) };
    let out_vh_slice = unsafe {
        slice::from_raw_parts_mut(outvh.data.byte_offset(outvh.offset), outvh_dim[0] * outvh_dim[1])
    };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let result = matrix.svd(true, true);
    out_u_slice.copy_from_slice(result.u.unwrap().transpose().as_slice());
    out_s_slice.copy_from_slice(result.singular_values.as_slice());
    out_vh_slice.copy_from_slice(result.v_t.unwrap().transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_inv(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out = unsafe { out.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "np_linalg_inv", file!(), line!(), column!(), &err_msg);
    }
    let dim1 = (*mat1).get_dims();

    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        report_error("LinAlgError", "np_linalg_inv", file!(), line!(), column!(), &err_msg);
    }

    let outdim = out.get_dims();
    let out_slice = unsafe {
        slice::from_raw_parts_mut(out.data.byte_offset(out.offset), outdim[0] * outdim[1])
    };
    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    if !matrix.is_invertible() {
        report_error(
            "LinAlgError",
            "np_linalg_inv",
            file!(),
            line!(),
            column!(),
            "no inverse for Singular Matrix",
        );
    }
    let inv = matrix.try_inverse().unwrap();
    out_slice.copy_from_slice(inv.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_pinv(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out = unsafe { out.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "np_linalg_pinv", file!(), line!(), column!(), &err_msg);
    }
    let dim1 = (*mat1).get_dims();
    let outdim = out.get_dims();
    let out_slice = unsafe {
        slice::from_raw_parts_mut(out.data.byte_offset(out.offset), outdim[0] * outdim[1])
    };
    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let svd = matrix.svd(true, true);
    let inv = svd.pseudo_inverse(1e-15);

    match inv {
        Ok(m) => {
            out_slice.copy_from_slice(m.transpose().as_slice());
        }
        Err(err_msg) => {
            report_error("LinAlgError", "np_linalg_pinv", file!(), line!(), column!(), err_msg);
        }
    }
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_matrix_power(
    mat1: *mut InputMatrix,
    mat2: *mut InputMatrix,
    out: *mut InputMatrix,
) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let mat2 = unsafe { mat2.as_mut().unwrap() };
    let out = unsafe { out.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D", mat1.ndims);
        report_error("ValueError", "np_linalg_matrix_power", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let power = unsafe { slice::from_raw_parts_mut(mat2.data.byte_offset(mat2.offset), 1) };
    let power = power[0];
    let outdim = out.get_dims();
    let out_slice = unsafe {
        slice::from_raw_parts_mut(out.data.byte_offset(out.offset), outdim[0] * outdim[1])
    };
    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };

    let abs_pow = power.abs();
    let matrix1 = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let mut result = matrix1.pow(abs_pow as u32);

    if power < 0.0 {
        if !result.is_invertible() {
            report_error(
                "LinAlgError",
                "np_linalg_inv",
                file!(),
                line!(),
                column!(),
                "no inverse for Singular Matrix",
            );
        }
        result = result.try_inverse().unwrap();
    }
    out_slice.copy_from_slice(result.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn np_linalg_det(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out = unsafe { out.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "np_linalg_det", file!(), line!(), column!(), &err_msg);
    }
    let dim1 = (*mat1).get_dims();
    let out_slice = unsafe { slice::from_raw_parts_mut(out.data.byte_offset(out.offset), 1) };
    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    if !matrix.is_square() {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        report_error("LinAlgError", "np_linalg_inv", file!(), line!(), column!(), &err_msg);
    }
    out_slice[0] = matrix.determinant();
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sp_linalg_lu(
    mat1: *mut InputMatrix,
    out_l: *mut InputMatrix,
    out_u: *mut InputMatrix,
) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out_l = unsafe { out_l.as_mut().unwrap() };
    let out_u = unsafe { out_u.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "sp_linalg_lu", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let outl_dim = (*out_l).get_dims();
    let outu_dim = (*out_u).get_dims();

    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };
    let out_l_slice = unsafe {
        slice::from_raw_parts_mut(out_l.data.byte_offset(out_l.offset), outl_dim[0] * outl_dim[1])
    };
    let out_u_slice = unsafe {
        slice::from_raw_parts_mut(out_u.data.byte_offset(out_u.offset), outu_dim[0] * outu_dim[1])
    };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let (_, l, u) = matrix.lu().unpack();

    out_l_slice.copy_from_slice(l.transpose().as_slice());
    out_u_slice.copy_from_slice(u.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sp_linalg_schur(
    mat1: *mut InputMatrix,
    out_t: *mut InputMatrix,
    out_z: *mut InputMatrix,
) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out_t = unsafe { out_t.as_mut().unwrap() };
    let out_z = unsafe { out_z.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "sp_linalg_schur", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();

    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        report_error("LinAlgError", "np_linalg_schur", file!(), line!(), column!(), &err_msg);
    }

    let out_t_dim = (*out_t).get_dims();
    let out_z_dim = (*out_z).get_dims();

    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };
    let out_t_slice = unsafe {
        slice::from_raw_parts_mut(out_t.data.byte_offset(out_t.offset), out_t_dim[0] * out_t_dim[1])
    };
    let out_z_slice = unsafe {
        slice::from_raw_parts_mut(out_z.data.byte_offset(out_z.offset), out_z_dim[0] * out_z_dim[1])
    };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let (z, t) = matrix.schur().unpack();

    out_t_slice.copy_from_slice(t.transpose().as_slice());
    out_z_slice.copy_from_slice(z.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sp_linalg_hessenberg(
    mat1: *mut InputMatrix,
    out_h: *mut InputMatrix,
    out_q: *mut InputMatrix,
) {
    let mat1 = unsafe { mat1.as_mut().unwrap() };
    let out_h = unsafe { out_h.as_mut().unwrap() };
    let out_q = unsafe { out_q.as_mut().unwrap() };

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        report_error("ValueError", "sp_linalg_hessenberg", file!(), line!(), column!(), &err_msg);
    }

    let dim1 = (*mat1).get_dims();

    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {} != {}", dim1[0], dim1[1]);
        report_error("LinAlgError", "sp_linalg_hessenberg", file!(), line!(), column!(), &err_msg);
    }

    let out_h_dim = (*out_h).get_dims();
    let out_q_dim = (*out_q).get_dims();

    let data_slice1 =
        unsafe { slice::from_raw_parts_mut(mat1.data.byte_offset(mat1.offset), dim1[0] * dim1[1]) };
    let out_h_slice = unsafe {
        slice::from_raw_parts_mut(out_h.data.byte_offset(out_h.offset), out_h_dim[0] * out_h_dim[1])
    };
    let out_q_slice = unsafe {
        slice::from_raw_parts_mut(out_q.data.byte_offset(out_q.offset), out_q_dim[0] * out_q_dim[1])
    };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let (q, h) = matrix.hessenberg().unpack();

    out_h_slice.copy_from_slice(h.transpose().as_slice());
    out_q_slice.copy_from_slice(q.transpose().as_slice());
}
