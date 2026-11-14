//! Raw-pointer exercise of the `sillstone_math` exports no other target
//! drove: the DST family, the sample-based trapezoid / Simpson rules,
//! `complex_log`, the arbitrary-length `fft_n` / `ifft_n` inverse pair
//! and the 2D DCT-III. Each result is checked against an independent
//! O(n^2) textbook model written here, not against the crate's own
//! kernel, so a wrong scale factor or an off-by-one cannot hide.

use core::f64::consts::{PI, SQRT_2};

use sillstone_math::ffi::{
    SILLSTONE_E_INVALID, SILLSTONE_E_REJECTED, SILLSTONE_OK, sillstone_math_complex_log,
    sillstone_math_dct2_2d, sillstone_math_dct3_2d, sillstone_math_dst1, sillstone_math_dst2,
    sillstone_math_dst3, sillstone_math_fft_n, sillstone_math_free, sillstone_math_ifft_n,
    sillstone_math_simpson_samples, sillstone_math_trapezoid_samples,
};

type DstFn = unsafe extern "C" fn(*const f64, usize, *mut f64) -> i32;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + b.abs())
}

fn assert_all_close(got: &[f64], want: &[f64], tol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(close(*g, *w, tol), "{what}[{i}]: got {g}, want {w}");
    }
}

fn signal(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| ((i * 7 + 3) % 11) as f64 * 0.37 - 1.9 + (i as f64 * 0.91).sin())
        .collect()
}

fn run_dst(f: DstFn, x: &[f64]) -> Vec<f64> {
    let mut out = vec![f64::NAN; x.len()];
    let status = unsafe { f(x.as_ptr(), x.len(), out.as_mut_ptr()) };
    assert_eq!(status, SILLSTONE_OK);
    out
}

fn model_dst1(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let scale = (2.0 / (n as f64 + 1.0)).sqrt();
    (0..n)
        .map(|k| {
            scale
                * x.iter()
                    .enumerate()
                    .map(|(j, v)| {
                        v * (PI * (k as f64 + 1.0) * (j as f64 + 1.0) / (n as f64 + 1.0)).sin()
                    })
                    .sum::<f64>()
        })
        .collect()
}

fn model_dst2(x: &[f64]) -> Vec<f64> {
    let n = x.len() as f64;
    (0..x.len())
        .map(|k| {
            let row = if k == x.len() - 1 { 1.0 / SQRT_2 } else { 1.0 };
            row * (2.0 / n).sqrt()
                * x.iter()
                    .enumerate()
                    .map(|(j, v)| v * (PI * (k as f64 + 1.0) * (j as f64 + 0.5) / n).sin())
                    .sum::<f64>()
        })
        .collect()
}

#[test]
fn dst_family_matches_the_textbook_definitions() {
    for n in [1usize, 2, 3, 5, 8, 13] {
        let x = signal(n);
        assert_all_close(
            &run_dst(sillstone_math_dst1, &x),
            &model_dst1(&x),
            1e-12,
            "dst1",
        );
        assert_all_close(
            &run_dst(sillstone_math_dst2, &x),
            &model_dst2(&x),
            1e-12,
            "dst2",
        );
    }
}

#[test]
fn dst_pairs_invert_each_other() {
    for n in [1usize, 4, 7, 16] {
        let x = signal(n);
        // DST-I is its own inverse; DST-III inverts DST-II.
        let twice = run_dst(sillstone_math_dst1, &run_dst(sillstone_math_dst1, &x));
        assert_all_close(&twice, &x, 1e-12, "dst1 involution");
        let back = run_dst(sillstone_math_dst3, &run_dst(sillstone_math_dst2, &x));
        assert_all_close(&back, &x, 1e-12, "dst3(dst2(x))");
        let back = run_dst(sillstone_math_dst2, &run_dst(sillstone_math_dst3, &x));
        assert_all_close(&back, &x, 1e-12, "dst2(dst3(x))");
    }
}

#[test]
fn dst_refuses_null_and_empty() {
    let x = [1.0, 2.0, 3.0];
    let mut out = [0.0; 3];
    for f in [
        sillstone_math_dst1,
        sillstone_math_dst2,
        sillstone_math_dst3,
    ] {
        unsafe {
            assert_eq!(
                f(core::ptr::null(), 3, out.as_mut_ptr()),
                SILLSTONE_E_INVALID
            );
            assert_eq!(f(x.as_ptr(), 3, core::ptr::null_mut()), SILLSTONE_E_INVALID);
            assert_eq!(f(x.as_ptr(), 0, out.as_mut_ptr()), SILLSTONE_E_INVALID);
        }
    }
}

type RuleFn = unsafe extern "C" fn(*const f64, usize, f64, *mut f64) -> i32;

fn run_rule(f: RuleFn, y: &[f64], dx: f64) -> (i32, f64) {
    let mut out = f64::NAN;
    let status = unsafe { f(y.as_ptr(), y.len(), dx, &mut out) };
    (status, out)
}

#[test]
fn trapezoid_integrates_a_line_exactly() {
    // y = 3x + 1 on [0, 2] with 9 samples: integral = 3*2 + 2 = 8.
    let dx = 2.0 / 8.0;
    let y: Vec<f64> = (0..9).map(|i| 3.0 * (i as f64 * dx) + 1.0).collect();
    let (status, got) = run_rule(sillstone_math_trapezoid_samples, &y, dx);
    assert_eq!(status, SILLSTONE_OK);
    assert!(close(got, 8.0, 1e-12), "got {got}");

    // Two samples are one trapezoid; one sample has no width.
    assert_eq!(
        run_rule(sillstone_math_trapezoid_samples, &[2.0, 4.0], 0.5),
        (SILLSTONE_OK, 1.5)
    );
    assert_eq!(
        run_rule(sillstone_math_trapezoid_samples, &[9.0], 0.5),
        (SILLSTONE_OK, 0.0)
    );
}

#[test]
fn trapezoid_refuses_null_and_empty() {
    let y = [1.0, 2.0];
    let mut out = 0.0;
    unsafe {
        assert_eq!(
            sillstone_math_trapezoid_samples(core::ptr::null(), 2, 1.0, &mut out),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_trapezoid_samples(y.as_ptr(), 2, 1.0, core::ptr::null_mut()),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_trapezoid_samples(y.as_ptr(), 0, 1.0, &mut out),
            SILLSTONE_E_INVALID
        );
    }
}

#[test]
fn simpson_integrates_a_cubic_exactly() {
    // x^3 - 2x on [0, 2] with 9 samples (8 intervals): 4 - 4 = 0.
    let dx = 2.0 / 8.0;
    let y: Vec<f64> = (0..9)
        .map(|i| {
            let x = i as f64 * dx;
            x * x * x - 2.0 * x
        })
        .collect();
    let (status, got) = run_rule(sillstone_math_simpson_samples, &y, dx);
    assert_eq!(status, SILLSTONE_OK);
    assert!(got.abs() < 1e-12, "got {got}");

    // x^2 on [0, 1] with 3 samples: 1/3.
    let (status, got) = run_rule(sillstone_math_simpson_samples, &[0.0, 0.25, 1.0], 0.5);
    assert_eq!(status, SILLSTONE_OK);
    assert!(close(got, 1.0 / 3.0, 1e-12), "got {got}");
}

#[test]
fn simpson_needs_an_even_interval_count() {
    // 4 samples = 3 intervals, and fewer than 3 samples, are both refused.
    assert_eq!(
        run_rule(sillstone_math_simpson_samples, &[1.0; 4], 1.0).0,
        SILLSTONE_E_INVALID
    );
    assert_eq!(
        run_rule(sillstone_math_simpson_samples, &[1.0; 2], 1.0).0,
        SILLSTONE_E_INVALID
    );
    let y = [1.0; 3];
    let mut out = 0.0;
    unsafe {
        assert_eq!(
            sillstone_math_simpson_samples(core::ptr::null(), 3, 1.0, &mut out),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_simpson_samples(y.as_ptr(), 3, 1.0, core::ptr::null_mut()),
            SILLSTONE_E_INVALID
        );
    }
}

/// Calls an allocating export, copies the handed-out buffer, frees it.
fn alloc_call(
    op: impl Fn(*const f64, usize, *mut *mut f64, *mut usize) -> i32,
    input: &[f64],
) -> (i32, Vec<f64>) {
    let mut out: *mut f64 = core::ptr::null_mut();
    let mut out_len: usize = 0;
    let status = op(input.as_ptr(), input.len(), &mut out, &mut out_len);
    if status != SILLSTONE_OK {
        return (status, Vec::new());
    }
    let count = out_len / core::mem::size_of::<f64>();
    let copied = unsafe { core::slice::from_raw_parts(out, count) }.to_vec();
    unsafe { sillstone_math_free(out, out_len) };
    (status, copied)
}

#[test]
fn complex_log_is_the_principal_branch() {
    let run = |z: [f64; 2]| {
        alloc_call(
            |i, n, o, l| unsafe { sillstone_math_complex_log(i, n, o, l) },
            &z,
        )
    };

    let (status, got) = run([1.0, 1.0]);
    assert_eq!(status, SILLSTONE_OK);
    assert_all_close(&got, &[0.5 * 2.0f64.ln(), PI / 4.0], 1e-14, "ln(1+i)");

    // A negative real sits on the cut: arg = +pi.
    let (status, got) = run([-1.0, 0.0]);
    assert_eq!(status, SILLSTONE_OK);
    assert_all_close(&got, &[0.0, PI], 1e-14, "ln(-1)");

    let (status, got) = run([0.0, -2.0]);
    assert_eq!(status, SILLSTONE_OK);
    assert_all_close(&got, &[2.0f64.ln(), -PI / 2.0], 1e-14, "ln(-2i)");

    // z = 0 is outside the domain; a wrong-sized input is malformed.
    assert_eq!(run([0.0, 0.0]).0, SILLSTONE_E_REJECTED);
    assert_eq!(
        alloc_call(
            |i, n, o, l| unsafe { sillstone_math_complex_log(i, n, o, l) },
            &[1.0]
        )
        .0,
        SILLSTONE_E_INVALID
    );
    assert_eq!(
        alloc_call(
            |i, n, o, l| unsafe { sillstone_math_complex_log(i, n, o, l) },
            &[1.0; 4]
        )
        .0,
        SILLSTONE_E_INVALID
    );
}

fn naive_dft(flat: &[f64], sign: f64) -> Vec<f64> {
    let n = flat.len() / 2;
    let mut out = Vec::with_capacity(flat.len());
    for k in 0..n {
        let (mut re, mut im) = (0.0, 0.0);
        for j in 0..n {
            let ang = sign * 2.0 * PI * (k * j) as f64 / n as f64;
            let (s, c) = ang.sin_cos();
            re += flat[2 * j] * c - flat[2 * j + 1] * s;
            im += flat[2 * j] * s + flat[2 * j + 1] * c;
        }
        out.push(re);
        out.push(im);
    }
    out
}

#[test]
fn fft_n_and_ifft_n_cover_non_power_of_two_lengths() {
    for n in [1usize, 2, 3, 5, 6, 7, 12, 17] {
        let flat = signal(2 * n);
        let (status, spectrum) = alloc_call(
            |i, l, o, ol| unsafe { sillstone_math_fft_n(i, l, o, ol) },
            &flat,
        );
        assert_eq!(status, SILLSTONE_OK, "fft_n n={n}");
        assert_all_close(&spectrum, &naive_dft(&flat, -1.0), 1e-10, "fft_n vs DFT");

        let (status, back) = alloc_call(
            |i, l, o, ol| unsafe { sillstone_math_ifft_n(i, l, o, ol) },
            &spectrum,
        );
        assert_eq!(status, SILLSTONE_OK, "ifft_n n={n}");
        assert_all_close(&back, &flat, 1e-10, "ifft_n(fft_n(x))");

        // ifft_n alone is the conjugate-sign DFT scaled by 1/n.
        let want: Vec<f64> = naive_dft(&flat, 1.0).iter().map(|v| v / n as f64).collect();
        let (status, got) = alloc_call(
            |i, l, o, ol| unsafe { sillstone_math_ifft_n(i, l, o, ol) },
            &flat,
        );
        assert_eq!(status, SILLSTONE_OK);
        assert_all_close(&got, &want, 1e-10, "ifft_n vs DFT");
    }
}

#[test]
fn fft_n_and_ifft_n_refuse_malformed_input() {
    let fft = |x: &[f64]| {
        alloc_call(
            |i, l, o, ol| unsafe { sillstone_math_fft_n(i, l, o, ol) },
            x,
        )
        .0
    };
    let ifft = |x: &[f64]| {
        alloc_call(
            |i, l, o, ol| unsafe { sillstone_math_ifft_n(i, l, o, ol) },
            x,
        )
        .0
    };
    assert_eq!(fft(&[]), SILLSTONE_E_INVALID);
    assert_eq!(fft(&[1.0, 2.0, 3.0]), SILLSTONE_E_INVALID);
    assert_eq!(ifft(&[]), SILLSTONE_E_INVALID);
    assert_eq!(ifft(&[1.0, 2.0, 3.0]), SILLSTONE_E_INVALID);

    let x = [1.0, 0.0];
    let mut out: *mut f64 = core::ptr::null_mut();
    let mut out_len = 0usize;
    unsafe {
        assert_eq!(
            sillstone_math_ifft_n(core::ptr::null(), 2, &mut out, &mut out_len),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_ifft_n(x.as_ptr(), 2, core::ptr::null_mut(), &mut out_len),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_ifft_n(x.as_ptr(), 2, &mut out, core::ptr::null_mut()),
            SILLSTONE_E_INVALID
        );
    }
}

#[test]
fn dct3_2d_inverts_dct2_2d_on_rectangles() {
    for (w, h) in [(1usize, 1usize), (4, 4), (5, 3), (2, 7), (8, 6)] {
        let data = signal(w * h);
        let (status, coeffs) = alloc_call(
            |i, n, o, l| unsafe { sillstone_math_dct2_2d(i, n, w, h, o, l) },
            &data,
        );
        assert_eq!(status, SILLSTONE_OK, "dct2_2d {w}x{h}");
        let (status, back) = alloc_call(
            |i, n, o, l| unsafe { sillstone_math_dct3_2d(i, n, w, h, o, l) },
            &coeffs,
        );
        assert_eq!(status, SILLSTONE_OK, "dct3_2d {w}x{h}");
        assert_all_close(&back, &data, 1e-12, "dct3_2d(dct2_2d(x))");
    }
}

#[test]
fn dct3_2d_refuses_bad_geometry_and_null() {
    let data = [0.0; 12];
    let call = |len: usize, w: usize, h: usize| {
        alloc_call(
            |i, _n, o, l| unsafe { sillstone_math_dct3_2d(i, len, w, h, o, l) },
            &data,
        )
        .0
    };
    assert_eq!(call(12, 0, 12), SILLSTONE_E_INVALID);
    assert_eq!(call(12, 12, 0), SILLSTONE_E_INVALID);
    assert_eq!(call(12, 5, 2), SILLSTONE_E_INVALID);
    assert_eq!(call(11, 4, 3), SILLSTONE_E_INVALID);
    assert_eq!(call(12, usize::MAX, 2), SILLSTONE_E_INVALID);

    let mut out: *mut f64 = core::ptr::null_mut();
    let mut out_len = 0usize;
    unsafe {
        assert_eq!(
            sillstone_math_dct3_2d(core::ptr::null(), 12, 4, 3, &mut out, &mut out_len),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_dct3_2d(data.as_ptr(), 12, 4, 3, core::ptr::null_mut(), &mut out_len),
            SILLSTONE_E_INVALID
        );
        assert_eq!(
            sillstone_math_dct3_2d(data.as_ptr(), 12, 4, 3, &mut out, core::ptr::null_mut()),
            SILLSTONE_E_INVALID
        );
    }
}
