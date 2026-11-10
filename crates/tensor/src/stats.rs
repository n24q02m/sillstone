//! Variance, standard deviation and the half-precision mean.
//!
//! The moments are accumulated in `f64` (two-pass: mean, then squared
//! deviations), which is what NumPy does for `float32`/`float16` inputs and
//! what keeps `float16` from losing every bit of a variance computed on small
//! numbers. The result is narrowed back to the element type.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use crate::dtype::Dtype;
use crate::float::Float;
use crate::half::{Bf16, F16};
use crate::layout::{advance_index, c_strides, linear_offset};
use crate::tensor::Tensor;
use sillstone_digest::{Error, Result};

/// Offset of `idx` inside the reduced output, skipping the reduced axis.
fn lane_offset(idx: &[usize], ax: usize, out_strides: &[isize]) -> usize {
    let mut off = 0isize;
    let mut j = 0;
    for (i, &c) in idx.iter().enumerate() {
        if i != ax {
            off += c as isize * out_strides[j];
            j += 1;
        }
    }
    off as usize
}

impl<T: Float> Tensor<T> {
    /// Shape and per-lane element sums of a reduction, accumulated in `f64`.
    fn lane_sums(&self, axis: Option<usize>) -> Result<(Vec<usize>, Vec<f64>, usize)> {
        let (out_shape, count) = match axis {
            None => (Vec::new(), self.numel()),
            Some(ax) => {
                if ax >= self.ndim() {
                    return Err(Error::BadValue("axis out of bounds"));
                }
                let mut shape = Vec::with_capacity(self.ndim() - 1);
                for (i, &dim) in self.shape().iter().enumerate() {
                    if i != ax {
                        shape.push(dim);
                    }
                }
                (shape, self.shape()[ax])
            }
        };
        let lanes: usize = if out_shape.is_empty() {
            1
        } else {
            out_shape.iter().product()
        };
        let mut sums = vec![0.0f64; lanes];
        if lanes == 0 || count == 0 || self.numel() == 0 {
            return Ok((out_shape, sums, count));
        }

        match axis {
            None => {
                let contig = self.contiguous();
                let slice = contig.as_slice().unwrap_or(&[]);
                let mut acc = 0.0f64;
                for &v in slice {
                    acc += v.to_f64();
                }
                sums[0] = acc;
            }
            Some(ax) => {
                let out_strides = c_strides(&out_shape);
                let mut idx = vec![0usize; self.ndim()];
                loop {
                    let val = self.data[linear_offset(&idx, self.strides(), self.offset) as usize];
                    sums[lane_offset(&idx, ax, &out_strides)] += val.to_f64();
                    if !advance_index(&mut idx, self.shape()) {
                        break;
                    }
                }
            }
        }
        Ok((out_shape, sums, count))
    }

    /// Per-lane variance (`ddof = 0` is the population variance).
    fn variance_values(&self, axis: Option<usize>, ddof: usize) -> Result<(Vec<usize>, Vec<f64>)> {
        let (out_shape, sums, count) = self.lane_sums(axis)?;
        if count == 0 {
            return Err(Error::BadValue(
                "cannot compute variance of an empty tensor",
            ));
        }
        if count <= ddof {
            return Err(Error::BadValue(
                "degrees of freedom must be smaller than the sample count",
            ));
        }
        let lanes = sums.len();
        let mut out = vec![0.0f64; lanes];
        if lanes == 0 || self.numel() == 0 {
            return Ok((out_shape, out));
        }
        let means: Vec<f64> = sums.iter().map(|s| s / count as f64).collect();
        let mut squared = vec![0.0f64; lanes];
        match axis {
            None => {
                let contig = self.contiguous();
                let slice = contig.as_slice().unwrap_or(&[]);
                let mean = means[0];
                let mut acc = 0.0f64;
                for &v in slice {
                    let d = v.to_f64() - mean;
                    acc += d * d;
                }
                squared[0] = acc;
            }
            Some(ax) => {
                let out_strides = c_strides(&out_shape);
                let mut idx = vec![0usize; self.ndim()];
                loop {
                    let val = self.data[linear_offset(&idx, self.strides(), self.offset) as usize];
                    let lane = lane_offset(&idx, ax, &out_strides);
                    let d = val.to_f64() - means[lane];
                    squared[lane] += d * d;
                    if !advance_index(&mut idx, self.shape()) {
                        break;
                    }
                }
            }
        }
        let denom = (count - ddof) as f64;
        for (slot, &ss) in out.iter_mut().zip(squared.iter()) {
            *slot = ss / denom;
        }
        Ok((out_shape, out))
    }

    /// Variance along an optional axis. `ddof = 0` is the population variance,
    /// `ddof = 1` the sample variance (NumPy's default is `0`).
    ///
    /// An empty reduction, or a `ddof` at least as large as the sample count,
    /// is an error rather than `nan` — the same choice [`Tensor::mean`] makes.
    pub fn var(&self, axis: Option<usize>, ddof: usize) -> Result<Tensor<T>> {
        let (shape, values) = self.variance_values(axis, ddof)?;
        Ok(Tensor {
            strides: c_strides(&shape),
            shape,
            offset: 0,
            data: Arc::new(values.into_iter().map(T::from_f64).collect()),
        })
    }

    /// Standard deviation along an optional axis: the square root of [`Tensor::var`].
    pub fn std(&self, axis: Option<usize>, ddof: usize) -> Result<Tensor<T>> {
        let (shape, values) = self.variance_values(axis, ddof)?;
        Ok(Tensor {
            strides: c_strides(&shape),
            shape,
            offset: 0,
            data: Arc::new(
                values
                    .into_iter()
                    .map(|v| T::from_f64(crate::kernels::sqrt(v)))
                    .collect(),
            ),
        })
    }
}

macro_rules! impl_half_mean {
    ($t:ident) => {
        impl Tensor<$t> {
            /// Arithmetic mean along an optional axis, accumulated in `f64`.
            ///
            /// `f32`/`f64` keep their own [`Tensor::mean`]; the half widths have
            /// no native accumulator worth summing in, so they widen.
            pub fn mean(&self, axis: Option<usize>) -> Result<Tensor<$t>> {
                let (shape, sums, count) = self.lane_sums(axis)?;
                if count == 0 {
                    return Err(Error::BadValue("cannot compute mean of empty tensor"));
                }
                let values: Vec<$t> = sums
                    .into_iter()
                    .map(|s| <$t>::from_f64(s / count as f64))
                    .collect();
                Ok(Tensor {
                    strides: c_strides(&shape),
                    shape,
                    offset: 0,
                    data: Arc::new(values),
                })
            }
        }
    };
}

impl_half_mean!(F16);
impl_half_mean!(Bf16);

/// Keeps the zero-size contract of the reductions above honest: a tensor with a
/// zero-length dimension still has a shape, and the moments of it are defined
/// only when the reduced count is positive.
pub fn _zero_size_guard<T: Dtype>(t: &Tensor<T>) -> bool {
    t.numel() == 0
}
