//! QOI (Quite OK Image) integration with [`Image`].

use alloc::vec::Vec;

use crate::raster::{Image, Rgb, Rgba};
use sillstone_digest::Result;

/// Decodes QOI image data as an RGB [`Image`].
pub fn qoi_decode_rgb(bytes: &[u8]) -> Result<Image<Rgb, u8>> {
    let (header, pixels) = sillstone_qoi::decode(bytes)?;
    if header.channels == 3 {
        Image::from_vec(header.width, header.height, pixels)
    } else {
        // Drop alpha channel
        let mut rgb = Vec::with_capacity((header.width * header.height * 3) as usize);
        for chunk in pixels.chunks_exact(4) {
            rgb.extend_from_slice(&chunk[..3]);
        }
        Image::from_vec(header.width, header.height, rgb)
    }
}

/// Decodes QOI image data as an RGBA [`Image`].
pub fn qoi_decode_rgba(bytes: &[u8]) -> Result<Image<Rgba, u8>> {
    let (header, pixels) = sillstone_qoi::decode(bytes)?;
    if header.channels == 4 {
        Image::from_vec(header.width, header.height, pixels)
    } else {
        // Expand to RGBA with alpha = 255
        let mut rgba = Vec::with_capacity((header.width * header.height * 4) as usize);
        for chunk in pixels.chunks_exact(3) {
            rgba.push(chunk[0]);
            rgba.push(chunk[1]);
            rgba.push(chunk[2]);
            rgba.push(255);
        }
        Image::from_vec(header.width, header.height, rgba)
    }
}

/// Encodes an RGB [`Image`] to QOI bytes.
pub fn qoi_encode_rgb(img: &Image<Rgb, u8>) -> Result<Vec<u8>> {
    sillstone_qoi::encode(img.width(), img.height(), 3, 0, img.as_slice())
}

/// Encodes an RGBA [`Image`] to QOI bytes.
pub fn qoi_encode_rgba(img: &Image<Rgba, u8>) -> Result<Vec<u8>> {
    sillstone_qoi::encode(img.width(), img.height(), 4, 0, img.as_slice())
}
