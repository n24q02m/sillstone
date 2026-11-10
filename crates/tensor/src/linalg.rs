//! High-performance Linear Algebra and cache-tiled Matrix Multiplication (GEMM).

use alloc::sync::Arc;
use alloc::vec;
use core::ops::{Add, Mul};

use crate::dtype::Dtype;
use crate::float::Float;
use crate::layout::c_strides;
use crate::tensor::Tensor;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// Cache-blocked row-major GEMM kernel that writes into a caller-owned buffer.
///
/// `a` is `m x k`, `b` is `k x n`, `out` is `m x n`; the three slices are
/// validated but no allocation or temporary buffer is created, so a caller
/// holding raw buffers (the C ABI, an SDK) can consume the result in place.
/// `out` is fully overwritten, so it need not be zeroed first.
pub fn gemm_into<T>(a: &[T], m: usize, k: usize, b: &[T], n: usize, out: &mut [T]) -> Result<()>
where
    T: Dtype + Add<Output = T> + Mul<Output = T>,
{
    let (Some(a_len), Some(b_len), Some(out_len)) =
        (m.checked_mul(k), k.checked_mul(n), m.checked_mul(n))
    else {
        return Err(Error::BadValue("matrix dimensions overflow usize"));
    };
    if a.len() != a_len || b.len() != b_len || out.len() != out_len {
        return Err(Error::BadValue(
            "matrix buffer length does not match its shape",
        ));
    }

    // Cache-blocked GEMM: Tile sizes (BLOCK_M, BLOCK_K, BLOCK_N)
    const BS: usize = 32;

    for i in 0..m {
        let c_row_off = i * n;
        for j in 0..n {
            out[c_row_off + j] = T::ZERO;
        }
    }

    for m_blk in (0..m).step_by(BS) {
        let m_end = (m_blk + BS).min(m);
        for k_blk in (0..k).step_by(BS) {
            let k_end = (k_blk + BS).min(k);
            for n_blk in (0..n).step_by(BS) {
                let n_end = (n_blk + BS).min(n);

                // Micro-kernel inside the 32x32 block
                for i in m_blk..m_end {
                    let a_row_off = i * k;
                    let c_row_off = i * n;
                    for p in k_blk..k_end {
                        let a_val = a[a_row_off + p];
                        let b_row_off = p * n;
                        for j in n_blk..n_end {
                            out[c_row_off + j] = out[c_row_off + j] + a_val * b[b_row_off + j];
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

impl<T: Dtype + Add<Output = T> + Mul<Output = T>> Tensor<T> {
    /// 2D Matrix multiplication (GEMM) with cache-blocking tiling.
    /// A is [M, K], B is [K, N], output is [M, N].
    pub fn matmul(&self, other: &Self) -> Result<Self> {
        if self.ndim() != 2 || other.ndim() != 2 {
            return Err(Error::BadValue("matmul currently requires 2D matrices"));
        }

        let m = self.shape()[0];
        let k1 = self.shape()[1];
        let k2 = other.shape()[0];
        let n = other.shape()[1];

        if k1 != k2 {
            return Err(Error::BadValue(
                "inner matrix dimensions must agree for matmul",
            ));
        }
        let k = k1;

        let a_contig = self.contiguous();
        let b_contig = other.contiguous();
        let Some(out_len) = m.checked_mul(n) else {
            return Err(Error::BadValue("matrix dimensions overflow usize"));
        };
        let mut c_data = vec![T::ZERO; out_len];

        gemm_into(
            a_contig.as_slice().unwrap_or(&[]),
            m,
            k,
            b_contig.as_slice().unwrap_or(&[]),
            n,
            &mut c_data,
        )?;

        let out_shape = vec![m, n];
        Ok(Self {
            strides: c_strides(&out_shape),
            shape: out_shape,
            offset: 0,
            data: Arc::new(c_data),
        })
    }
}

/// Order used by [`Tensor::norm_p`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormOrd {
    /// Sum of the absolute values (p = 1).
    One,
    /// Euclidean norm (p = 2).
    Two,
    /// Largest absolute value (p = infinity).
    Infinity,
}

/// Factors produced by [`Tensor::lu`], with `P * A == L * U`.
#[derive(Clone, Debug)]
pub struct Lu<T: Dtype> {
    /// Unit lower-triangular factor.
    pub l: Tensor<T>,
    /// Upper-triangular factor.
    pub u: Tensor<T>,
    /// Row permutation: `perm[i]` is the row of `A` that ended up at row `i`.
    pub perm: Vec<usize>,
}

/// Builds a float tensor of `T` from values computed in `f64`.
fn tensor_from_f64<T: Float>(shape: Vec<usize>, values: Vec<f64>) -> Tensor<T> {
    Tensor {
        strides: c_strides(&shape),
        shape,
        offset: 0,
        data: Arc::new(values.into_iter().map(T::from_f64).collect()),
    }
}

/// Row-major `f64` copy of a tensor that must be materializable.
fn to_f64_buffer<T: Float>(t: &Tensor<T>) -> Result<Vec<f64>> {
    let contig = t.contiguous();
    let slice = contig
        .as_slice()
        .ok_or(Error::BadValue("expected a materializable tensor"))?;
    Ok(slice.iter().map(|v| v.to_f64()).collect())
}

/// LU factorization with partial pivoting, in place on a row-major `n x n`
/// buffer. The multipliers end up below the diagonal, `U` on and above it.
/// Returns the row permutation, or an error when an exact zero pivot shows the
/// matrix is singular.
fn lu_factor(a: &mut [f64], n: usize) -> Result<Vec<usize>> {
    if a.len() != n * n {
        return Err(Error::BadValue(
            "matrix buffer length does not match its shape",
        ));
    }
    let mut perm: Vec<usize> = (0..n).collect();
    for k in 0..n {
        let mut pivot = k;
        let mut best = a[k * n + k].abs();
        for i in (k + 1)..n {
            let v = a[i * n + k].abs();
            if v > best {
                best = v;
                pivot = i;
            }
        }
        if best == 0.0 {
            return Err(Error::BadValue("matrix is singular to working precision"));
        }
        if pivot != k {
            for j in 0..n {
                a.swap(k * n + j, pivot * n + j);
            }
            perm.swap(k, pivot);
        }
        let p = a[k * n + k];
        for i in (k + 1)..n {
            let factor = a[i * n + k] / p;
            a[i * n + k] = factor;
            for j in (k + 1)..n {
                a[i * n + j] -= factor * a[k * n + j];
            }
        }
    }
    Ok(perm)
}

/// Solves `A x = b` from an LU factorization; `m` right-hand sides, row-major.
fn lu_solve(lu: &[f64], perm: &[usize], n: usize, b: &[f64], m: usize) -> Vec<f64> {
    let mut y = vec![0.0f64; n * m];
    for i in 0..n {
        for col in 0..m {
            let mut acc = b[perm[i] * m + col];
            for j in 0..i {
                acc -= lu[i * n + j] * y[j * m + col];
            }
            y[i * m + col] = acc;
        }
    }
    let mut x = vec![0.0f64; n * m];
    for i in (0..n).rev() {
        for col in 0..m {
            let mut acc = y[i * m + col];
            for j in (i + 1)..n {
                acc -= lu[i * n + j] * x[j * m + col];
            }
            x[i * m + col] = acc / lu[i * n + i];
        }
    }
    x
}

impl<T: Float> Tensor<T> {
    /// Dot product (NumPy `dot`) for 1-D and 2-D operands.
    ///
    /// `1-D . 1-D` returns a 0-d tensor, `2-D . 2-D` is [`Tensor::matmul`], and
    /// the mixed cases are matrix-vector and vector-matrix products. The vector
    /// cases accumulate in `f64`; `2-D . 2-D` keeps `matmul`'s accumulation in
    /// the element type. Higher ranks are refused rather than guessed at.
    pub fn dot(&self, other: &Self) -> Result<Self> {
        match (self.ndim(), other.ndim()) {
            (1, 1) => {
                let k = self.shape()[0];
                if other.shape()[0] != k {
                    return Err(Error::BadValue("vector lengths must agree for dot"));
                }
                let a = to_f64_buffer(self)?;
                let b = to_f64_buffer(other)?;
                let mut acc = 0.0f64;
                for i in 0..k {
                    acc += a[i] * b[i];
                }
                Ok(tensor_from_f64(Vec::new(), vec![acc]))
            }
            (2, 2) => self.matmul(other),
            (2, 1) => {
                let (m, k) = (self.shape()[0], self.shape()[1]);
                if other.shape()[0] != k {
                    return Err(Error::BadValue("inner dimensions must agree for dot"));
                }
                let a = to_f64_buffer(self)?;
                let b = to_f64_buffer(other)?;
                let mut out = vec![0.0f64; m];
                for i in 0..m {
                    let mut acc = 0.0f64;
                    for p in 0..k {
                        acc += a[i * k + p] * b[p];
                    }
                    out[i] = acc;
                }
                Ok(tensor_from_f64(vec![m], out))
            }
            (1, 2) => {
                let k = self.shape()[0];
                let (k2, n) = (other.shape()[0], other.shape()[1]);
                if k2 != k {
                    return Err(Error::BadValue("inner dimensions must agree for dot"));
                }
                let a = to_f64_buffer(self)?;
                let b = to_f64_buffer(other)?;
                let mut out = vec![0.0f64; n];
                for j in 0..n {
                    let mut acc = 0.0f64;
                    for p in 0..k {
                        acc += a[p] * b[p * n + j];
                    }
                    out[j] = acc;
                }
                Ok(tensor_from_f64(vec![n], out))
            }
            _ => Err(Error::BadValue(
                "dot currently requires 1-D or 2-D operands",
            )),
        }
    }

    /// Euclidean norm of a vector, Frobenius norm of a matrix — NumPy's
    /// `linalg.norm` with no `ord`.
    pub fn norm(&self) -> Result<T> {
        self.norm_p(NormOrd::Two)
    }

    /// p-norm of the *flattened* elements (p = 1, 2 or infinity).
    ///
    /// For a matrix this is the elementwise norm, i.e.
    /// `np.linalg.norm(a.ravel(), ord)`. NumPy's induced operator norms for
    /// matrices (`ord=1` is the largest column sum, `ord=inf` the largest row
    /// sum) are not implemented, so a matrix `ord=1` result differs from
    /// `np.linalg.norm(a, ord=1)` by design.
    pub fn norm_p(&self, ord: NormOrd) -> Result<T> {
        let values = to_f64_buffer(self)?;
        match ord {
            NormOrd::One => {
                let mut acc = 0.0f64;
                for v in values {
                    acc += v.abs();
                }
                Ok(T::from_f64(acc))
            }
            NormOrd::Two => {
                let mut acc = 0.0f64;
                for v in values {
                    acc += v * v;
                }
                Ok(T::from_f64(crate::kernels::sqrt(acc)))
            }
            NormOrd::Infinity => {
                let mut best = 0.0f64;
                for v in values {
                    let x = v.abs();
                    if x.is_nan() {
                        return Ok(T::NAN);
                    }
                    if x > best {
                        best = x;
                    }
                }
                Ok(T::from_f64(best))
            }
        }
    }

    /// LU factorization with partial pivoting: `P * A == L * U`.
    pub fn lu(&self) -> Result<Lu<T>> {
        let n = self.square_dimension()?;
        let mut a = to_f64_buffer(self)?;
        let perm = lu_factor(&mut a, n)?;

        let mut lower = vec![0.0f64; n * n];
        let mut upper = vec![0.0f64; n * n];
        for i in 0..n {
            for j in 0..n {
                if i > j {
                    lower[i * n + j] = a[i * n + j];
                } else if i == j {
                    lower[i * n + j] = 1.0;
                    upper[i * n + j] = a[i * n + j];
                } else {
                    upper[i * n + j] = a[i * n + j];
                }
            }
        }
        Ok(Lu {
            l: tensor_from_f64(vec![n, n], lower),
            u: tensor_from_f64(vec![n, n], upper),
            perm,
        })
    }

    /// Determinant via the LU factorization; `1.0` for the empty matrix.
    pub fn det(&self) -> Result<T> {
        let n = self.square_dimension()?;
        if n == 0 {
            return Ok(T::from_f64(1.0));
        }
        let mut a = to_f64_buffer(self)?;
        let perm = match lu_factor(&mut a, n) {
            Ok(p) => p,
            // A singular matrix has determinant zero; `lu_factor` refuses it
            // because a factorization does not exist, but the determinant does.
            Err(_) => return Ok(T::from_f64(0.0)),
        };
        let mut inversions = 0usize;
        for i in 0..n {
            for j in (i + 1)..n {
                if perm[i] > perm[j] {
                    inversions += 1;
                }
            }
        }
        let mut d = if inversions % 2 == 0 { 1.0 } else { -1.0 };
        for i in 0..n {
            d *= a[i * n + i];
        }
        Ok(T::from_f64(d))
    }

    /// Matrix inverse via the LU factorization; singular matrices are refused.
    pub fn inv(&self) -> Result<Self> {
        let n = self.square_dimension()?;
        let mut a = to_f64_buffer(self)?;
        let perm = lu_factor(&mut a, n)?;
        let mut identity = vec![0.0f64; n * n];
        for i in 0..n {
            identity[i * n + i] = 1.0;
        }
        let x = lu_solve(&a, &perm, n, &identity, n);
        Ok(tensor_from_f64(vec![n, n], x))
    }

    /// Solves `A x = b` for a vector or a matrix of right-hand sides.
    pub fn solve(&self, b: &Self) -> Result<Self> {
        let n = self.square_dimension()?;
        let m = match b.ndim() {
            1 => {
                if b.shape()[0] != n {
                    return Err(Error::BadValue(
                        "right-hand side length must match the matrix",
                    ));
                }
                1
            }
            2 => {
                if b.shape()[0] != n {
                    return Err(Error::BadValue(
                        "right-hand side rows must match the matrix",
                    ));
                }
                b.shape()[1]
            }
            _ => return Err(Error::BadValue("right-hand side must be 1-D or 2-D")),
        };
        let mut a = to_f64_buffer(self)?;
        let perm = lu_factor(&mut a, n)?;
        let rhs = to_f64_buffer(b)?;
        let x = lu_solve(&a, &perm, n, &rhs, m);
        if b.ndim() == 1 {
            Ok(tensor_from_f64(vec![n], x))
        } else {
            Ok(tensor_from_f64(vec![n, m], x))
        }
    }

    /// Validates a square 2-D operand and returns its side length.
    fn square_dimension(&self) -> Result<usize> {
        if self.ndim() != 2 {
            return Err(Error::BadValue("expected a 2-D matrix"));
        }
        if self.shape()[0] != self.shape()[1] {
            return Err(Error::BadValue("expected a square matrix"));
        }
        Ok(self.shape()[0])
    }
}
