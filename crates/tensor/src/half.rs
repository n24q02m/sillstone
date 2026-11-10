//! Half-precision element types: IEEE 754 `binary16` and `bfloat16`.
//!
//! The crate takes no third-party dependencies, so both formats live here as
//! raw bit patterns with their own conversion arithmetic. Every narrowing
//! conversion rounds to nearest, ties to even, the mode NumPy uses for
//! `astype(np.float16)`, overflow goes to infinity and underflow to a
//! subnormal or zero.

use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

use crate::dtype::Dtype;

/// IEEE 754 `binary16` (a.k.a. `float16`): 5 exponent bits, 10 mantissa bits.
#[derive(Clone, Copy, Default)]
pub struct F16(u16);

/// `bfloat16`: 8 exponent bits, 7 mantissa bits — the top half of an `f32`.
#[derive(Clone, Copy, Default)]
pub struct Bf16(u16);

/// Rounds `v` right by `shift` bits, ties to even.
fn rne_shift(v: u64, shift: u32) -> u64 {
    if shift == 0 {
        return v;
    }
    if shift >= 64 {
        // The value is far below half of the smallest quantum.
        return 0;
    }
    let q = v >> shift;
    let rem = v & ((1u64 << shift) - 1);
    let half = 1u64 << (shift - 1);
    if rem > half || (rem == half && (q & 1) == 1) {
        q + 1
    } else {
        q
    }
}

/// Rounds `x` to the nearest value of a binary float format with `mant`
/// explicit mantissa bits and `exp_bits` exponent bits, and returns its packed
/// bit pattern (sign in the top bit), ties to even.
fn round_to_format(x: f64, mant: u32, exp_bits: u32) -> u64 {
    let sign_shift = exp_bits + mant;
    let sign = (x.to_bits() >> 63) << sign_shift;
    let exp_max = (1u64 << exp_bits) - 1;
    let bias = (1i64 << (exp_bits - 1)) - 1;

    if x.is_nan() {
        // A NaN payload has no meaning in a narrower format; emit a quiet NaN.
        return sign | (exp_max << mant) | (1u64 << (mant - 1));
    }
    if x.is_infinite() {
        return sign | (exp_max << mant);
    }
    if x == 0.0 {
        return sign;
    }

    let bits = x.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i64;
    if raw_exp == 0 {
        // An f64 subnormal is smaller than the smallest nonzero value of both
        // formats this function serves.
        return sign;
    }

    let m = (bits & 0x000f_ffff_ffff_ffff) | (1u64 << 52);
    let mut e = raw_exp - 1023;
    let mut exp_field = e + bias;
    let mant_mask = (1u64 << mant) - 1;

    if exp_field >= 1 {
        let mut q = rne_shift(m, 52 - mant);
        if q == 1u64 << (mant + 1) {
            // Rounding carried out of the mantissa into the next binade.
            q >>= 1;
            e += 1;
            exp_field = e + bias;
        }
        if exp_field >= exp_max as i64 {
            return sign | (exp_max << mant);
        }
        return sign | ((exp_field as u64) << mant) | (q & mant_mask);
    }

    // Subnormal here: the quantum is 2^(1 - bias - mant).
    let shift = 52 - mant as i64 + (1 - bias - e);
    if shift <= 0 {
        return sign;
    }
    // A carry out of the subnormal mantissa lands on the smallest normal, whose
    // bit pattern is exactly `1 << mant`.
    sign | rne_shift(m, shift as u32)
}

impl F16 {
    /// Wraps a raw `binary16` bit pattern.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the raw `binary16` bit pattern.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Widens to `f32`; every `binary16` value is exactly representable.
    pub fn to_f32(self) -> f32 {
        let sign = u32::from(self.0 & 0x8000) << 16;
        let exp = (self.0 >> 10) & 0x1f;
        let mant = u32::from(self.0 & 0x03ff);
        match exp {
            0 => {
                if mant == 0 {
                    f32::from_bits(sign)
                } else {
                    // Subnormal: mant * 2^-24, exact in f32.
                    let mag = mant as f32 * f32::from_bits(0x3380_0000);
                    if sign == 0 { mag } else { -mag }
                }
            }
            0x1f => {
                if mant == 0 {
                    f32::from_bits(sign | 0x7f80_0000)
                } else {
                    f32::NAN
                }
            }
            _ => f32::from_bits(sign | ((u32::from(exp) + 127 - 15) << 23) | (mant << 13)),
        }
    }

    /// Widens to `f64`.
    pub fn to_f64(self) -> f64 {
        f64::from(self.to_f32())
    }

    /// Narrows an `f32` with round-to-nearest-even.
    pub fn from_f32(x: f32) -> Self {
        Self::from_f64(f64::from(x))
    }

    /// Narrows an `f64` with round-to-nearest-even.
    pub fn from_f64(x: f64) -> Self {
        Self(round_to_format(x, 10, 5) as u16)
    }

    /// True for a NaN pattern.
    pub fn is_nan(self) -> bool {
        (self.0 & 0x7c00) == 0x7c00 && (self.0 & 0x03ff) != 0
    }

    /// True when the value is neither NaN nor infinite.
    pub fn is_finite(self) -> bool {
        (self.0 & 0x7c00) != 0x7c00
    }

    /// True when the sign bit is set, including for `-0.0`.
    pub fn is_sign_negative(self) -> bool {
        self.0 & 0x8000 != 0
    }
}

impl Bf16 {
    /// Wraps a raw `bfloat16` bit pattern.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the raw `bfloat16` bit pattern.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Widens to `f32`; a `bfloat16` is the top half of an `f32`, so this is exact.
    pub fn to_f32(self) -> f32 {
        f32::from_bits(u32::from(self.0) << 16)
    }

    /// Widens to `f64`.
    pub fn to_f64(self) -> f64 {
        f64::from(self.to_f32())
    }

    /// Narrows an `f32` with round-to-nearest-even.
    pub fn from_f32(x: f32) -> Self {
        Self::from_f64(f64::from(x))
    }

    /// Narrows an `f64` with round-to-nearest-even.
    pub fn from_f64(x: f64) -> Self {
        Self(round_to_format(x, 7, 8) as u16)
    }

    /// True for a NaN pattern.
    pub fn is_nan(self) -> bool {
        (self.0 & 0x7f80) == 0x7f80 && (self.0 & 0x007f) != 0
    }

    /// True when the value is neither NaN nor infinite.
    pub fn is_finite(self) -> bool {
        (self.0 & 0x7f80) != 0x7f80
    }

    /// True when the sign bit is set, including for `-0.0`.
    pub fn is_sign_negative(self) -> bool {
        self.0 & 0x8000 != 0
    }
}

impl fmt::Debug for F16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "F16({})", self.to_f32())
    }
}

impl fmt::Debug for Bf16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bf16({})", self.to_f32())
    }
}

impl PartialEq for F16 {
    fn eq(&self, other: &Self) -> bool {
        self.to_f32() == other.to_f32()
    }
}

impl PartialOrd for F16 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_f32().partial_cmp(&other.to_f32())
    }
}

impl PartialEq for Bf16 {
    fn eq(&self, other: &Self) -> bool {
        self.to_f32() == other.to_f32()
    }
}

impl PartialOrd for Bf16 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_f32().partial_cmp(&other.to_f32())
    }
}

macro_rules! impl_half_ops {
    ($t:ident) => {
        impl Add for $t {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self::from_f32(self.to_f32() + rhs.to_f32())
            }
        }
        impl Sub for $t {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self::from_f32(self.to_f32() - rhs.to_f32())
            }
        }
        impl Mul for $t {
            type Output = Self;
            fn mul(self, rhs: Self) -> Self {
                Self::from_f32(self.to_f32() * rhs.to_f32())
            }
        }
        impl Div for $t {
            type Output = Self;
            fn div(self, rhs: Self) -> Self {
                Self::from_f32(self.to_f32() / rhs.to_f32())
            }
        }
        impl Neg for $t {
            type Output = Self;
            fn neg(self) -> Self {
                Self(self.0 ^ 0x8000)
            }
        }
    };
}

impl_half_ops!(F16);
impl_half_ops!(Bf16);

impl Dtype for F16 {
    const ZERO: Self = F16(0x0000);
    const ONE: Self = F16(0x3c00);
}

impl Dtype for Bf16 {
    const ZERO: Self = Bf16(0x0000);
    const ONE: Self = Bf16(0x3f80);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2^k built from its bit pattern: `f64::powi` lives in `std`, not `core`.
    fn pow2(k: i32) -> f64 {
        f64::from_bits(((k + 1023) as u64) << 52)
    }

    #[test]
    fn every_binary16_pattern_widens_exactly() {
        // Exhaustive: the reference value is computed from the format's own
        // definition, independent of the bit-slicing in `to_f32`.
        for bits in 0u16..=u16::MAX {
            let h = F16::from_bits(bits);
            let got = h.to_f32();
            let sign = if bits & 0x8000 != 0 { -1.0f64 } else { 1.0 };
            let exp = ((bits >> 10) & 0x1f) as i32;
            let mant = (bits & 0x03ff) as f64;
            let expect = if exp == 0 && mant == 0.0 {
                sign * 0.0
            } else if exp == 0 {
                sign * mant * pow2(-24)
            } else if exp == 0x1f {
                if mant == 0.0 {
                    sign * f64::INFINITY
                } else {
                    f64::NAN
                }
            } else {
                sign * (1.0 + mant / 1024.0) * pow2(exp - 15)
            };
            if expect.is_nan() {
                assert!(got.is_nan(), "bits {bits:#06x} should be NaN");
            } else {
                assert_eq!(f64::from(got), expect, "bits {bits:#06x}");
                assert!(
                    got.is_sign_negative() == (bits & 0x8000 != 0),
                    "bits {bits:#06x} lost its sign"
                );
            }
        }
    }

    #[test]
    fn widening_and_narrowing_round_trip_through_f32() {
        for bits in 0u16..=u16::MAX {
            let h = F16::from_bits(bits);
            if h.is_nan() {
                assert!(F16::from_f32(h.to_f32()).is_nan(), "bits {bits:#06x}");
                continue;
            }
            assert_eq!(
                F16::from_f32(h.to_f32()).to_bits(),
                bits,
                "bits {bits:#06x}"
            );
        }
        for bits in 0u16..=u16::MAX {
            let b = Bf16::from_bits(bits);
            if b.is_nan() {
                assert!(Bf16::from_f32(b.to_f32()).is_nan(), "bits {bits:#06x}");
                continue;
            }
            assert_eq!(
                Bf16::from_f32(b.to_f32()).to_bits(),
                bits,
                "bits {bits:#06x}"
            );
        }
    }

    #[test]
    fn known_patterns_match_numpy() {
        // Values NumPy's float16 prints with an exact decimal expansion.
        let cases: &[(f64, u16)] = &[
            (0.0, 0x0000),
            (-0.0, 0x8000),
            (1.0, 0x3c00),
            (-2.0, 0xc000),
            (0.5, 0x3800),
            (65504.0, 0x7bff),
            (65519.0, 0x7bff),
            (65520.0, 0x7c00),
            (1e-7, 0x0002),
            (pow2(-24), 0x0001),
            (pow2(-25), 0x0000),
            (pow2(-24) * 1.5, 0x0002),
            (-0.333251953125, 0xb555),
            (1.0 / 3.0, 0x3555),
        ];
        for &(value, bits) in cases {
            assert_eq!(F16::from_f64(value).to_bits(), bits, "f16({value})");
        }
        assert_eq!(F16::from_f64(f64::INFINITY).to_bits(), 0x7c00);
        assert_eq!(F16::from_f64(f64::NEG_INFINITY).to_bits(), 0xfc00);
        assert!(F16::from_f64(f64::NAN).is_nan());

        // bfloat16 keeps the top 16 bits of the f32 pattern, rounding the rest.
        assert_eq!(Bf16::from_f64(1.0).to_bits(), 0x3f80);
        assert_eq!(Bf16::from_f64(-2.5).to_bits(), 0xc020);
        assert_eq!(Bf16::from_f64(3.3895313892515355e38).to_bits(), 0x7f7f);
        assert_eq!(Bf16::from_f64(f64::INFINITY).to_bits(), 0x7f80);
        assert!(Bf16::from_f64(f64::NAN).is_nan());
    }

    #[test]
    fn narrowing_rounds_ties_to_even() {
        // 1 + 2^-11 is exactly halfway between 1.0 and 1 + 2^-10 in binary16.
        let half_way = 1.0 + pow2(-11);
        assert_eq!(
            F16::from_f64(half_way).to_bits(),
            0x3c00,
            "tie goes to even (1.0)"
        );
        let next_tie = 1.0 + 3.0 * pow2(-11); // halfway between 1+2^-10 and 1+2^-9
        assert_eq!(
            F16::from_f64(next_tie).to_bits(),
            0x3c02,
            "tie goes to even (1+2^-9)"
        );
        // Just above a tie rounds up.
        assert_eq!(F16::from_f64(half_way * (1.0 + 1e-12)).to_bits(), 0x3c01);
        // bfloat16: 1 + 2^-8 is halfway between 1.0 and 1 + 2^-7.
        assert_eq!(Bf16::from_f64(1.0 + pow2(-8)).to_bits(), 0x3f80);
        assert_eq!(Bf16::from_f64(1.0 + 3.0 * pow2(-8)).to_bits(), 0x3f82);
    }

    #[test]
    fn arithmetic_rounds_through_the_wider_type() {
        let a = F16::from_f32(0.1);
        let b = F16::from_f32(0.2);
        assert_eq!(
            (a + b).to_f32(),
            F16::from_f32(a.to_f32() + b.to_f32()).to_f32()
        );
        assert_eq!(-a, F16::from_f32(-a.to_f32()));
        assert!((a / F16::ZERO).to_f32().is_infinite());
        assert!((F16::ZERO / F16::ZERO).is_nan());
        assert_eq!(F16::from_f32(1.0) / F16::from_f32(4.0), F16::from_f32(0.25));
        // NaN never compares equal, matching float semantics.
        let nan = F16::from_f64(f64::NAN);
        assert_ne!(nan, nan);
        assert_eq!(nan.partial_cmp(&nan), None);
        assert!(F16::from_f64(1.0) < F16::from_f64(2.0));
        assert!(F16::from_f64(-2.0) < F16::from_f64(-1.0));
    }
}
