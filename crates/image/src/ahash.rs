//! Average Hash (aHash): 64-bit perceptual hash using mean luminance thresholding.
//!
//! Pipeline:
//! 1. Convert to grayscale (BT.601 luma)
//! 2. Integer box-average downsample to 8×8
//! 3. Compute arithmetic mean of the 64 pixels
//! 4. Threshold each pixel: bit is 1 if pixel >= mean, 0 otherwise (MSB row-major).

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

/// Computes the 64-bit Average Hash (aHash) of an image.
pub fn image_ahash<L: Layout, T: Sample>(img: &Image<L, T>) -> Result<u64, Error> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let src = img.as_slice();
    let c = L::CHANNELS;

    let mut gray = Image::<Gray, u8>::new(img.width(), img.height())
        .map_err(|_| Error::BadValue("ahash buffer allocation"))?;
    {
        let out = gray.as_mut_slice();
        if out.len() < w * h || src.len() < w * h * c {
            return Err(Error::BadValue("ahash image buffer undersized"));
        }
        for i in 0..w * h {
            let p = &src[i * c..i * c + c];
            out[i] = match c {
                1 => to_u8(p[0]),
                _ => luma_bt601(to_u8(p[0]), to_u8(p[1]), to_u8(p[2])),
            };
        }
    }

    let small = box_average(&gray, 8, 8)
        .map_err(|_| Error::BadValue("ahash 8x8 box average"))?;
    let pixels = small.as_slice();

    let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
    // Use floating or integer mean. Using integer sum with round or direct comparison:
    // pixel * 64 >= sum is exact without float rounding
    let mut hash: u64 = 0;
    for (i, &p) in pixels.iter().enumerate() {
        if (p as u64) * 64 >= sum {
            hash |= 1u64 << (63 - i);
        }
    }

    Ok(hash)
}
