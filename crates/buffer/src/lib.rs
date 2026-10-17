//! Core contiguous memory, strided views, and buffer primitives for Sillstone.
//!
//! This crate provides the foundational data representations used across
//! multimedia, hashing, image decoding, and array computations.
//!
//! All operations strictly adhere to safe Rust with zero external dependencies.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Supported scalar data types within a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    /// Unsigned 8-bit integer.
    U8,
    /// Unsigned 16-bit integer.
    U16,
    /// Unsigned 32-bit integer.
    U32,
    /// Unsigned 64-bit integer.
    U64,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// 32-bit IEEE 754 floating point.
    F32,
    /// 64-bit IEEE 754 floating point.
    F64,
}

impl DataType {
    /// Returns the size in bytes of a single element of this data type.
    #[inline]
    #[must_use]
    pub const fn item_size(self) -> usize {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }
}

/// Error kinds encountered during buffer manipulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// The provided data length does not match shape and element size requirements.
    DimensionMismatch {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// The requested slice range is out of bounds.
    OutOfBounds {
        /// Requested index or offset.
        offset: usize,
        /// Maximum allowable offset.
        bound: usize,
    },
    /// Strides are inconsistent with the shape.
    InvalidStrides,
}

impl core::fmt::Display for BufferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "dimension mismatch: expected {expected} bytes, got {actual}"
                )
            }
            Self::OutOfBounds { offset, bound } => {
                write!(f, "out of bounds: offset {offset} exceeds bound {bound}")
            }
            Self::InvalidStrides => write!(f, "invalid strides for specified shape"),
        }
    }
}

impl std::error::Error for BufferError {}

/// An owned buffer storing raw byte contents alongside type, shape, and striding metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    dtype: DataType,
    shape: Vec<usize>,
    strides: Vec<usize>,
    data: Vec<u8>,
}

impl Buffer {
    /// Creates a new owned contiguous buffer from raw bytes, type, and shape.
    ///
    /// # Errors
    /// Returns `BufferError::DimensionMismatch` if `data.len()` does not equal
    /// the product of the dimensions multiplied by the data type's item size.
    pub fn from_vec(
        dtype: DataType,
        shape: Vec<usize>,
        data: Vec<u8>,
    ) -> Result<Self, BufferError> {
        let total_elements: usize = if shape.is_empty() {
            0
        } else {
            shape.iter().product()
        };
        let expected_bytes = total_elements.saturating_mul(dtype.item_size());
        if data.len() != expected_bytes {
            return Err(BufferError::DimensionMismatch {
                expected: expected_bytes,
                actual: data.len(),
            });
        }
        let strides = Self::compute_c_contiguous_strides(&shape, dtype.item_size());
        Ok(Self {
            dtype,
            shape,
            strides,
            data,
        })
    }

    /// Creates a zero-initialized buffer of the specified data type and shape.
    #[must_use]
    pub fn zeros(dtype: DataType, shape: Vec<usize>) -> Self {
        let total_elements: usize = if shape.is_empty() {
            0
        } else {
            shape.iter().product()
        };
        let total_bytes = total_elements.saturating_mul(dtype.item_size());
        let strides = Self::compute_c_contiguous_strides(&shape, dtype.item_size());
        Self {
            dtype,
            shape,
            strides,
            data: vec![0u8; total_bytes],
        }
    }

    /// Computes default row-major (C-contiguous) byte strides for a given shape.
    #[must_use]
    pub fn compute_c_contiguous_strides(shape: &[usize], item_size: usize) -> Vec<usize> {
        if shape.is_empty() {
            return Vec::new();
        }
        let mut strides = vec![0; shape.len()];
        let mut current_stride = item_size;
        for i in (0..shape.len()).rev() {
            strides[i] = current_stride;
            current_stride = current_stride.saturating_mul(shape[i]);
        }
        strides
    }

    /// Returns the scalar data type of the buffer.
    #[inline]
    #[must_use]
    pub const fn dtype(&self) -> DataType {
        self.dtype
    }

    /// Returns the shape slice.
    #[inline]
    #[must_use]
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the byte strides slice.
    #[inline]
    #[must_use]
    pub fn strides(&self) -> &[usize] {
        &self.strides
    }

    /// Returns the total element count.
    #[must_use]
    pub fn element_count(&self) -> usize {
        if self.shape.is_empty() {
            0
        } else {
            self.shape.iter().product()
        }
    }

    /// Returns the underlying raw byte slice.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns the underlying mutable raw byte slice.
    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Computes a standard 64-bit FNV-1a checksum of the buffer's raw byte payload.
    ///
    /// This provides a fast, deterministic, cross-language baseline for verifying
    /// memory integrity and ABI bridge consistency without external dependencies.
    #[must_use]
    pub fn checksum(&self) -> u64 {
        checksum_bytes(&self.data)
    }
}

/// Computes an FNV-1a 64-bit checksum over an arbitrary byte slice.
#[inline]
#[must_use]
pub fn checksum_bytes(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeros_and_properties() {
        let buf = Buffer::zeros(DataType::U8, vec![2, 4]);
        assert_eq!(buf.dtype(), DataType::U8);
        assert_eq!(buf.shape(), &[2, 4]);
        assert_eq!(buf.strides(), &[4, 1]);
        assert_eq!(buf.element_count(), 8);
        assert_eq!(buf.as_bytes().len(), 8);
        assert!(buf.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_dimension_mismatch() {
        let err = Buffer::from_vec(DataType::U16, vec![2, 3], vec![0u8; 10]);
        assert_eq!(
            err,
            Err(BufferError::DimensionMismatch {
                expected: 12,
                actual: 10
            })
        );
    }

    #[test]
    fn test_checksum_deterministic() {
        let data = b"sillstone pure rust zero dep".to_vec();
        let buf = Buffer::from_vec(DataType::U8, vec![data.len()], data).unwrap();
        let c1 = buf.checksum();
        let c2 = buf.checksum();
        assert_eq!(c1, c2);
        assert_ne!(c1, 0);
    }
}
