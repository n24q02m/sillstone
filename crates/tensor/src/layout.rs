//! N-dimensional array layout, shape, and strides management.

use alloc::vec;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// Computes standard C-order (row-major) contiguous strides for a given shape.
pub fn c_strides(shape: &[usize]) -> Vec<isize> {
    let mut strides = vec![0isize; shape.len()];
    let mut s = 1isize;
    for (i, &dim) in shape.iter().enumerate().rev() {
        strides[i] = s;
        s *= dim as isize;
    }
    strides
}

/// Checks whether a shape and strides describe a contiguous C-order buffer.
pub fn is_c_contiguous(shape: &[usize], strides: &[isize]) -> bool {
    if shape.is_empty() {
        return true;
    }
    let expected = c_strides(shape);
    strides == expected.as_slice()
}

/// Computes the linear memory offset for an N-dimensional index coordinate.
#[inline]
pub fn linear_offset(index: &[usize], strides: &[isize], base_offset: usize) -> isize {
    let mut offset = base_offset as isize;
    for (i, &idx) in index.iter().enumerate() {
        offset += idx as isize * strides[i];
    }
    offset
}

/// Validates that an index coordinate lies strictly within the shape bounds.
#[inline]
pub fn check_bounds(index: &[usize], shape: &[usize]) -> Result<()> {
    if index.len() != shape.len() {
        return Err(Error::BadValue("index dimension count mismatch"));
    }
    for (&idx, &dim) in index.iter().zip(shape.iter()) {
        if idx >= dim {
            return Err(Error::BadValue("index out of bounds"));
        }
    }
    Ok(())
}

/// Advances an N-dimensional index coordinate in row-major order.
/// Returns false when all coordinates have been traversed.
pub fn advance_index(index: &mut [usize], shape: &[usize]) -> bool {
    for i in (0..index.len()).rev() {
        index[i] += 1;
        if index[i] < shape[i] {
            return true;
        }
        index[i] = 0;
    }
    false
}
