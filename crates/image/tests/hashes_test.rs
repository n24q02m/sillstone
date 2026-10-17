//! Conformance and invariant tests for image perceptual hashing:
//! aHash, dHash, wHash, and colorhash.

use sillstone_digest::Digest;
use sillstone_digest::hamming;
use sillstone_image::raster::{Gray, Image, Rgb};
use sillstone_image::{image_ahash, image_colorhash, image_dhash, image_whash};

/// Independent O(N^2) reference implementation of aHash.
fn ref_ahash(pixels: &[u8], width: usize, height: usize) -> u64 {
    // 8x8 box average
    let mut grid = [0u64; 64];
    let block_w = width / 8;
    let block_h = height / 8;
    for by in 0..8 {
        for bx in 0..8 {
            let mut sum = 0u64;
            let mut count = 0u64;
            for y in (by * block_h)..((by + 1) * block_h) {
                for x in (bx * block_w)..((bx + 1) * block_w) {
                    sum += pixels[y * width + x] as u64;
                    count += 1;
                }
            }
            grid[by * 8 + bx] = sum / count;
        }
    }
    let total_sum: u64 = grid.iter().sum();
    let mut hash = 0u64;
    for (i, &val) in grid.iter().enumerate() {
        if val * 64 >= total_sum {
            hash |= 1u64 << (63 - i);
        }
    }
    hash
}

/// Independent O(N^2) reference implementation of dHash.
fn ref_dhash(pixels: &[u8], width: usize, height: usize) -> u64 {
    // 9x8 box average
    let mut grid = [0u64; 72];
    let block_w = width / 9;
    let block_h = height / 8;
    for by in 0..8 {
        for bx in 0..9 {
            let mut sum = 0u64;
            let mut count = 0u64;
            for y in (by * block_h)..((by + 1) * block_h) {
                for x in (bx * block_w)..((bx + 1) * block_w) {
                    sum += pixels[y * width + x] as u64;
                    count += 1;
                }
            }
            grid[by * 9 + bx] = sum / count;
        }
    }
    let mut hash = 0u64;
    let mut bit = 63;
    for y in 0..8 {
        for x in 0..8 {
            if grid[y * 9 + x] > grid[y * 9 + x + 1] {
                hash |= 1u64 << bit;
            }
            if bit > 0 {
                bit -= 1;
            }
        }
    }
    hash
}

#[test]
fn test_constant_image_invariants() {
    let mut flat = vec![128u8; 64 * 64];
    let img = Image::<Gray, u8>::from_vec(64, 64, flat.clone()).unwrap();

    let h_ahash = image_ahash(&img).unwrap();
    let h_dhash = image_dhash(&img).unwrap();
    let h_whash = image_whash(&img, 2).unwrap();
    let h_color = image_colorhash(&img, 4).unwrap();

    // Constant image: every pixel equals the mean, so every bit is 1 in aHash
    assert_eq!(h_ahash, 0xFFFF_FFFF_FFFF_FFFF);
    // dHash: adjacent pixels are equal, none is strictly greater, so all bits are 0
    assert_eq!(h_dhash, 0);
    // Fixed deterministic values
    assert_ne!(h_whash, 0);
    assert_ne!(h_color, 0);

    // Uniform brightness shift: +30 to all pixels
    for p in &mut flat {
        *p = (*p).saturating_add(30);
    }
    let shifted = Image::<Gray, u8>::from_vec(64, 64, flat).unwrap();
    assert_eq!(image_ahash(&shifted).unwrap(), h_ahash);
    assert_eq!(image_dhash(&shifted).unwrap(), h_dhash);
}

#[test]
fn test_independent_reference_parity() {
    // Generate gradient / pattern image
    let (w, h) = (72, 64);
    let mut buf = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            buf.push(((x * 3 + y * 5) % 256) as u8);
        }
    }

    let img = Image::<Gray, u8>::from_vec(w as u32, h as u32, buf.clone()).unwrap();
    let actual_ahash = image_ahash(&img).unwrap();
    let ref_a = ref_ahash(&buf, w, h);
    assert_eq!(actual_ahash, ref_a);

    let actual_dhash = image_dhash(&img).unwrap();
    let ref_d = ref_dhash(&buf, w, h);
    assert_eq!(actual_dhash, ref_d);
}

#[test]
fn test_resize_distance_invariants() {
    // 32x32 image with structured content
    let (w1, h1) = (32, 32);
    let mut buf1 = Vec::with_capacity(w1 * h1);
    for y in 0..h1 {
        for x in 0..w1 {
            let val = if (x / 4 + y / 4) % 2 == 0 { 200u8 } else { 50u8 };
            buf1.push(val);
        }
    }
    let img1 = Image::<Gray, u8>::from_vec(w1 as u32, h1 as u32, buf1.clone()).unwrap();

    // 2x scaled image: 64x64 with identical checkerboard
    let (w2, h2) = (64, 64);
    let mut buf2 = Vec::with_capacity(w2 * h2);
    for y in 0..h2 {
        for x in 0..w2 {
            let val = if (x / 8 + y / 8) % 2 == 0 { 200u8 } else { 50u8 };
            buf2.push(val);
        }
    }
    let img2 = Image::<Gray, u8>::from_vec(w2 as u32, h2 as u32, buf2).unwrap();

    let h1_a = image_ahash(&img1).unwrap();
    let h2_a = image_ahash(&img2).unwrap();
    assert_eq!(hamming(&Digest::from_bytes(h1_a.to_be_bytes()), &Digest::from_bytes(h2_a.to_be_bytes())), 0);

    let h1_d = image_dhash(&img1).unwrap();
    let h2_d = image_dhash(&img2).unwrap();
    assert!(hamming(&Digest::from_bytes(h1_d.to_be_bytes()), &Digest::from_bytes(h2_d.to_be_bytes())) <= 4);

    let h1_w = image_whash(&img1, 2).unwrap();
    let h2_w = image_whash(&img2, 2).unwrap();
    assert!(hamming(&Digest::from_bytes(h1_w.to_be_bytes()), &Digest::from_bytes(h2_w.to_be_bytes())) <= 4);
}

#[test]
fn test_colorhash_discrimination() {
    // Red image vs Blue image
    let red_pixels = vec![255, 0, 0, 255, 0, 0];
    let blue_pixels = vec![0, 0, 255, 0, 0, 255];

    let img_red = Image::<Rgb, u8>::from_vec(2, 1, red_pixels).unwrap();
    let img_blue = Image::<Rgb, u8>::from_vec(2, 1, blue_pixels).unwrap();

    let h_red = image_colorhash(&img_red, 4).unwrap();
    let h_blue = image_colorhash(&img_blue, 4).unwrap();

    assert!(hamming(&Digest::from_bytes(h_red.to_be_bytes()), &Digest::from_bytes(h_blue.to_be_bytes())) > 0);
}

#[test]
fn test_ffi_surface() {
    use sillstone_image::ffi::*;

    let data = [128u8; 16 * 16];
    let mut out: u64 = 0;

    let status = unsafe {
        sillstone_image_ahash(data.as_ptr(), data.len(), 16, 16, PITH_LAYOUT_GRAY8, &mut out)
    };
    assert_eq!(status, PITH_OK);
    assert_eq!(out, 0xFFFF_FFFF_FFFF_FFFF);

    let status = unsafe {
        sillstone_image_dhash(data.as_ptr(), data.len(), 16, 16, PITH_LAYOUT_GRAY8, &mut out)
    };
    assert_eq!(status, PITH_OK);
    assert_eq!(out, 0);

    let status = unsafe {
        sillstone_image_whash(data.as_ptr(), data.len(), 16, 16, PITH_LAYOUT_GRAY8, 1, &mut out)
    };
    assert_eq!(status, PITH_OK);

    let status = unsafe {
        sillstone_image_colorhash(data.as_ptr(), data.len(), 16, 16, PITH_LAYOUT_GRAY8, 4, &mut out)
    };
    assert_eq!(status, PITH_OK);

    // Error cases
    assert_eq!(
        unsafe { sillstone_image_ahash(core::ptr::null(), 0, 16, 16, PITH_LAYOUT_GRAY8, &mut out) },
        PITH_E_INVALID
    );
    assert_eq!(
        unsafe { sillstone_image_ahash(data.as_ptr(), data.len(), 16, 16, 999, &mut out) },
        PITH_E_INVALID
    );
    assert_eq!(
        unsafe { sillstone_image_ahash(data.as_ptr(), 5, 16, 16, PITH_LAYOUT_GRAY8, &mut out) },
        PITH_E_INVALID
    );
}
