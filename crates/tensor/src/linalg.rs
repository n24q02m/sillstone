//! High-performance Linear Algebra and cache-tiled Matrix Multiplication (GEMM).

use alloc::sync::Arc;
use alloc::vec;
use core::ops::{Add, Mul};

use crate::dtype::Dtype;
use crate::layout::c_strides;
use crate::tensor::Tensor;
use sillstone_digest::{Error, Result};

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
            return Err(Error::BadValue("inner matrix dimensions must agree for matmul"));
        }
        let k = k1;

        let a_contig = self.contiguous();
        let b_contig = other.contiguous();

        let a_slice = a_contig.as_slice().unwrap();
        let b_slice = b_contig.as_slice().unwrap();

        let mut c_data = vec![T::ZERO; m * n];

        // Cache-blocked GEMM: Tile sizes (BLOCK_M, BLOCK_K, BLOCK_N)
        const BS: usize = 32;

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
                            let a_val = a_slice[a_row_off + p];
                            let b_row_off = p * n;
                            for j in n_blk..n_end {
                                c_data[c_row_off + j] =
                                    c_data[c_row_off + j] + a_val * b_slice[b_row_off + j];
                            }
                        }
                    }
                }
            }
        }

        let out_shape = vec![m, n];
        Ok(Self {
            strides: c_strides(&out_shape),
            shape: out_shape,
            offset: 0,
            data: Arc::new(c_data),
        })
    }
}
