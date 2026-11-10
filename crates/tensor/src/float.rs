//! The floating-point element capability shared by the math and statistics layers.

use core::ops::{Add, Div, Mul, Neg, Sub};

use crate::dtype::Dtype;
use crate::half::{Bf16, F16};

/// A real-number element type: the four IEEE float widths this crate computes in.
///
/// Every kernel works in `f64` and narrows back to the element type, which is
/// how NumPy evaluates `float32`/`float16` loops on a host with no native
/// half-precision arithmetic. For `f32` inputs that means the result is the
/// correctly rounded value of the exact answer, which can differ from NumPy's
/// own `f32` kernel by one unit in the last place.
pub trait Float:
    Dtype
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + PartialOrd
{
    /// Widens to `f64`, the working precision of every kernel here.
    fn to_f64(self) -> f64;

    /// Narrows from `f64` using this type's own rounding.
    fn from_f64(x: f64) -> Self;

    /// True for NaN and infinite values.
    fn is_finite(self) -> bool;

    /// A NaN in this type.
    const NAN: Self;

    /// Positive infinity in this type.
    const INFINITY: Self;
}

impl Float for f32 {
    #[inline]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    #[inline]
    fn from_f64(x: f64) -> Self {
        x as f32
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.to_bits() & 0x7f80_0000 != 0x7f80_0000
    }

    const NAN: Self = f32::NAN;
    const INFINITY: Self = f32::INFINITY;
}

impl Float for f64 {
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }

    #[inline]
    fn from_f64(x: f64) -> Self {
        x
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.to_bits() & 0x7ff0_0000_0000_0000 != 0x7ff0_0000_0000_0000
    }

    const NAN: Self = f64::NAN;
    const INFINITY: Self = f64::INFINITY;
}

impl Float for F16 {
    #[inline]
    fn to_f64(self) -> f64 {
        F16::to_f64(self)
    }

    #[inline]
    fn from_f64(x: f64) -> Self {
        F16::from_f64(x)
    }

    #[inline]
    fn is_finite(self) -> bool {
        F16::is_finite(self)
    }

    const NAN: Self = F16::from_bits(0x7e00);
    const INFINITY: Self = F16::from_bits(0x7c00);
}

impl Float for Bf16 {
    #[inline]
    fn to_f64(self) -> f64 {
        Bf16::to_f64(self)
    }

    #[inline]
    fn from_f64(x: f64) -> Self {
        Bf16::from_f64(x)
    }

    #[inline]
    fn is_finite(self) -> bool {
        Bf16::is_finite(self)
    }

    const NAN: Self = Bf16::from_bits(0x7fc0);
    const INFINITY: Self = Bf16::from_bits(0x7f80);
}
