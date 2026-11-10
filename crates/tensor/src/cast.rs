//! Numeric dtype conversion (`astype`).
//!
//! The conversion table is fixed at compile time and the semantics are the ones
//! the processors rely on:
//!
//! - integer to integer **wraps** (two's complement), as NumPy's `astype` does;
//! - float to integer **truncates toward zero** and **saturates** at the
//!   target's bounds, including `NaN` to `0`. NumPy leaves an out-of-range
//!   float→int conversion undefined (x86 builds typically store the minimum
//!   integer), so saturating is a deliberate, documented choice;
//! - integer to float rounds to nearest even, losing precision above `2^53`;
//! - float to float rounds to nearest even, `float16`/`bfloat16` included;
//! - any width to `bool` is `value != 0` (`NaN` is true), and `bool` to any
//!   number is `0`/`1`.
//!
//! Every pair is total: `Tensor::astype` never fails, because the conversion
//! table is checked by the compiler.

use crate::dtype::Dtype;
use crate::half::{Bf16, F16};
use crate::layout::{advance_index, c_strides, linear_offset};
use crate::tensor::Tensor;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

/// Element conversion used by [`Tensor::astype`].
pub trait Cast<S: Dtype>: Dtype {
    /// Converts a single element.
    fn cast(self) -> S;
}

/// Implements `Cast` for one source type against a list of destinations.
macro_rules! cast_to_numbers {
    ($src:ty => $($dst:ty),+ $(,)?) => {
        $(
            impl Cast<$dst> for $src {
                #[inline]
                fn cast(self) -> $dst {
                    self as $dst
                }
            }
        )+
    };
}

cast_to_numbers!(f32 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(f64 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(i8 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(i16 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(i32 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(i64 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(u8 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(u16 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(u32 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(u64 => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_to_numbers!(usize => f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);

/// `bool` has no `as` conversion in Rust, so every width spells out its test.
macro_rules! cast_to_bool {
    ($($src:ty),+ $(,)?) => {
        $(
            impl Cast<bool> for $src {
                #[inline]
                fn cast(self) -> bool {
                    self != 0
                }
            }
        )+
    };
}

cast_to_bool!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

/// `bool` has no `as` conversion in Rust either, so it spells out `0` and `1`.
macro_rules! cast_from_bool {
    ($($dst:ty),+ $(,)?) => {
        $(
            impl Cast<$dst> for bool {
                #[inline]
                fn cast(self) -> $dst {
                    if self { 1 as $dst } else { 0 as $dst }
                }
            }
        )+
    };
}

cast_from_bool!(f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);

impl Cast<bool> for f32 {
    #[inline]
    fn cast(self) -> bool {
        self != 0.0
    }
}

impl Cast<bool> for f64 {
    #[inline]
    fn cast(self) -> bool {
        self != 0.0
    }
}

impl Cast<bool> for bool {
    #[inline]
    fn cast(self) -> bool {
        self
    }
}

impl Cast<bool> for F16 {
    #[inline]
    fn cast(self) -> bool {
        self.to_f32() != 0.0
    }
}

impl Cast<bool> for Bf16 {
    #[inline]
    fn cast(self) -> bool {
        self.to_f32() != 0.0
    }
}

/// Every primitive width narrows to `binary16` through `f64`, so the conversion
/// is a single round-to-nearest-even step.
macro_rules! cast_to_half {
    ($($src:ty),+ $(,)?) => {
        $(
            impl Cast<F16> for $src {
                #[inline]
                fn cast(self) -> F16 {
                    F16::from_f64(self as f64)
                }
            }
            impl Cast<Bf16> for $src {
                #[inline]
                fn cast(self) -> Bf16 {
                    Bf16::from_f64(self as f64)
                }
            }
        )+
    };
}

cast_to_half!(f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);

impl Cast<F16> for bool {
    #[inline]
    fn cast(self) -> F16 {
        F16::from_f64(if self { 1.0 } else { 0.0 })
    }
}

impl Cast<Bf16> for bool {
    #[inline]
    fn cast(self) -> Bf16 {
        Bf16::from_f64(if self { 1.0 } else { 0.0 })
    }
}

impl Cast<F16> for F16 {
    #[inline]
    fn cast(self) -> F16 {
        self
    }
}

impl Cast<Bf16> for Bf16 {
    #[inline]
    fn cast(self) -> Bf16 {
        self
    }
}

impl Cast<Bf16> for F16 {
    #[inline]
    fn cast(self) -> Bf16 {
        Bf16::from_f64(self.to_f64())
    }
}

impl Cast<F16> for Bf16 {
    #[inline]
    fn cast(self) -> F16 {
        F16::from_f64(self.to_f64())
    }
}

macro_rules! cast_from_half {
    ($src:ident, $($dst:ty),+ $(,)?) => {
        $(
            impl Cast<$dst> for $src {
                #[inline]
                fn cast(self) -> $dst {
                    self.to_f64() as $dst
                }
            }
        )+
    };
}

cast_from_half!(F16, f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);
cast_from_half!(Bf16, f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize);

impl<T: Dtype> Tensor<T> {
    /// Converts every element to `S` (NumPy `astype`).
    ///
    /// Works on views as well as on owned buffers and always materializes a
    /// fresh C-order result, so the element type of the output never shares a
    /// buffer with the input.
    pub fn astype<S>(&self) -> Tensor<S>
    where
        T: Cast<S>,
        S: Dtype,
    {
        let numel = self.numel();
        let mut out = Vec::with_capacity(numel);
        if numel > 0 {
            if self.ndim() == 0 {
                if let Some(val) = self.get(&[]) {
                    out.push(val.cast());
                }
            } else {
                let mut idx = vec![0usize; self.ndim()];
                loop {
                    let off = linear_offset(&idx, self.strides(), self.offset);
                    out.push(self.data[off as usize].cast());
                    if !advance_index(&mut idx, self.shape()) {
                        break;
                    }
                }
            }
        }
        Tensor {
            shape: self.shape().to_vec(),
            strides: c_strides(self.shape()),
            offset: 0,
            data: Arc::new(out),
        }
    }
}
