//! C-ABI conformance for the tensor exports, called the way the language SDKs
//! call them: raw pointers in, status code out, results written to `out`.

use core::ptr;
use sillstone_tensor::ffi::{
    sillstone_tensor_add_f64, sillstone_tensor_matmul_f64, sillstone_tensor_sum_f64,
};

const OK: i32 = 0;
const E_INVALID: i32 = -1;
const E_REJECTED: i32 = -2;

#[test]
fn matmul_multiplies_and_writes_the_output() {
    // [2, 3] x [3, 2]
    let a = [1.0f64, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [7.0f64, 8.0, 9.0, 1.0, 2.0, 3.0];
    let mut out = [0.0f64; 4];
    let rc = unsafe {
        sillstone_tensor_matmul_f64(a.as_ptr(), 2, 3, b.as_ptr(), 3, 2, out.as_mut_ptr())
    };
    assert_eq!(rc, OK);
    assert_eq!(out, [31.0, 19.0, 85.0, 55.0]);
}

#[test]
fn matmul_rejects_null_pointers_and_mismatched_inner_dimensions() {
    let a = [1.0f64; 6];
    let b = [1.0f64; 6];
    let mut out = [0.0f64; 4];
    unsafe {
        assert_eq!(
            sillstone_tensor_matmul_f64(ptr::null(), 2, 3, b.as_ptr(), 3, 2, out.as_mut_ptr()),
            E_INVALID
        );
        assert_eq!(
            sillstone_tensor_matmul_f64(a.as_ptr(), 2, 3, ptr::null(), 3, 2, out.as_mut_ptr()),
            E_INVALID
        );
        assert_eq!(
            sillstone_tensor_matmul_f64(a.as_ptr(), 2, 3, b.as_ptr(), 3, 2, ptr::null_mut()),
            E_INVALID
        );
        // k1 != k2 is a well-formed call with incompatible shapes.
        assert_eq!(
            sillstone_tensor_matmul_f64(a.as_ptr(), 2, 3, b.as_ptr(), 2, 3, out.as_mut_ptr()),
            E_REJECTED
        );
    }
    assert_eq!(out, [0.0; 4], "a rejected call must not touch the output");
}

#[test]
fn matmul_rejects_size_overflow_before_touching_memory() {
    let a = [1.0f64; 1];
    let b = [1.0f64; 1];
    let mut out = [0.0f64; 1];
    let rc = unsafe {
        sillstone_tensor_matmul_f64(
            a.as_ptr(),
            usize::MAX,
            2,
            b.as_ptr(),
            2,
            1,
            out.as_mut_ptr(),
        )
    };
    assert_eq!(rc, E_INVALID);
    assert_eq!(out, [0.0]);
}

#[test]
fn add_is_elementwise_and_validates_pointers() {
    let a = [1.5f64, -2.0, 3.0];
    let b = [0.5f64, 2.0, -4.0];
    let mut out = [0.0f64; 3];
    unsafe {
        assert_eq!(
            sillstone_tensor_add_f64(a.as_ptr(), b.as_ptr(), 3, out.as_mut_ptr()),
            OK
        );
        assert_eq!(out, [2.0, 0.0, -1.0]);
        assert_eq!(
            sillstone_tensor_add_f64(ptr::null(), b.as_ptr(), 3, out.as_mut_ptr()),
            E_INVALID
        );
        assert_eq!(
            sillstone_tensor_add_f64(a.as_ptr(), ptr::null(), 3, out.as_mut_ptr()),
            E_INVALID
        );
        assert_eq!(
            sillstone_tensor_add_f64(a.as_ptr(), b.as_ptr(), 3, ptr::null_mut()),
            E_INVALID
        );
    }
}

#[test]
fn sum_reduces_and_validates_pointers() {
    let a = [1.0f64, 2.0, 3.5];
    let mut out = -1.0f64;
    unsafe {
        assert_eq!(sillstone_tensor_sum_f64(a.as_ptr(), 3, &mut out), OK);
        assert_eq!(out, 6.5);
        assert_eq!(sillstone_tensor_sum_f64(a.as_ptr(), 0, &mut out), OK);
        assert_eq!(out, 0.0);
        assert_eq!(
            sillstone_tensor_sum_f64(ptr::null(), 3, &mut out),
            E_INVALID
        );
        assert_eq!(
            sillstone_tensor_sum_f64(a.as_ptr(), 3, ptr::null_mut()),
            E_INVALID
        );
    }
}
