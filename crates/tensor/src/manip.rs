//! Shape combination: `concat`, `stack` and constant `pad`.
//!
//! All three walk the *output* index space in C order and pull each element
//! through the input's own strides, so a transposed or sliced input contributes
//! exactly the elements it exposes and the result is written once, in order.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use crate::dtype::Dtype;
use crate::layout::{advance_index, c_strides, linear_offset};
use crate::tensor::Tensor;
use sillstone_digest::{Error, Result};

/// Joins tensors along an existing axis (NumPy `concatenate`).
///
/// Every input must have the same rank and the same shape apart from `axis`.
/// An empty input list is an error, matching NumPy.
pub fn concat<T: Dtype>(tensors: &[Tensor<T>], axis: usize) -> Result<Tensor<T>> {
    let first = tensors
        .first()
        .ok_or(Error::BadValue("need at least one tensor to concatenate"))?;
    let ndim = first.ndim();
    if axis >= ndim {
        return Err(Error::BadValue("axis out of bounds"));
    }
    let mut out_shape = first.shape().to_vec();
    let mut axis_total = 0usize;
    for t in tensors {
        if t.ndim() != ndim {
            return Err(Error::BadValue("all tensors must have the same rank"));
        }
        for (i, (&got, &want)) in t.shape().iter().zip(out_shape.iter()).enumerate() {
            if i != axis && got != want {
                return Err(Error::BadValue(
                    "shapes differ outside the concatenation axis",
                ));
            }
        }
        axis_total = axis_total
            .checked_add(t.shape()[axis])
            .ok_or(Error::BadValue("concatenated axis length overflows usize"))?;
    }
    out_shape[axis] = axis_total;

    let numel: usize = out_shape.iter().product();
    let mut out = Vec::with_capacity(numel);
    if numel > 0 {
        let mut idx = vec![0usize; ndim];
        loop {
            // Which input owns this slot along `axis`, and at what offset.
            let mut remaining = idx[axis];
            let mut owner = 0usize;
            while remaining >= tensors[owner].shape()[axis] {
                remaining -= tensors[owner].shape()[axis];
                owner += 1;
            }
            let mut src_idx = idx.clone();
            src_idx[axis] = remaining;
            let src =
                linear_offset(&src_idx, tensors[owner].strides(), tensors[owner].offset) as usize;
            out.push(tensors[owner].data[src]);
            if !advance_index(&mut idx, &out_shape) {
                break;
            }
        }
    }

    Ok(Tensor {
        strides: c_strides(&out_shape),
        shape: out_shape,
        offset: 0,
        data: Arc::new(out),
    })
}

/// Joins tensors along a new axis (NumPy `stack`).
///
/// `axis` may be `0..=ndim`; every input must have exactly the same shape.
pub fn stack<T: Dtype>(tensors: &[Tensor<T>], axis: usize) -> Result<Tensor<T>> {
    let first = tensors
        .first()
        .ok_or(Error::BadValue("need at least one tensor to stack"))?;
    let ndim = first.ndim();
    if axis > ndim {
        return Err(Error::BadValue("axis out of bounds"));
    }
    for t in tensors {
        if t.shape() != first.shape() {
            return Err(Error::BadValue(
                "all tensors must have the same shape to stack",
            ));
        }
    }
    let mut out_shape = Vec::with_capacity(ndim + 1);
    for (i, &dim) in first.shape().iter().enumerate() {
        if i == axis {
            out_shape.push(tensors.len());
        }
        out_shape.push(dim);
    }
    if axis == ndim {
        out_shape.push(tensors.len());
    }

    let numel: usize = out_shape.iter().product();
    let mut out = Vec::with_capacity(numel);
    if numel > 0 {
        let mut idx = vec![0usize; ndim + 1];
        loop {
            // The stacked axis selects the input; the rest is the input index.
            let which = idx[axis];
            let mut src_idx = Vec::with_capacity(ndim);
            for (i, &c) in idx.iter().enumerate() {
                if i != axis {
                    src_idx.push(c);
                }
            }
            let t = &tensors[which];
            let src = linear_offset(&src_idx, t.strides(), t.offset) as usize;
            out.push(t.data[src]);
            if !advance_index(&mut idx, &out_shape) {
                break;
            }
        }
    }

    Ok(Tensor {
        strides: c_strides(&out_shape),
        shape: out_shape,
        offset: 0,
        data: Arc::new(out),
    })
}

impl<T: Dtype> Tensor<T> {
    /// Pads each axis with `value` (NumPy `pad` with `mode="constant"`).
    ///
    /// `pads[i]` is the `(before, after)` amount for axis `i`, so its length must
    /// match the rank of the tensor. A zero-sized input pads to a constant
    /// tensor of the requested shape, as NumPy does.
    pub fn pad(&self, pads: &[(usize, usize)], value: T) -> Result<Self> {
        if pads.len() != self.ndim() {
            return Err(Error::BadValue(
                "pad width count must match the tensor rank",
            ));
        }
        let mut out_shape = Vec::with_capacity(self.ndim());
        for (i, &(before, after)) in pads.iter().enumerate() {
            out_shape.push(
                before
                    .checked_add(self.shape()[i])
                    .and_then(|v| v.checked_add(after))
                    .ok_or(Error::BadValue("padded dimension overflows usize"))?,
            );
        }
        let numel: usize = out_shape.iter().product();
        let mut out = Vec::with_capacity(numel);
        if numel > 0 && self.ndim() == 0 {
            // A 0-d tensor with no pad width has exactly one element.
            out.push(self.get(&[]).unwrap_or(value));
        } else if numel > 0 {
            let mut idx = vec![0usize; self.ndim()];
            loop {
                let mut src_idx = Vec::with_capacity(self.ndim());
                let mut inside = true;
                for (i, &c) in idx.iter().enumerate() {
                    match c.checked_sub(pads[i].0) {
                        Some(s) if s < self.shape()[i] => src_idx.push(s),
                        _ => {
                            inside = false;
                            break;
                        }
                    }
                }
                if inside {
                    let src = linear_offset(&src_idx, self.strides(), self.offset) as usize;
                    out.push(self.data[src]);
                } else {
                    out.push(value);
                }
                if !advance_index(&mut idx, &out_shape) {
                    break;
                }
            }
        }

        Ok(Tensor {
            strides: c_strides(&out_shape),
            shape: out_shape,
            offset: 0,
            data: Arc::new(out),
        })
    }
}
