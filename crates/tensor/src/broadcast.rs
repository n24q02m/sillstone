//! NumPy-compatible multidimensional broadcasting rules.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use crate::dtype::Dtype;
use crate::tensor::Tensor;
use sillstone_digest::{Error, Result};

/// Computes the broadcasted shape between two tensor shapes following standard NumPy semantics.
pub fn broadcast_shapes(a: &[usize], b: &[usize]) -> Result<Vec<usize>> {
    let max_len = a.len().max(b.len());
    let mut out_shape = vec![0usize; max_len];

    for i in 0..max_len {
        let dim_a = if i < a.len() { a[a.len() - 1 - i] } else { 1 };
        let dim_b = if i < b.len() { b[b.len() - 1 - i] } else { 1 };

        if dim_a == dim_b {
            out_shape[max_len - 1 - i] = dim_a;
        } else if dim_a == 1 {
            out_shape[max_len - 1 - i] = dim_b;
        } else if dim_b == 1 {
            out_shape[max_len - 1 - i] = dim_a;
        } else {
            return Err(Error::BadValue("operands could not be broadcast together"));
        }
    }

    Ok(out_shape)
}

impl<T: Dtype> Tensor<T> {
    /// Broadcasts a tensor to a target shape by setting strides to 0 for expanded dimensions.
    pub fn broadcast_to(&self, target_shape: &[usize]) -> Result<Self> {
        let curr_ndim = self.ndim();
        let target_ndim = target_shape.len();

        if target_ndim < curr_ndim {
            return Err(Error::BadValue("target shape has fewer dimensions than tensor"));
        }

        let mut new_strides = vec![0isize; target_ndim];

        for i in 0..target_ndim {
            let target_dim = target_shape[target_ndim - 1 - i];
            if i < curr_ndim {
                let curr_dim = self.shape()[curr_ndim - 1 - i];
                let curr_stride = self.strides()[curr_ndim - 1 - i];

                if curr_dim == target_dim {
                    new_strides[target_ndim - 1 - i] = curr_stride;
                } else if curr_dim == 1 {
                    // Stride of 0 enables zero-copy broadcast
                    new_strides[target_ndim - 1 - i] = 0;
                } else {
                    return Err(Error::BadValue("cannot broadcast dimension"));
                }
            } else {
                // Prepended leading dimensions
                new_strides[target_ndim - 1 - i] = 0;
            }
        }

        Ok(Self {
            shape: target_shape.to_vec(),
            strides: new_strides,
            offset: self.offset,
            data: Arc::clone(&self.data),
        })
    }
}
