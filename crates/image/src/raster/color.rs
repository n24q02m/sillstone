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
//! # Two luma formulas, on purpose
//!
//! Pillow's `convert("L")` is **not** the rational BT.601 form: it is the
//! fixed-point `(19595·R + 38470·G + 7471·B + 0x8000) >> 16`
//! ([`crate::raster::rgb_to_l`], from `libImaging/Convert.c`), whose
//! coefficients are the `u16` truncations of `0.299 / 0.587 / 0.114`.
//! [`luma_bt601`] rounds the exact rational instead. The two disagree by
//! one on 9040 of the 16 777 216 `sRGB` triples (`(0, 0, 250)` is 29
//! against Pillow's 28), so a caller cannot pick either one blindly:
//!
//! * `Image<Rgb>` / `Image<Rgba>` `to_gray` of `u8` samples is Pillow's
//!   `convert("L")`, so it goes through `rgb_to_l` — that is the whole
//!   point of the image crate, and `tests/pillow_parity.rs` replays the
//!   committed `convert` vectors through the public method.
//! * The modhash pHash oracle (`lab/phash_oracle.py`, pinned by
//!   `tests/phash_vectors.rs`) is defined on [`luma_bt601`], so `phash`
//!   keeps using it; `tests/color.rs` pins both numbers for one colour so
//!   a "unification" cannot silently move the hash.
//! * `u16` rasters have no Pillow mode to match (`I;16` is single-channel
//!   and `convert("L")` is 8-bit), so their `to_gray` uses the
//!   range-general [`luma_bt601`].
//!
//! `Image<Rgba>` ignores the alpha channel outright — luma of the picture
//! is wanted, not a premultiplied composite, which is a rendering concern
//! the hashing pipeline does not have. That matches Pillow's
//! `convert("L")`, which drops alpha.
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

/// Converts RGB to CMYK exactly as Pillow's `Image.convert("CMYK")` does
/// (`rgb2cmyk` in `libImaging/Convert.c`): the naive complement with no
/// black generation, so `K` is always 0.
#[inline]
pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    (255 - r, 255 - g, 255 - b, 0)
}

/// `round(a * b / 255)` with Pillow's `MULDIV255` integer trick.
#[inline]
fn muldiv255(a: u32, b: u32) -> u32 {
    let t = a * b + 128;
    ((t >> 8) + t) >> 8
}

/// Converts CMYK to RGB exactly as Pillow's `Image.convert("RGB")` does
/// (`cmyk2rgb` in `libImaging/Convert.c`): each channel is
/// `(255 - k) - round(ch * (255 - k) / 255)`.
#[inline]
pub fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
    let nk = 255 - u32::from(k);
    let channel = |v: u8| (nk - muldiv255(u32::from(v), nk)) as u8;
    (channel(c), channel(m), channel(y))
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
    /// Pillow's `convert("L")` luma as a `Gray` image of the same
    /// dimensions: the fixed-point `rgb_to_l`, not [`luma_bt601`].
    pub fn to_gray(&self) -> Image<Gray, u8> {
        let n = (self.width() * self.height()) as usize;
        let luma = super::pillow::rgb_plane_to_l(self.as_slice(), 3, n);
        match Image::from_vec(self.width(), self.height(), luma) {
            Ok(im) => im,
            Err(_) => unreachable!("luma buffer is exactly width*height samples"),
        }
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
    /// Range-general BT.601 luma as a `Gray` image of the same
    /// dimensions. Pillow has no 16-bit RGB mode, so this has no oracle;
    /// the 8-bit impl above is the Pillow match.
    pub fn to_gray(&self) -> Image<Gray, u16> {
        luma_bt601_image_rgb(self)
    }
}

impl Image<Rgba, u8> {
    /// Pillow's `convert("L")` luma of the RGB channels; **alpha is
    /// ignored**, not composited.
    pub fn to_gray(&self) -> Image<Gray, u8> {
        let n = (self.width() * self.height()) as usize;
        let luma = super::pillow::rgb_plane_to_l(self.as_slice(), 4, n);
        match Image::from_vec(self.width(), self.height(), luma) {
            Ok(im) => im,
            Err(_) => unreachable!("luma buffer is exactly width*height samples"),
        }
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
    /// Range-general BT.601 luma of the RGB channels; **alpha is
    /// ignored**, not composited. No Pillow oracle (see the 8-bit impl).
    pub fn to_gray(&self) -> Image<Gray, u16> {
        luma_bt601_image_rgba(self)
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

/// `Image<Rgb, u16>::to_gray`: the range-general BT.601 form, one sample
/// per pixel. Pillow has no 16-bit RGB mode, so there is no oracle to
/// match (the `u8` impls go through Pillow's plane luma instead).
fn luma_bt601_image_rgb<T: Sample>(img: &Image<Rgb, T>) -> Image<Gray, T> {
    let mut out = Vec::with_capacity((img.width() * img.height()) as usize);
    for px in img.as_slice().chunks_exact(3) {
        out.push(luma_bt601(px[0], px[1], px[2]));
    }
    match Image::from_vec(img.width(), img.height(), out) {
        Ok(im) => im,
        Err(_) => unreachable!("luma buffer is exactly width*height samples"),
    }
}

/// `Image<Rgba, u16>::to_gray`: as [`luma_bt601_image_rgb`], alpha dropped.
fn luma_bt601_image_rgba<T: Sample>(img: &Image<Rgba, T>) -> Image<Gray, T> {
    let mut out = Vec::with_capacity((img.width() * img.height()) as usize);
    for px in img.as_slice().chunks_exact(4) {
        out.push(luma_bt601(px[0], px[1], px[2]));
    }
    match Image::from_vec(img.width(), img.height(), out) {
        Ok(im) => im,
        Err(_) => unreachable!("luma buffer is exactly width*height samples"),
    }
}
