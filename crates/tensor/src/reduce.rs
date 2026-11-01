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
                // A 0-D result still holds one element; a zero-sized one holds none.
                let mut out_data = vec![T::ZERO; if out_shape.is_empty() { 1 } else { numel }];

                if self.ndim() > 0 && self.numel() > 0 {
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

/// True when `v` is unordered against itself, i.e. a floating-point NaN.
fn is_unordered<T: PartialOrd>(v: &T) -> bool {
    v.partial_cmp(v).is_none()
}

/// Whether `val` replaces `best` in a running maximum. A NaN wins once and
/// is never displaced, so the result does not depend on where the NaN sits
/// (NumPy propagates NaN through `max`/`argmax` the same way).
fn beats_max<T: PartialOrd>(val: &T, best: &T) -> bool {
    val > best || (is_unordered(val) && !is_unordered(best))
}

/// Minimum counterpart of [`beats_max`].
fn beats_min<T: PartialOrd>(val: &T, best: &T) -> bool {
    val < best || (is_unordered(val) && !is_unordered(best))
}

impl<T: Dtype + PartialOrd> Tensor<T> {
    /// Shape left after removing `axis`, or an error when it is out of range.
    fn shape_without(&self, axis: usize) -> Result<Vec<usize>> {
        if axis >= self.ndim() {
            return Err(Error::BadValue("axis out of bounds"));
        }
        Ok(self
            .shape()
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != axis)
            .map(|(_, &dim)| dim)
            .collect())
    }

    /// Running extreme (value and position along the reduced axis) over
    /// every lane; `better(val, best)` decides replacement, the first
    /// element of a lane always seeds it and ties keep the earlier one.
    fn extreme_along(
        &self,
        ax: usize,
        better: fn(&T, &T) -> bool,
    ) -> Result<(Vec<usize>, Vec<T>, Vec<usize>)> {
        let out_shape = self.shape_without(ax)?;
        let numel: usize = out_shape.iter().product();
        let out_strides = c_strides(&out_shape);
        let mut best: Vec<Option<T>> = vec![None; numel.max(1)];
        let mut best_at = vec![0usize; numel.max(1)];
        let mut idx = vec![0usize; self.ndim()];
        let mut out_idx = Vec::with_capacity(out_shape.len());
        loop {
            let val = self.data[linear_offset(&idx, self.strides(), self.offset) as usize];
            out_idx.clear();
            out_idx.extend(
                idx.iter()
                    .enumerate()
                    .filter(|&(i, _)| i != ax)
                    .map(|(_, &c)| c),
            );
            let out_off = if out_shape.is_empty() {
                0
            } else {
                linear_offset(&out_idx, &out_strides, 0) as usize
            };
            let replace = match &best[out_off] {
                Some(prev) => better(&val, prev),
                None => true,
            };
            if replace {
                best[out_off] = Some(val);
                best_at[out_off] = idx[ax];
            }
            if !advance_index(&mut idx, self.shape()) {
                break;
            }
        }
        let values = best.into_iter().map(|o| o.unwrap_or(T::ZERO)).collect();
        Ok((out_shape, values, best_at))
    }

    /// Flattened (C order) running extreme: value and position.
    fn extreme_all(&self, better: fn(&T, &T) -> bool) -> (T, usize) {
        let contig = self.contiguous();
        let mut best = contig.data[0];
        let mut best_at = 0usize;
        for (i, val) in contig.data.iter().enumerate().skip(1) {
            if better(val, &best) {
                best = *val;
                best_at = i;
            }
        }
        (best, best_at)
    }

    fn scalar_of<U: Dtype>(v: U) -> Tensor<U> {
        Tensor {
            shape: vec![],
            strides: vec![],
            offset: 0,
            data: Arc::new(vec![v]),
        }
    }

    fn reduce_value(
        &self,
        axis: Option<usize>,
        what: &'static str,
        better: fn(&T, &T) -> bool,
    ) -> Result<Tensor<T>> {
        if self.numel() == 0 {
            return Err(Error::BadValue(what));
        }
        match axis {
            None => Ok(Self::scalar_of(self.extreme_all(better).0)),
            Some(ax) => {
                let (shape, values, _) = self.extreme_along(ax, better)?;
                Ok(Tensor {
                    strides: c_strides(&shape),
                    shape,
                    offset: 0,
                    data: Arc::new(values),
                })
            }
        }
    }

    fn reduce_index(
        &self,
        axis: Option<usize>,
        what: &'static str,
        better: fn(&T, &T) -> bool,
    ) -> Result<Tensor<usize>> {
        if self.numel() == 0 {
            return Err(Error::BadValue(what));
        }
        match axis {
            None => Ok(Self::scalar_of(self.extreme_all(better).1)),
            Some(ax) => {
                let (shape, _, at) = self.extreme_along(ax, better)?;
                Ok(Tensor {
                    strides: c_strides(&shape),
                    shape,
                    offset: 0,
                    data: Arc::new(at),
                })
            }
        }
    }

    /// Computes maximum along an optional axis. A NaN anywhere in the lane
    /// makes the result NaN.
    pub fn max(&self, axis: Option<usize>) -> Result<Tensor<T>> {
        self.reduce_value(axis, "cannot compute max of empty tensor", beats_max)
    }

    /// Computes minimum along an optional axis. A NaN anywhere in the lane
    /// makes the result NaN.
    pub fn min(&self, axis: Option<usize>) -> Result<Tensor<T>> {
        self.reduce_value(axis, "cannot compute min of empty tensor", beats_min)
    }

    /// Computes index of the maximum element along the flattened tensor or
    /// specified axis (first occurrence; the first NaN if there is one).
    pub fn argmax(&self, axis: Option<usize>) -> Result<Tensor<usize>> {
        self.reduce_index(axis, "cannot compute argmax of empty tensor", beats_max)
    }

    /// Computes index of the minimum element along the flattened tensor or
    /// specified axis (first occurrence; the first NaN if there is one).
    pub fn argmin(&self, axis: Option<usize>) -> Result<Tensor<usize>> {
        self.reduce_index(axis, "cannot compute argmin of empty tensor", beats_min)
    }
}

macro_rules! impl_mean {
    ($t:ty) => {
        impl Tensor<$t> {
            /// Arithmetic mean along an optional axis (all elements when
            /// `None`). Refuses an empty reduction instead of dividing by
            /// zero.
            pub fn mean(&self, axis: Option<usize>) -> Result<Tensor<$t>> {
                let count = match axis {
                    None => self.numel(),
                    Some(ax) if ax < self.ndim() => self.shape()[ax],
                    Some(_) => return Err(Error::BadValue("axis out of bounds")),
                };
                if count == 0 {
                    return Err(Error::BadValue("cannot compute mean of empty tensor"));
                }
                let total = self.sum(axis)?;
                let data = total.data.iter().map(|&v| v / count as $t).collect();
                Ok(Tensor {
                    shape: total.shape,
                    strides: total.strides,
                    offset: 0,
                    data: Arc::new(data),
                })
            }
        }
    };
}

impl_mean!(f32);
impl_mean!(f64);
