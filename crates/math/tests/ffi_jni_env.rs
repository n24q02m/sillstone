//! The fake-env JNI battery: every `Java_io_github_sillstone_SillstoneMath_*` export
//! driven through a synthetic JNI function table — no JVM.
//!
//! The fake table installs stubs at the exact indices
//! [`sillstone_math::ffi_jni`] addresses, backed by a process-wide handle →
//! `Vec<f64>` map. This is both the correctness check for the JNI
//! glue and the execution pass the coverage gate needs: exports that
//! are never called do not count as covered lines.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use sillstone_math::ffi_jni::{
    Java_io_github_sillstone_SillstoneMath_complexArg,
    Java_io_github_sillstone_SillstoneMath_complexDiv,
    Java_io_github_sillstone_SillstoneMath_complexExp,
    Java_io_github_sillstone_SillstoneMath_complexLog,
    Java_io_github_sillstone_SillstoneMath_complexMul,
    Java_io_github_sillstone_SillstoneMath_complexPowi,
    Java_io_github_sillstone_SillstoneMath_complexSqrt,
    Java_io_github_sillstone_SillstoneMath_conv, Java_io_github_sillstone_SillstoneMath_corr,
    Java_io_github_sillstone_SillstoneMath_covariance, Java_io_github_sillstone_SillstoneMath_dct2,
    Java_io_github_sillstone_SillstoneMath_dct3, Java_io_github_sillstone_SillstoneMath_dct22d,
    Java_io_github_sillstone_SillstoneMath_dct32d, Java_io_github_sillstone_SillstoneMath_det3,
    Java_io_github_sillstone_SillstoneMath_dst1, Java_io_github_sillstone_SillstoneMath_dst2,
    Java_io_github_sillstone_SillstoneMath_dst3, Java_io_github_sillstone_SillstoneMath_dtw,
    Java_io_github_sillstone_SillstoneMath_fft, Java_io_github_sillstone_SillstoneMath_fftN,
    Java_io_github_sillstone_SillstoneMath_fftReal, Java_io_github_sillstone_SillstoneMath_idct2,
    Java_io_github_sillstone_SillstoneMath_ifft, Java_io_github_sillstone_SillstoneMath_ifftN,
    Java_io_github_sillstone_SillstoneMath_inverse3,
    Java_io_github_sillstone_SillstoneMath_lagrange,
    Java_io_github_sillstone_SillstoneMath_mat3Mul,
    Java_io_github_sillstone_SillstoneMath_mat3MulVec, Java_io_github_sillstone_SillstoneMath_mean,
    Java_io_github_sillstone_SillstoneMath_median,
    Java_io_github_sillstone_SillstoneMath_ransacLine,
    Java_io_github_sillstone_SillstoneMath_solve3,
    Java_io_github_sillstone_SillstoneMath_transpose3,
    Java_io_github_sillstone_SillstoneMath_variance,
};

type Env = *const *const [usize; 256];
type Arr = *mut core::ffi::c_void;

const IDX_GET_ARRAY_LENGTH: usize = 171;
const IDX_NEW_DOUBLE_ARRAY: usize = 182;
const IDX_GET_DOUBLE_ARRAY_ELEMENTS: usize = 190;
const IDX_RELEASE_DOUBLE_ARRAY_ELEMENTS: usize = 198;
const IDX_SET_DOUBLE_ARRAY_REGION: usize = 214;

/// Handle → contents for every array the fake JVM has allocated.
static ARRAYS: Mutex<Option<HashMap<usize, Vec<f64>>>> = Mutex::new(None);

fn with_maps<R>(f: impl FnOnce(&mut HashMap<usize, Vec<f64>>) -> R) -> R {
    let mut guard = ARRAYS.lock().unwrap();
    f(guard.get_or_insert_with(HashMap::new))
}

unsafe extern "system" fn stub_len(_env: Env, arr: Arr, _copy: *mut u8) -> i32 {
    with_maps(|m| m.get(&(arr as usize)).map_or(0, |v| v.len() as i32))
}

thread_local! {
    /// When set, `NewDoubleArray` on this thread reports allocation failure (NULL).
    static FAIL_NEW: Cell<bool> = const { Cell::new(false) };
    /// Count of `SetDoubleArrayRegion` calls on this thread that named no live array.
    static SET_ON_DEAD_ARRAY: Cell<usize> = const { Cell::new(0) };
}

unsafe extern "system" fn stub_new(_env: Env, len: i32) -> Arr {
    if FAIL_NEW.get() {
        return core::ptr::null_mut();
    }
    with_maps(|m| {
        let handle = (0x5000_0000usize + m.len() * 8 + 8) | 1;
        m.insert(handle, vec![0.0; len.max(0) as usize]);
        handle as Arr
    })
}

unsafe extern "system" fn stub_elements(_env: Env, arr: Arr, _copy: *mut u8) -> *mut f64 {
    with_maps(|m| {
        m.get(&(arr as usize))
            .map_or(core::ptr::null_mut(), |v| v.as_ptr() as *mut f64)
    })
}

unsafe extern "system" fn stub_release(_env: Env, _arr: Arr, _ptr: *mut f64, _mode: i32) {}

unsafe extern "system" fn stub_set_region(
    _env: Env,
    arr: Arr,
    start: i32,
    len: i32,
    data: *const f64,
) {
    let src = unsafe { core::slice::from_raw_parts(data, len.max(0) as usize) };
    with_maps(|m| {
        if let Some(v) = m.get_mut(&(arr as usize)) {
            let s = start.max(0) as usize;
            v[s..s + src.len()].copy_from_slice(src);
        } else {
            SET_ON_DEAD_ARRAY.set(SET_ON_DEAD_ARRAY.get() + 1);
        }
    })
}

/// A raw table pointer wrapper: immutable after init, so sharing
/// across test threads is sound.
struct TablePtr(*const [usize; 256]);
unsafe impl Send for TablePtr {}
unsafe impl Sync for TablePtr {}

/// The fake env, owned for the process lifetime by the static: the
/// tuple keeps the table alive and `FAKE.1` carries the pointer the
/// JNI glue dereferences once to reach the table.
static FAKE: LazyLock<(Box<[usize; 256]>, TablePtr)> = LazyLock::new(|| {
    let mut table = Box::new([0usize; 256]);
    table[IDX_GET_ARRAY_LENGTH] = stub_len as *const () as usize;
    table[IDX_NEW_DOUBLE_ARRAY] = stub_new as *const () as usize;
    table[IDX_GET_DOUBLE_ARRAY_ELEMENTS] = stub_elements as *const () as usize;
    table[IDX_RELEASE_DOUBLE_ARRAY_ELEMENTS] = stub_release as *const () as usize;
    table[IDX_SET_DOUBLE_ARRAY_REGION] = stub_set_region as *const () as usize;
    let ptr: *const [usize; 256] = &*table;
    (table, TablePtr(ptr))
});

/// The fake env handle handed to every export under test.
fn env() -> Env {
    &FAKE.1.0 as *const *const [usize; 256]
}

/// Registers `vals` and returns its fake array handle.
fn arr(vals: &[f64]) -> Arr {
    with_maps(|m| {
        let handle = (0x5000_0000usize + m.len() * 8 + 8) | 1;
        m.insert(handle, vals.to_vec());
        handle as Arr
    })
}

/// Reads back a fake array's contents.
fn read(arr: Arr) -> Vec<f64> {
    with_maps(|m| m.get(&(arr as usize)).cloned().unwrap_or_default())
}

const NULL: Arr = core::ptr::null_mut();

#[test]
fn dct_and_fft_surfaces_round_trip() {
    let e = env();
    let x = arr(&[0.5, -1.25, 2.0, 3.75, -0.125, 1.5, -2.5, 0.75]);
    let c8: Vec<f64> = (0..16).map(|i| i as f64 * 0.25 - 1.5).collect();
    let xc = arr(&c8);

    let out = unsafe { Java_io_github_sillstone_SillstoneMath_dct2(e, NULL, x) };
    assert_eq!(read(out).len(), 8);
    let out = unsafe { Java_io_github_sillstone_SillstoneMath_idct2(e, NULL, x) };
    assert_eq!(read(out).len(), 8);
    let out = unsafe { Java_io_github_sillstone_SillstoneMath_dct3(e, NULL, x) };
    assert_eq!(read(out).len(), 8);
    // idct2(dct2(x)) == x, through the JNI surface.
    let fwd = unsafe { Java_io_github_sillstone_SillstoneMath_dct2(e, NULL, x) };
    let back = unsafe { Java_io_github_sillstone_SillstoneMath_idct2(e, NULL, fwd) };
    for (g, w) in read(back)
        .iter()
        .zip([0.5, -1.25, 2.0, 3.75, -0.125, 1.5, -2.5, 0.75])
    {
        assert!((g - w).abs() < 1e-13);
    }

    let m64: Vec<f64> = (0..64).map(|i| ((i % 7) as f64) - 2.0).collect();
    let xm = arr(&m64);
    let out = unsafe { Java_io_github_sillstone_SillstoneMath_dct22d(e, NULL, xm, 8, 8) };
    assert_eq!(read(out).len(), 64);

    let out = unsafe { Java_io_github_sillstone_SillstoneMath_fft(e, NULL, xc) };
    assert_eq!(read(out).len(), 16);
    let out = unsafe { Java_io_github_sillstone_SillstoneMath_ifft(e, NULL, xc) };
    assert_eq!(read(out).len(), 16);
    let out = unsafe { Java_io_github_sillstone_SillstoneMath_fftReal(e, NULL, x) };
    assert_eq!(read(out).len(), 16);

    // fftN on n = 12 — a length the radix-2 surface refuses.
    let c12: Vec<f64> = (0..24).map(|i| i as f64 * 0.125 - 1.0).collect();
    let x12 = arr(&c12);
    let out = unsafe { Java_io_github_sillstone_SillstoneMath_fftN(e, NULL, x12) };
    assert_eq!(read(out).len(), 24);
}

#[test]
fn linalg_and_median_surfaces_match_the_reference_values() {
    let e = env();
    let det_in = arr(&[6.0, 1.0, 1.0, 4.0, -2.0, 5.0, 2.0, 8.0, 7.0]);
    assert_eq!(
        unsafe { Java_io_github_sillstone_SillstoneMath_det3(e, NULL, det_in) },
        -306.0
    );

    let m = arr(&[2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 1.0, 0.0, 2.0]);
    let inv = unsafe { Java_io_github_sillstone_SillstoneMath_inverse3(e, NULL, m) };
    assert_eq!(read(inv).len(), 9);
    let tr = unsafe { Java_io_github_sillstone_SillstoneMath_transpose3(e, NULL, m) };
    assert_eq!(read(tr), vec![2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 1.0, 0.0, 2.0]);

    // solve3 on the textbook system: answer 2, 3, −1 within ulps.
    let a = arr(&[2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0]);
    let b = arr(&[8.0, -11.0, -3.0]);
    let x = unsafe { Java_io_github_sillstone_SillstoneMath_solve3(e, NULL, a, b) };
    let got = read(x);
    assert_eq!(got.len(), 3);
    for (g, w) in got.iter().zip([2.0, 3.0, -1.0]) {
        assert!((g - w).abs() < 1e-12);
    }

    let prod = unsafe { Java_io_github_sillstone_SillstoneMath_mat3Mul(e, NULL, m, m) };
    assert_eq!(read(prod).len(), 9);
    let v = arr(&[1.0, 2.0, 3.0]);
    let mv = unsafe { Java_io_github_sillstone_SillstoneMath_mat3MulVec(e, NULL, m, v) };
    assert_eq!(read(mv).len(), 3);

    let med = unsafe {
        Java_io_github_sillstone_SillstoneMath_median(e, NULL, arr(&[1.0, 2.0, 3.0, 4.0]))
    };
    assert_eq!(med, 2.0);
}

#[test]
fn tier1_surfaces_produce_the_exact_vectors() {
    let e = env();

    // conv / corr.
    let a = arr(&[1.0, 2.0, 3.0]);
    let b = arr(&[0.0, 1.0, 2.0]);
    let got = read(unsafe { Java_io_github_sillstone_SillstoneMath_conv(e, NULL, a, b) });
    assert_eq!(got, vec![0.0, 1.0, 4.0, 7.0, 6.0]);
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_corr(e, NULL, arr(&[1.0, 2.0]), arr(&[3.0, 4.0]))
    });
    assert_eq!(got, vec![4.0, 11.0, 6.0]);

    // stats.
    assert_eq!(
        unsafe { Java_io_github_sillstone_SillstoneMath_mean(e, NULL, arr(&[1.0, 2.0, 3.0, 4.0])) },
        2.5
    );
    let v = unsafe { Java_io_github_sillstone_SillstoneMath_variance(e, NULL, arr(&[1.0, 3.0])) };
    assert_eq!(v.to_bits(), 2.0_f64.to_bits());
    let c = unsafe {
        Java_io_github_sillstone_SillstoneMath_covariance(
            e,
            NULL,
            arr(&[1.0, 2.0, 3.0]),
            arr(&[2.0, 4.0, 6.0]),
        )
    };
    assert_eq!(c.to_bits(), 2.0_f64.to_bits());

    // lagrange through (0,1),(1,4),(2,9) at 1.5 = 6.25 exactly.
    let l = unsafe {
        Java_io_github_sillstone_SillstoneMath_lagrange(
            e,
            NULL,
            arr(&[0.0, 1.0, 2.0]),
            arr(&[1.0, 4.0, 9.0]),
            1.5,
        )
    };
    assert_eq!(l.to_bits(), 6.25_f64.to_bits());

    // dtw textbook.
    let d = unsafe {
        Java_io_github_sillstone_SillstoneMath_dtw(
            e,
            NULL,
            arr(&[1.0, 2.0, 3.0]),
            arr(&[2.0, 2.0, 2.0]),
        )
    };
    assert_eq!(d, 2.0);

    // ransac: the planted line, exact model out.
    let fit = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_ransacLine(
            e,
            NULL,
            arr(&[0.0, 2.0, 4.0, 6.0, 2.0, 4.0]),
            arr(&[1.0, 2.0, 3.0, 4.0, 10.0, -9.0]),
            0.5,
            64,
            42,
        )
    });
    assert_eq!(fit, vec![0.5, 1.0, 4.0]);

    // complex ops.
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_complexMul(
            e,
            NULL,
            arr(&[3.0, 2.0]),
            arr(&[1.0, -4.0]),
        )
    });
    assert_eq!(got, vec![11.0, -10.0]);
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_complexDiv(
            e,
            NULL,
            arr(&[5.0, 1.0]),
            arr(&[1.0, -1.0]),
        )
    });
    assert_eq!(got, vec![2.0, 3.0]);
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_complexExp(e, NULL, arr(&[0.0, 0.0]))
    });
    assert_eq!(got, vec![1.0, 0.0]);
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_complexSqrt(e, NULL, arr(&[4.0, 0.0]))
    });
    assert_eq!(got, vec![2.0, 0.0]);
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_complexPowi(e, NULL, arr(&[1.0, 1.0]), 4)
    });
    assert_eq!(got, vec![-4.0, 0.0]);
    let got =
        unsafe { Java_io_github_sillstone_SillstoneMath_complexArg(e, NULL, arr(&[1.0, 1.0])) };
    assert_eq!(got, core::f64::consts::FRAC_PI_4);
}

#[test]
fn refusal_conventions_hold_across_the_surface() {
    let e = env();

    // NULL arrays → NULL / NaN, never a crash.
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct2(e, NULL, NULL) }.is_null());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_solve3(e, NULL, NULL, NULL) }.is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_conv(e, NULL, NULL, NULL) }.is_null());
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_ransacLine(e, NULL, NULL, NULL, 1.0, 8, 1)
        }
        .is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_mean(e, NULL, NULL) }.is_nan());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_det3(e, NULL, NULL) }.is_nan());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dtw(e, NULL, NULL, NULL) }.is_nan());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_lagrange(e, NULL, NULL, NULL, 0.5) }
            .is_nan()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_complexArg(e, NULL, NULL) }.is_nan());

    // Empty inputs are the Java pre-validation's domain, mirrored: NULL.
    let empty = arr(&[]);
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct3(e, NULL, empty) }.is_null());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_conv(e, NULL, empty, arr(&[1.0])) }
            .is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_fftReal(e, NULL, empty) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_fftN(e, NULL, empty) }.is_null());

    // Broken geometry: fft on a non-power-of-two, dct2_2d mismatched,
    // linalg with wrong lengths.
    let c6 = arr(&[0.0; 12]); // 6 complex pairs — not a power of two.
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_fft(e, NULL, c6) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_ifft(e, NULL, c6) }.is_null());
    let odd_complex = arr(&[0.0; 6]); // 3 pairs — odd.
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_fft(e, NULL, odd_complex) }.is_null());
    let m64 = arr(&[0.0; 64]);
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct22d(e, NULL, m64, 4, 8) }.is_null());
    let short = arr(&[1.0, 2.0]);
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_inverse3(e, NULL, short) }.is_null());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_mat3Mul(e, NULL, short, short) }.is_null()
    );
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_mat3MulVec(e, NULL, m64, arr(&[1.0])) }
            .is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_det3(e, NULL, short) }.is_nan());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_solve3(e, NULL, short, arr(&[1.0])) }
            .is_null()
    );

    // Data refusals: singular solve3 → NULL; duplicated Lagrange nodes
    // → NaN; all-vertical RANSAC → NULL; covariance length mismatch
    // → NaN.
    let zeros = arr(&[0.0; 9]);
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_solve3(e, NULL, zeros, arr(&[0.0; 3])) }
            .is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_inverse3(e, NULL, zeros) }.is_null());
    let l = unsafe {
        Java_io_github_sillstone_SillstoneMath_lagrange(
            e,
            NULL,
            arr(&[1.0, 1.0]),
            arr(&[2.0, 3.0]),
            0.5,
        )
    };
    assert!(l.is_nan());
    let fit = unsafe {
        Java_io_github_sillstone_SillstoneMath_ransacLine(
            e,
            NULL,
            arr(&[2.0, 2.0, 2.0]),
            arr(&[1.0, 5.0, 9.0]),
            0.5,
            64,
            1,
        )
    };
    assert!(fit.is_null());
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_ransacLine(
                e,
                NULL,
                arr(&[0.0]),
                arr(&[1.0]),
                0.5,
                64,
                1,
            )
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_ransacLine(
                e,
                NULL,
                arr(&[0.0, 1.0]),
                arr(&[1.0, 2.0]),
                0.0,
                64,
                1,
            )
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_ransacLine(
                e,
                NULL,
                arr(&[0.0, 1.0]),
                arr(&[1.0, 2.0]),
                0.5,
                0,
                1,
            )
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_covariance(
                e,
                NULL,
                arr(&[1.0]),
                arr(&[1.0, 2.0]),
            )
        }
        .is_nan()
    );
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_variance(e, NULL, arr(&[1.0])) }.is_nan()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_mean(e, NULL, empty) }.is_nan());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_dtw(e, NULL, empty, arr(&[1.0])) }.is_nan()
    );
    // Wrong complex pair counts → NULL/NaN.
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexMul(
                e,
                NULL,
                arr(&[1.0]),
                arr(&[1.0, 2.0]),
            )
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexExp(e, NULL, arr(&[1.0, 2.0, 3.0]))
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexSqrt(e, NULL, arr(&[1.0, 2.0, 3.0]))
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexPowi(e, NULL, arr(&[1.0, 2.0, 3.0]), 2)
        }
        .is_null()
    );
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexArg(e, NULL, arr(&[1.0, 2.0, 3.0]))
        }
        .is_nan()
    );
    // Median of nothing.
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_median(e, NULL, empty) }.is_nan());

    // Remaining geometry refusals across the surface.
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct22d(e, NULL, m64, 0, 8) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct22d(e, NULL, m64, 8, 0) }.is_null());
    let r6 = arr(&[0.0; 6]); // non-power-of-two real length
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_fftReal(e, NULL, r6) }.is_null());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_fftN(e, NULL, arr(&[0.0; 5])) }.is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_transpose3(e, NULL, short) }.is_null());
    let lag = unsafe {
        Java_io_github_sillstone_SillstoneMath_lagrange(e, NULL, arr(&[0.0, 1.0]), arr(&[1.0]), 0.5)
    };
    assert!(lag.is_nan());
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexDiv(
                e,
                NULL,
                arr(&[1.0, 2.0]),
                arr(&[1.0, 2.0, 3.0]),
            )
        }
        .is_null()
    );
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_ifftN(e, NULL, arr(&[0.0; 5])) }.is_null()
    );
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct32d(e, NULL, m64, 0, 8) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct32d(e, NULL, m64, 4, 8) }.is_null());
    assert!(
        unsafe { Java_io_github_sillstone_SillstoneMath_complexLog(e, NULL, arr(&[0.0, 0.0])) }
            .is_null()
    );

    // Happy paths for the newest exports.
    let x12 = arr(&[
        1.0, 0.0, 0.5, -0.5, -1.0, 2.0, 0.25, 1.5, -0.75, 0.0, 2.0, -1.0,
    ]);
    let fwd = unsafe { Java_io_github_sillstone_SillstoneMath_fftN(e, NULL, x12) };
    let inv = unsafe { Java_io_github_sillstone_SillstoneMath_ifftN(e, NULL, fwd) };
    let rt = read(inv);
    assert_eq!(rt.len(), 12);
    assert!((rt[0] - 1.0).abs() < 1e-12 && rt[1].abs() < 1e-12);
    let m64f: Vec<f64> = (0..64).map(|i| ((i % 7) as f64) - 2.0).collect();
    let m16 = unsafe { Java_io_github_sillstone_SillstoneMath_dct32d(e, NULL, arr(&m64f), 8, 8) };
    assert_eq!(read(m16).len(), 64);
    let lg = unsafe {
        Java_io_github_sillstone_SillstoneMath_complexLog(e, NULL, arr(&[std::f64::consts::E, 0.0]))
    };
    let lv = read(lg);
    assert_eq!(lv.len(), 2);
    assert!((lv[0] - 1.0).abs() < 1e-15 && lv[1].abs() < 1e-15);
}

/// Orthonormal DST-I/II/III of one fixed signal, from `scipy.fft.dst(x, type=t,
/// norm="ortho")` (scipy 1.18.1), printed with `repr` so every digit is kept.
#[test]
fn dst_surfaces_match_the_scipy_orthonormal_vectors() {
    let e = env();
    let x = arr(&[0.5, -1.25, 2.0, 3.75, -0.125, 1.5, -2.5, 0.75]);
    let want: [[f64; 8]; 3] = [
        [
            2.1769878720901037,
            1.3334400693938764,
            -2.500520779091161,
            -1.2928228670647344,
            0.8544684946372053,
            0.9695896898516747,
            2.964087672417664,
            -2.0138905007628165,
        ],
        [
            2.312982406608636,
            1.5020083121091012,
            -2.6573716138889445,
            -1.193242693252299,
            -0.5555290394487742,
            1.3396836493572537,
            2.790643022449363,
            -1.7235727791422097,
        ],
        [
            1.371117124075703,
            2.3430445985967543,
            -2.155303440030729,
            -1.51439128275975,
            0.7601242242691456,
            1.6948914677156566,
            1.9505988192238548,
            -2.7183283995743284,
        ],
    ];
    let got = [
        read(unsafe { Java_io_github_sillstone_SillstoneMath_dst1(e, NULL, x) }),
        read(unsafe { Java_io_github_sillstone_SillstoneMath_dst2(e, NULL, x) }),
        read(unsafe { Java_io_github_sillstone_SillstoneMath_dst3(e, NULL, x) }),
    ];
    for (t, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g.len(), 8, "dst{} length", t + 1);
        for (i, (a, b)) in g.iter().zip(w).enumerate() {
            assert!((a - b).abs() < 1e-12, "dst{} [{i}]: {a} vs {b}", t + 1);
        }
    }

    // Same refusal convention as the rest of the surface: NULL and empty give NULL.
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dst1(e, NULL, NULL) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dst2(e, NULL, arr(&[])) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dst3(e, NULL, NULL) }.is_null());
}

/// Clears the allocation-failure switch even when an assertion unwinds.
struct FailNewGuard;

impl FailNewGuard {
    fn arm() -> Self {
        FAIL_NEW.set(true);
        Self
    }
}

impl Drop for FailNewGuard {
    fn drop(&mut self) {
        FAIL_NEW.set(false);
    }
}

#[test]
fn a_failed_jvm_array_allocation_surfaces_as_null() {
    let e = env();
    let x = arr(&[1.0, 2.0, 3.0, 4.0]);

    // Baseline: the same call succeeds while allocation works.
    assert_eq!(
        read(unsafe { Java_io_github_sillstone_SillstoneMath_dct2(e, NULL, x) }).len(),
        4
    );

    // `NewDoubleArray` returning NULL (the JVM has already thrown OutOfMemoryError)
    // must come back as a NULL result, never a write through a null array.
    let _armed = FailNewGuard::arm();
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dct2(e, NULL, x) }.is_null());
    assert!(unsafe { Java_io_github_sillstone_SillstoneMath_dst2(e, NULL, x) }.is_null());
    assert_eq!(
        SET_ON_DEAD_ARRAY.get(),
        0,
        "the result was copied into a NULL array"
    );
}

#[test]
fn complex_div_by_a_zero_denominator_is_null_not_nan() {
    let e = env();
    assert!(
        unsafe {
            Java_io_github_sillstone_SillstoneMath_complexDiv(
                e,
                NULL,
                arr(&[5.0, 1.0]),
                arr(&[0.0, 0.0]),
            )
        }
        .is_null()
    );
    // (5 + 1j) / 2j = 0.5 - 2.5j (numpy), exact in binary.
    let got = read(unsafe {
        Java_io_github_sillstone_SillstoneMath_complexDiv(
            e,
            NULL,
            arr(&[5.0, 1.0]),
            arr(&[0.0, 2.0]),
        )
    });
    assert_eq!(got, vec![0.5, -2.5]);
}

/// Whether two exports gave the same array result: both NULL, or both live
/// with bit-identical contents.
fn same_array(alias: Arr, canonical: Arr) -> bool {
    alias.is_null() == canonical.is_null()
        && read(alias)
            .iter()
            .map(|v| v.to_bits())
            .eq(read(canonical).iter().map(|v| v.to_bits()))
}

/// Whether two exports gave the same scalar result (NaN refusals compare equal).
fn same_scalar(alias: f64, canonical: f64) -> bool {
    alias.to_bits() == canonical.to_bits() || (alias.is_nan() && canonical.is_nan())
}

/// Every deprecated pre-rename `Java_sillstone_math_PithMath_*` export forwards to its
/// `Java_io_github_sillstone_SillstoneMath_*` twin: on a real happy-path input it returns
/// the identical result, and on a NULL input it refuses the same way.
#[test]
#[allow(deprecated)]
fn legacy_pith_math_exports_match_the_sillstone_exports() {
    use sillstone_math::ffi_jni as jni;
    type Unary = unsafe extern "system" fn(Env, Arr, Arr) -> Arr;
    type UnaryScalar = unsafe extern "system" fn(Env, Arr, Arr) -> f64;
    type Binary = unsafe extern "system" fn(Env, Arr, Arr, Arr) -> Arr;
    type BinaryScalar = unsafe extern "system" fn(Env, Arr, Arr, Arr) -> f64;
    type Grid = unsafe extern "system" fn(Env, Arr, Arr, i32, i32) -> Arr;
    /// One unary row: label, legacy alias, canonical export, input.
    type Case<'a, F> = (&'a str, F, F, &'a [f64]);
    /// One binary row: label, legacy alias, canonical export, both inputs.
    type PairCase<'a, F> = (&'a str, F, F, &'a [f64], &'a [f64]);

    let e = env();
    let real8 = [0.5, -1.25, 2.0, 3.75, -0.125, 1.5, -2.5, 0.75];
    let complex8: Vec<f64> = (0..16).map(|i| i as f64 * 0.25 - 1.5).collect();
    let complex12: Vec<f64> = (0..24).map(|i| i as f64 * 0.125 - 1.0).collect();
    let mat = [2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 1.0, 0.0, 2.0];
    let grid: Vec<f64> = (0..64).map(|i| ((i % 7) as f64) - 2.0).collect();
    let mut covered = 0;

    let unary: [Case<Unary>; 16] = [
        (
            "dct2",
            jni::Java_sillstone_math_PithMath_dct2,
            jni::Java_io_github_sillstone_SillstoneMath_dct2,
            &real8,
        ),
        (
            "idct2",
            jni::Java_sillstone_math_PithMath_idct2,
            jni::Java_io_github_sillstone_SillstoneMath_idct2,
            &real8,
        ),
        (
            "dct3",
            jni::Java_sillstone_math_PithMath_dct3,
            jni::Java_io_github_sillstone_SillstoneMath_dct3,
            &real8,
        ),
        (
            "dst1",
            jni::Java_sillstone_math_PithMath_dst1,
            jni::Java_io_github_sillstone_SillstoneMath_dst1,
            &real8,
        ),
        (
            "dst2",
            jni::Java_sillstone_math_PithMath_dst2,
            jni::Java_io_github_sillstone_SillstoneMath_dst2,
            &real8,
        ),
        (
            "dst3",
            jni::Java_sillstone_math_PithMath_dst3,
            jni::Java_io_github_sillstone_SillstoneMath_dst3,
            &real8,
        ),
        (
            "fft",
            jni::Java_sillstone_math_PithMath_fft,
            jni::Java_io_github_sillstone_SillstoneMath_fft,
            &complex8,
        ),
        (
            "ifft",
            jni::Java_sillstone_math_PithMath_ifft,
            jni::Java_io_github_sillstone_SillstoneMath_ifft,
            &complex8,
        ),
        (
            "fftReal",
            jni::Java_sillstone_math_PithMath_fftReal,
            jni::Java_io_github_sillstone_SillstoneMath_fftReal,
            &real8,
        ),
        (
            "fftN",
            jni::Java_sillstone_math_PithMath_fftN,
            jni::Java_io_github_sillstone_SillstoneMath_fftN,
            &complex12,
        ),
        (
            "ifftN",
            jni::Java_sillstone_math_PithMath_ifftN,
            jni::Java_io_github_sillstone_SillstoneMath_ifftN,
            &complex12,
        ),
        (
            "inverse3",
            jni::Java_sillstone_math_PithMath_inverse3,
            jni::Java_io_github_sillstone_SillstoneMath_inverse3,
            &mat,
        ),
        (
            "transpose3",
            jni::Java_sillstone_math_PithMath_transpose3,
            jni::Java_io_github_sillstone_SillstoneMath_transpose3,
            &mat,
        ),
        (
            "complexExp",
            jni::Java_sillstone_math_PithMath_complexExp,
            jni::Java_io_github_sillstone_SillstoneMath_complexExp,
            &[0.5, 1.0],
        ),
        (
            "complexSqrt",
            jni::Java_sillstone_math_PithMath_complexSqrt,
            jni::Java_io_github_sillstone_SillstoneMath_complexSqrt,
            &[4.0, 0.0],
        ),
        (
            "complexLog",
            jni::Java_sillstone_math_PithMath_complexLog,
            jni::Java_io_github_sillstone_SillstoneMath_complexLog,
            &[std::f64::consts::E, 1.0],
        ),
    ];
    for (name, alias, canonical, input) in unary {
        let want = unsafe { canonical(e, NULL, arr(input)) };
        assert!(!want.is_null(), "{name}: canonical happy path");
        assert!(
            same_array(unsafe { alias(e, NULL, arr(input)) }, want),
            "{name}"
        );
        let refused = unsafe { canonical(e, NULL, NULL) };
        assert!(refused.is_null(), "{name}: canonical refusal");
        assert!(
            same_array(unsafe { alias(e, NULL, NULL) }, refused),
            "{name}: refusal"
        );
        covered += 1;
    }

    let unary_scalar: [Case<UnaryScalar>; 5] = [
        (
            "det3",
            jni::Java_sillstone_math_PithMath_det3,
            jni::Java_io_github_sillstone_SillstoneMath_det3,
            &[6.0, 1.0, 1.0, 4.0, -2.0, 5.0, 2.0, 8.0, 7.0],
        ),
        (
            "median",
            jni::Java_sillstone_math_PithMath_median,
            jni::Java_io_github_sillstone_SillstoneMath_median,
            &[1.0, 2.0, 3.0, 4.0],
        ),
        (
            "mean",
            jni::Java_sillstone_math_PithMath_mean,
            jni::Java_io_github_sillstone_SillstoneMath_mean,
            &[1.0, 2.0, 3.0, 4.0],
        ),
        (
            "variance",
            jni::Java_sillstone_math_PithMath_variance,
            jni::Java_io_github_sillstone_SillstoneMath_variance,
            &[1.0, 3.0],
        ),
        (
            "complexArg",
            jni::Java_sillstone_math_PithMath_complexArg,
            jni::Java_io_github_sillstone_SillstoneMath_complexArg,
            &[1.0, 1.0],
        ),
    ];
    for (name, alias, canonical, input) in unary_scalar {
        let want = unsafe { canonical(e, NULL, arr(input)) };
        assert!(!want.is_nan(), "{name}: canonical happy path");
        assert!(
            same_scalar(unsafe { alias(e, NULL, arr(input)) }, want),
            "{name}"
        );
        let refused = unsafe { canonical(e, NULL, NULL) };
        assert!(refused.is_nan(), "{name}: canonical refusal");
        assert!(
            same_scalar(unsafe { alias(e, NULL, NULL) }, refused),
            "{name}: refusal"
        );
        covered += 1;
    }

    let binary: [PairCase<Binary>; 7] = [
        (
            "solve3",
            jni::Java_sillstone_math_PithMath_solve3,
            jni::Java_io_github_sillstone_SillstoneMath_solve3,
            &[2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0],
            &[8.0, -11.0, -3.0],
        ),
        (
            "mat3Mul",
            jni::Java_sillstone_math_PithMath_mat3Mul,
            jni::Java_io_github_sillstone_SillstoneMath_mat3Mul,
            &mat,
            &mat,
        ),
        (
            "mat3MulVec",
            jni::Java_sillstone_math_PithMath_mat3MulVec,
            jni::Java_io_github_sillstone_SillstoneMath_mat3MulVec,
            &mat,
            &[1.0, 2.0, 3.0],
        ),
        (
            "conv",
            jni::Java_sillstone_math_PithMath_conv,
            jni::Java_io_github_sillstone_SillstoneMath_conv,
            &[1.0, 2.0, 3.0],
            &[0.0, 1.0, 2.0],
        ),
        (
            "corr",
            jni::Java_sillstone_math_PithMath_corr,
            jni::Java_io_github_sillstone_SillstoneMath_corr,
            &[1.0, 2.0],
            &[3.0, 4.0],
        ),
        (
            "complexMul",
            jni::Java_sillstone_math_PithMath_complexMul,
            jni::Java_io_github_sillstone_SillstoneMath_complexMul,
            &[3.0, 2.0],
            &[1.0, -4.0],
        ),
        (
            "complexDiv",
            jni::Java_sillstone_math_PithMath_complexDiv,
            jni::Java_io_github_sillstone_SillstoneMath_complexDiv,
            &[5.0, 1.0],
            &[1.0, -1.0],
        ),
    ];
    for (name, alias, canonical, a, b) in binary {
        let want = unsafe { canonical(e, NULL, arr(a), arr(b)) };
        assert!(!want.is_null(), "{name}: canonical happy path");
        assert!(
            same_array(unsafe { alias(e, NULL, arr(a), arr(b)) }, want),
            "{name}"
        );
        let refused = unsafe { canonical(e, NULL, NULL, NULL) };
        assert!(refused.is_null(), "{name}: canonical refusal");
        assert!(
            same_array(unsafe { alias(e, NULL, NULL, NULL) }, refused),
            "{name}: refusal"
        );
        covered += 1;
    }

    let binary_scalar: [PairCase<BinaryScalar>; 2] = [
        (
            "covariance",
            jni::Java_sillstone_math_PithMath_covariance,
            jni::Java_io_github_sillstone_SillstoneMath_covariance,
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 6.0],
        ),
        (
            "dtw",
            jni::Java_sillstone_math_PithMath_dtw,
            jni::Java_io_github_sillstone_SillstoneMath_dtw,
            &[1.0, 2.0, 3.0],
            &[2.0, 2.0, 2.0],
        ),
    ];
    for (name, alias, canonical, a, b) in binary_scalar {
        let want = unsafe { canonical(e, NULL, arr(a), arr(b)) };
        assert!(!want.is_nan(), "{name}: canonical happy path");
        assert!(
            same_scalar(unsafe { alias(e, NULL, arr(a), arr(b)) }, want),
            "{name}"
        );
        let refused = unsafe { canonical(e, NULL, NULL, NULL) };
        assert!(refused.is_nan(), "{name}: canonical refusal");
        assert!(
            same_scalar(unsafe { alias(e, NULL, NULL, NULL) }, refused),
            "{name}: refusal"
        );
        covered += 1;
    }

    let grids: [(&str, Grid, Grid); 2] = [
        (
            "dct22d",
            jni::Java_sillstone_math_PithMath_dct22d,
            jni::Java_io_github_sillstone_SillstoneMath_dct22d,
        ),
        (
            "dct32d",
            jni::Java_sillstone_math_PithMath_dct32d,
            jni::Java_io_github_sillstone_SillstoneMath_dct32d,
        ),
    ];
    for (name, alias, canonical) in grids {
        let want = unsafe { canonical(e, NULL, arr(&grid), 8, 8) };
        assert!(!want.is_null(), "{name}: canonical happy path");
        assert!(
            same_array(unsafe { alias(e, NULL, arr(&grid), 8, 8) }, want),
            "{name}"
        );
        let refused = unsafe { canonical(e, NULL, NULL, 8, 8) };
        assert!(refused.is_null(), "{name}: canonical refusal");
        assert!(
            same_array(unsafe { alias(e, NULL, NULL, 8, 8) }, refused),
            "{name}: refusal"
        );
        covered += 1;
    }

    // The three exports with a scalar parameter, one by one.
    let (xs, ys) = ([0.0, 1.0, 2.0], [1.0, 4.0, 9.0]);
    let want = unsafe {
        jni::Java_io_github_sillstone_SillstoneMath_lagrange(e, NULL, arr(&xs), arr(&ys), 1.5)
    };
    assert_eq!(want.to_bits(), 6.25_f64.to_bits());
    let got =
        unsafe { jni::Java_sillstone_math_PithMath_lagrange(e, NULL, arr(&xs), arr(&ys), 1.5) };
    assert!(same_scalar(got, want), "lagrange");
    let got = unsafe { jni::Java_sillstone_math_PithMath_lagrange(e, NULL, NULL, NULL, 0.5) };
    assert!(got.is_nan(), "lagrange: refusal");
    covered += 1;

    let (xs, ys) = (
        [0.0, 2.0, 4.0, 6.0, 2.0, 4.0],
        [1.0, 2.0, 3.0, 4.0, 10.0, -9.0],
    );
    let want = unsafe {
        jni::Java_io_github_sillstone_SillstoneMath_ransacLine(
            e,
            NULL,
            arr(&xs),
            arr(&ys),
            0.5,
            64,
            42,
        )
    };
    assert_eq!(read(want), vec![0.5, 1.0, 4.0]);
    let got = unsafe {
        jni::Java_sillstone_math_PithMath_ransacLine(e, NULL, arr(&xs), arr(&ys), 0.5, 64, 42)
    };
    assert!(same_array(got, want), "ransacLine");
    let got =
        unsafe { jni::Java_sillstone_math_PithMath_ransacLine(e, NULL, NULL, NULL, 1.0, 8, 1) };
    assert!(got.is_null(), "ransacLine: refusal");
    covered += 1;

    let want = unsafe {
        jni::Java_io_github_sillstone_SillstoneMath_complexPowi(e, NULL, arr(&[1.0, 1.0]), 4)
    };
    assert_eq!(read(want), vec![-4.0, 0.0]);
    let got =
        unsafe { jni::Java_sillstone_math_PithMath_complexPowi(e, NULL, arr(&[1.0, 1.0]), 4) };
    assert!(same_array(got, want), "complexPowi");
    let got = unsafe { jni::Java_sillstone_math_PithMath_complexPowi(e, NULL, NULL, 2) };
    assert!(got.is_null(), "complexPowi: refusal");
    covered += 1;

    // Every legacy export in `ffi_jni.rs` is exercised above.
    assert_eq!(covered, 35);
}
