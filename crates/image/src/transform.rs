//! Geometric image transformations: crop, transpose, and arbitrary rotation.

use alloc::vec;
use alloc::vec::Vec;

use crate::raster::{Image, Layout, ResamplingFilter, Sample};
use sillstone_digest::{Error, Result};

/// Transpose and 90-degree step rotation methods matching Pillow's `Image.Transpose`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TransposeMethod {
    /// Flips the image horizontally (left to right).
    FlipHorizontal,
    /// Flips the image vertically (top to bottom).
    FlipVertical,
    /// Rotates 90 degrees counter-clockwise (or 270 clockwise).
    Rotate90,
    /// Rotates 180 degrees.
    Rotate180,
    /// Rotates 270 degrees counter-clockwise (or 90 clockwise).
    Rotate270,
    /// Transposes pixels across the main diagonal (x <-> y).
    Transpose,
    /// Transverse transpose across the anti-diagonal.
    Transverse,
}

/// Cuts a rectangular region `[x0, y0, x1, y1)` from `img`.
///
/// Refuses invalid or zero-area crops with [`Error::BadValue`].
pub fn crop<L: Layout, T: Sample>(
    img: &Image<L, T>,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
) -> Result<Image<L, T>> {
    let in_w = img.width();
    let in_h = img.height();

    if x0 >= x1 || y0 >= y1 || x1 > in_w || y1 > in_h {
        return Err(Error::BadValue("invalid crop box coordinates"));
    }

    let out_w = x1 - x0;
    let out_h = y1 - y0;
    let c = L::CHANNELS;
    let src = img.as_slice();
    let mut out = Vec::with_capacity((out_w * out_h) as usize * c);

    for y in y0..y1 {
        let row_start = (y as usize * in_w as usize + x0 as usize) * c;
        let row_end = row_start + out_w as usize * c;
        out.extend_from_slice(&src[row_start..row_end]);
    }

    Image::from_vec(out_w, out_h, out)
}

/// Transposes or rotates `img` by 90-degree steps without resampling.
pub fn transpose<L: Layout, T: Sample>(
    img: &Image<L, T>,
    method: TransposeMethod,
) -> Result<Image<L, T>> {
    let in_w = img.width() as usize;
    let in_h = img.height() as usize;
    let c = L::CHANNELS;
    let src = img.as_slice();

    let (out_w, out_h) = match method {
        TransposeMethod::FlipHorizontal | TransposeMethod::FlipVertical | TransposeMethod::Rotate180 => {
            (in_w, in_h)
        }
        TransposeMethod::Rotate90 | TransposeMethod::Rotate270 | TransposeMethod::Transpose | TransposeMethod::Transverse => {
            (in_h, in_w)
        }
    };

    let mut out = vec![T::default(); out_w * out_h * c];

    for y in 0..in_h {
        for x in 0..in_w {
            let (dst_x, dst_y) = match method {
                TransposeMethod::FlipHorizontal => (in_w - 1 - x, y),
                TransposeMethod::FlipVertical => (x, in_h - 1 - y),
                TransposeMethod::Rotate90 => (y, in_w - 1 - x),
                TransposeMethod::Rotate180 => (in_w - 1 - x, in_h - 1 - y),
                TransposeMethod::Rotate270 => (in_h - 1 - y, x),
                TransposeMethod::Transpose => (y, x),
                TransposeMethod::Transverse => (in_h - 1 - y, in_w - 1 - x),
            };

            let src_idx = (y * in_w + x) * c;
            let dst_idx = (dst_y * out_w + dst_x) * c;

            out[dst_idx..dst_idx + c].copy_from_slice(&src[src_idx..src_idx + c]);
        }
    }

    Image::from_vec(out_w as u32, out_h as u32, out)
}

/// Output dimensions [`rotate`] produces for an `in_w`×`in_h` image: swapped for exact
/// 90°/270° turns, unchanged for 0°/180° and for `expand == false`, else the bounding box
/// of the rotated rectangle. The FFI exposes it so SDKs can size the output buffer.
#[must_use]
pub fn rotated_size(in_w: usize, in_h: usize, angle_degrees: f64, expand: bool) -> (usize, usize) {
    let mut norm_angle = angle_degrees % 360.0;
    if norm_angle < 0.0 {
        norm_angle += 360.0;
    }
    if (norm_angle - 90.0).abs() < 1e-6 || (norm_angle - 270.0).abs() < 1e-6 {
        return (in_h, in_w);
    }
    if !expand || norm_angle.abs() < 1e-6 || (norm_angle - 180.0).abs() < 1e-6 {
        return (in_w, in_h);
    }
    let rad = norm_angle * core::f64::consts::PI / 180.0;
    let (sin_a, cos_a) = (rad.sin(), rad.cos());
    let w_f = in_w as f64;
    let h_f = in_h as f64;
    let p1_x = -w_f * 0.5 * cos_a - -h_f * 0.5 * sin_a;
    let p1_y = -w_f * 0.5 * sin_a + -h_f * 0.5 * cos_a;
    let p2_x = w_f * 0.5 * cos_a - -h_f * 0.5 * sin_a;
    let p2_y = w_f * 0.5 * sin_a + -h_f * 0.5 * cos_a;
    let p3_x = w_f * 0.5 * cos_a - h_f * 0.5 * sin_a;
    let p3_y = w_f * 0.5 * sin_a + h_f * 0.5 * cos_a;
    let p4_x = -w_f * 0.5 * cos_a - h_f * 0.5 * sin_a;
    let p4_y = -w_f * 0.5 * sin_a + h_f * 0.5 * cos_a;

    let min_x = p1_x.min(p2_x).min(p3_x).min(p4_x);
    let max_x = p1_x.max(p2_x).max(p3_x).max(p4_x);
    let min_y = p1_y.min(p2_y).min(p3_y).min(p4_y);
    let max_y = p1_y.max(p2_y).max(p3_y).max(p4_y);

    (
        (max_x - min_x).ceil().max(1.0) as usize,
        (max_y - min_y).ceil().max(1.0) as usize,
    )
}

/// Rotates `img` counter-clockwise by `angle_degrees`.
///
/// If `expand` is true, expands output dimensions to encompass the entire rotated image.
/// If `expand` is false, keeps original dimensions and crops.
pub fn rotate<L: Layout, T: Sample>(
    img: &Image<L, T>,
    angle_degrees: f64,
    expand: bool,
    filter: ResamplingFilter,
) -> Result<Image<L, T>> {
    let mut norm_angle = angle_degrees % 360.0;
    if norm_angle < 0.0 {
        norm_angle += 360.0;
    }

    // Fast-path for exact 90-degree multiples
    if (norm_angle - 0.0).abs() < 1e-6 {
        return Ok(img.clone());
    } else if (norm_angle - 90.0).abs() < 1e-6 {
        return transpose(img, TransposeMethod::Rotate90);
    } else if (norm_angle - 180.0).abs() < 1e-6 {
        return transpose(img, TransposeMethod::Rotate180);
    } else if (norm_angle - 270.0).abs() < 1e-6 {
        return transpose(img, TransposeMethod::Rotate270);
    }

    let in_w = img.width() as usize;
    let in_h = img.height() as usize;
    let rad = norm_angle * core::f64::consts::PI / 180.0;
    let (sin_a, cos_a) = (rad.sin(), rad.cos());

    let (out_w, out_h) = rotated_size(in_w, in_h, angle_degrees, expand);

    let c = L::CHANNELS;
    let src = img.as_slice();
    let mut out = vec![T::default(); out_w * out_h * c];

    let cx_in = (in_w as f64 - 1.0) * 0.5;
    let cy_in = (in_h as f64 - 1.0) * 0.5;
    let cx_out = (out_w as f64 - 1.0) * 0.5;
    let cy_out = (out_h as f64 - 1.0) * 0.5;

    for y_out in 0..out_h {
        let dy = y_out as f64 - cy_out;
        for x_out in 0..out_w {
            let dx = x_out as f64 - cx_out;
            // Inverse mapping: rotate backwards by rad
            let src_x = cos_a * dx + sin_a * dy + cx_in;
            let src_y = -sin_a * dx + cos_a * dy + cy_in;

            let out_idx = (y_out * out_w + x_out) * c;

            if filter == ResamplingFilter::Nearest {
                let rx = src_x.round() as isize;
                let ry = src_y.round() as isize;
                if rx >= 0 && rx < in_w as isize && ry >= 0 && ry < in_h as isize {
                    let in_idx = (ry as usize * in_w + rx as usize) * c;
                    out[out_idx..out_idx + c].copy_from_slice(&src[in_idx..in_idx + c]);
                }
            } else {
                // Bilinear sampling
                let x0 = src_x.floor() as isize;
                let y0 = src_y.floor() as isize;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                if x0 >= 0 && x1 < in_w as isize && y0 >= 0 && y1 < in_h as isize {
                    let fx = src_x - x0 as f64;
                    let fy = src_y - y0 as f64;
                    let w00 = (1.0 - fx) * (1.0 - fy);
                    let w10 = fx * (1.0 - fy);
                    let w01 = (1.0 - fx) * fy;
                    let w11 = fx * fy;

                    let idx00 = (y0 as usize * in_w + x0 as usize) * c;
                    let idx10 = (y0 as usize * in_w + x1 as usize) * c;
                    let idx01 = (y1 as usize * in_w + x0 as usize) * c;
                    let idx11 = (y1 as usize * in_w + x1 as usize) * c;

                    for k in 0..c {
                        let v = w00 * src[idx00 + k].to_u64() as f64
                            + w10 * src[idx10 + k].to_u64() as f64
                            + w01 * src[idx01 + k].to_u64() as f64
                            + w11 * src[idx11 + k].to_u64() as f64;
                        out[out_idx + k] = T::from_u64(v.round().clamp(0.0, T::MAX_U64 as f64) as u64);
                    }
                }
            }
        }
    }

    Image::from_vec(out_w as u32, out_h as u32, out)
}
