/// Uses `nalgebra` crate to invoke `np_linalg` and `sp_linalg` functions
/// When converting between `nalgebra::Matrix` and `NDArray` following considerations are necessary
///
/// * Both `nalgebra::Matrix` and `NDArray` require their content to be stored in row-major order
/// * `NDArray` data pointer can be directly read and converted to `nalgebra::Matrix` (row and column number must be known)
/// * `nalgebra::Matrix::as_slice` returns the content of matrix in column-major order and initial data needs to be transposed before storing it in `NDArray` data pointer
mod runtime_exception;
use core::slice;
use nalgebra::DMatrix;

macro_rules! raise_exn {
    ($name:expr, $fn_name:expr, $message:expr, $param0:expr, $param1:expr, $param2:expr) => {{
        use cslice::AsCSlice;
        let name_id = $crate::runtime_exception::get_exception_id($name);
        let exn = $crate::runtime_exception::Exception {
            id: name_id,
            file: file!().as_c_slice(),
            line: line!(),
            column: column!(),
            // https://github.com/rust-lang/rfcs/pull/1719
            function: $fn_name.as_c_slice(),
            message: $message.as_c_slice(),
            param: [$param0, $param1, $param2],
        };
        #[allow(unused_unsafe)]
        unsafe {
            $crate::runtime_exception::raise(&exn)
        }
    }};
    ($name:expr, $fn_name:expr, $message:expr) => {{
        raise_exn!($name, $fn_name, $message, 0, 0, 0)
    }};
}
pub struct InputMatrix {
    pub ndims: usize,
    pub dims: *const usize,
    pub data: *mut f64,
}
impl InputMatrix {
    fn get_dims(&mut self) -> Vec<usize> {
        let dims = unsafe { slice::from_raw_parts(self.dims, self.ndims) };
        dims.to_vec()
    }
}

/// # Safety
///
/// `mat1` and `mat2` should point to a valid 1DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn np_dot(mat1: *mut InputMatrix, mat2: *mut InputMatrix) -> f64 {
    let mat1 = mat1.as_mut().unwrap();
    let mat2 = mat2.as_mut().unwrap();

    if !(mat1.ndims == 1 && mat2.ndims == 1) {
        let err_msg = format!(
            "expected 1D Vector Input, but received {}D and {}D input",
            mat1.ndims, mat2.ndims
        );
        raise_exn!("ValueError", "np_dot", err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let dim2 = (*mat2).get_dims();

    if dim1[0] != dim2[0] {
        let err_msg = format!("shapes ({},) and ({},) not aligned", dim1[0], dim2[0]);
        raise_exn!("ValueError", "np_dot", err_msg);
    }
    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0]) };
    let data_slice2 = unsafe { slice::from_raw_parts_mut(mat2.data, dim2[0]) };

    let matrix1 = DMatrix::from_row_slice(dim1[0], 1, data_slice1);
    let matrix2 = DMatrix::from_row_slice(dim2[0], 1, data_slice2);

    matrix1.dot(&matrix2)
}

/// # Safety
///
/// `mat1` and `mat2` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn np_linalg_matmul(
    mat1: *mut InputMatrix,
    mat2: *mut InputMatrix,
    out: *mut InputMatrix,
) {
    let mat1 = mat1.as_mut().unwrap();
    let mat2 = mat2.as_mut().unwrap();
    let out = out.as_mut().unwrap();

    if !(mat1.ndims == 2 && mat2.ndims == 2) {
        let err_msg = format!(
            "expected 2D Vector Input, but received {}D and {}D input",
            mat1.ndims, mat2.ndims
        );
        raise_exn!("ValueError", "np_matmul", err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let dim2 = (*mat2).get_dims();

    if dim1[1] != dim2[0] {
        let err_msg = format!(
            "shapes ({},{}) and ({},{}) not aligned: {} (dim 1) != {} (dim 0)",
            dim1[0], dim1[1], dim2[0], dim2[1], dim1[1], dim2[0]
        );
        raise_exn!("ValueError", "np_matmul", err_msg);
    }

    let outdim = out.get_dims();
    let out_slice = unsafe { slice::from_raw_parts_mut(out.data, outdim[0] * outdim[1]) };
    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };
    let data_slice2 = unsafe { slice::from_raw_parts_mut(mat2.data, dim2[0] * dim2[1]) };

    let matrix1 = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let matrix2 = DMatrix::from_row_slice(dim2[0], dim2[1], data_slice2);
    let mut result = DMatrix::<f64>::zeros(outdim[0], outdim[1]);

    matrix1.mul_to(&matrix2, &mut result);
    out_slice.copy_from_slice(result.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn np_linalg_cholesky(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = mat1.as_mut().unwrap();
    let out = out.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "np_linalg_cholesky", err_msg);
    }

    let dim1 = (*mat1).get_dims();
    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        raise_exn!("LinAlgError", "np_linalg_cholesky", err_msg);
    }

    let outdim = out.get_dims();
    let out_slice = unsafe { slice::from_raw_parts_mut(out.data, outdim[0] * outdim[1]) };
    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };

    let matrix1 = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let result = matrix1.cholesky();
    match result {
        Some(res) => {
            out_slice.copy_from_slice(res.unpack().transpose().as_slice());
        }
        None => {
            raise_exn!("LinAlgError", "np_linalg_cholesky", "Matrix is not positive definite");
        }
    };
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn np_linalg_qr(
    mat1: *mut InputMatrix,
    out_q: *mut InputMatrix,
    out_r: *mut InputMatrix,
) {
    let mat1 = mat1.as_mut().unwrap();
    let out_q = out_q.as_mut().unwrap();
    let out_r = out_r.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "np_linalg_cholesky", err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let outq_dim = (*out_q).get_dims();
    let outr_dim = (*out_r).get_dims();

    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };
    let out_q_slice = unsafe { slice::from_raw_parts_mut(out_q.data, outq_dim[0] * outq_dim[1]) };
    let out_r_slice = unsafe { slice::from_raw_parts_mut(out_r.data, outr_dim[0] * outr_dim[1]) };

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
#[no_mangle]
pub unsafe extern "C" fn np_linalg_svd(
    mat1: *mut InputMatrix,
    outu: *mut InputMatrix,
    outs: *mut InputMatrix,
    outvh: *mut InputMatrix,
) {
    let mat1 = mat1.as_mut().unwrap();
    let outu = outu.as_mut().unwrap();
    let outs = outs.as_mut().unwrap();
    let outvh = outvh.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "np_linalg_svd", err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let outu_dim = (*outu).get_dims();
    let outs_dim = (*outs).get_dims();
    let outvh_dim = (*outvh).get_dims();

    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };
    let out_u_slice = unsafe { slice::from_raw_parts_mut(outu.data, outu_dim[0] * outu_dim[1]) };
    let out_s_slice = unsafe { slice::from_raw_parts_mut(outs.data, outs_dim[0]) };
    let out_vh_slice =
        unsafe { slice::from_raw_parts_mut(outvh.data, outvh_dim[0] * outvh_dim[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let result = matrix.svd(true, true);
    out_u_slice.copy_from_slice(result.u.unwrap().transpose().as_slice());
    out_s_slice.copy_from_slice(result.singular_values.as_slice());
    out_vh_slice.copy_from_slice(result.v_t.unwrap().transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn np_linalg_inv(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = mat1.as_mut().unwrap();
    let out = out.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "np_linalg_inv", err_msg);
    }
    let dim1 = (*mat1).get_dims();

    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        raise_exn!("LinAlgError", "np_linalg_inv", err_msg);
    }

    let outdim = out.get_dims();
    let out_slice = unsafe { slice::from_raw_parts_mut(out.data, outdim[0] * outdim[1]) };
    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    if !matrix.is_invertible() {
        raise_exn!("LinAlgError", "np_linalg_inv", "no inverse for Singular Matrix");
    }
    let inv = matrix.try_inverse().unwrap();
    out_slice.copy_from_slice(inv.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn np_linalg_pinv(mat1: *mut InputMatrix, out: *mut InputMatrix) {
    let mat1 = mat1.as_mut().unwrap();
    let out = out.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "np_linalg_pinv", err_msg);
    }
    let dim1 = (*mat1).get_dims();
    let outdim = out.get_dims();
    let out_slice = unsafe { slice::from_raw_parts_mut(out.data, outdim[0] * outdim[1]) };
    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let svd = matrix.svd(true, true);
    let inv = svd.pseudo_inverse(1e-15);

    match inv {
        Ok(m) => {
            out_slice.copy_from_slice(m.transpose().as_slice());
        }
        Err(e) => {
            raise_exn!("LinAlgError", "np_linalg_pinv", e);
        }
    }
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn sp_linalg_lu(
    mat1: *mut InputMatrix,
    out_l: *mut InputMatrix,
    out_u: *mut InputMatrix,
) {
    let mat1 = mat1.as_mut().unwrap();
    let out_l = out_l.as_mut().unwrap();
    let out_u = out_u.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "sp_linalg_lu", err_msg);
    }

    let dim1 = (*mat1).get_dims();
    let outl_dim = (*out_l).get_dims();
    let outu_dim = (*out_u).get_dims();

    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };
    let out_l_slice = unsafe { slice::from_raw_parts_mut(out_l.data, outl_dim[0] * outl_dim[1]) };
    let out_u_slice = unsafe { slice::from_raw_parts_mut(out_u.data, outu_dim[0] * outu_dim[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let (_, l, u) = matrix.lu().unpack();

    out_l_slice.copy_from_slice(l.transpose().as_slice());
    out_u_slice.copy_from_slice(u.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn sp_linalg_schur(
    mat1: *mut InputMatrix,
    out_t: *mut InputMatrix,
    out_z: *mut InputMatrix,
) {
    let mat1 = mat1.as_mut().unwrap();
    let out_t = out_t.as_mut().unwrap();
    let out_z = out_z.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "sp_linalg_schur", err_msg);
    }

    let dim1 = (*mat1).get_dims();

    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {0} != {1}", dim1[0], dim1[1]);
        raise_exn!("LinAlgError", "np_linalg_schur", err_msg);
    }

    let out_t_dim = (*out_t).get_dims();
    let out_z_dim = (*out_z).get_dims();

    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };
    let out_t_slice = unsafe { slice::from_raw_parts_mut(out_t.data, out_t_dim[0] * out_t_dim[1]) };
    let out_z_slice = unsafe { slice::from_raw_parts_mut(out_z.data, out_z_dim[0] * out_z_dim[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let (z, t) = matrix.schur().unpack();

    out_t_slice.copy_from_slice(t.transpose().as_slice());
    out_z_slice.copy_from_slice(z.transpose().as_slice());
}

/// # Safety
///
/// `mat1` should point to a valid 2DArray of `f64` floats in row-major order
#[no_mangle]
pub unsafe extern "C" fn sp_linalg_hessenberg(
    mat1: *mut InputMatrix,
    out_h: *mut InputMatrix,
    out_q: *mut InputMatrix,
) {
    let mat1 = mat1.as_mut().unwrap();
    let out_h = out_h.as_mut().unwrap();
    let out_q = out_q.as_mut().unwrap();

    if mat1.ndims != 2 {
        let err_msg = format!("expected 2D Vector Input, but received {}D input", mat1.ndims);
        raise_exn!("ValueError", "sp_linalg_hessenberg", err_msg);
    }

    let dim1 = (*mat1).get_dims();

    if dim1[0] != dim1[1] {
        let err_msg =
            format!("last 2 dimensions of the array must be square: {} != {}", dim1[0], dim1[1]);
        raise_exn!("LinAlgError", "sp_linalg_hessenberg", err_msg);
    }

    let out_h_dim = (*out_h).get_dims();
    let out_q_dim = (*out_q).get_dims();

    let data_slice1 = unsafe { slice::from_raw_parts_mut(mat1.data, dim1[0] * dim1[1]) };
    let out_h_slice = unsafe { slice::from_raw_parts_mut(out_h.data, out_h_dim[0] * out_h_dim[1]) };
    let out_q_slice = unsafe { slice::from_raw_parts_mut(out_q.data, out_q_dim[0] * out_q_dim[1]) };

    let matrix = DMatrix::from_row_slice(dim1[0], dim1[1], data_slice1);
    let (q, h) = matrix.hessenberg().unpack();

    out_h_slice.copy_from_slice(h.transpose().as_slice());
    out_q_slice.copy_from_slice(q.transpose().as_slice());
}
