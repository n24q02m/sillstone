//! Box-average resampling: the only resizer the kit needs.
//!
//! Output pixel `(x, y)` averages the axis-aligned source rectangle that
//! maps to it under integer floor boundaries — column `x` covers
//! `⌊x·W/W'⌋ .. ⌊(x+1)·W/W'⌋` and row `y` likewise. The boundaries are
//! the spec's accumulated-remainder rule made exact: box widths differ by
//! at most one source pixel and are fully determined by the four
//! dimensions, so two independent implementations bit-match.
//!
//! Each output sample is `round_half_up(mean)` of its box, computed
//! entirely in `u64` — `(2·sum + n) / (2·n)`. No fractional coefficients,
//! no float, one rounding step. A separable two-pass filter would round
//! twice and drift off the exact box mean; the one-pass form is both
//! simpler and exact, and `box_average(im, 1, 1)` is the exact mean of
//! the whole buffer.
//!
//! Upscaling is defined but not what the kit does: when a box is empty
//! (`out > in` on an axis), the pixel samples the nearest source point —
//! the box centre `⌊(2j+1)·in/(2·out)⌋`. Documented so the function is
use alloc::vec;
use alloc::vec::Vec;

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

#[inline]
fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-8 {
        1.0
    } else {
        let px = core::f64::consts::PI * x;
        px.sin() / px
    }
}

#[inline]
fn filter_weight(f: ResamplingFilter, x: f64) -> f64 {
    let x = x.abs();
    match f {
        ResamplingFilter::Nearest => {
            if x < 0.5 {
                1.0
            } else {
                0.0
            }
        }
        ResamplingFilter::Box => {
            if x < 0.5 {
                1.0
            } else {
                0.0
            }
        }
        ResamplingFilter::Bilinear => {
            if x < 1.0 {
                1.0 - x
            } else {
                0.0
            }
        }
        ResamplingFilter::Bicubic => {
            // Catmull-Rom spline (a = -0.5)
            if x <= 1.0 {
                1.5 * x * x * x - 2.5 * x * x + 1.0
            } else if x < 2.0 {
                -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
            } else {
                0.0
            }
        }
        ResamplingFilter::Lanczos => {
            // Lanczos3: sinc(x) * sinc(x / 3) for |x| < 3
            if x < 3.0 {
                sinc(x) * sinc(x / 3.0)
            } else {
                0.0
            }
        }
    }
}

#[inline]
fn filter_radius(f: ResamplingFilter) -> f64 {
    match f {
        ResamplingFilter::Nearest | ResamplingFilter::Box => 0.5,
        ResamplingFilter::Bilinear => 1.0,
        ResamplingFilter::Bicubic => 2.0,
        ResamplingFilter::Lanczos => 3.0,
    }
}

struct Contribution {
    pixel: usize,
    weight: f64,
}

struct FilterWeights {
    entries: Vec<Contribution>,
}

fn compute_weights(
    in_size: usize,
    out_size: usize,
    filter: ResamplingFilter,
) -> Vec<FilterWeights> {
    let scale = out_size as f64 / in_size as f64;
    let (filter_scale, radius) = if scale < 1.0 {
        // Downsampling: stretch filter
        (scale, filter_radius(filter) / scale)
    } else {
        // Upsampling
        (1.0, filter_radius(filter))
    };

    let mut weights = Vec::with_capacity(out_size);
    for out_idx in 0..out_size {
        let center = (out_idx as f64 + 0.5) / scale - 0.5;
        let left = (center - radius).floor().max(0.0) as usize;
        let right = (center + radius).ceil().min((in_size - 1) as f64) as usize;

        let mut sum = 0.0;
        let mut entries = Vec::with_capacity(right - left + 1);
        for in_idx in left..=right {
            let dist = (in_idx as f64 - center) * filter_scale;
            let w = filter_weight(filter, dist);
            if w != 0.0 {
                sum += w;
                entries.push(Contribution {
                    pixel: in_idx,
                    weight: w,
                });
            }
        }

        if sum != 0.0 {
            for entry in &mut entries {
                entry.weight /= sum;
            }
        } else if !entries.is_empty() {
            let eq_w = 1.0 / entries.len() as f64;
            for entry in &mut entries {
                entry.weight = eq_w;
            }
        }

        weights.push(FilterWeights { entries });
    }
    weights
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

/// General-purpose multi-algorithm 2-pass separable image resizer.
///
/// 8-bit rasters (`Gray`/`Rgb`/`Rgba` over `u8`) reproduce Pillow's
/// `Image.resize` bit for bit for `Nearest`, `Bilinear`, `Bicubic` and
/// `Lanczos` (RGBA is filtered per channel, without premultiplying
/// alpha); `Box` is the integer area average of [`box_average`]. Wider
/// samples use a floating-point convolution.
///
/// Supports [`ResamplingFilter::Nearest`], [`ResamplingFilter::Box`],
/// [`ResamplingFilter::Bilinear`], [`ResamplingFilter::Bicubic`], and
/// [`ResamplingFilter::Lanczos`].
pub fn resize<L: Layout, T: Sample>(
    img: &Image<L, T>,
    out_w: u32,
    out_h: u32,
    filter: ResamplingFilter,
) -> Result<Image<L, T>> {
    if out_w == 0 || out_h == 0 {
        return Err(Error::BadValue("output dimension is zero"));
    }
    if filter == ResamplingFilter::Box {
        return box_average(img, out_w, out_h);
    }

    let in_w = img.width() as usize;
    let in_h = img.height() as usize;
    let out_w_usize = out_w as usize;
    let out_h_usize = out_h as usize;
    let c = L::CHANNELS;
    let max_val = T::MAX_U64 as f64;

    if in_w == out_w_usize && in_h == out_h_usize {
        return Ok(img.clone());
    }

    let src = img.as_slice();

    // 8-bit rasters take the Pillow-exact fixed-point path.
    if T::MAX_U64 == 255 {
        let bytes: Vec<u8> = src.iter().map(|v| v.to_u64() as u8).collect();
        let out = super::pillow::resize_u8(&bytes, in_w, in_h, c, out_w_usize, out_h_usize, filter);
        return Image::from_vec(
            out_w,
            out_h,
            out.into_iter().map(|v| T::from_u64(u64::from(v))).collect(),
        );
    }

    // Wider samples: floating-point separable convolution.
    // Pass 1: Horizontal resize (in_w, in_h) -> (out_w, in_h)
    let h_weights = compute_weights(in_w, out_w_usize, filter);
    let mut intermediate = vec![0.0f64; out_w_usize * in_h * c];

    for y in 0..in_h {
        let in_row_offset = y * in_w * c;
        let out_row_offset = y * out_w_usize * c;
        for (x, w_info) in h_weights.iter().enumerate() {
            let out_base = out_row_offset + x * c;
            for k in 0..c {
                let mut accum = 0.0f64;
                for contrib in &w_info.entries {
                    let in_val = src[in_row_offset + contrib.pixel * c + k].to_u64() as f64;
                    accum += in_val * contrib.weight;
                }
                intermediate[out_base + k] = accum;
            }
        }
    }

    // Pass 2: Vertical resize (out_w, in_h) -> (out_w, out_h)
    let v_weights = compute_weights(in_h, out_h_usize, filter);
    let mut final_out = vec![T::default(); out_w_usize * out_h_usize * c];

    for (y, w_info) in v_weights.iter().enumerate() {
        let out_row_offset = y * out_w_usize * c;
        for x in 0..out_w_usize {
            let out_base = out_row_offset + x * c;
            for k in 0..c {
                let mut accum = 0.0f64;
                for contrib in &w_info.entries {
                    let in_val = intermediate[contrib.pixel * out_w_usize * c + x * c + k];
                    accum += in_val * contrib.weight;
                }
                let clamped = accum.round().clamp(0.0, max_val) as u64;
                final_out[out_base + k] = T::from_u64(clamped);
            }
        }
    }

    Image::from_vec(out_w, out_h, final_out)
}
