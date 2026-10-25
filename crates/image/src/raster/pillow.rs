//! Pillow-exact 8-bit primitives: separable convolution resampling, the
//! `RGB → L` luma and the `RGB → HSV` conversion.
//!
//! The perceptual hashes of `imagehash` are defined by what Pillow does to
//! the pixels before the hash proper runs (`convert("L")`, then
//! `resize(..., LANCZOS)`), so bit-exact parity with that library needs
//! bit-exact parity with those two Pillow steps. Everything here mirrors
//! `libImaging/Resample.c` and `libImaging/Convert.c`:
//!
//! * coefficients are computed in `f64` per output pixel with the
//!   `(center, support)` window rule of `precompute_coeffs`, normalised to
//!   sum 1, then quantised to `PRECISION_BITS = 22` fixed point with
//!   round-half-away-from-zero;
//! * the horizontal pass runs first (unless the height shrinks by more than
//!   twice the width's shrink) and stores an 8-bit intermediate (clipped),
//!   the second pass reads that intermediate — two roundings, exactly as
//!   Pillow does;
//! * accumulators start at `1 << 21` (the rounding term) and are shifted
//!   right by 22 and clamped to `0..=255`.
//!
//! With the horizontal pass first, only the rows the vertical pass reads
//! are filtered.

use alloc::vec;
use alloc::vec::Vec;

use crate::raster::ResamplingFilter;

/// Fixed-point precision of the quantised coefficients (Pillow's
/// `PRECISION_BITS = 32 - 8 - 2`).
const PRECISION_BITS: u32 = 22;

/// Pillow's `ImagingConvert` RGB → L luma: ITU-R 601-2 on non-linear RGB,
/// `(19595·r + 38470·g + 7471·b + 0x8000) >> 16`.
#[inline]
pub fn rgb_to_l(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 19595 + u32::from(g) * 38470 + u32::from(b) * 7471 + 0x8000) >> 16) as u8
}

/// Pillow's RGB → HSV (`rgb2hsv_row`): all three outputs scaled to
/// `0..=255`, computed in single precision exactly like the C source.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (ri, gi, bi) = (i32::from(r), i32::from(g), i32::from(b));
    let maxc = ri.max(gi).max(bi);
    let minc = ri.min(gi).min(bi);
    let v = maxc as u8;
    if minc == maxc {
        return (0, 0, v);
    }
    let cr = (maxc - minc) as f32;
    let s = cr / maxc as f32;
    let rc = (maxc - ri) as f32 / cr;
    let gc = (maxc - gi) as f32 / cr;
    let bc = (maxc - bi) as f32 / cr;
    let h = if ri == maxc {
        bc - gc
    } else if gi == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    // C `fmodf(h / 6 + 1, 1)`; the operand is positive, so `%` is `fmodf`.
    let h = (h / 6.0 + 1.0) % 1.0;
    let uh = ((h * 255.0) as i32).clamp(0, 255) as u8;
    let us = ((s * 255.0) as i32).clamp(0, 255) as u8;
    (uh, us, v)
}

/// The `(support, kernel)` pair of a convolution filter, as in
/// `libImaging/Resample.c`.
fn kernel(filter: ResamplingFilter) -> (f64, fn(f64) -> f64) {
    fn sinc(x: f64) -> f64 {
        if x == 0.0 {
            1.0
        } else {
            let px = x * core::f64::consts::PI;
            px.sin() / px
        }
    }
    fn lanczos(x: f64) -> f64 {
        if (-3.0..3.0).contains(&x) {
            sinc(x) * sinc(x / 3.0)
        } else {
            0.0
        }
    }
    fn bilinear(x: f64) -> f64 {
        let x = x.abs();
        if x < 1.0 { 1.0 - x } else { 0.0 }
    }
    fn bicubic(x: f64) -> f64 {
        const A: f64 = -0.5;
        let x = x.abs();
        if x < 1.0 {
            ((A + 2.0) * x - (A + 3.0)) * x * x + 1.0
        } else if x < 2.0 {
            (((x - 5.0) * x + 8.0) * x - 4.0) * A
        } else {
            0.0
        }
    }
    fn boxf(x: f64) -> f64 {
        if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
    }
    match filter {
        ResamplingFilter::Lanczos => (3.0, lanczos),
        ResamplingFilter::Bicubic => (2.0, bicubic),
        ResamplingFilter::Bilinear => (1.0, bilinear),
        ResamplingFilter::Box | ResamplingFilter::Nearest => (0.5, boxf),
    }
}

/// Per-output-pixel source windows and fixed-point coefficients along one
/// axis (`precompute_coeffs` + `normalize_coeffs_8bpc`).
struct Coeffs {
    /// Stride of one output pixel's coefficient row in `kk`.
    ksize: usize,
    /// `(first source index, tap count)` per output pixel.
    bounds: Vec<(usize, usize)>,
    kk: Vec<i32>,
}

fn precompute(in_size: usize, out_size: usize, filter: ResamplingFilter) -> Coeffs {
    let (fsupport, f) = kernel(filter);
    let scale = in_size as f64 / out_size as f64;
    let filterscale = scale.max(1.0);
    let support = fsupport * filterscale;
    let ksize = support.ceil() as usize * 2 + 1;
    let ss = 1.0 / filterscale;

    let mut bounds = Vec::with_capacity(out_size);
    let mut kk = vec![0i32; out_size * ksize];
    let mut tmp = vec![0.0f64; ksize];
    for xx in 0..out_size {
        let center = (xx as f64 + 0.5) * scale;
        // C `(int)(x + 0.5)` truncates toward zero; the clamp below makes
        // the negative case irrelevant.
        let xmin = ((center - support + 0.5) as i64).max(0) as usize;
        let xmax = (((center + support + 0.5) as i64).max(0) as usize).min(in_size);
        let taps = xmax.saturating_sub(xmin);
        let mut ww = 0.0f64;
        for (x, slot) in tmp.iter_mut().enumerate().take(taps) {
            let w = f((x as f64 + xmin as f64 - center + 0.5) * ss);
            *slot = w;
            ww += w;
        }
        let row = &mut kk[xx * ksize..(xx + 1) * ksize];
        for x in 0..taps {
            let w = if ww != 0.0 { tmp[x] / ww } else { tmp[x] };
            // `(int)(±0.5 + w·2^22)`: truncation toward zero.
            let scaled = w * f64::from(1u32 << PRECISION_BITS);
            row[x] = if w < 0.0 {
                (-0.5 + scaled) as i32
            } else {
                (0.5 + scaled) as i32
            };
        }
        bounds.push((xmin, taps));
    }
    Coeffs { ksize, bounds, kk }
}

#[inline]
fn clip8(acc: i32) -> u8 {
    (acc >> PRECISION_BITS).clamp(0, 255) as u8
}

/// Resizes an interleaved 8-bit raster (`c` channels per pixel) with
/// Pillow's separable fixed-point convolution.
///
/// Mirrors `Image.resize(size, resample=filter)` for modes `L`, `RGB`
/// (and, channel-wise and without premultiplication, `RGBA`). `Nearest`
/// follows Pillow's affine nearest-neighbour path
/// (`src = ⌊(x + ½)·in/out⌋` with the centre accumulated by addition).
pub fn resize_u8(
    src: &[u8],
    in_w: usize,
    in_h: usize,
    c: usize,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
) -> Vec<u8> {
    if in_w == out_w && in_h == out_h {
        return src.to_vec();
    }
    if filter == ResamplingFilter::Nearest {
        return nearest(src, in_w, in_h, c, out_w, out_h);
    }
    let need_h = in_w != out_w;
    let need_v = in_h != out_h;

    // Pillow runs the vertical pass first when the height shrinks by more
    // than twice the width's shrink (`ImagingResampleInner`).
    let dh = in_h as i64 - out_h as i64;
    let dw = in_w as i64 - out_w as i64;
    let horizontal_first = !(dh > 0 && dh > dw * 2);

    let hc = need_h.then(|| precompute(in_w, out_w, filter));
    let vc = need_v.then(|| precompute(in_h, out_h, filter));

    if horizontal_first {
        // Rows of `src` the vertical pass reads.
        let (ybox_first, ybox_last) = match &vc {
            Some(vc) => {
                let (first, _) = vc.bounds[0];
                let (lm, ln) = vc.bounds[out_h - 1];
                (first, lm + ln)
            }
            None => (0, in_h),
        };
        let mid = match &hc {
            Some(hc) => horizontal(
                &src[ybox_first * in_w * c..ybox_last * in_w * c],
                in_w,
                c,
                hc,
            ),
            None => src[ybox_first * in_w * c..ybox_last * in_w * c].to_vec(),
        };
        match &vc {
            Some(vc) => vertical(&mid, out_w * c, ybox_first, vc),
            None => mid,
        }
    } else {
        let mid = match &vc {
            Some(vc) => vertical(src, in_w * c, 0, vc),
            None => src.to_vec(),
        };
        match &hc {
            Some(hc) => horizontal(&mid, in_w, c, hc),
            None => mid,
        }
    }
}

/// Horizontal pass over every row of `src` (`in_w` pixels of `c` channels
/// per row) → 8-bit rows of `hc.bounds.len()` pixels.
fn horizontal(src: &[u8], in_w: usize, c: usize, hc: &Coeffs) -> Vec<u8> {
    let out_w = hc.bounds.len();
    let rows = src.len() / (in_w * c);
    let mut buf = vec![0u8; rows * out_w * c];
    for r in 0..rows {
        let srow = &src[r * in_w * c..(r + 1) * in_w * c];
        let drow = &mut buf[r * out_w * c..(r + 1) * out_w * c];
        for (xx, &(xmin, taps)) in hc.bounds.iter().enumerate() {
            let k = &hc.kk[xx * hc.ksize..xx * hc.ksize + taps];
            if c == 1 {
                let px = &srow[xmin..xmin + taps];
                let acc = px
                    .iter()
                    .zip(k)
                    .fold(1i32 << (PRECISION_BITS - 1), |a, (&p, &w)| {
                        a + i32::from(p) * w
                    });
                drow[xx] = clip8(acc);
            } else {
                for ch in 0..c {
                    let mut acc = 1i32 << (PRECISION_BITS - 1);
                    for (t, &w) in k.iter().enumerate() {
                        acc += i32::from(srow[(xmin + t) * c + ch]) * w;
                    }
                    drow[xx * c + ch] = clip8(acc);
                }
            }
        }
    }
    buf
}

/// Vertical pass: `src` holds rows of `width` samples starting at source
/// row `first`; `vc.bounds` index absolute source rows.
fn vertical(src: &[u8], width: usize, first: usize, vc: &Coeffs) -> Vec<u8> {
    let out_h = vc.bounds.len();
    let mut out = vec![0u8; out_h * width];
    let mut acc = vec![0i32; width];
    for (yy, &(ymin, taps)) in vc.bounds.iter().enumerate() {
        let k = &vc.kk[yy * vc.ksize..yy * vc.ksize + taps];
        acc.fill(1i32 << (PRECISION_BITS - 1));
        for (t, &w) in k.iter().enumerate() {
            let row = &src[(ymin - first + t) * width..(ymin - first + t + 1) * width];
            for (a, &p) in acc.iter_mut().zip(row) {
                *a += i32::from(p) * w;
            }
        }
        for (o, &a) in out[yy * width..(yy + 1) * width].iter_mut().zip(&acc) {
            *o = clip8(a);
        }
    }
    out
}

/// Source index table of Pillow's `ImagingScaleAffine`: the sample centre
/// starts at `scale / 2` and is advanced by repeated *addition* (not
/// multiplication), so exact `.0` boundaries round the way Pillow's
/// accumulated `f64` does.
fn nearest_table(in_size: usize, out_size: usize) -> Vec<usize> {
    let scale = in_size as f64 / out_size as f64;
    let mut pos = scale * 0.5;
    (0..out_size)
        .map(|_| {
            let i = (pos as usize).min(in_size - 1);
            pos += scale;
            i
        })
        .collect()
}

fn nearest(src: &[u8], in_w: usize, in_h: usize, c: usize, out_w: usize, out_h: usize) -> Vec<u8> {
    let xtab = nearest_table(in_w, out_w);
    let ytab = nearest_table(in_h, out_h);
    let mut out = vec![0u8; out_w * out_h * c];
    for (y, &sy) in ytab.iter().enumerate() {
        let srow = &src[sy * in_w * c..(sy + 1) * in_w * c];
        let drow = &mut out[y * out_w * c..(y + 1) * out_w * c];
        for (x, &sx) in xtab.iter().enumerate() {
            drow[x * c..(x + 1) * c].copy_from_slice(&srow[sx * c..(sx + 1) * c]);
        }
    }
    out
}
