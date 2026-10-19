//! C-ABI exports for high-performance Tensor operations.

use crate::tensor::Tensor;

const PITH_OK: i32 = 0;
const PITH_E_INVALID: i32 = 1;
const PITH_E_REJECTED: i32 = 2;

/// Performs 2D GEMM matrix multiplication for float64 arrays: C = A @ B.
/// A is [m, k1], B is [k2, n].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_tensor_matmul_f64(
    a: *const f64,
    m: usize,
    k1: usize,
    b: *const f64,
    k2: usize,
    n: usize,
    out: *mut f64,
) -> i32 {
    if a.is_null() || b.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    if k1 != k2 {
        return PITH_E_REJECTED;
    }

    let a_slice = unsafe { core::slice::from_raw_parts(a, m * k1) };
    let b_slice = unsafe { core::slice::from_raw_parts(b, k2 * n) };
    let out_slice = unsafe { core::slice::from_raw_parts_mut(out, m * n) };

    let tensor_a: Tensor<f64> = match Tensor::from_vec(alloc::vec![m, k1], a_slice.to_vec()) {
        Ok(t) => t,
        Err(_) => return PITH_E_REJECTED,
    };
    let tensor_b: Tensor<f64> = match Tensor::from_vec(alloc::vec![k2, n], b_slice.to_vec()) {
        Ok(t) => t,
        Err(_) => return PITH_E_REJECTED,
    };

    let res = match tensor_a.matmul(&tensor_b) {
        Ok(t) => t,
        Err(_) => return PITH_E_REJECTED,
    };

    if let Some(res_slice) = res.as_slice() {
        out_slice.copy_from_slice(res_slice);
        PITH_OK
    } else {
        PITH_E_REJECTED
    }
}

/// Performs vectorized elementwise addition for float64 arrays: C = A + B.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_tensor_add_f64(
    a: *const f64,
    b: *const f64,
    len: usize,
    out: *mut f64,
) -> i32 {
    if a.is_null() || b.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let a_slice = unsafe { core::slice::from_raw_parts(a, len) };
    let b_slice = unsafe { core::slice::from_raw_parts(b, len) };
    let out_slice = unsafe { core::slice::from_raw_parts_mut(out, len) };

    for i in 0..len {
        out_slice[i] = a_slice[i] + b_slice[i];
    }
    PITH_OK
}

/// Performs fast sum reduction for float64 arrays.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_tensor_sum_f64(
    a: *const f64,
    len: usize,
    out: *mut f64,
) -> i32 {
    if a.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let a_slice = unsafe { core::slice::from_raw_parts(a, len) };
    let mut sum = 0.0f64;
    for &val in a_slice {
        sum += val;
    }
    unsafe {
        *out = sum;
    }
    PITH_OK
}
