//! Difference Hash (dHash): 64-bit perceptual hash using horizontal gradient.
//!
//! Pipeline:
//! 1. Convert to grayscale (BT.601 luma)
//! 2. Integer box-average downsample to 9×8 (width 9, height 8)
//! 3. Compare adjacent pixels per row: bit is 1 if P[x] > P[x+1], 0 otherwise
//! 4. 8 rows × 8 comparisons = 64 bits (MSB row-major).

use crate::raster::{Gray, Image, Layout, Sample, box_average, luma_bt601};
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

/// Computes the 64-bit Difference Hash (dHash) of an image.
pub fn image_dhash<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<u64, Error> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let src = img.as_slice();
    let c = L::CHANNELS;

    let mut gray = Image::<Gray, u8>::new(img.width(), img.height())
        .map_err(|_| Error::BadValue("dhash buffer allocation"))?;
    {
        let out = gray.as_mut_slice();
        if out.len() < w * h || src.len() < w * h * c {
            return Err(Error::BadValue("dhash image buffer undersized"));
        }
        for i in 0..w * h {
            let p = &src[i * c..i * c + c];
            out[i] = match c {
                1 => to_u8(p[0]),
                _ => luma_bt601(to_u8(p[0]), to_u8(p[1]), to_u8(p[2])),
            };
        }
    }

    let small = box_average(&gray, 9, 8)
        .map_err(|_| Error::BadValue("dhash 9x8 box average"))?;
    let pixels = small.as_slice();

    let mut hash: u64 = 0;
    let mut bit = 63;
    for y in 0..8 {
        for x in 0..8 {
            let left = pixels[y * 9 + x];
            let right = pixels[y * 9 + x + 1];
            if left > right {
                hash |= 1u64 << bit;
            }
            if bit > 0 {
                bit -= 1;
            }
        }
    }

    Ok(hash)
}
