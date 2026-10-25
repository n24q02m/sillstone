//! Conformance tests for the `imagehash`-compatible image hashes.
//!
//! Every expected value was produced by `imagehash` 4.3.2 on Pillow 13.0.0
//! (`average_hash`, `dhash`, `phash`, `whash`, `colorhash(binbits=3)`,
//! default parameters); the arguments are listed next to each vector so it
//! can be regenerated. `scripts/bench/bench_hash_parity.py` re-checks the
//! same property against a live `imagehash` on random images.

use sillstone_digest::Digest;
use sillstone_digest::hamming;
use sillstone_image::raster::{Gray, Image, Rgb, Rgba};
use sillstone_image::{
    image_ahash, image_colorhash, image_dhash, image_phash_imagehash, image_whash,
};

/// `(ahash, dhash, phash, whash, colorhash)`.
type Five = (u64, u64, u64, u64, u64);

fn all_gray(img: &Image<Gray, u8>) -> Five {
    (
        image_ahash(img).unwrap(),
        image_dhash(img).unwrap(),
        image_phash_imagehash(img).unwrap(),
        image_whash(img, 0).unwrap(),
        image_colorhash(img, 3).unwrap(),
    )
}

fn dist(a: u64, b: u64) -> u32 {
    hamming(
        &Digest::from_bytes(a.to_be_bytes()),
        &Digest::from_bytes(b.to_be_bytes()),
    )
}

/// `Image.fromarray(((x*3 + y*5) % 256), "L")`, 72×64.
#[test]
fn gradient_matches_imagehash() {
    let (w, h) = (72usize, 64usize);
    let buf: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).map(move |x| ((x * 3 + y * 5) % 256) as u8))
        .collect();
    let img = Image::<Gray, u8>::from_vec(w as u32, h as u32, buf).unwrap();
    assert_eq!(
        all_gray(&img),
        (
            0x0f1e_7cf0_e083_071f,
            0xfffc_f8e3_8f1f_7ffe,
            0x95c2_953f_e50f_5852,
            0x0f3e_7cf0_e083_071f,
            0x0000_0070_0000_0000,
        )
    );
}

/// Checkerboard of `200`/`50` tiles; 32×32 with 4-px tiles and 64×64 with
/// 8-px tiles must hash identically (same content, scaled). `phash` is
/// not pinned: the DCT of such a symmetric image is exactly zero outside a
/// few coefficients and `imagehash` thresholds the rounding noise there
/// (see `docs/parity.md`).
#[test]
fn checkerboard_is_scale_invariant_and_matches_imagehash() {
    let make = |n: usize, tile: usize| {
        let buf: Vec<u8> = (0..n)
            .flat_map(|y| {
                (0..n).map(move |x| {
                    if (x / tile + y / tile) % 2 == 0 {
                        200
                    } else {
                        50
                    }
                })
            })
            .collect();
        Image::<Gray, u8>::from_vec(n as u32, n as u32, buf).unwrap()
    };
    let expected = (
        0xaa55_aa55_aa55_aa55,
        0x5aa5_5aa5_5aa5_5aa5,
        0xaa55_aa55_aa55_aa55,
        0x0000_0070_0000_0000,
    );
    let four = |img: &Image<Gray, u8>| {
        let (a, d, _, w, c) = all_gray(img);
        (a, d, w, c)
    };
    assert_eq!(four(&make(32, 4)), expected);
    assert_eq!(four(&make(64, 8)), expected);
}

/// A flat image: nothing is strictly greater than the mean, so `ahash`,
/// `dhash` and `whash` are 0 (`phash` is not pinned, see above).
#[test]
fn flat_image_matches_imagehash() {
    let img = Image::<Gray, u8>::from_vec(64, 64, vec![128u8; 64 * 64]).unwrap();
    let (a, d, _, w, c) = all_gray(&img);
    assert_eq!((a, d, w, c), (0, 0, 0, 0x0000_0070_0000_0000));
}

/// `colorhash` of single-colour 2×1 RGB images.
#[test]
fn colorhash_matches_imagehash() {
    let red = Image::<Rgb, u8>::from_vec(2, 1, vec![255, 0, 0, 255, 0, 0]).unwrap();
    let blue = Image::<Rgb, u8>::from_vec(2, 1, vec![0, 0, 255, 0, 0, 255]).unwrap();
    assert_eq!(image_colorhash(&red, 3).unwrap(), 0x0000_0000_0003_8000);
    assert_eq!(image_colorhash(&blue, 3).unwrap(), 0x0000_0380_0000_0000);
    assert!(
        dist(
            image_colorhash(&red, 3).unwrap(),
            image_colorhash(&blue, 3).unwrap()
        ) > 0
    );
}

/// The three committed fixture dumps through every hash (RGB, 16-bit gray
/// reduced with `>> 8`, RGBA).
#[test]
fn fixture_dumps_match_imagehash() {
    let read = |name: &str| {
        let file = format!(
            "{}/tests/fixtures/phash/{name}.raw",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(&file).unwrap_or_else(|e| panic!("cannot read {file}: {e}"))
    };

    let rgb = Image::<Rgb, u8>::from_vec(32, 24, read("base_444")).unwrap();
    assert_eq!(
        (
            image_ahash(&rgb).unwrap(),
            image_dhash(&rgb).unwrap(),
            image_phash_imagehash(&rgb).unwrap(),
            image_whash(&rgb, 0).unwrap(),
            image_colorhash(&rgb, 3).unwrap(),
        ),
        (
            0x030f_1f3f_7cf0_e286,
            0xffff_fffc_f0e2_8e36,
            0x858a_3f41_1ef5_0f78,
            0x030f_1f3f_7cf0_e282,
            0x0000_0000_0900_0000,
        )
    );

    let rgba = Image::<Rgba, u8>::from_vec(48, 40, read("phash_rgba8_48x40")).unwrap();
    assert_eq!(
        (
            image_ahash(&rgba).unwrap(),
            image_dhash(&rgba).unwrap(),
            image_phash_imagehash(&rgba).unwrap(),
            image_whash(&rgba, 0).unwrap(),
            image_colorhash(&rgba, 3).unwrap(),
        ),
        (
            0x0307_064f_dffe_b8f0,
            0xeece_dc9d_bb70_71e7,
            0x8433_679d_2c5a_76a9,
            0x0307_060f_dfbe_b8f0,
            0x0000_0000_0000_0200,
        )
    );

    let raw = read("phash_gray16_36x28");
    let samples: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let gray16 = Image::<Gray, u16>::from_vec(36, 28, samples).unwrap();
    assert_eq!(
        (
            image_ahash(&gray16).unwrap(),
            image_dhash(&gray16).unwrap(),
            image_phash_imagehash(&gray16).unwrap(),
            image_whash(&gray16, 0).unwrap(),
            image_colorhash(&gray16, 3).unwrap(),
        ),
        (
            0x3666_fff9_00cd_c900,
            0xeecc_dd55_4d99_3b2a,
            0xd386_0541_67f9_c277,
            0x3676_ffad_20c9_d900,
            0x0000_00e0_0000_0000,
        )
    );
}

/// `level` is `log2(image_scale)`: 0 selects the default, 1..=2 and >15 are
/// rejected, an explicit level hashes at that scale.
#[test]
fn whash_level_and_binbits_domain() {
    let img = Image::<Gray, u8>::from_vec(
        64,
        64,
        (0..64 * 64)
            .map(|i| ((i * 7 + i / 64) % 251) as u8)
            .collect(),
    )
    .unwrap();
    assert_eq!(image_whash(&img, 0).unwrap(), image_whash(&img, 6).unwrap());
    assert!(image_whash(&img, 1).is_err());
    assert!(image_whash(&img, 2).is_err());
    assert!(image_whash(&img, 16).is_err());
    assert!(image_whash(&img, 3).is_ok());
    assert_eq!(
        image_colorhash(&img, 0).unwrap(),
        image_colorhash(&img, 3).unwrap()
    );
    assert!(image_colorhash(&img, 5).is_err());
}

#[test]
fn ffi_surface() {
    use sillstone_image::ffi::*;

    let data = [128u8; 16 * 16];
    let mut out: u64 = 0;
    let call = |f: unsafe extern "C" fn(*const u8, usize, u32, u32, u32, *mut u64) -> i32,
                out: &mut u64| unsafe {
        f(data.as_ptr(), data.len(), 16, 16, PITH_LAYOUT_GRAY8, out)
    };

    assert_eq!(call(sillstone_image_ahash, &mut out), PITH_OK);
    assert_eq!(out, 0);
    assert_eq!(call(sillstone_image_dhash, &mut out), PITH_OK);
    assert_eq!(out, 0);
    assert_eq!(call(sillstone_image_phash, &mut out), PITH_OK);

    // Default level / binbits (0) and an explicit pair.
    assert_eq!(
        unsafe {
            sillstone_image_whash(
                data.as_ptr(),
                data.len(),
                16,
                16,
                PITH_LAYOUT_GRAY8,
                0,
                &mut out,
            )
        },
        PITH_OK
    );
    assert_eq!(out, 0);
    assert_eq!(
        unsafe {
            sillstone_image_colorhash(
                data.as_ptr(),
                data.len(),
                16,
                16,
                PITH_LAYOUT_GRAY8,
                3,
                &mut out,
            )
        },
        PITH_OK
    );
    assert_eq!(out, 0x0000_0070_0000_0000);
    assert_eq!(
        unsafe {
            sillstone_image_whash(
                data.as_ptr(),
                data.len(),
                16,
                16,
                PITH_LAYOUT_GRAY8,
                2,
                &mut out,
            )
        },
        PITH_E_REJECTED
    );

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

/// The SIMD luma plane must equal the scalar Pillow formula for every
/// pixel count (vector body plus scalar tail) and both pixel strides.
#[test]
fn luma_plane_matches_scalar_formula() {
    use sillstone_image::raster::{rgb_plane_to_l, rgb_to_l};
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 56) as u8
    };
    for c in [3usize, 4] {
        for n in (0..70usize).chain([255, 1000]) {
            let mut src: Vec<u8> = (0..n * c).map(|_| next()).collect();
            if n > 2 {
                // Saturated corners exercise the rounding term.
                src[..3].copy_from_slice(&[255, 255, 255]);
                src[(n - 1) * c..(n - 1) * c + 3].copy_from_slice(&[0, 0, 0]);
            }
            let want: Vec<u8> = src
                .chunks_exact(c)
                .map(|p| rgb_to_l(p[0], p[1], p[2]))
                .collect();
            assert_eq!(rgb_plane_to_l(&src, c, n), want, "c={c} n={n}");
        }
    }
}
