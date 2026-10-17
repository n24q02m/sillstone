//! Numerical integration rules: trapezoid, composite Simpson's 1/3 and 3/8,
//! and Gauss-Legendre quadrature.

/// Composite trapezoidal rule for numerical integration.
///
/// Integrates `f` over `[a, b]` with `n` intervals (`n >= 1`).
pub fn trapezoid<F>(f: F, a: f64, b: f64, n: usize) -> f64
where
    F: Fn(f64) -> f64,
{
    if n == 0 || (a - b).abs() < f64::EPSILON {
        return 0.0;
    }
    let h = (b - a) / n as f64;
    let mut sum = 0.5 * (f(a) + f(b));
    for i in 1..n {
        let x = a + i as f64 * h;
        sum += f(x);
    }
    sum * h
}

/// Composite Simpson's 1/3 rule.
///
/// Integrates `f` over `[a, b]` with `n` intervals (`n` must be even and `>= 2`).
/// If `n` is odd, it is rounded up to `n + 1`.
pub fn simpson<F>(f: F, a: f64, b: f64, n: usize) -> f64
where
    F: Fn(f64) -> f64,
{
    let intervals = if n < 2 { 2 } else if n % 2 != 0 { n + 1 } else { n };
    let h = (b - a) / intervals as f64;
    let mut sum = f(a) + f(b);
    for i in 1..intervals {
        let x = a + i as f64 * h;
        if i % 2 == 0 {
            sum += 2.0 * f(x);
        } else {
            sum += 4.0 * f(x);
        }
    }
    sum * (h / 3.0)
}

/// Composite Simpson's 3/8 rule.
///
/// Integrates `f` over `[a, b]` with `n` intervals (`n` multiple of 3, `>= 3`).
/// If `n % 3 != 0`, it is rounded up to the next multiple of 3.
pub fn simpson_38<F>(f: F, a: f64, b: f64, n: usize) -> f64
where
    F: Fn(f64) -> f64,
{
    let intervals = if n < 3 {
        3
    } else {
        let rem = n % 3;
        if rem == 0 { n } else { n + (3 - rem) }
    };
    let h = (b - a) / intervals as f64;
    let mut sum = f(a) + f(b);
    for i in 1..intervals {
        let x = a + i as f64 * h;
        if i % 3 == 0 {
            sum += 2.0 * f(x);
        } else {
            sum += 3.0 * f(x);
        }
    }
    sum * (3.0 * h / 8.0)
}

/// Gauss-Legendre quadrature for polynomial exact integration.
///
/// Supported nodes: `2, 3, 4, 5`.
/// If `nodes` is not between 2 and 5, defaults to 5.
pub fn gauss_legendre<F>(f: F, a: f64, b: f64, nodes: usize) -> f64
where
    F: Fn(f64) -> f64,
{
    let (roots, weights): (&[f64], &[f64]) = match nodes {
        2 => {
            const R: [f64; 2] = [-0.5773502691896257, 0.5773502691896257];
            const W: [f64; 2] = [1.0, 1.0];
            (&R, &W)
        }
        3 => {
            const R: [f64; 3] = [-0.7745966692414834, 0.0, 0.7745966692414834];
            const W: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];
            (&R, &W)
        }
        4 => {
            const R: [f64; 4] = [
                -0.8611363115940526,
                -0.3399810435848563,
                0.3399810435848563,
                0.8611363115940526,
            ];
            const W: [f64; 4] = [
                0.34785484513745386,
                0.6521451548625461,
                0.6521451548625461,
                0.34785484513745386,
            ];
            (&R, &W)
        }
        _ => {
            // 5 nodes default
            const R: [f64; 5] = [
                -0.906179845938664,
                -0.5384693101056831,
                0.0,
                0.5384693101056831,
                0.906179845938664,
            ];
            const W: [f64; 5] = [
                0.2369268850561891,
                0.4786286704993665,
                0.5688888888888889,
                0.4786286704993665,
                0.2369268850561891,
            ];
            (&R, &W)
        }
    };

    let half_width = 0.5 * (b - a);
    let mid = 0.5 * (b + a);
    let mut sum = 0.0;
    for (&r, &w) in roots.iter().zip(weights.iter()) {
        sum += w * f(mid + half_width * r);
    }
    half_width * sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trapezoid_and_simpson() {
        // Integrate x^2 from 0 to 3: exact is 3^3 / 3 = 9.0
        let f = |x: f64| x * x;
        let trap = trapezoid(f, 0.0, 3.0, 100);
        assert!((trap - 9.0).abs() < 1e-2);

        let simp = simpson(f, 0.0, 3.0, 10);
        assert!((simp - 9.0).abs() < 1e-12);

        let simp38 = simpson_38(f, 0.0, 3.0, 12);
        assert!((simp38 - 9.0).abs() < 1e-12);
    }

    #[test]
    fn test_gauss_legendre() {
        // Integrate x^4 from 0 to 2: exact is 32/5 = 6.4
        let f = |x: f64| x * x * x * x;
        let res = gauss_legendre(f, 0.0, 2.0, 3);
        assert!((res - 6.4).abs() < 1e-12);
    }
}
