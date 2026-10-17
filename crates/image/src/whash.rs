//! Wavelet Hash (wHash): 64-bit perceptual hash using 2D Haar Discrete Wavelet Transform.
//!
//! Pipeline:
//! 1. Convert to grayscale (BT.601 luma)
//! 2. Downsample to power-of-two square grid (8 * 2^level, capped at 64×64)
//! 3. Compute 2D Haar DWT over `level` decomposition stages
//! 4. Extract the 8×8 LL (low-frequency approximation) subband
//! 5. Compute median of the 64 coefficients
//! 6. Threshold: bit is 1 if coeff >= median, 0 otherwise (MSB row-major).

use crate::raster::{Gray, Image, Layout, Sample, box_average, luma_bt601};
use sillstone_digest::Error;
use sillstone_math::median;

/// Reduces one sample of any depth to `u8`.
#[inline]
fn to_u8<T: Sample>(v: T) -> u8 {
    if T::MAX_U64 == 255 {
        v.to_u64() as u8
    } else {
        (v.to_u64() >> 8) as u8
    }
}

/// Computes the 64-bit Wavelet Hash (wHash) of an image using Haar DWT.
pub fn image_whash<L: Layout, T: Sample>(img: &Image<L, T>, level: u32) -> Result<u64, Error> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let src = img.as_slice();
    let c = L::CHANNELS;

    let mut gray = Image::<Gray, u8>::new(img.width(), img.height())
        .map_err(|_| Error::BadValue("whash buffer allocation"))?;
    {
        let out = gray.as_mut_slice();
        if out.len() < w * h || src.len() < w * h * c {
            return Err(Error::BadValue("whash image buffer undersized"));
        }
        for i in 0..w * h {
            let p = &src[i * c..i * c + c];
            out[i] = match c {
                1 => to_u8(p[0]),
                _ => luma_bt601(to_u8(p[0]), to_u8(p[1]), to_u8(p[2])),
            };
        }
    }

    // Clamp level to 0..=3 (size 8..=64)
    let eff_level = level.min(3);
    let size = (8 << eff_level) as usize;

    let small = box_average(&gray, size as u32, size as u32)
        .map_err(|_| Error::BadValue("whash box average"))?;

    // Allocate 2D grid as f64
    let mut grid = alloc::vec![0.0f64; size * size];
    for (dst, &src_val) in grid.iter_mut().zip(small.as_slice()) {
        *dst = f64::from(src_val);
    }

    // Perform Haar 2D DWT for `eff_level` stages
    let mut current_size = size;
    let inv_sqrt2 = 1.0 / core::f64::consts::SQRT_2;

    for _ in 0..eff_level {
        let half = current_size / 2;
        let mut row_tmp = alloc::vec![0.0f64; current_size];

        // 1D Haar on rows
        for y in 0..current_size {
            let row_offset = y * size;
            for x in 0..half {
                let a = grid[row_offset + 2 * x];
                let b = grid[row_offset + 2 * x + 1];
                row_tmp[x] = (a + b) * inv_sqrt2;
                row_tmp[half + x] = (a - b) * inv_sqrt2;
            }
            grid[row_offset..row_offset + current_size].copy_from_slice(&row_tmp);
        }

        // 1D Haar on columns
        let mut col_tmp = alloc::vec![0.0f64; current_size];
        for x in 0..current_size {
            for y in 0..half {
                let a = grid[(2 * y) * size + x];
                let b = grid[(2 * y + 1) * size + x];
                col_tmp[y] = (a + b) * inv_sqrt2;
                col_tmp[half + y] = (a - b) * inv_sqrt2;
            }
            for y in 0..current_size {
                grid[y * size + x] = col_tmp[y];
            }
        }

        current_size = half;
    }

    // Extract the top-left 8×8 LL subband
    let mut ll = [0.0f64; 64];
    for y in 0..8 {
        for x in 0..8 {
            ll[y * 8 + x] = grid[y * size + x];
        }
    }

    // Threshold by the median of the 64 coefficients
    let mut ll_copy = ll;
    let med = median(&mut ll_copy).unwrap_or(0.0);

    let mut hash: u64 = 0;
    for (i, &val) in ll.iter().enumerate() {
        if val >= med {
            hash |= 1u64 << (63 - i);
        }
    }

    Ok(hash)
}
