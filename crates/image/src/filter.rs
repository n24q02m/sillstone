//! Image convolution filters, Gaussian blur, and point color adjustments.

use alloc::vec;
use alloc::vec::Vec;

use crate::raster::{Image, Layout, Sample};
use sillstone_digest::{Error, Result};

/// Built-in 3x3 convolution kernels matching Pillow's `ImageFilter`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuiltinFilter {
    /// 3x3 box blur.
    Blur,
    /// 3x3 sharpening kernel.
    Sharpen,
    /// 3x3 contour detection.
    Contour,
    /// 3x3 detail enhancement.
    Detail,
    /// 3x3 edge enhancement.
    EdgeEnhance,
    /// 3x3 find edges.
    FindEdges,
    /// 3x3 emboss with +128 gray offset.
    Emboss,
}

/// Convolves an image with a 3x3 kernel.
///
/// Output is computed as `clamp((sum(pixel * kernel) / divisor) + offset)`.
pub fn convolve_3x3<L: Layout, T: Sample>(
    img: &Image<L, T>,
    kernel: &[f64; 9],
    divisor: f64,
    offset: f64,
) -> Result<Image<L, T>> {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let c = L::CHANNELS;

    if w == 0 || h == 0 {
        return Err(Error::BadValue("image dimensions must be positive"));
    }

    let src = img.as_slice();
    let mut out = vec![T::default(); w * h * c];
    let max_val = T::MAX_U64 as f64;
    let div = if divisor.abs() < 1e-8 { 1.0 } else { divisor };

    for y in 0..h {
        let y_prev = y.saturating_sub(1);
        let y_next = (y + 1).min(h - 1);

        for x in 0..w {
            let x_prev = x.saturating_sub(1);
            let x_next = (x + 1).min(w - 1);

            let neighbors = [
                (y_prev * w + x_prev) * c,
                (y_prev * w + x) * c,
                (y_prev * w + x_next) * c,
                (y * w + x_prev) * c,
                (y * w + x) * c,
                (y * w + x_next) * c,
                (y_next * w + x_prev) * c,
                (y_next * w + x) * c,
                (y_next * w + x_next) * c,
            ];

            let out_idx = (y * w + x) * c;
            for k in 0..c {
                let mut accum = 0.0f64;
                for (i, &idx) in neighbors.iter().enumerate() {
                    accum += (src[idx + k].to_u64() as f64) * kernel[i];
                }
                let val = (accum / div) + offset;
                out[out_idx + k] = T::from_u64(val.round().clamp(0.0, max_val) as u64);
            }
        }
    }

    Image::from_vec(w as u32, h as u32, out)
}

/// Applies one of the standard [`BuiltinFilter`] convolutions.
pub fn apply_filter<L: Layout, T: Sample>(
    img: &Image<L, T>,
    filter: BuiltinFilter,
) -> Result<Image<L, T>> {
    match filter {
        BuiltinFilter::Blur => {
            let k = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
            convolve_3x3(img, &k, 9.0, 0.0)
        }
        BuiltinFilter::Sharpen => {
            let k = [-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0];
            convolve_3x3(img, &k, 1.0, 0.0)
        }
        BuiltinFilter::Contour => {
            let k = [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0];
            convolve_3x3(img, &k, 1.0, 255.0)
        }
        BuiltinFilter::Detail => {
            let k = [0.0, -1.0, 0.0, -1.0, 6.0, -1.0, 0.0, -1.0, 0.0];
            convolve_3x3(img, &k, 2.0, 0.0)
        }
        BuiltinFilter::EdgeEnhance => {
            let k = [-1.0, -1.0, -1.0, -1.0, 10.0, -1.0, -1.0, -1.0, -1.0];
            convolve_3x3(img, &k, 2.0, 0.0)
        }
        BuiltinFilter::FindEdges => {
            let k = [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0];
            convolve_3x3(img, &k, 1.0, 0.0)
        }
        BuiltinFilter::Emboss => {
            let k = [-2.0, -1.0, 0.0, -1.0, 1.0, 1.0, 0.0, 1.0, 2.0];
            convolve_3x3(img, &k, 1.0, 128.0)
        }
    }
}

/// Applies a 2-pass separable Gaussian blur with given radius.
///
/// Uses $\sigma = \text{radius} / 2.0$ and a kernel radius of $\lceil 2 \sigma \rceil$.
pub fn gaussian_blur<L: Layout, T: Sample>(
    img: &Image<L, T>,
    radius: f64,
) -> Result<Image<L, T>> {
    if radius <= 0.0 {
        return Ok(img.clone());
    }

    let w = img.width() as usize;
    let h = img.height() as usize;
    let c = L::CHANNELS;

    let sigma = radius / 2.0;
    let k_radius = (2.0 * sigma).ceil().max(1.0) as isize;
    let two_sigma_sq = 2.0 * sigma * sigma;

    // Generate 1D Gaussian kernel
    let mut kernel = Vec::with_capacity((2 * k_radius + 1) as usize);
    let mut k_sum = 0.0f64;
    for i in -k_radius..=k_radius {
        let weight = (-(i as f64 * i as f64) / two_sigma_sq).exp();
        kernel.push(weight);
        k_sum += weight;
    }
    for w_val in &mut kernel {
        *w_val /= k_sum;
    }

    let src = img.as_slice();
    let mut intermediate = vec![0.0f64; w * h * c];

    // Pass 1: Horizontal 1D convolution
    for y in 0..h {
        let row_offset = y * w * c;
        for x in 0..w {
            let out_base = row_offset + x * c;
            for ch in 0..c {
                let mut accum = 0.0f64;
                for (k_idx, &weight) in kernel.iter().enumerate() {
                    let offset = k_idx as isize - k_radius;
                    let src_x = (x as isize + offset).clamp(0, w as isize - 1) as usize;
                    accum += (src[row_offset + src_x * c + ch].to_u64() as f64) * weight;
                }
                intermediate[out_base + ch] = accum;
            }
        }
    }

    // Pass 2: Vertical 1D convolution
    let max_val = T::MAX_U64 as f64;
    let mut final_out = vec![T::default(); w * h * c];

    for y in 0..h {
        let out_row_offset = y * w * c;
        for x in 0..w {
            let out_base = out_row_offset + x * c;
            for ch in 0..c {
                let mut accum = 0.0f64;
                for (k_idx, &weight) in kernel.iter().enumerate() {
                    let offset = k_idx as isize - k_radius;
                    let src_y = (y as isize + offset).clamp(0, h as isize - 1) as usize;
                    accum += intermediate[src_y * w * c + x * c + ch] * weight;
                }
                final_out[out_base + ch] = T::from_u64(accum.round().clamp(0.0, max_val) as u64);
            }
        }
    }

    Image::from_vec(w as u32, h as u32, final_out)
}

/// Adjusts image brightness: `pixel * factor`.
/// `factor = 1.0` is identity, `0.0` is black, `> 1.0` is brighter.
pub fn adjust_brightness<L: Layout, T: Sample>(
    img: &Image<L, T>,
    factor: f64,
) -> Result<Image<L, T>> {
    let mut out = img.as_slice().to_vec();
    let max_val = T::MAX_U64 as f64;
    for sample in &mut out {
        let v = (sample.to_u64() as f64 * factor).round().clamp(0.0, max_val) as u64;
        *sample = T::from_u64(v);
    }
    Image::from_vec(img.width(), img.height(), out)
}

/// Inverts image colors: `MAX_U64 - pixel`.
pub fn invert<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<Image<L, T>> {
    let mut out = img.as_slice().to_vec();
    let max_val = T::MAX_U64;
    for sample in &mut out {
        *sample = T::from_u64(max_val - sample.to_u64());
    }
    Image::from_vec(img.width(), img.height(), out)
}
