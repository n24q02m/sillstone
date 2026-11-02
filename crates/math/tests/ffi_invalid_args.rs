//! Refusals of the C ABI that no other target drives: malformed arguments to
//! `complex_div`, `dct2_2d`, `corr` and `ransac_line`.
//!
//! Every case asserts the exact status code and that the output slots stay
//! untouched, so a refusal can never hand a half-built buffer to the caller.

use sillstone_math::ffi::{
    PITH_E_INVALID, PITH_E_REJECTED, PITH_OK, sillstone_math_complex_div, sillstone_math_corr,
    sillstone_math_dct2_2d, sillstone_math_free, sillstone_math_ransac_line,
};

/// Output slots as the caller would pass them.
struct Slots {
    out: *mut f64,
    out_len: usize,
}

impl Slots {
    fn new() -> Self {
        Self {
            out: core::ptr::null_mut(),
            out_len: 0,
        }
    }

    /// The refusal contract: nothing was allocated and nothing was written.
    fn assert_untouched(&self, what: &str) {
        assert!(self.out.is_null(), "{what}: out pointer was written");
        assert_eq!(self.out_len, 0, "{what}: out_len was written");
    }

    /// Copies the handed-out buffer (element count = bytes / 8) and frees it.
    fn take(&mut self) -> Vec<f64> {
        let count = self.out_len / core::mem::size_of::<f64>();
        let copied = unsafe { core::slice::from_raw_parts(self.out, count) }.to_vec();
        unsafe { sillstone_math_free(self.out, self.out_len) };
        copied
    }
}

#[test]
fn complex_div_by_zero_is_rejected_and_by_nonzero_succeeds() {
    // (5 + i) / 0: the domain refusal, not an argument error.
    let zero_den = [5.0, 1.0, 0.0, 0.0];
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_math_complex_div(
            zero_den.as_ptr(),
            zero_den.len(),
            &mut s.out,
            &mut s.out_len,
        )
    };
    assert_eq!(status, PITH_E_REJECTED);
    s.assert_untouched("complex_div by zero");

    // Only an all-zero denominator is refused: a purely imaginary one is fine.
    // scipy/numpy: (5 + 1j) / 2j = 0.5 - 2.5j (exact in binary).
    let imag_den = [5.0, 1.0, 0.0, 2.0];
    let status = unsafe {
        sillstone_math_complex_div(
            imag_den.as_ptr(),
            imag_den.len(),
            &mut s.out,
            &mut s.out_len,
        )
    };
    assert_eq!(status, PITH_OK);
    assert_eq!(s.take(), vec![0.5, -2.5]);

    // A wrong pair count is an argument error, not a rejection.
    let three = [1.0, 2.0, 3.0];
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_math_complex_div(three.as_ptr(), three.len(), &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, PITH_E_INVALID);
    s.assert_untouched("complex_div wrong length");
}

#[test]
fn dct2_2d_refuses_each_null_argument() {
    let data = [0.0f64; 4];
    let mut out: *mut f64 = core::ptr::null_mut();
    let mut out_len: usize = 0;

    let status =
        unsafe { sillstone_math_dct2_2d(core::ptr::null(), 4, 2, 2, &mut out, &mut out_len) };
    assert_eq!(status, PITH_E_INVALID, "null input");

    let status = unsafe {
        sillstone_math_dct2_2d(
            data.as_ptr(),
            data.len(),
            2,
            2,
            core::ptr::null_mut(),
            &mut out_len,
        )
    };
    assert_eq!(status, PITH_E_INVALID, "null out");

    let status = unsafe {
        sillstone_math_dct2_2d(
            data.as_ptr(),
            data.len(),
            2,
            2,
            &mut out,
            core::ptr::null_mut(),
        )
    };
    assert_eq!(status, PITH_E_INVALID, "null out_len");

    assert!(out.is_null());
    assert_eq!(out_len, 0);
}

#[test]
fn corr_refuses_a_bad_split_between_the_two_operands() {
    let packed = [1.0, 2.0, 3.0, 4.0];
    for (a_len, what) in [
        (0usize, "empty first operand"),
        (4, "empty second operand (a_len == len)"),
        (5, "first operand longer than the buffer"),
    ] {
        let mut s = Slots::new();
        let status = unsafe {
            sillstone_math_corr(
                packed.as_ptr(),
                packed.len(),
                a_len,
                &mut s.out,
                &mut s.out_len,
            )
        };
        assert_eq!(status, PITH_E_INVALID, "{what}");
        s.assert_untouched(what);
    }

    // The nearest valid split still works: a = [1,2,3], b = [4].
    // numpy.correlate(a, b, 'full') = a * 4 = [4, 8, 12].
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_math_corr(packed.as_ptr(), packed.len(), 3, &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, PITH_OK);
    assert_eq!(s.take(), vec![4.0, 8.0, 12.0]);
}

#[test]
fn ransac_line_refuses_null_and_malformed_configuration() {
    let line = [0.0, 1.0, 1.0, 2.0, 2.0, 3.0]; // (0,1) (1,2) (2,3)
    let mut out: *mut f64 = core::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        sillstone_math_ransac_line(core::ptr::null(), 6, 0.5, 16, 1, &mut out, &mut out_len)
    };
    assert_eq!(status, PITH_E_INVALID, "null input");

    // Configuration errors: too few points, odd packing, zero iterations,
    // non-positive or NaN threshold.
    let cases: [(&[f64], f64, usize, &str); 6] = [
        (&line[..2], 0.5, 16, "a single point"),
        (&line[..5], 0.5, 16, "odd packed length"),
        (&line, 0.5, 0, "zero iterations"),
        (&line, 0.0, 16, "zero threshold"),
        (&line, -1.0, 16, "negative threshold"),
        (&line, f64::NAN, 16, "NaN threshold"),
    ];
    for (pts, threshold, iterations, what) in cases {
        let mut s = Slots::new();
        let status = unsafe {
            sillstone_math_ransac_line(
                pts.as_ptr(),
                pts.len(),
                threshold,
                iterations,
                1,
                &mut s.out,
                &mut s.out_len,
            )
        };
        assert_eq!(status, PITH_E_INVALID, "{what}");
        s.assert_untouched(what);
    }

    // A well-formed run that can only draw vertical pairs finds no model:
    // the domain refusal, distinct from the argument errors above.
    let vertical = [2.0, 1.0, 2.0, 5.0, 2.0, 9.0];
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_math_ransac_line(
            vertical.as_ptr(),
            vertical.len(),
            0.5,
            64,
            1,
            &mut s.out,
            &mut s.out_len,
        )
    };
    assert_eq!(status, PITH_E_REJECTED);
    s.assert_untouched("all-vertical points");

    // And the valid call right next to those refusals succeeds, with the
    // exact model for collinear points: slope 1, intercept 1, 3 inliers.
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_math_ransac_line(
            line.as_ptr(),
            line.len(),
            0.5,
            16,
            1,
            &mut s.out,
            &mut s.out_len,
        )
    };
    assert_eq!(status, PITH_OK);
    assert_eq!(s.take(), vec![1.0, 1.0, 3.0]);
}
