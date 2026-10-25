//! `imagehash`-compatible perceptual hashes: `average_hash`, `dhash`,
//! `phash`, `whash` and `colorhash`, bit-exact with the Python
//! [`imagehash`](https://github.com/JohannesBuchner/imagehash) library
//! (4.x, default parameters) on 8-bit RGB / RGBA / L input.
//!
//! Every hash starts from the same two Pillow steps — `convert("L")`
//! and `resize(.., LANCZOS)` — which [`crate::raster::rgb_to_l`] and
//! [`crate::raster::resize_u8`] reproduce bit for bit. After that:
//!
//! | hash | resize target | decision |
//! |---|---|---|
//! | [`image_ahash`] | 8×8 | `pixel > mean` |
//! | [`image_dhash`] | 9×8 (w×h) | `right > left` per row |
//! | [`image_phash_imagehash`] | 32×32 | 2-D DCT-II, low 8×8 `> median` |
//! | [`image_whash`] | `S×S`, `S = 2^⌊log2 min(w,h)⌋` | 8×8 Haar LL, DC removed, `> median` |
//! | [`image_colorhash`] | none | HSV bins: black, gray, 6+6 hue bins |
//!
//! Bit order: row-major, first element is the most significant bit (the
//! integer value of `imagehash`'s hex string). `colorhash` yields
//! `14 · binbits` bits in the low end of the `u64`.
//!
//! `imagehash` decides on floating-point values that tie at the median
//! (tiny or flat images), so those bits are defined by the reference's
//! rounding. `whash` therefore replays pywt's Haar transform operation
//! for operation in `f64` (verified bitwise against `pywt`); `phash`
//! evaluates the DCT sums in a fixed order and can differ from scipy's
//! FFT-based `dct` only on such ties.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use std::sync::LazyLock;

use crate::raster::{Image, Layout, ResamplingFilter, Sample, resize_u8, rgb_plane_to_l, rgb_to_hsv, rgb_to_l};
use sillstone_digest::Error;

/// Reduces one sample of any depth to `u8`: identity for `u8`, `v >> 8`
/// for `u16`.
#[inline]
fn to_u8<T: Sample>(v: T) -> u8 {
    if T::MAX_U64 == 255 {
        v.to_u64() as u8
    } else {
        (v.to_u64() >> 8) as u8
    }
}

/// Splits the image into 8-bit `(r, g, b)` triples through `f`.
fn map_pixels<L: Layout, T: Sample, R>(
    img: &Image<L, T>,
    what: &'static str,
    mut f: impl FnMut(u8, u8, u8) -> R,
) -> Result<Vec<R>, Error> {
    let n = img.width() as usize * img.height() as usize;
    let c = L::CHANNELS;
    let src = img.as_slice();
    if src.len() < n * c {
        return Err(Error::BadValue(what));
    }
    Ok(src
        .chunks_exact(c)
        .take(n)
        .map(|p| {
            if c >= 3 {
                f(to_u8(p[0]), to_u8(p[1]), to_u8(p[2]))
            } else {
                let l = to_u8(p[0]);
                f(l, l, l)
            }
        })
        .collect())
}

/// Pillow's `convert("L")` of the image. Gray 8-bit input is borrowed:
/// Pillow does not convert it either.
fn luma_plane<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<Cow<'_, [u8]>, Error> {
    let n = img.width() as usize * img.height() as usize;
    let c = L::CHANNELS;
    let src = img.as_slice();
    if src.len() < n * c {
        return Err(Error::BadValue("image buffer undersized"));
    }
    if let Some(bytes) = T::as_bytes(src) {
        return Ok(if c >= 3 {
            Cow::Owned(rgb_plane_to_l(bytes, c, n))
        } else {
            Cow::Borrowed(&bytes[..n])
        });
    }
    if c >= 3 {
        map_pixels(img, "image buffer undersized", rgb_to_l).map(Cow::Owned)
    } else {
        map_pixels(img, "image buffer undersized", |l, _, _| l).map(Cow::Owned)
    }
}

/// `convert("L").resize((w, h), LANCZOS)`.
fn lanczos_gray<L: Layout, T: Sample>(
    img: &Image<L, T>,
    w: usize,
    h: usize,
) -> Result<Vec<u8>, Error> {
    let gray = luma_plane(img)?;
    Ok(resize_u8(
        &gray,
        img.width() as usize,
        img.height() as usize,
        1,
        w,
        h,
        ResamplingFilter::Lanczos,
    ))
}

/// `imagehash.average_hash` (`hash_size = 8`).
pub fn image_ahash<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<u64, Error> {
    let px = lanczos_gray(img, 8, 8)?;
    let sum: u64 = px.iter().map(|&p| u64::from(p)).sum();
    // `pixel > mean(pixels)` with mean = sum / 64, exact in binary FP.
    let mut hash = 0u64;
    for (i, &p) in px.iter().enumerate() {
        if u64::from(p) * 64 > sum {
            hash |= 1u64 << (63 - i);
        }
    }
    Ok(hash)
}

/// `imagehash.dhash` (`hash_size = 8`): horizontal gradient on a 9×8 grid.
pub fn image_dhash<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<u64, Error> {
    let px = lanczos_gray(img, 9, 8)?;
    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            if px[y * 9 + x + 1] > px[y * 9 + x] {
                hash |= 1u64 << (63 - (y * 8 + x));
            }
        }
    }
    Ok(hash)
}

/// `cos(π·k·(2n+1) / 64)` for `k < 8`, `n < 32`.
static DCT_TABLE: LazyLock<[[f64; 32]; 8]> = LazyLock::new(|| {
    let mut t = [[0.0f64; 32]; 8];
    for (k, row) in t.iter_mut().enumerate() {
        for (n, v) in row.iter_mut().enumerate() {
            *v = (core::f64::consts::PI * k as f64 * (2 * n + 1) as f64 / 64.0).cos();
        }
    }
    t
});

/// Median of 64 floats as `numpy.median` computes it (mean of the two
/// middle order statistics).
fn median64(values: &[f64; 64]) -> f64 {
    let mut s = *values;
    s.sort_by(f64::total_cmp);
    (s[31] + s[32]) / 2.0
}

/// `imagehash.phash` (`hash_size = 8`, `highfreq_factor = 4`): the
/// unnormalised 2-D DCT-II (`scipy.fftpack.dct`, rows then columns) of
/// the 32×32 Lanczos thumbnail, low 8×8 block thresholded at its median
/// (the DC term included).
///
/// Distinct from [`crate::image_phash`], the box-average / DC-excluded
/// pHash the curator signatures pin.
pub fn image_phash_imagehash<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<u64, Error> {
    let px = lanczos_gray(img, 32, 32)?;
    let c = &*DCT_TABLE;
    // Axis 0 (down the rows) for the 8 kept frequencies.
    let mut t = [[0.0f64; 32]; 8];
    for (k, trow) in t.iter_mut().enumerate() {
        for (n2, out) in trow.iter_mut().enumerate() {
            let mut acc = 0.0;
            for n1 in 0..32 {
                acc += f64::from(px[n1 * 32 + n2]) * c[k][n1];
            }
            *out = 2.0 * acc;
        }
    }
    // Axis 1 for the 8 kept frequencies.
    let mut z = [0.0f64; 64];
    for k in 0..8 {
        for j in 0..8 {
            let mut acc = 0.0;
            for n2 in 0..32 {
                acc += t[k][n2] * c[j][n2];
            }
            z[k * 8 + j] = 2.0 * acc;
        }
    }
    let med = median64(&z);
    let mut hash = 0u64;
    for (i, &v) in z.iter().enumerate() {
        if v > med {
            hash |= 1u64 << (63 - i);
        }
    }
    Ok(hash)
}

/// `imagehash.whash` (`hash_size = 8`, Haar, `remove_max_haar_ll`).
///
/// `level` is `log2(image_scale)`; `0` selects the library default
/// `2^⌊log2(min(width, height))⌋` (never below 8). Values `1..=2` are
/// rejected (`hash_size` must not exceed the scale); the upper bound is
/// 15.
pub fn image_whash<L: Layout, T: Sample>(img: &Image<L, T>, level: u32) -> Result<u64, Error> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let max_level = if level == 0 {
        (usize::BITS - 1 - w.min(h).leading_zeros()).max(3)
    } else if (3..=15).contains(&level) {
        level
    } else {
        return Err(Error::BadValue("whash level must be 0 or 3..=15"));
    };
    let scale = 1usize << max_level;
    let px = lanczos_gray(img, scale, scale)?;

    let mut pixels: Vec<f64> = px.iter().map(|&p| f64::from(p) / 255.0).collect();

    // `remove_max_haar_ll`: full-depth `wavedec2`, zero the final LL,
    // `waverec2`.
    let mut details: Vec<[Vec<f64>; 3]> = Vec::with_capacity(max_level as usize);
    let mut a = core::mem::take(&mut pixels);
    let mut n = scale;
    for _ in 0..max_level {
        let (ll, det) = haar_dwt2(&a, n);
        details.push(det);
        a = ll;
        n /= 2;
    }
    a.fill(0.0);
    for det in details.iter().rev() {
        a = haar_idwt2(&a, det, n);
        n *= 2;
    }

    // `wavedec2(.., level = max_level − 3)`: keep the LL band.
    for _ in 0..max_level - 3 {
        a = haar_dwt_ll(&a, n);
        n /= 2;
    }
    debug_assert_eq!(n, 8);
    let a: [f64; 64] = a
        .as_slice()
        .try_into()
        .map_err(|_| Error::BadValue("whash LL band is not 8×8"))?;
    let med = median64(&a);
    let mut hash = 0u64;
    for (i, &v) in a.iter().enumerate() {
        if v > med {
            hash |= 1u64 << (63 - i);
        }
    }
    Ok(hash)
}

/// `pywt`'s Haar low-pass tap, `1/√2` as the library stores it.
const HAAR: f64 = core::f64::consts::FRAC_1_SQRT_2;

/// One level of `pywt.dwt2(x, 'haar')` on an `n × n` raster (`n` even):
/// `(LL, [HL, LH, HH])` with the exact `f64` operation order of the
/// library (axis 0 first, then axis 1; each output is `c·x₁ ± c·x₀`).
fn haar_dwt2(x: &[f64], n: usize) -> (Vec<f64>, [Vec<f64>; 3]) {
    let m = n / 2;
    let mut ll = vec![0.0; m * m];
    let mut hl = vec![0.0; m * m];
    let mut lh = vec![0.0; m * m];
    let mut hh = vec![0.0; m * m];
    for i in 0..m {
        let (r0, r1) = (
            &x[2 * i * n..(2 * i + 1) * n],
            &x[(2 * i + 1) * n..(2 * i + 2) * n],
        );
        for k in 0..m {
            // axis 0 → (lo, hi) of the four source samples of this cell.
            let lo = |j: usize| HAAR * r1[j] + HAAR * r0[j];
            let hi = |j: usize| -HAAR * r1[j] + HAAR * r0[j];
            let (l0, l1) = (lo(2 * k), lo(2 * k + 1));
            let (h0, h1) = (hi(2 * k), hi(2 * k + 1));
            ll[i * m + k] = HAAR * l1 + HAAR * l0;
            lh[i * m + k] = -HAAR * l1 + HAAR * l0;
            hl[i * m + k] = HAAR * h1 + HAAR * h0;
            hh[i * m + k] = -HAAR * h1 + HAAR * h0;
        }
    }
    (ll, [hl, lh, hh])
}

/// Only the LL band of [`haar_dwt2`].
fn haar_dwt_ll(x: &[f64], n: usize) -> Vec<f64> {
    let m = n / 2;
    let mut ll = vec![0.0; m * m];
    for i in 0..m {
        let (r0, r1) = (
            &x[2 * i * n..(2 * i + 1) * n],
            &x[(2 * i + 1) * n..(2 * i + 2) * n],
        );
        for k in 0..m {
            let l0 = HAAR * r1[2 * k] + HAAR * r0[2 * k];
            let l1 = HAAR * r1[2 * k + 1] + HAAR * r0[2 * k + 1];
            ll[i * m + k] = HAAR * l1 + HAAR * l0;
        }
    }
    ll
}

/// Inverse of [`haar_dwt2`] (`pywt.idwt2`): `ll` and the details are
/// `m × m`, the result `2m × 2m`; axis 1 is reconstructed first.
fn haar_idwt2(ll: &[f64], det: &[Vec<f64>; 3], m: usize) -> Vec<f64> {
    let n = 2 * m;
    let [hl, lh, hh] = det;
    let mut out = vec![0.0; n * n];
    for i in 0..m {
        for k in 0..m {
            let at = i * m + k;
            // axis 1: approximation column pair (LL, LH), detail pair (HL, HH).
            let a0 = HAAR * ll[at] + HAAR * lh[at];
            let a1 = HAAR * ll[at] - HAAR * lh[at];
            let d0 = HAAR * hl[at] + HAAR * hh[at];
            let d1 = HAAR * hl[at] - HAAR * hh[at];
            // axis 0.
            out[2 * i * n + 2 * k] = HAAR * a0 + HAAR * d0;
            out[(2 * i + 1) * n + 2 * k] = HAAR * a0 - HAAR * d0;
            out[2 * i * n + 2 * k + 1] = HAAR * a1 + HAAR * d1;
            out[(2 * i + 1) * n + 2 * k + 1] = HAAR * a1 - HAAR * d1;
        }
    }
    out
}

/// `imagehash.colorhash(binbits)`: `14 · binbits` bits, `binbits` in
/// `1..=4`. `0` selects the library default of 3.
///
/// Fractions of the image that are black (`L < 32`), gray (`S < 85`) and,
/// for the remaining pixels, faint (`S < 170`) / bright (`S > 170`) colour
/// per each of six hue bins, each quantised to `binbits` bits.
pub fn image_colorhash<L: Layout, T: Sample>(
    img: &Image<L, T>,
    binbits: u32,
) -> Result<u64, Error> {
    let binbits = match binbits {
        0 => 3,
        1..=4 => binbits,
        _ => return Err(Error::BadValue("colorhash binbits must be 0 or 1..=4")),
    };
    let n = img.width() as usize * img.height() as usize;
    let hsvl = map_pixels(img, "colorhash image buffer undersized", |r, g, b| {
        let (h, s, _v) = rgb_to_hsv(r, g, b);
        (h, s, rgb_to_l(r, g, b))
    })?;

    let mut black = 0u64;
    let mut gray = 0u64;
    let mut colors = 0u64;
    let mut faint = [0u64; 6];
    let mut bright = [0u64; 6];
    for &(h, s, l) in &hsvl {
        let is_black = l < 32;
        let is_gray = s < 85;
        if is_black {
            black += 1;
        } else if is_gray {
            gray += 1;
        } else {
            colors += 1;
            // Edges of `linspace(0, 255, 7)`: 42.5, 85, 127.5, 170, 212.5.
            let bin = match h {
                0..=42 => 0,
                43..=84 => 1,
                85..=127 => 2,
                128..=169 => 3,
                170..=212 => 4,
                _ => 5,
            };
            if s < 170 {
                faint[bin] += 1;
            } else if s > 170 {
                bright[bin] += 1;
            }
        }
    }

    let maxvalue = 1u32 << binbits;
    let cap = i64::from(maxvalue) - 1;
    let total = n as f64;
    let c = colors.max(1) as f64;
    let mut values: Vec<i64> = Vec::with_capacity(14);
    values.push(cap.min((black as f64 / total * f64::from(maxvalue)) as i64));
    values.push(cap.min((gray as f64 / total * f64::from(maxvalue)) as i64));
    for &count in faint.iter().chain(bright.iter()) {
        values.push(cap.min((count as f64 * f64::from(maxvalue) * 1.0 / c) as i64));
    }

    let mut hash = 0u64;
    for v in values {
        for i in 0..binbits {
            let bit = (v >> (binbits - i - 1)) % (1i64 << (binbits - i)) > 0;
            hash = (hash << 1) | u64::from(bit);
        }
    }
    Ok(hash)
}
