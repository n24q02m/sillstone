//! Color Hash (colorhash): 64-bit perceptual hash using RGB color space quantization.
//!
//! Pipeline:
//! 1. Map each pixel's RGB into 64 3D color bins (4 bins per channel: R, G, B)
//! 2. Accumulate frequency histogram across the 64 bins
//! 3. Compute arithmetic mean frequency
//! 4. Threshold each bin: bit is 1 if count >= mean, 0 otherwise (MSB row-major).

use crate::raster::{Image, Layout, Sample};
use sillstone_digest::Error;

/// Reduces one sample of any depth to `u8`.
#[inline]
fn to_u8<T: Sample>(v: T) -> u8 {
    if T::MAX_U64 == 255 {
        v.to_u64() as u8
    } else {
        (v.to_u64() >> 8) as u8
    }
}

/// Computes the 64-bit Color Hash of an image.
///
/// `bins` parameter controls color bin sensitivity (default 4 bins per channel).
pub fn image_colorhash<L: Layout, T: Sample>(img: &Image<L, T>, _bins: u32) -> Result<u64, Error> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let src = img.as_slice();
    let c = L::CHANNELS;

    if src.len() < w * h * c {
        return Err(Error::BadValue("colorhash image buffer undersized"));
    }

    let mut hist = [0u64; 64];

    for i in 0..w * h {
        let p = &src[i * c..i * c + c];
        let (r, g, b) = match c {
            1 => {
                let l = to_u8(p[0]);
                (l, l, l)
            }
            _ => (to_u8(p[0]), to_u8(p[1]), to_u8(p[2])),
        };

        // 4 bins per channel (2 bits each: 0..4)
        let r_bin = (r >> 6) as usize;
        let g_bin = (g >> 6) as usize;
        let b_bin = (b >> 6) as usize;
        let bin_idx = (r_bin << 4) | (g_bin << 2) | b_bin;
        hist[bin_idx] += 1;
    }

    let total: u64 = (w * h) as u64;
    let mut hash: u64 = 0;
    for (i, &count) in hist.iter().enumerate() {
        // count >= mean <=> count * 64 >= total
        if count * 64 >= total {
            hash |= 1u64 << (63 - i);
        }
    }

    Ok(hash)
}
