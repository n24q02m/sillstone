//! Core N-dimensional array and Tensor data structure.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::dtype::Dtype;
use crate::layout::{advance_index, c_strides, check_bounds, is_c_contiguous, linear_offset};
use sillstone_digest::{Error, Result};

/// An N-dimensional strided tensor supporting zero-copy views and broadcasting.
#[derive(Clone)]
pub struct Tensor<T: Dtype> {
    pub(crate) shape: Vec<usize>,
    pub(crate) strides: Vec<isize>,
    pub(crate) offset: usize,
    pub(crate) data: Arc<Vec<T>>,
}
impl<T: Dtype> Tensor<T> {
    /// Creates a new contiguous tensor from shape and raw data buffer.
    pub fn from_vec(shape: Vec<usize>, data: Vec<T>) -> Result<Self> {
        let numel: usize = shape.iter().product();
        if numel != data.len() {
            return Err(Error::BadValue("data length does not match shape product"));
        }
        let strides = c_strides(&shape);
        Ok(Self {
            shape,
            strides,
            offset: 0,
            data: Arc::new(data),
        })
    }

    /// Creates a contiguous tensor that shares an existing buffer instead of copying it.
    ///
    /// The buffer must hold exactly `shape.iter().product()` elements; a buffer
    /// with slack would make `as_slice` hand out elements the shape does not
    /// describe. Use [`Tensor::slice`] on a parent tensor to view a prefix of a
    /// larger buffer.
    pub fn from_arc(shape: Vec<usize>, data: Arc<Vec<T>>) -> Result<Self> {
        let numel: usize = shape.iter().product();
        if numel != data.len() {
            return Err(Error::BadValue("data length does not match shape product"));
        }
        let strides = c_strides(&shape);
        Ok(Self {
            shape,
            strides,
            offset: 0,
            data,
        })
    }

    /// Creates a new contiguous tensor filled with zeros.
    pub fn zeros(shape: &[usize]) -> Self {
        let numel: usize = shape.iter().product();
        let data = vec![T::ZERO; numel];
        let strides = c_strides(shape);
        Self {
            shape: shape.to_vec(),
            strides,
            offset: 0,
            data: Arc::new(data),
        }
    }

    /// Creates a new contiguous tensor filled with ones.
    pub fn ones(shape: &[usize]) -> Self {
        let numel: usize = shape.iter().product();
        let data = vec![T::ONE; numel];
        let strides = c_strides(shape);
        Self {
            shape: shape.to_vec(),
            strides,
            offset: 0,
            data: Arc::new(data),
        }
    }

    /// Creates a new contiguous tensor filled with a scalar value.
    pub fn full(shape: &[usize], val: T) -> Self {
        let numel: usize = shape.iter().product();
        let data = vec![val; numel];
        let strides = c_strides(shape);
        Self {
            shape: shape.to_vec(),
            strides,
            offset: 0,
            data: Arc::new(data),
        }
    }

    /// Returns the shape slice.
    #[inline]
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the strides slice.
    #[inline]
    pub fn strides(&self) -> &[isize] {
        &self.strides
    }

    /// Returns the number of dimensions.
    #[inline]
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Returns the total number of elements.
    #[inline]
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Checks if elements are stored contiguously in standard C-order.
    #[inline]
    pub fn is_contiguous(&self) -> bool {
        self.offset == 0 && is_c_contiguous(&self.shape, &self.strides)
    }

    /// Returns a direct slice if the buffer is contiguous.
    pub fn as_slice(&self) -> Option<&[T]> {
        if self.is_contiguous() {
            Some(&self.data[..self.numel()])
        } else {
            None
        }
    }

    /// Consumes the tensor and returns its buffer without copying when this
    /// tensor is the only owner of an exactly-sized C-order buffer.
    ///
    /// Returns `None` when a copy is unavoidable — the buffer is still shared
    /// with another view, the view is strided or offset, or the buffer carries
    /// elements outside the shape. Callers that only need the elements can
    /// fall back to [`Tensor::to_vec`].
    pub fn into_data(self) -> Option<Vec<T>> {
        let numel = self.numel();
        if !self.is_contiguous() || self.data.len() != numel {
            return None;
        }
        Arc::try_unwrap(self.data).ok()
    }

    /// Materializes the elements in C order into a fresh `Vec`, copying them.
    pub fn to_vec(&self) -> Vec<T> {
        let numel = self.numel();
        if self.is_contiguous() && self.data.len() == numel {
            return self.data.to_vec();
        }
        let mut out = Vec::with_capacity(numel);
        if numel == 0 {
            return out;
        }
        if self.ndim() == 0 {
            if let Some(val) = self.get(&[]) {
                out.push(val);
            }
            return out;
        }
        let mut idx = vec![0usize; self.ndim()];
        loop {
            out.push(self.data[linear_offset(&idx, &self.strides, self.offset) as usize]);
            if !advance_index(&mut idx, &self.shape) {
                break;
            }
        }
        out
    }

    /// Reads element at the given N-dimensional index coordinate.
    pub fn get(&self, index: &[usize]) -> Option<T> {
        check_bounds(index, &self.shape).ok()?;
        let off = linear_offset(index, &self.strides, self.offset);
        if off < 0 || (off as usize) >= self.data.len() {
            None
        } else {
            Some(self.data[off as usize])
        }
    }

    /// Creates a reshaped view or copy if contiguous.
    pub fn reshape(&self, new_shape: &[usize]) -> Result<Self> {
        let current_numel = self.numel();
        let target_shape = new_shape.to_vec();

        // Handle negative dimension inference if requested (-1 as usize is not possible in usize, but 0 is empty)
        let new_numel: usize = target_shape.iter().product();
        if current_numel != new_numel {
            return Err(Error::BadValue(
                "total elements must remain constant in reshape",
            ));
        }

        if self.is_contiguous() {
            Ok(Self {
                shape: target_shape.clone(),
                strides: c_strides(&target_shape),
                offset: 0,
                data: Arc::clone(&self.data),
            })
        } else {
            // Need a contiguous copy first
            let contig = self.contiguous();
            Ok(Self {
                shape: target_shape.clone(),
                strides: c_strides(&target_shape),
                offset: 0,
                data: contig.data,
            })
        }
    }

    /// Transposes dimensions (permuting axes).
    /// If `axes` is None, reverses the order of dimensions (like NumPy's `.T`).
    pub fn transpose(&self, axes: Option<&[usize]>) -> Result<Self> {
        let n = self.ndim();
        let perm: Vec<usize> = match axes {
            Some(ax) => {
                if ax.len() != n {
                    return Err(Error::BadValue("axes length must match ndim"));
                }
                ax.to_vec()
            }
            None => (0..n).rev().collect(),
        };

        let mut new_shape = vec![0usize; n];
        let mut new_strides = vec![0isize; n];

        for (i, &axis) in perm.iter().enumerate() {
            if axis >= n {
                return Err(Error::BadValue("axis index out of bounds"));
            }
            new_shape[i] = self.shape[axis];
            new_strides[i] = self.strides[axis];
        }

        Ok(Self {
            shape: new_shape,
            strides: new_strides,
            offset: self.offset,
            data: Arc::clone(&self.data),
        })
    }

    /// Extracts a strided sub-slice without copying: `[start..end]` per dimension.
    pub fn slice(&self, ranges: &[(usize, usize)]) -> Result<Self> {
        if ranges.len() != self.ndim() {
            return Err(Error::BadValue("range dimension count mismatch"));
        }

        let mut new_shape = vec![0usize; self.ndim()];
        let mut new_offset = self.offset as isize;

        for (i, &(start, end)) in ranges.iter().enumerate() {
            if start > end || end > self.shape[i] {
                return Err(Error::BadValue("slice range out of bounds"));
            }
            new_shape[i] = end - start;
            new_offset += start as isize * self.strides[i];
        }

        if new_offset < 0 {
            return Err(Error::BadValue("negative memory offset in slice"));
        }

        Ok(Self {
            shape: new_shape,
            strides: self.strides.clone(),
            offset: new_offset as usize,
            data: Arc::clone(&self.data),
        })
    }

    /// Materializes any strided or sliced view into a fresh contiguous buffer.
    ///
    /// A view that already starts at the buffer origin with C-order strides is
    /// returned as-is only when the buffer holds exactly its elements; a leading
    /// slice of a larger tensor shares that prefix property but not the length,
    /// so it is copied to keep `data` and `shape` in agreement.
    pub fn contiguous(&self) -> Self {
        let numel = self.numel();
        if self.is_contiguous() && self.data.len() == numel {
            return self.clone();
        }

        let mut out_data = Vec::with_capacity(numel);

        if numel == 0 {
            // Nothing to copy; walking the index space would read past an empty buffer.
        } else if self.ndim() == 0 {
            if let Some(val) = self.get(&[]) {
                out_data.push(val);
            }
        } else {
            let mut idx = vec![0usize; self.ndim()];
            loop {
                let off = linear_offset(&idx, &self.strides, self.offset);
                out_data.push(self.data[off as usize]);
                if !advance_index(&mut idx, &self.shape) {
                    break;
                }
            }
        }

        Self {
            shape: self.shape.clone(),
            strides: c_strides(&self.shape),
            offset: 0,
            data: Arc::new(out_data),
        }
    }
}

impl<T: Dtype> fmt::Debug for Tensor<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Tensor(shape={:?}, strides={:?}, offset={})",
            self.shape, self.strides, self.offset
        )
    }
}
