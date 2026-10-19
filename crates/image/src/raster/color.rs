//! BT.601 luma: `Y = round_half_up(0.299·R + 0.587·G + 0.114·B)`.
//!
//! The spec fixes the formula and the rounding; this module fixes the
//! arithmetic. `(299·R + 587·G + 114·B + 500) / 1000` in `u64` **is**
//! `round_half_up(0.299·R + 0.587·G + 0.114·B)` exactly — the decimal
//! coefficients are rationals with denominator 1000, so integer division
//! computes the mathematical value with no float representation error.
//! A float `0.299` is not exactly `299/1000`; that difference is what
//! this form eliminates.
//!
//! Half-up (`.5` away from zero) is the kit's convention: `round_half_up`
//! everywhere, pinned by tests. The result is clamped to the sample
//! range defensively, though a sum of three weights totalling exactly 1
//! over in-range inputs can never exceed it.
//!
//! `Image<Rgb>` converts per pixel through [`luma_bt601`]; `Image<Rgba>`
//! ignores the alpha channel outright — luma of the picture is wanted,
//! not a premultiplied composite, which is a rendering concern the
//! hashing pipeline does not have.
use alloc::vec::Vec;

use crate::raster::{Cmyk, Gray, Image, La, Rgb, Rgba, Sample};

/// BT.601 luma of one pixel: `round_half_up(0.299·r + 0.587·g +
/// 0.114·b)`, clamped to the sample's range.
///
/// Exact integer form: `round_half_up((299·r + 587·g + 114·b) / 1000)`.
/// Works for `u8` and `u16` samples; intermediates never leave `u64`.
#[inline]
pub fn luma_bt601<T: Sample>(r: T, g: T, b: T) -> T {
    let sum = 299 * r.to_u64() + 587 * g.to_u64() + 114 * b.to_u64();
    let y = (sum + 500) / 1000;
    T::from_u64(y.min(T::MAX_U64))
}

/// Converts RGB to YCbCr using standard JFIF / BT.601 full-range integer arithmetic.
#[inline]
pub fn rgb_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = r as i32;
    let g = g as i32;
    let b = b as i32;
    let y = (19595 * r + 38470 * g + 7471 * b + 32768) >> 16;
    let cb = ((-11059 * r - 21709 * g + 32768 * b + 32768) >> 16) + 128;
    let cr = ((32768 * r - 27439 * g - 5329 * b + 32768) >> 16) + 128;
    (
        y.clamp(0, 255) as u8,
        cb.clamp(0, 255) as u8,
        cr.clamp(0, 255) as u8,
    )
}

/// Converts YCbCr to RGB using libjpeg / JFIF fixed-point formulation.
#[inline]
pub fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
    let y = y as i32;
    let cb = cb as i32 - 128;
    let cr = cr as i32 - 128;
    let r = y + ((91881 * cr + 32768) >> 16);
    let g = y - ((22554 * cb + 46802 * cr + 32768) >> 16);
    let b = y + ((116130 * cb + 32768) >> 16);
    (
        r.clamp(0, 255) as u8,
        g.clamp(0, 255) as u8,
        b.clamp(0, 255) as u8,
    )
}

/// Converts RGB to CMYK.
#[inline]
pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    if r == 0 && g == 0 && b == 0 {
        return (0, 0, 0, 255);
    }
    let max_val = r.max(g).max(b) as u32;
    let k = 255 - max_val;
    let c = ((max_val - r as u32) * 255) / max_val;
    let m = ((max_val - g as u32) * 255) / max_val;
    let y = ((max_val - b as u32) * 255) / max_val;
    (c as u8, m as u8, y as u8, k as u8)
}

/// Converts CMYK to RGB.
#[inline]
pub fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
    let k_inv = 255 - k as u32;
    let r = (k_inv * (255 - c as u32)) / 255;
    let g = (k_inv * (255 - m as u32)) / 255;
    let b = (k_inv * (255 - y as u32)) / 255;
    (r as u8, g as u8, b as u8)
}

/// Converts single YUV420p / Rec.601 pixel to RGB.
#[inline]
pub fn yuv420p_to_rgb_pixel(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;
    let r = (298 * c + 409 * e + 128) >> 8;
    let g = (298 * c - 100 * d - 208 * e + 128) >> 8;
    let b = (298 * c + 516 * d + 128) >> 8;
    (
        r.clamp(0, 255) as u8,
        g.clamp(0, 255) as u8,
        b.clamp(0, 255) as u8,
    )
}

/// Converts RGB pixel to YUV420p / Rec.601 pixel.
#[inline]
pub fn rgb_to_yuv420p_pixel(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = r as i32;
    let g = g as i32;
    let b = b as i32;
    let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
    let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
    let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
    (
        y.clamp(0, 255) as u8,
        u.clamp(0, 255) as u8,
        v.clamp(0, 255) as u8,
    )
}

impl Image<Rgb, u8> {
    /// Per-pixel BT.601 luma as a `Gray` image of the same dimensions.
    pub fn to_gray(&self) -> Image<Gray, u8> {
        to_gray_rgb(self)
    }

    /// Converts to RGBA with alpha = 255.
    pub fn to_rgba(&self) -> Image<Rgba, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height() * 4) as usize);
        for px in self.as_slice().chunks_exact(3) {
            out.push(px[0]);
            out.push(px[1]);
            out.push(px[2]);
            out.push(255);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }
}

impl Image<Rgb, u16> {
    /// Per-pixel BT.601 luma as a `Gray` image of the same dimensions.
    pub fn to_gray(&self) -> Image<Gray, u16> {
        to_gray_rgb(self)
    }
}

impl Image<Rgba, u8> {
    /// Per-pixel BT.601 luma of the RGB channels; **alpha is ignored**,
    /// not composited.
    pub fn to_gray(&self) -> Image<Gray, u8> {
        to_gray_rgba(self)
    }

    /// Converts to RGB by dropping alpha channel.
    pub fn to_rgb(&self) -> Image<Rgb, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height() * 3) as usize);
        for px in self.as_slice().chunks_exact(4) {
            out.push(px[0]);
            out.push(px[1]);
            out.push(px[2]);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }
}

impl Image<Rgba, u16> {
    /// Per-pixel BT.601 luma of the RGB channels; **alpha is ignored**,
    /// not composited.
    pub fn to_gray(&self) -> Image<Gray, u16> {
        to_gray_rgba(self)
    }
}

impl Image<Gray, u8> {
    /// Expands single-channel Gray to RGB (R=G=B).
    pub fn to_rgb(&self) -> Image<Rgb, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height() * 3) as usize);
        for &px in self.as_slice() {
            out.push(px);
            out.push(px);
            out.push(px);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }

    /// Expands single-channel Gray to RGBA (R=G=B, A=255).
    pub fn to_rgba(&self) -> Image<Rgba, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height() * 4) as usize);
        for &px in self.as_slice() {
            out.push(px);
            out.push(px);
            out.push(px);
            out.push(255);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }
}

impl Image<La, u8> {
    /// Converts Luma-Alpha to Gray by dropping alpha.
    pub fn to_gray(&self) -> Image<Gray, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height()) as usize);
        for px in self.as_slice().chunks_exact(2) {
            out.push(px[0]);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }

    /// Converts Luma-Alpha to RGBA (R=G=B=L, A=A).
    pub fn to_rgba(&self) -> Image<Rgba, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height() * 4) as usize);
        for px in self.as_slice().chunks_exact(2) {
            out.push(px[0]);
            out.push(px[0]);
            out.push(px[0]);
            out.push(px[1]);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }
}

impl Image<Cmyk, u8> {
    /// Converts CMYK to RGB.
    pub fn to_rgb(&self) -> Image<Rgb, u8> {
        let mut out = Vec::with_capacity((self.width() * self.height() * 3) as usize);
        for px in self.as_slice().chunks_exact(4) {
            let (r, g, b) = cmyk_to_rgb(px[0], px[1], px[2], px[3]);
            out.push(r);
            out.push(g);
            out.push(b);
        }
        match Image::from_vec(self.width(), self.height(), out) {
            Ok(im) => im,
            Err(_) => unreachable!("exact capacity"),
        }
    }
}

fn to_gray_rgb<T: Sample>(img: &Image<Rgb, T>) -> Image<Gray, T> {
    let mut out = Vec::with_capacity((img.width() * img.height()) as usize);
    for px in img.as_slice().chunks_exact(3) {
        out.push(luma_bt601(px[0], px[1], px[2]));
    }
    match Image::from_vec(img.width(), img.height(), out) {
        Ok(im) => im,
        Err(_) => unreachable!("luma buffer is exactly width*height samples"),
    }
}

fn to_gray_rgba<T: Sample>(img: &Image<Rgba, T>) -> Image<Gray, T> {
    let mut out = Vec::with_capacity((img.width() * img.height()) as usize);
    for px in img.as_slice().chunks_exact(4) {
        out.push(luma_bt601(px[0], px[1], px[2]));
    }
    match Image::from_vec(img.width(), img.height(), out) {
        Ok(im) => im,
        Err(_) => unreachable!("luma buffer is exactly width*height samples"),
    }
}
