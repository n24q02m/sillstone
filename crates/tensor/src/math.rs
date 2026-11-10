//! Elementwise elementary functions on tensors.
//!
//! Every function maps over the tensor's own elements, so a strided view
//! computes the same values as its materialized copy, and NaN/Inf propagate
//! through the kernels by IEEE rules. Results keep the element type: `sqrt` of
//! a `float32` tensor is a `float32` tensor, the same way NumPy's `float32`
//! loop returns `float32`.
//!
//! Integer tensors have no `sqrt`/`exp` here on purpose: NumPy promotes them to
//! `float64`, and this kernel has no dtype promotion — call
//! [`Tensor::astype`](crate::tensor::Tensor::astype) first.

use crate::float::Float;
use crate::kernels;
use crate::tensor::Tensor;
use sillstone_digest::Result;

impl<T: Float> Tensor<T> {
    /// Elementwise square root.
    pub fn sqrt(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::sqrt(x.to_f64())))
    }

    /// Elementwise `e^x`.
    pub fn exp(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::exp(x.to_f64())))
    }

    /// Elementwise natural logarithm.
    pub fn ln(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::ln(x.to_f64())))
    }

    /// Elementwise base-2 logarithm.
    pub fn log2(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::log2(x.to_f64())))
    }

    /// Elementwise base-10 logarithm.
    pub fn log10(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::log10(x.to_f64())))
    }

    /// Elementwise sine, in radians.
    pub fn sin(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::sin(x.to_f64())))
    }

    /// Elementwise cosine, in radians.
    pub fn cos(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::cos(x.to_f64())))
    }

    /// Elementwise tangent, in radians.
    pub fn tan(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::tan(x.to_f64())))
    }

    /// Elementwise arcsine, in radians; values outside `[-1, 1]` become NaN.
    pub fn asin(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::asin(x.to_f64())))
    }

    /// Elementwise arccosine, in radians; values outside `[-1, 1]` become NaN.
    pub fn acos(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::acos(x.to_f64())))
    }

    /// Elementwise arctangent, in radians.
    pub fn atan(&self) -> Self {
        self.unary_op(|x| T::from_f64(kernels::atan(x.to_f64())))
    }

    /// Elementwise `self ** exponent` with a scalar exponent (NumPy `power`).
    pub fn powf(&self, exponent: T) -> Self {
        let e = exponent.to_f64();
        self.unary_op(move |x| T::from_f64(kernels::pow(x.to_f64(), e)))
    }

    /// Elementwise `self ** other` with broadcasting (NumPy `power`).
    pub fn pow(&self, other: &Self) -> Result<Self> {
        self.binary_op(other, |a, b| {
            T::from_f64(kernels::pow(a.to_f64(), b.to_f64()))
        })
    }

    /// Elementwise two-argument arctangent with broadcasting (NumPy `arctan2`).
    pub fn atan2(&self, other: &Self) -> Result<Self> {
        self.binary_op(other, |a, b| {
            T::from_f64(kernels::atan2(a.to_f64(), b.to_f64()))
        })
    }
}
