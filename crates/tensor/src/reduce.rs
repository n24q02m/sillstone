//! Reduction operations along specified axes (sum, mean, min, max, argmin, argmax).

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Add;

use crate::dtype::Dtype;
use crate::layout::{advance_index, c_strides, linear_offset};
use crate::tensor::Tensor;
use sillstone_digest::{Error, Result};

impl<T: Dtype + Add<Output = T>> Tensor<T> {
    /// Computes sum along an optional axis. If None, reduces all elements to a scalar (0D tensor).
    pub fn sum(&self, axis: Option<usize>) -> Result<Tensor<T>> {
        match axis {
            None => {
                let mut acc = T::ZERO;
                let contig = self.contiguous();
                for &val in contig.data.iter() {
                    acc = acc + val;
                }
                Ok(Tensor {
                    shape: vec![],
                    strides: vec![],
                    offset: 0,
                    data: Arc::new(vec![acc]),
                })
            }
            Some(ax) => {
                if ax >= self.ndim() {
                    return Err(Error::BadValue("axis out of bounds"));
                }
                let mut out_shape = Vec::with_capacity(self.ndim() - 1);
                for (i, &dim) in self.shape().iter().enumerate() {
                    if i != ax {
                        out_shape.push(dim);
                    }
                }

                let numel: usize = out_shape.iter().product();
                let mut out_data = vec![T::ZERO; numel.max(1)];

                if self.ndim() > 0 {
                    let mut idx = vec![0usize; self.ndim()];
                    loop {
                        let off = linear_offset(&idx, self.strides(), self.offset);
                        let val = self.data[off as usize];

                        // Compute coordinate in out_shape
                        let mut out_idx = Vec::with_capacity(out_shape.len());
                        for (i, &c) in idx.iter().enumerate() {
                            if i != ax {
                                out_idx.push(c);
                            }
                        }

                        let out_strides = c_strides(&out_shape);
                        let out_off = if out_shape.is_empty() {
                            0
                        } else {
                            linear_offset(&out_idx, &out_strides, 0) as usize
                        };

                        out_data[out_off] = out_data[out_off] + val;

                        if !advance_index(&mut idx, self.shape()) {
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
        }
    }
}

impl<T: Dtype + PartialOrd> Tensor<T> {
    /// Computes maximum along an optional axis.
    pub fn max(&self, axis: Option<usize>) -> Result<Tensor<T>> {
        if self.numel() == 0 {
            return Err(Error::BadValue("cannot compute max of empty tensor"));
        }
        match axis {
            None => {
                let contig = self.contiguous();
                let mut m = contig.data[0];
                for &val in contig.data.iter().skip(1) {
                    if val > m {
                        m = val;
                    }
                }
                Ok(Tensor {
                    shape: vec![],
                    strides: vec![],
                    offset: 0,
                    data: Arc::new(vec![m]),
                })
            }
            Some(ax) => {
                if ax >= self.ndim() {
                    return Err(Error::BadValue("axis out of bounds"));
                }
                let mut out_shape = Vec::with_capacity(self.ndim() - 1);
                for (i, &dim) in self.shape().iter().enumerate() {
                    if i != ax {
                        out_shape.push(dim);
                    }
                }

                let numel: usize = out_shape.iter().product();
                let mut out_data: Vec<Option<T>> = vec![None; numel.max(1)];

                let mut idx = vec![0usize; self.ndim()];
                loop {
                    let off = linear_offset(&idx, self.strides(), self.offset);
                    let val = self.data[off as usize];

                    let mut out_idx = Vec::with_capacity(out_shape.len());
                    for (i, &c) in idx.iter().enumerate() {
                        if i != ax {
                            out_idx.push(c);
                        }
                    }

                    let out_strides = c_strides(&out_shape);
                    let out_off = if out_shape.is_empty() {
                        0
                    } else {
                        linear_offset(&out_idx, &out_strides, 0) as usize
                    };

                    match out_data[out_off] {
                        Some(prev) => {
                            if val > prev {
                                out_data[out_off] = Some(val);
                            }
                        }
                        None => {
                            out_data[out_off] = Some(val);
                        }
                    }

                    if !advance_index(&mut idx, self.shape()) {
                        break;
                    }
                }

                let final_data = out_data.into_iter().map(|o| o.unwrap_or(T::ZERO)).collect();
                Ok(Tensor {
                    strides: c_strides(&out_shape),
                    shape: out_shape,
                    offset: 0,
                    data: Arc::new(final_data),
                })
            }
        }
    }

    /// Computes minimum along an optional axis.
    pub fn min(&self, axis: Option<usize>) -> Result<Tensor<T>> {
        if self.numel() == 0 {
            return Err(Error::BadValue("cannot compute min of empty tensor"));
        }
        match axis {
            None => {
                let contig = self.contiguous();
                let mut m = contig.data[0];
                for &val in contig.data.iter().skip(1) {
                    if val < m {
                        m = val;
                    }
                }
                Ok(Tensor {
                    shape: vec![],
                    strides: vec![],
                    offset: 0,
                    data: Arc::new(vec![m]),
                })
            }
            Some(ax) => {
                if ax >= self.ndim() {
                    return Err(Error::BadValue("axis out of bounds"));
                }
                let mut out_shape = Vec::with_capacity(self.ndim() - 1);
                for (i, &dim) in self.shape().iter().enumerate() {
                    if i != ax {
                        out_shape.push(dim);
                    }
                }

                let numel: usize = out_shape.iter().product();
                let mut out_data: Vec<Option<T>> = vec![None; numel.max(1)];

                let mut idx = vec![0usize; self.ndim()];
                loop {
                    let off = linear_offset(&idx, self.strides(), self.offset);
                    let val = self.data[off as usize];

                    let mut out_idx = Vec::with_capacity(out_shape.len());
                    for (i, &c) in idx.iter().enumerate() {
                        if i != ax {
                            out_idx.push(c);
                        }
                    }

                    let out_strides = c_strides(&out_shape);
                    let out_off = if out_shape.is_empty() {
                        0
                    } else {
                        linear_offset(&out_idx, &out_strides, 0) as usize
                    };

                    match out_data[out_off] {
                        Some(prev) => {
                            if val < prev {
                                out_data[out_off] = Some(val);
                            }
                        }
                        None => {
                            out_data[out_off] = Some(val);
                        }
                    }

                    if !advance_index(&mut idx, self.shape()) {
                        break;
                    }
                }

                let final_data = out_data.into_iter().map(|o| o.unwrap_or(T::ZERO)).collect();
                Ok(Tensor {
                    strides: c_strides(&out_shape),
                    shape: out_shape,
                    offset: 0,
                    data: Arc::new(final_data),
                })
            }
        }
    }

    /// Computes index of the maximum element along the flattened tensor or specified axis.
    pub fn argmax(&self, axis: Option<usize>) -> Result<Tensor<usize>> {
        if self.numel() == 0 {
            return Err(Error::BadValue("cannot compute argmax of empty tensor"));
        }
        match axis {
            None => {
                let contig = self.contiguous();
                let mut best_idx = 0usize;
                let mut best_val = contig.data[0];
                for (i, &val) in contig.data.iter().enumerate().skip(1) {
                    if val > best_val {
                        best_val = val;
                        best_idx = i;
                    }
                }
                Ok(Tensor {
                    shape: vec![],
                    strides: vec![],
                    offset: 0,
                    data: Arc::new(vec![best_idx]),
                })
            }
            Some(ax) => {
                if ax >= self.ndim() {
                    return Err(Error::BadValue("axis out of bounds"));
                }
                let mut out_shape = Vec::with_capacity(self.ndim() - 1);
                for (i, &dim) in self.shape().iter().enumerate() {
                    if i != ax {
                        out_shape.push(dim);
                    }
                }

                let numel: usize = out_shape.iter().product();
                let mut best_vals: Vec<Option<T>> = vec![None; numel.max(1)];
                let mut best_indices = vec![0usize; numel.max(1)];

                let mut idx = vec![0usize; self.ndim()];
                loop {
                    let off = linear_offset(&idx, self.strides(), self.offset);
                    let val = self.data[off as usize];

                    let mut out_idx = Vec::with_capacity(out_shape.len());
                    for (i, &c) in idx.iter().enumerate() {
                        if i != ax {
                            out_idx.push(c);
                        }
                    }

                    let out_strides = c_strides(&out_shape);
                    let out_off = if out_shape.is_empty() {
                        0
                    } else {
                        linear_offset(&out_idx, &out_strides, 0) as usize
                    };

                    match best_vals[out_off] {
                        Some(prev) => {
                            if val > prev {
                                best_vals[out_off] = Some(val);
                                best_indices[out_off] = idx[ax];
                            }
                        }
                        None => {
                            best_vals[out_off] = Some(val);
                            best_indices[out_off] = idx[ax];
                        }
                    }

                    if !advance_index(&mut idx, self.shape()) {
                        break;
                    }
                }

                Ok(Tensor {
                    strides: c_strides(&out_shape),
                    shape: out_shape,
                    offset: 0,
                    data: Arc::new(best_indices),
                })
            }
        }
    }
}
