//! Elementwise arithmetic and comparison operations with automatic broadcasting.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Add, Div, Mul, Neg, Sub};

use crate::broadcast::broadcast_shapes;
use crate::dtype::Dtype;
use crate::layout::{advance_index, c_strides, linear_offset};
use crate::tensor::Tensor;
use sillstone_digest::Result;

impl<T: Dtype + Add<Output = T>> Tensor<T> {
    /// Elementwise addition with broadcasting.
    pub fn add(&self, other: &Self) -> Result<Self> {
        self.binary_op(other, |a, b| a + b)
    }
}

impl<T: Dtype + Sub<Output = T>> Tensor<T> {
    /// Elementwise subtraction with broadcasting.
    pub fn sub(&self, other: &Self) -> Result<Self> {
        self.binary_op(other, |a, b| a - b)
    }
}

impl<T: Dtype + Mul<Output = T>> Tensor<T> {
    /// Elementwise multiplication with broadcasting.
    pub fn mul(&self, other: &Self) -> Result<Self> {
        self.binary_op(other, |a, b| a * b)
    }
}

impl<T: Dtype + Div<Output = T>> Tensor<T> {
    /// Elementwise division with broadcasting.
    pub fn div(&self, other: &Self) -> Result<Self> {
        self.binary_op(other, |a, b| a / b)
    }
}

impl<T: Dtype + Neg<Output = T>> Tensor<T> {
    /// Elementwise negation.
    pub fn neg(&self) -> Self {
        self.unary_op(|x| -x)
    }
}

impl<T: Dtype> Tensor<T> {
    /// General binary elementwise operation with broadcasting.
    pub fn binary_op<O: Dtype, F>(&self, other: &Self, op: F) -> Result<Tensor<O>>
    where
        F: Fn(T, T) -> O,
    {
        let out_shape = broadcast_shapes(self.shape(), other.shape())?;
        let a_bcast = self.broadcast_to(&out_shape)?;
        let b_bcast = other.broadcast_to(&out_shape)?;

        let numel: usize = out_shape.iter().product();
        let mut out_data = Vec::with_capacity(numel);

        if out_shape.is_empty() {
            let val_a = a_bcast.get(&[]).unwrap_or(T::ZERO);
            let val_b = b_bcast.get(&[]).unwrap_or(T::ZERO);
            out_data.push(op(val_a, val_b));
        } else {
            let mut idx = vec![0usize; out_shape.len()];
            loop {
                let off_a = linear_offset(&idx, a_bcast.strides(), a_bcast.offset);
                let off_b = linear_offset(&idx, b_bcast.strides(), b_bcast.offset);
                let val_a = a_bcast.data[off_a as usize];
                let val_b = b_bcast.data[off_b as usize];
                out_data.push(op(val_a, val_b));
                if !advance_index(&mut idx, &out_shape) {
                    break;
                }
            }
        }

        Ok(Tensor {
            strides: c_strides(&out_shape),
            shape: out_shape,
            offset: 0,
            data: Arc::new(out_data),
        })
    }

    /// General unary elementwise operation.
    pub fn unary_op<O: Dtype, F>(&self, op: F) -> Tensor<O>
    where
        F: Fn(T) -> O,
    {
        let numel = self.numel();
        let mut out_data = Vec::with_capacity(numel);

        if self.ndim() == 0 {
            if let Some(val) = self.get(&[]) {
                out_data.push(op(val));
            }
        } else {
            let mut idx = vec![0usize; self.ndim()];
            loop {
                let off = linear_offset(&idx, self.strides(), self.offset);
                out_data.push(op(self.data[off as usize]));
                if !advance_index(&mut idx, self.shape()) {
                    break;
                }
            }
        }

        Tensor {
            strides: c_strides(self.shape()),
            shape: self.shape().to_vec(),
            offset: 0,
            data: Arc::new(out_data),
        }
    }

    /// Elementwise equality comparison (`==`).
    pub fn eq(&self, other: &Self) -> Result<Tensor<bool>> {
        self.binary_op(other, |a, b| a == b)
    }

    /// Elementwise inequality comparison (`!=`).
    pub fn ne(&self, other: &Self) -> Result<Tensor<bool>> {
        self.binary_op(other, |a, b| a != b)
    }
}
