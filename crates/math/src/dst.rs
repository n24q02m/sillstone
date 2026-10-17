//! Discrete Sine Transforms: DST-I, DST-II, and DST-III.
//!
//! Standard and orthonormal variants conforming to the zero-dependency
//! mathematical suite.

use core::f64::consts::PI;

/// Computes the unscaled standard Discrete Sine Transform of type I (DST-I).
///
/// Formula:
/// `X_k = sum_{n=0}^{N-1} x_n * sin(pi * (k + 1) * (n + 1) / (N + 1))`
pub fn dst1(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    let denom = (n + 1) as f64;
    for k in 0..n {
        let mut sum = 0.0;
        let k1 = (k + 1) as f64;
        for (i, &val) in x.iter().enumerate() {
            let n1 = (i + 1) as f64;
            sum += val * (PI * k1 * n1 / denom).sin();
        }
        out.push(sum);
    }
    out
}

/// Computes the orthonormal Discrete Sine Transform of type I (DST-I).
///
/// Scaled by `sqrt(2 / (N + 1))`. This matrix is symmetric and orthogonal,
/// meaning `dst1_orthonormal(dst1_orthonormal(x)) == x`.
pub fn dst1_orthonormal(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let factor = (2.0 / (n + 1) as f64).sqrt();
    let mut out = dst1(x);
    for v in &mut out {
        *v *= factor;
    }
    out
}

/// Computes the standard Discrete Sine Transform of type II (DST-II).
///
/// Formula:
/// `X_k = sum_{n=0}^{N-1} x_n * sin(pi * (k + 1) * (n + 0.5) / N)`
pub fn dst2(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    let n_f = n as f64;
    for k in 0..n {
        let mut sum = 0.0;
        let k1 = (k + 1) as f64;
        for (i, &val) in x.iter().enumerate() {
            let n_half = i as f64 + 0.5;
            sum += val * (PI * k1 * n_half / n_f).sin();
        }
        out.push(sum);
    }
    out
}

/// Computes the orthonormal Discrete Sine Transform of type II (DST-II).
///
/// Multiplied by `sqrt(2 / N)` with the last row multiplied by `1 / sqrt(2)`.
pub fn dst2_orthonormal(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let base_factor = (2.0 / n as f64).sqrt();
    let mut out = dst2(x);
    for (k, v) in out.iter_mut().enumerate() {
        let ck = if k == n - 1 {
            1.0 / core::f64::consts::SQRT_2
        } else {
            1.0
        };
        *v *= base_factor * ck;
    }
    out
}

/// Computes the standard Discrete Sine Transform of type III (DST-III).
///
/// Inverse transform corresponding to DST-II.
pub fn dst3(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    let n_f = n as f64;
    for k in 0..n {
        let mut sum = 0.0;
        let k_half = k as f64 + 0.5;
        for (i, &val) in x.iter().enumerate() {
            let term = if i == n - 1 { 0.5 } else { 1.0 };
            let i1 = (i + 1) as f64;
            sum += term * val * (PI * k_half * i1 / n_f).sin();
        }
        out.push(sum);
    }
    out
}

/// Computes the orthonormal Discrete Sine Transform of type III (DST-III).
///
/// The exact transpose and inverse of `dst2_orthonormal`.
pub fn dst3_orthonormal(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let base_factor = (2.0 / n as f64).sqrt();
    let mut out = Vec::with_capacity(n);
    let n_f = n as f64;
    for k in 0..n {
        let mut sum = 0.0;
        let k_half = k as f64 + 0.5;
        for (i, &val) in x.iter().enumerate() {
            let ck = if i == n - 1 {
                1.0 / core::f64::consts::SQRT_2
            } else {
                1.0
            };
            let i1 = (i + 1) as f64;
            sum += ck * val * (PI * k_half * i1 / n_f).sin();
        }
        out.push(base_factor * sum);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dst1_inversion() {
        let x = [1.0, -2.5, 3.2, 0.8, -1.1];
        let fwd = dst1_orthonormal(&x);
        let back = dst1_orthonormal(&fwd);
        for (a, b) in x.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_dst2_dst3_inversion() {
        let x = [2.0, 1.5, -0.7, 4.1];
        let fwd = dst2_orthonormal(&x);
        let back = dst3_orthonormal(&fwd);
        for (a, b) in x.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }
}
