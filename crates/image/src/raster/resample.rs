//! Resampling: Pillow-compatible convolution resizing and the integer box
//! average used by the perceptual hashes.
//!
//! [`resize`] / [`resize_with_gap`] / [`resize_f32`] /
//! [`resize_f32_with_gap`] / [`reduce`] reproduce
//! Pillow's `Image.resize`, `Image.resize(reducing_gap=...)` and
//! `Image.reduce` (see `pillow.rs` for the fixed-point kernels), including
//! the premultiplied-alpha handling of `RGBA` / `LA`.
//!
//! [`box_average`] is a different, exact operation: output pixel `(x, y)`
//! averages the axis-aligned source rectangle that maps to it under
//! integer floor boundaries - column `x` covers `floor(x*W/W') ..
//! floor((x+1)*W/W')` and row `y` likewise. Each output sample is
//! `round_half_up(mean)` of its box, computed entirely in `u64`:
//! `(2*sum + n) / (2*n)`. When a box is empty (`out > in` on an axis) the
//! pixel samples the nearest source point, the box centre
//! `floor((2j+1)*in/(2*out))`.
use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;

use super::pillow::{self, Quant};
use crate::raster::{Image, Layout, Sample};
use sillstone_digest::{Error, Result};

/// Supported resampling filters matching Pillow / standard image processing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResamplingFilter {
    /// Nearest neighbor: fast, sharp edges, no smoothing.
    Nearest,
    /// Box average (integer area average).
    Box,
    /// Bilinear (triangle filter): smooth, linear interpolation.
    Bilinear,
    /// Bicubic (Catmull-Rom cubic spline): high-quality smooth curves.
    Bicubic,
    /// Lanczos (windowed sinc with 3 lobes): highest sharpness and detail preservation.
    Lanczos,
}

/// Inclusive-exclusive source index range of output slot `j` along one
/// axis: `[⌊j·n/n'⌋, ⌊(j+1)·n/n'⌋)` clamped to `n`. When the range is
/// empty — upscaling makes `⌊(j+1)·n/n'⌋ == ⌊j·n/n'⌋` — it collapses to
/// the single nearest source index, the box centre
/// `⌊(2j+1)·n/(2·n')⌋`, which is at most `n−1`.
#[inline]
fn box_range(j: u32, n_in: u32, n_out: u32) -> (u32, u32) {
    let lo = (u64::from(j) * u64::from(n_in) / u64::from(n_out)) as u32;
    let hi = (u64::from(j + 1) * u64::from(n_in) / u64::from(n_out)) as u32;
    if lo < hi {
        return (lo, hi);
    }
    let c = ((u64::from(2 * j + 1) * u64::from(n_in)) / (2 * u64::from(n_out))) as u32;
    (c.min(n_in - 1), c.min(n_in - 1) + 1)
}

/// Resizes `img` to `out_w`×`out_h` by integer box averaging.
///
/// `Error::BadValue` on a zero output dimension. Layout, sample type and
/// channel count pass through unchanged; `Image`'s own construction
/// rules keep the result inside [`crate::raster::MAX_BUFFER_BYTES`].
pub fn box_average<L: Layout, T: Sample>(
    img: &Image<L, T>,
    out_w: u32,
    out_h: u32,
) -> Result<Image<L, T>> {
    if out_w == 0 || out_h == 0 {
        return Err(Error::BadValue("output dimension is zero"));
    }
    let c = L::CHANNELS;
    let (in_w, in_h) = (img.width(), img.height());
    let src = img.as_slice();
    let mut out = vec![T::default(); out_w as usize * out_h as usize * c];

    for y in 0..out_h {
        let (y0, y1) = box_range(y, in_h, out_h);
        for x in 0..out_w {
            let (x0, x1) = box_range(x, in_w, out_w);
            let n = u64::from(y1 - y0) * u64::from(x1 - x0);
            let base = (y as usize) * out_w as usize * c + x as usize * c;
            for (k, slot) in out[base..base + c].iter_mut().enumerate() {
                let mut sum = 0u64;
                for sy in y0..y1 {
                    let row = (sy as usize) * in_w as usize * c;
                    for sx in x0..x1 {
                        sum += src[row + sx as usize * c + k].to_u64();
                    }
                }
                *slot = T::from_u64((2 * sum + n) / (2 * n));
            }
        }
    }
    Image::from_vec(out_w, out_h, out)
}

/// Pillow's `_filters_support[filter] - 0.5`, the margin `_get_safe_box`
/// adds around the region a `reducing_gap` pre-reduction keeps.
fn safe_box_support(filter: ResamplingFilter) -> f64 {
    match filter {
        ResamplingFilter::Nearest | ResamplingFilter::Box => 0.0,
        ResamplingFilter::Bilinear => 0.5,
        ResamplingFilter::Bicubic => 1.5,
        ResamplingFilter::Lanczos => 2.5,
    }
}

/// Premultiplies colour by alpha in place: `round_half_up(c * a / MAX)`.
/// For `u8` this is Pillow's `MULDIV255` (`RGBA -> RGBa`, `LA -> La`).
fn premultiply<T: Sample>(samples: &mut [T], c: usize, alpha: usize) {
    let max = T::MAX_U64;
    for px in samples.chunks_exact_mut(c) {
        let a = px[alpha].to_u64();
        for (k, v) in px.iter_mut().enumerate() {
            if k != alpha {
                *v = T::from_u64((v.to_u64() * a + max / 2) / max);
            }
        }
    }
}

/// Inverse of [`premultiply`] (Pillow's `rgbA2rgba`): `min(MAX * c / a, MAX)`,
/// colour untouched where alpha is `0` or `MAX`.
fn unpremultiply<T: Sample>(samples: &mut [T], c: usize, alpha: usize) {
    let max = T::MAX_U64;
    for px in samples.chunks_exact_mut(c) {
        let a = px[alpha].to_u64();
        if a == 0 || a == max {
            continue;
        }
        for (k, v) in px.iter_mut().enumerate() {
            if k != alpha {
                *v = T::from_u64((max * v.to_u64() / a).min(max));
            }
        }
    }
}

/// Reduces the pixel rectangle `rb = [x0, y0, x1, y1]` of `src` by the
/// integer factors `(fx, fy)`: every `fx x fy` block becomes one pixel,
/// edge blocks average only the pixels they hold (Pillow's `Image.reduce`).
/// Returns the samples and the `ceil(w/fx) x ceil(h/fy)` size.
///
/// 8-bit samples use Pillow's `(sum + n/2) * floor(2^24/n) >> 24` (not
/// exactly the rounded mean when `n` is not a power of two); wider samples
/// use the rounded mean.
fn reduce_region<T: Sample>(
    src: &[T],
    in_w: usize,
    c: usize,
    rb: [usize; 4],
    fx: usize,
    fy: usize,
) -> (Vec<T>, usize, usize) {
    let (bw, bh) = (rb[2] - rb[0], rb[3] - rb[1]);
    let (ow, oh) = (bw.div_ceil(fx), bh.div_ceil(fy));
    let mut out = vec![T::default(); ow * oh * c];
    for oy in 0..oh {
        let (y0, y1) = (rb[1] + oy * fy, (rb[1] + (oy + 1) * fy).min(rb[3]));
        for ox in 0..ow {
            let (x0, x1) = (rb[0] + ox * fx, (rb[0] + (ox + 1) * fx).min(rb[2]));
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            for k in 0..c {
                let mut sum = 0u64;
                for y in y0..y1 {
                    for x in x0..x1 {
                        sum += src[(y * in_w + x) * c + k].to_u64();
                    }
                }
                let v = if T::MAX_U64 == 255 {
                    ((sum + n / 2) * ((1u64 << 24) / n)) >> 24
                } else {
                    (2 * sum + n) / (2 * n)
                };
                out[(oy * ow + ox) * c + k] = T::from_u64(v);
            }
        }
    }
    (out, ow, oh)
}

/// Pillow's `Image.reduce((fx, fy))`: every `fx x fy` block of `img` becomes
/// one pixel; the size is rounded up and edge blocks average only the pixels
/// they hold. RGBA / LA are premultiplied around the average.
///
/// `Error::BadValue` on a zero factor.
pub fn reduce<L: Layout, T: Sample>(img: &Image<L, T>, fx: u32, fy: u32) -> Result<Image<L, T>> {
    if fx == 0 || fy == 0 {
        return Err(Error::BadValue("reduce factor is zero"));
    }
    if fx == 1 && fy == 1 {
        return Ok(img.clone());
    }
    let c = L::CHANNELS;
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut buf = Cow::Borrowed(img.as_slice());
    if let Some(a) = L::ALPHA {
        premultiply(buf.to_mut(), c, a);
    }
    let (mut out, ow, oh) = reduce_region(&buf, w, c, [0, 0, w, h], fx as usize, fy as usize);
    if let Some(a) = L::ALPHA {
        unpremultiply(&mut out, c, a);
    }
    Image::from_vec(ow as u32, oh as u32, out)
}

/// One resize of `src` over the source rectangle `bx` (no premultiply, no
/// reduction): the 8-bit fixed-point path, or the `f64` path for `u16`.
#[allow(clippy::too_many_arguments)]
fn resize_box<T: Sample>(
    src: &[T],
    in_w: usize,
    in_h: usize,
    c: usize,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
    bx: [f64; 4],
) -> Vec<T> {
    if T::MAX_U64 == 255 {
        let bytes: Vec<u8> = src.iter().map(|v| v.to_u64() as u8).collect();
        let out = pillow::resize_u8_box(&bytes, in_w, in_h, c, out_w, out_h, filter, bx);
        return out.into_iter().map(|v| T::from_u64(u64::from(v))).collect();
    }
    let out = pillow::resize_wide(
        src,
        |v| v.to_u64() as f64,
        in_w,
        in_h,
        c,
        out_w,
        out_h,
        filter,
        bx,
        Quant::U16,
    );
    out.into_iter().map(|v| T::from_u64(v as u64)).collect()
}

/// Pillow's `reducing_gap` pre-reduction plan for a whole-image resize:
/// the integer factors `(fx, fy)` and the safe source rectangle
/// `_get_safe_box` keeps. `None` when the gap does not reduce (factor 1
/// on both axes, `Nearest`, or no gap).
///
/// `Image.resize` computes `int((box[2] - box[0]) / size[0] /
/// reducing_gap) or 1` per axis, then `_get_safe_box` grows the region by
/// `_filters_support[f] - 0.5` output-scale supports before the integer
/// reduction.
fn gap_plan(
    in_w: usize,
    in_h: usize,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
    reducing_gap: Option<f64>,
) -> Option<(usize, usize, [usize; 4])> {
    let gap = reducing_gap?;
    if filter == ResamplingFilter::Nearest {
        return None;
    }
    // C `int(x) or 1`: truncation, and a zero factor becomes 1.
    let fx = ((in_w as f64 / out_w as f64 / gap) as usize).max(1);
    let fy = ((in_h as f64 / out_h as f64 / gap) as usize).max(1);
    if fx == 1 && fy == 1 {
        return None;
    }
    let support = safe_box_support(filter);
    let sx = support * in_w as f64 / out_w as f64;
    let sy = support * in_h as f64 / out_h as f64;
    // `_get_safe_box`: `int()` truncates, `math.ceil` rounds up.
    let rb = [
        0,
        0,
        ((in_w as f64 + sx).ceil() as usize).min(in_w),
        ((in_h as f64 + sy).ceil() as usize).min(in_h),
    ];
    Some((fx, fy, rb))
}

/// The pre-reduced source rectangle as Pillow rewrites `box` after the
/// reduction: `(box - reduce_box) / factor`.
fn gap_box(rb: [usize; 4], fx: usize, fy: usize, w: usize, h: usize) -> [f64; 4] {
    [
        0.0,
        0.0,
        (w - rb[0]) as f64 / fx as f64,
        (h - rb[1]) as f64 / fy as f64,
    ]
}

/// Pillow's `reducing_gap` pre-reduction followed by the resize
/// (`Image.resize` after the premultiply step).
fn resize_reduced<L: Layout, T: Sample>(
    img: &Image<L, T>,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
    reducing_gap: Option<f64>,
) -> Result<Image<L, T>> {
    let c = L::CHANNELS;
    let (mut w, mut h) = (img.width() as usize, img.height() as usize);
    let mut bx = [0.0, 0.0, w as f64, h as f64];
    let mut cur = Cow::Borrowed(img.as_slice());

    if let Some((fx, fy, rb)) = gap_plan(w, h, out_w, out_h, filter, reducing_gap) {
        let (reduced, rw, rh) = reduce_region(&cur, w, c, rb, fx, fy);
        bx = gap_box(rb, fx, fy, w, h);
        cur = Cow::Owned(reduced);
        (w, h) = (rw, rh);
    }

    let out = if h > w * 100 && out_h < h {
        // Pillow resizes extremely tall images in two steps to bound the
        // intermediate buffer: first the height, then the width.
        let mid = resize_box(
            &cur,
            w,
            h,
            c,
            w,
            out_h,
            filter,
            [0.0, bx[1], w as f64, bx[3]],
        );
        resize_box(
            &mid,
            w,
            out_h,
            c,
            out_w,
            out_h,
            filter,
            [bx[0], 0.0, bx[2], out_h as f64],
        )
    } else {
        resize_box(&cur, w, h, c, out_w, out_h, filter, bx)
    };
    Image::from_vec(out_w as u32, out_h as u32, out)
}

/// Pillow-compatible `Image.resize(size, resample=filter)`.
///
/// 8-bit rasters (`Gray`, `La`, `Rgb`, `Rgba` over `u8`) reproduce Pillow
/// bit for bit for every filter, including `Box` (a convolution with the
/// box kernel, *not* the integer boxes of [`box_average`]). `Rgba` and `La`
/// are premultiplied by alpha around the convolution, as Pillow does for
/// every filter but `Nearest`. `u16` rasters follow Pillow's `I;16`
/// convolution (`f64` coefficients and accumulator, rounding and clamping
/// after each pass); there is no 16-bit alpha mode in Pillow, so `u16`
/// `Rgba` / `La` use the same premultiplied scheme without an oracle.
///
/// `Error::BadValue` on a zero output dimension.
pub fn resize<L: Layout, T: Sample>(
    img: &Image<L, T>,
    out_w: u32,
    out_h: u32,
    filter: ResamplingFilter,
) -> Result<Image<L, T>> {
    resize_with_gap(img, out_w, out_h, filter, None)
}

/// [`resize`] with Pillow's `reducing_gap`: the source is first averaged
/// down by an integer factor (see [`reduce`]) so the final convolution
/// works on at most `reducing_gap` times the output size, which is faster
/// and slightly different, exactly as in Pillow.
///
/// `reducing_gap` must be at least 1.0. It has no effect for `Nearest`.
/// Alpha layouts (`Rgba`, `La`) forward it to the premultiplied resize, as
/// Pillow 13 does (Pillow 12 dropped it on that path).
///
/// `Error::BadValue` on a zero output dimension or an invalid gap.
pub fn resize_with_gap<L: Layout, T: Sample>(
    img: &Image<L, T>,
    out_w: u32,
    out_h: u32,
    filter: ResamplingFilter,
    reducing_gap: Option<f64>,
) -> Result<Image<L, T>> {
    if out_w == 0 || out_h == 0 {
        return Err(Error::BadValue("output dimension is zero"));
    }
    if let Some(gap) = reducing_gap {
        // The negated comparison also rejects NaN.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(gap >= 1.0) {
            return Err(Error::BadValue("reducing_gap must be 1.0 or greater"));
        }
    }
    if img.width() == out_w && img.height() == out_h {
        return Ok(img.clone());
    }
    let (ow, oh) = (out_w as usize, out_h as usize);
    match L::ALPHA {
        Some(a) if filter != ResamplingFilter::Nearest => {
            let mut pre = img.as_slice().to_vec();
            premultiply(&mut pre, L::CHANNELS, a);
            let pre = Image::<L, T>::from_vec(img.width(), img.height(), pre)?;
            let mut out = resize_reduced(&pre, ow, oh, filter, reducing_gap)?;
            unpremultiply(out.as_mut_slice(), L::CHANNELS, a);
            Ok(out)
        }
        _ => resize_reduced(img, ow, oh, filter, reducing_gap),
    }
}

/// Pillow's resize of a float raster (mode `F`): `f64` coefficients and
/// accumulator, every stored sample (the intermediate between the two
/// passes and the result) rounded to `f32`. `src` holds `c` interleaved
/// channels per pixel, row-major.
///
/// `Error::BadValue` on a zero dimension or when `src` is not exactly
/// `in_w * in_h * c` samples.
pub fn resize_f32(
    src: &[f32],
    in_w: u32,
    in_h: u32,
    c: usize,
    out_w: u32,
    out_h: u32,
    filter: ResamplingFilter,
) -> Result<Vec<f32>> {
    resize_f32_with_gap(src, in_w, in_h, c, out_w, out_h, filter, None)
}

/// Pillow's `Image.reduce` on a float raster. The arithmetic is
/// `ImagingReduceNxN_32bpc` + `ImagingReduceCorners_32bpc`: a full
/// `fx x fy` block sums its pixels the way the C loop does — each `2x2`
/// group as a `float` sub-sum (`((v00 + v01) + v10) + v11`, rounded to
/// `f32` once per group), the odd trailing column as a `float` pair and
/// the odd trailing row likewise — and accumulates those `f32` partials
/// in an `f64`. Blocks cut off by the right or bottom edge (the "corners"
/// pass) add their samples one at a time straight into `f64` in row-major
/// order. The stored sample is `ss * (1.0 / n)` in `f64`, narrowed to
/// `f32`.
///
/// Both deviations from an exact `f64` mean matter: the pairwise `f32`
/// partials (`Reduce.c` adds four `FLOAT32` operands in `float` before
/// widening) and the reciprocal multiply instead of a divide. They move
/// the last bit on roughly one output pixel in ten.
///
/// Returns the samples and the `ceil(w/fx) x ceil(h/fy)` size.
fn reduce_region_f32(
    src: &[f32],
    in_w: usize,
    c: usize,
    rb: [usize; 4],
    fx: usize,
    fy: usize,
) -> (Vec<f32>, usize, usize) {
    let (bw, bh) = (rb[2] - rb[0], rb[3] - rb[1]);
    let (ow, oh) = (bw.div_ceil(fx), bh.div_ceil(fy));
    let mut out = vec![0.0f32; ow * oh * c];
    for oy in 0..oh {
        let (y0, y1) = (rb[1] + oy * fy, (rb[1] + (oy + 1) * fy).min(rb[3]));
        for ox in 0..ow {
            let (x0, x1) = (rb[0] + ox * fx, (rb[0] + (ox + 1) * fx).min(rb[2]));
            let n = ((y1 - y0) * (x1 - x0)) as f64;
            let scale = 1.0 / n;
            let full = (y1 - y0) == fy && (x1 - x0) == fx;
            for k in 0..c {
                let at = |x: usize, y: usize| src[(y * in_w + x) * c + k];
                let ss = if full {
                    let mut ss = 0.0f64;
                    let mut y = y0;
                    while y < y1 - 1 {
                        let mut x = x0;
                        while x < x1 - 1 {
                            // `line0[xx] + line0[xx+1] + line1[xx] + line1[xx+1]`
                            // in `f32`, then widened for the accumulator.
                            let quad = (at(x, y) + at(x + 1, y)) + at(x, y + 1);
                            ss += f64::from(quad + at(x + 1, y + 1));
                            x += 2;
                        }
                        if fx & 1 == 1 {
                            ss += f64::from(at(x, y) + at(x, y + 1));
                        }
                        y += 2;
                    }
                    if fy & 1 == 1 {
                        let mut x = x0;
                        while x < x1 - 1 {
                            ss += f64::from(at(x, y) + at(x + 1, y));
                            x += 2;
                        }
                        if fx & 1 == 1 {
                            ss += f64::from(at(x, y));
                        }
                    }
                    ss
                } else {
                    let mut ss = 0.0f64;
                    for y in y0..y1 {
                        for x in x0..x1 {
                            ss += f64::from(at(x, y));
                        }
                    }
                    ss
                };
                out[(oy * ow + ox) * c + k] = (ss * scale) as f32;
            }
        }
    }
    (out, ow, oh)
}

/// [`resize_f32`] with Pillow's `reducing_gap`: the source is first
/// averaged down by an integer factor (Pillow's `Image.reduce` on `F`,
/// with the `float`-grouped block arithmetic described on
/// [`reduce_region_f32`]) and the final convolution runs on the result,
/// exactly as `Image.resize(reducing_gap=...)`.
///
/// Pillow refuses `Image.reduce` on `I;16` ("image has wrong mode"), so a
/// 16-bit gap has no oracle; this float path is the one Pillow itself
/// exercises.
///
/// `Error::BadValue` on a zero dimension, a sample count that does not
/// match the dimensions, or a gap below 1.0.
#[allow(clippy::too_many_arguments)]
pub fn resize_f32_with_gap(
    src: &[f32],
    in_w: u32,
    in_h: u32,
    c: usize,
    out_w: u32,
    out_h: u32,
    filter: ResamplingFilter,
    reducing_gap: Option<f64>,
) -> Result<Vec<f32>> {
    if in_w == 0 || in_h == 0 || out_w == 0 || out_h == 0 || c == 0 {
        return Err(Error::BadValue("dimension or channel count is zero"));
    }
    if let Some(gap) = reducing_gap {
        // The negated comparison also rejects NaN.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(gap >= 1.0) {
            return Err(Error::BadValue("reducing_gap must be 1.0 or greater"));
        }
    }
    let (iw, ih) = (in_w as usize, in_h as usize);
    if iw.checked_mul(ih).and_then(|n| n.checked_mul(c)) != Some(src.len()) {
        return Err(Error::BadValue("sample count does not match dimensions"));
    }
    let (ow, oh) = (out_w as usize, out_h as usize);
    ow.checked_mul(oh)
        .and_then(|n| n.checked_mul(c))
        .filter(|n| n.saturating_mul(4) <= crate::raster::MAX_BUFFER_BYTES)
        .ok_or(Error::BadValue("output exceeds the buffer limit"))?;
    if iw == ow && ih == oh {
        return Ok(src.to_vec());
    }

    let mut cur: Vec<f32> = src.to_vec();
    let (mut w, mut h) = (iw, ih);
    let mut bx = [0.0, 0.0, iw as f64, ih as f64];
    if let Some((fx, fy, rb)) = gap_plan(iw, ih, ow, oh, filter, reducing_gap) {
        let (reduced, rw, rh) = reduce_region_f32(&cur, iw, c, rb, fx, fy);
        bx = gap_box(rb, fx, fy, iw, ih);
        cur = reduced;
        (w, h) = (rw, rh);
    }

    // Same extremely-tall two-step as the 8-bit path (Pillow bounds the
    // intermediate buffer the same way for every mode).
    let out = if h > w * 100 && oh < h {
        let mid = pillow::resize_wide(
            &cur,
            f64::from,
            w,
            h,
            c,
            w,
            oh,
            filter,
            [0.0, bx[1], w as f64, bx[3]],
            Quant::F32,
        );
        pillow::resize_wide(
            &mid,
            |v| v,
            w,
            oh,
            c,
            ow,
            oh,
            filter,
            [bx[0], 0.0, bx[2], oh as f64],
            Quant::F32,
        )
    } else {
        pillow::resize_wide(&cur, f64::from, w, h, c, ow, oh, filter, bx, Quant::F32)
    };
    Ok(out.into_iter().map(|v| v as f32).collect())
}
