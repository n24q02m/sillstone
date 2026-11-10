//! Elementary function kernels written in plain `f64` arithmetic.
//!
//! `core` provides no `sqrt`, `exp`, `ln` or trigonometry, and this crate takes
//! no third-party dependencies, so the functions a processor needs are built
//! here from argument reduction plus polynomial approximations. The accuracy of
//! each kernel against NumPy is measured by the oracle tests in
//! `tests/numpy_oracle.rs` and recorded in `docs/numpy_parity.md`.
//!
//! Constants that are splits of a transcendental value are written as their bit
//! patterns: the split itself is what matters (24 significant bits per part for
//! the reduction constants), and a rounded decimal literal would silently
//! change it.

use core::f64::consts::{FRAC_2_PI, FRAC_PI_2, FRAC_PI_4, LOG10_E, PI, SQRT_2};

/// High part of `ln 2` with 33 significant bits, so `k * LN2_HI` is exact for
/// every exponent an `f64` can carry.
const LN2_HI: f64 = f64::from_bits(0x3fe6_2e42_fee0_0000);
/// The remaining bits of `ln 2`.
const LN2_LO: f64 = f64::from_bits(0x3dea_39ef_3579_3c76);
/// `1 / ln 2`.
const INV_LN2: f64 = f64::from_bits(0x3ff7_1547_652b_82fe);

/// `pi / 2`, first 24 significant bits.
const PIO2_1: f64 = f64::from_bits(0x3ff9_21fb_5440_0000);
/// `pi / 2`, next 24 bits.
const PIO2_2: f64 = f64::from_bits(0x3dd0_b461_1a60_0000);
/// `pi / 2`, next 24 bits.
const PIO2_3: f64 = f64::from_bits(0x3ba3_198a_2e00_0000);

/// 2^54, used to lift subnormals into the normal range.
const TWO_POW_54: f64 = f64::from_bits(0x4350_0000_0000_0000);
/// 2^-27, the matching correction.
const TWO_POW_NEG_27: f64 = f64::from_bits(0x3e40_0000_0000_0000);
/// Smallest positive normal `f64`.
const MIN_POSITIVE: f64 = f64::from_bits(0x0010_0000_0000_0000);

/// Above this magnitude the three-part `pi / 2` reduction is no longer exact,
/// because `n * PIO2_1` needs more than 53 bits. `f64::MAX |x| / 4096`.
const REDUCTION_LIMIT: f64 = 5.497_558_138_8e8;

/// Returns 2^k for `-1022 <= k <= 1023`, the normal exponent range.
fn pow2(k: i64) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// Multiplies `y` by 2^k, including into the subnormal range and to infinity.
fn scale_by_pow2(y: f64, k: i64) -> f64 {
    if k >= 1024 {
        return y * f64::INFINITY;
    }
    if k <= -1076 {
        return y * 0.0;
    }
    if k >= -1022 {
        return y * pow2(k);
    }
    // Subnormal result: split so the intermediate stays normal and only the
    // final multiply rounds.
    (y * pow2(k + 1022)) * MIN_POSITIVE
}

/// Truncates toward zero.
///
/// `f64::trunc` lives in `std`, not `core`, so the fraction bits are masked off
/// directly. Every exponent, including zero and the whole subnormal range, is
/// handled by the two early returns.
pub fn trunc(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32 - 1023;
    if exp >= 52 {
        return x;
    }
    if exp < 0 {
        return f64::from_bits(bits & 0x8000_0000_0000_0000);
    }
    let drop = 52 - exp as u32;
    let mask = (1u64 << drop) - 1;
    f64::from_bits(bits & !mask)
}

/// Square root, correct to within one unit in the last place.
pub fn sqrt(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 {
        return x;
    }
    // Negative input, negative infinity included, is a domain error.
    if x < 0.0 {
        return f64::NAN;
    }
    if x.is_infinite() {
        return x;
    }
    // Subnormals have no usable exponent for the seed below; scale them up and
    // undo the scaling after convergence.
    let (x, post) = if x < MIN_POSITIVE {
        (x * TWO_POW_54, TWO_POW_NEG_27)
    } else {
        (x, 1.0)
    };
    // Bit-trick seed (about 5-8 correct bits), then Newton iterations: each one
    // doubles the number of correct bits, so five reach the `f64` limit.
    let mut y = f64::from_bits(0x1ff8_0000_0000_0000u64.wrapping_add(x.to_bits() >> 1));
    let mut i = 0;
    while i < 5 {
        y = 0.5 * (y + x / y);
        i += 1;
    }
    y * post
}

/// `e^x`, correct to within a few units in the last place.
pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    // ln(f64::MAX) and ln of the smallest subnormal, with a little slack.
    if x > 709.782_712_893_384 {
        return f64::INFINITY;
    }
    if x < -745.133_219_101_941_1 {
        return 0.0;
    }
    // x = k * ln2 + r with |r| <= ln2 / 2.
    let k = if x >= 0.0 {
        (x * INV_LN2 + 0.5) as i64
    } else {
        (x * INV_LN2 - 0.5) as i64
    };
    let kf = k as f64;
    let mut r = x - kf * LN2_HI;
    r -= kf * LN2_LO;
    // Taylor series; at |r| <= 0.347 the 16th term is below 1e-21.
    let mut term = 1.0f64;
    let mut sum = 1.0f64;
    let mut i = 1;
    while i <= 16 {
        term *= r / i as f64;
        sum += term;
        i += 1;
    }
    scale_by_pow2(sum, k)
}

/// Natural logarithm.
pub fn ln(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    let mut e = 0i64;
    let mut x = x;
    if x < MIN_POSITIVE {
        x *= TWO_POW_54;
        e = -54;
    }
    let bits = x.to_bits();
    e += ((bits >> 52) & 0x7ff) as i64 - 1023;
    // Mantissa in [1, 2), folded into [sqrt(1/2), sqrt(2)) so the series variable
    // stays small.
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    // ln(m) = 2 atanh(s) with s = (m - 1) / (m + 1), |s| <= 0.172.
    let s = (m - 1.0) / (m + 1.0);
    let z = s * s;
    // sum_{i=0}^{10} z^i / (2i + 1), Horner form.
    let mut p = 1.0;
    let mut i = 10;
    while i > 0 {
        p = 1.0 / (2 * i + 1) as f64 + z * p;
        i -= 1;
    }
    let ln_m = 2.0 * s * (1.0 + z * p);
    let ef = e as f64;
    (ln_m + ef * LN2_LO) + ef * LN2_HI
}

/// Base-2 logarithm; exact for powers of two.
pub fn log2(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    let bits = x.to_bits();
    let raw = ((bits >> 52) & 0x7ff) as i64;
    if raw != 0 && bits & 0x000f_ffff_ffff_ffff == 0 {
        return (raw - 1023) as f64;
    }
    ln(x) * core::f64::consts::LOG2_E
}

/// Base-10 logarithm; exact at `1.0`.
pub fn log10(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    if x == 1.0 {
        return 0.0;
    }
    ln(x) * LOG10_E
}

/// Sine kernel on `|r| <= pi / 4`.
fn sin_kernel(r: f64) -> f64 {
    const S1: f64 = -1.666_666_666_666_663_2e-1;
    const S2: f64 = 0.008_333_333_333_322_49;
    const S3: f64 = -0.000_198_412_698_298_579_5;
    const S4: f64 = 2.755_731_370_707_006_8e-6;
    const S5: f64 = -2.505_076_025_340_686_3e-8;
    const S6: f64 = 1.589_690_995_211_55e-10;
    let z = r * r;
    let s = S1 + z * (S2 + z * (S3 + z * (S4 + z * (S5 + z * S6))));
    r + (z * r) * s
}

/// Cosine kernel on `|r| <= pi / 4`.
fn cos_kernel(r: f64) -> f64 {
    const C1: f64 = 0.041_666_666_666_666_6;
    const C2: f64 = -1.388_888_888_887_411e-3;
    const C3: f64 = 0.000_024_801_587_289_476_73;
    const C4: f64 = -2.755_731_435_139_066_3e-7;
    const C5: f64 = 0.000_000_002_087_572_321_298_175;
    const C6: f64 = -1.135_964_755_778_819_5e-11;
    let z = r * r;
    let poly = C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6))));
    // 1 - z/2 + z^2 * poly
    1.0 - (0.5 * z - (z * z) * poly)
}

/// Reduces `x` to `r` in `[-pi/4, pi/4]` plus a quadrant, using a three-part
/// split of `pi / 2` so every product `n * PIO2_i` is exact.
fn reduce_pio2(x: f64) -> (f64, u8) {
    let n = if x >= 0.0 {
        (x * FRAC_2_PI + 0.5) as i64
    } else {
        (x * FRAC_2_PI - 0.5) as i64
    };
    let nf = n as f64;
    let mut r = x - nf * PIO2_1;
    r -= nf * PIO2_2;
    r -= nf * PIO2_3;
    (r, (n & 3) as u8)
}

/// Sine of `x` radians.
///
/// The argument reduction is exact for `|x| <= 2^29`; beyond that the reduced
/// angle loses bits and the result degrades (see `docs/numpy_parity.md`).
pub fn sin(x: f64) -> f64 {
    if !x.is_finite() || x.abs() > REDUCTION_LIMIT * 4096.0 {
        return f64::NAN;
    }
    let (r, q) = reduce_pio2(x);
    match q {
        0 => sin_kernel(r),
        1 => cos_kernel(r),
        2 => -sin_kernel(r),
        _ => -cos_kernel(r),
    }
}

/// Cosine of `x` radians; same reduction limits as [`sin`].
pub fn cos(x: f64) -> f64 {
    if !x.is_finite() || x.abs() > REDUCTION_LIMIT * 4096.0 {
        return f64::NAN;
    }
    let (r, q) = reduce_pio2(x);
    match q {
        0 => cos_kernel(r),
        1 => -sin_kernel(r),
        2 => -cos_kernel(r),
        _ => sin_kernel(r),
    }
}

/// Tangent of `x` radians; same reduction limits as [`sin`].
pub fn tan(x: f64) -> f64 {
    if !x.is_finite() || x.abs() > REDUCTION_LIMIT * 4096.0 {
        return f64::NAN;
    }
    let (r, q) = reduce_pio2(x);
    if q & 1 == 0 {
        sin_kernel(r) / cos_kernel(r)
    } else {
        -cos_kernel(r) / sin_kernel(r)
    }
}

/// `atan` table from the classic three-branch reduction: arctangents of the
/// reduction's fixed points, split into a high part and its tail.
const ATAN_HI: [f64; 4] = [
    0.463_647_609_000_806_1,
    FRAC_PI_4,
    0.982_793_723_247_329,
    FRAC_PI_2,
];
const ATAN_LO: [f64; 4] = [
    2.269_877_745_296_168_8e-17,
    3.061_616_997_868_383e-17,
    1.390_331_103_123_099_9e-17,
    6.123_233_995_736_766e-17,
];
const AT: [f64; 11] = [
    0.333_333_333_333_329_3,
    -1.999_999_999_987_648_3e-1,
    1.428_571_427_250_346_6e-1,
    -1.111_111_040_546_235_6e-1,
    9.090_887_133_436_507e-2,
    -7.691_876_205_044_83e-2,
    6.661_073_137_387_531e-2,
    -5.833_570_133_790_573e-2,
    4.976_877_994_615_932e-2,
    -3.653_157_274_421_691_5e-2,
    1.628_582_011_536_578_2e-2,
];

/// Two-argument arctangent on the full circle.
pub fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if y == 0.0 {
        if x.is_sign_negative() {
            return if y.is_sign_negative() { -PI } else { PI };
        }
        return y;
    }
    if x == 0.0 {
        return if y > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
    }
    if x.is_infinite() {
        if y.is_infinite() {
            let angle = if x > 0.0 { FRAC_PI_4 } else { 3.0 * FRAC_PI_4 };
            return if y > 0.0 { angle } else { -angle };
        }
        let angle = if x > 0.0 { 0.0 } else { PI };
        return if y > 0.0 { angle } else { -angle };
    }
    if y.is_infinite() {
        return if y > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
    }
    let a = atan(y / x);
    if x > 0.0 {
        a
    } else if y > 0.0 {
        a + PI
    } else {
        a - PI
    }
}

/// Arctangent, within a few units in the last place over the whole range.
pub fn atan(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x.is_infinite() {
        return if x > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
    }
    let sign = x < 0.0;
    let mut ax = if sign { -x } else { x };
    // Reduce to |a| <= tan(pi/12) before evaluating the odd polynomial.
    let id: usize;
    if ax < 0.4375 {
        id = 4;
    } else if ax < 1.1875 {
        if ax < 0.6875 {
            id = 0;
            ax = (2.0 * ax - 1.0) / (2.0 + ax);
        } else {
            id = 1;
            ax = (ax - 1.0) / (ax + 1.0);
        }
    } else if ax < 2.4375 {
        id = 2;
        ax = (ax - 1.5) / (1.0 + 1.5 * ax);
    } else {
        id = 3;
        ax = -1.0 / ax;
    }
    let z = ax * ax;
    let w = z * z;
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    let r = if id == 4 {
        ax - ax * (s1 + s2)
    } else {
        ATAN_HI[id] - ((ax * (s1 + s2) - ATAN_LO[id]) - ax)
    };
    if sign { -r } else { r }
}

/// Arcsine, exact at `+-1`.
pub fn asin(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 1.0 || x < -1.0 {
        return f64::NAN;
    }
    if x == 1.0 {
        return FRAC_PI_2;
    }
    if x == -1.0 {
        return -FRAC_PI_2;
    }
    let ax = x.abs();
    // (1 - ax) is exact by Sterbenz, so the complementary leg keeps its
    // precision even when ax is a hair below 1.
    let leg = sqrt((1.0 - ax) * (1.0 + ax));
    let r = atan2(ax, leg);
    if x < 0.0 { -r } else { r }
}

/// Arccosine, exact at `1`, `0` and `-1`.
pub fn acos(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 1.0 || x < -1.0 {
        return f64::NAN;
    }
    if x == 1.0 {
        return 0.0;
    }
    if x == -1.0 {
        return PI;
    }
    let ax = x.abs();
    let leg = sqrt((1.0 - ax) * (1.0 + ax));
    let r = atan2(leg, ax);
    if x < 0.0 { PI - r } else { r }
}

/// Splits `a` into two parts whose sum is `a` and whose product is exact in
/// `f64`; the basis of the compensated multiply below (Dekker).
fn split(a: f64) -> (f64, f64) {
    const SPLITTER: f64 = 134_217_729.0; // 2^27 + 1
    let c = SPLITTER * a;
    let hi = c - (c - a);
    (hi, a - hi)
}

/// Exact product: `(p, e)` with `p = a * b` and `p + e` the exact product.
fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    // Overflow in the splitter would poison the correction; the product itself
    // is already the best available answer in that case.
    if !p.is_finite() || p == 0.0 {
        return (p, 0.0);
    }
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    let e = ((ah * bh - p) + ah * bl + al * bh) + al * bl;
    (p, e)
}

/// True for a finite value with no fractional part.
fn is_integer(x: f64) -> bool {
    x.is_finite() && trunc(x) == x
}

/// True for an integer value that cannot be divided by two.
fn is_odd_integer(x: f64) -> bool {
    is_integer(x) && x.abs() < 9.007_199_254_740_992e15 && trunc(x * 0.5) * 2.0 != x
}

/// Raises `x` to the power `y` with the IEEE 754 special cases and a
/// compensated `exp(y * ln |x|)` elsewhere.
///
/// The relative error is a few units in the last place while `|y * ln x|` stays
/// moderate; it grows with that product because the exponent itself is an
/// `f64` (NumPy's `power` is accurate to one unit in the last place for all
/// inputs, so extreme exponents differ in the last digits).
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return 1.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if y == 1.0 {
        return x;
    }
    if y == 2.0 {
        return x * x;
    }
    if y == 0.5 {
        return if x == 0.0 { 0.0 } else { sqrt(x) };
    }
    if x == 0.0 {
        let neg = x.is_sign_negative() && is_odd_integer(y);
        return if y > 0.0 {
            if neg { -0.0 } else { 0.0 }
        } else if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    if x.is_infinite() {
        let odd = is_odd_integer(y);
        return if y > 0.0 {
            if x > 0.0 || !odd {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            }
        } else if x > 0.0 || !odd {
            0.0
        } else {
            -0.0
        };
    }
    if y.is_infinite() {
        let ax = x.abs();
        if ax == 1.0 {
            return 1.0;
        }
        return if (ax > 1.0) == (y > 0.0) {
            f64::INFINITY
        } else {
            0.0
        };
    }

    // Small integral exponents multiply out exactly: `4 ** 3` is `64.0`, while
    // `exp(3 * ln 4)` lands a few ulp away. Above 16 the repeated squaring costs
    // more roundings than the logarithm path, so the ordinary route resumes.
    if is_integer(y) && y.abs() <= 16.0 {
        let mut acc = 1.0f64;
        let mut base = x.abs();
        let mut k = y.abs() as u32;
        while k > 0 {
            if k & 1 == 1 {
                acc *= base;
            }
            base *= base;
            k >>= 1;
        }
        if y < 0.0 {
            acc = 1.0 / acc;
        }
        return if x < 0.0 && is_odd_integer(y) {
            -acc
        } else {
            acc
        };
    }

    let (hi, lo) = two_prod(y, ln(x.abs()));
    let mut r = exp(hi) * (1.0 + lo);
    if x < 0.0 {
        if !is_integer(y) {
            return f64::NAN;
        }
        if is_odd_integer(y) {
            r = -r;
        }
    }
    r
}
