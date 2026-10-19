//! BT.601 luma, pinning the rounding convention: `round_half_up`, not
//! truncation. `luma_bt601_rounds_half_up_not_down` is the mutation
//! proof's tripwire — it fails if `(sum + 500)/1000` becomes
//! truncation.

use sillstone_image::raster::{
    Cmyk, Gray, Image, La, Rgb, Rgba, cmyk_to_rgb, luma_bt601, rgb_to_cmyk, rgb_to_ycbcr,
    rgb_to_yuv420p_pixel, ycbcr_to_rgb, yuv420p_to_rgb_pixel,
};

#[test]
fn gray_r_equals_g_equals_b_is_identity() {
    // 0.299 + 0.587 + 0.114 == 1 exactly, so equal channels pass through.
    for v in [0u8, 1, 100, 128, 254, 255] {
        assert_eq!(luma_bt601(v, v, v), v);
    }
}

#[test]
fn primaries_and_extremes() {
    assert_eq!(luma_bt601(255u8, 0, 0), 76); // 76.245 -> 76
    assert_eq!(luma_bt601(0u8, 255, 0), 150); // 149.685 -> 150
    assert_eq!(luma_bt601(0u8, 0, 255), 29); // 29.07 -> 29
    assert_eq!(luma_bt601(0u8, 0, 0), 0);
    assert_eq!(luma_bt601(255u8, 255, 255), 255);
}

#[test]
fn luma_bt601_rounds_half_up_not_down() {
    // The .5 boundary case: (0,0,250) -> 28.5 must go to 29; truncation
    // gives 28. Without this case a change of rounding mode stays green.
    assert_eq!(luma_bt601(0u8, 0, 250), 29);
    // A second pin well above .5 where truncation diverges too:
    // (76,0,0) -> 22.724 -> 23, not 22.
    assert_eq!(luma_bt601(76u8, 0, 0), 23);
    // And a value that truncates the same either way, so the test does
    // not degenerate to "everything rounds up": (0,0,255) -> 29.07 -> 29
    // is already covered; (1,0,0) -> 0.299 -> 0.
    assert_eq!(luma_bt601(1u8, 0, 0), 0);
}

#[test]
fn luma_of_u16_scales_with_the_sample_range() {
    // u16 white is 65535 exactly; a mid value keeps proportionality.
    assert_eq!(luma_bt601(65535u16, 65535, 65535), 65535);
    assert_eq!(luma_bt601(1000u16, 1000, 1000), 1000);
    // (0,0,1000) -> 114.0 -> 114; (0,0,999) -> 113.886 -> 114.
    assert_eq!(luma_bt601(0u16, 0, 999), 114);
}

#[test]
fn to_gray_maps_every_rgb_pixel() {
    // 2×1: red pixel and a gray pixel.
    let im: Image<Rgb> = Image::from_vec(2, 1, vec![255, 0, 0, 100, 100, 100]).unwrap();
    let gray: Image<Gray> = im.to_gray();
    assert_eq!((gray.width(), gray.height(), gray.channels()), (2, 1, 1));
    assert_eq!(gray.as_slice(), &[76, 100]);
}

#[test]
fn to_gray_on_rgba_ignores_alpha() {
    // Two identical RGB values with different alpha must give identical
    // luma — alpha is not composited.
    let im: Image<Rgba> = Image::from_vec(2, 1, vec![255, 0, 0, 0, 255, 0, 0, 255]).unwrap();
    let gray = im.to_gray();
    assert_eq!(gray.as_slice(), &[76, 76]);
}

#[test]
fn to_gray_u16_matches_the_scalar_function() {
    let im: Image<Rgb, u16> = Image::from_vec(1, 2, vec![0, 0, 999, 65535, 65535, 65535]).unwrap();
    let gray = im.to_gray();
    assert_eq!(gray.as_slice(), &[114, 65535]);
}

#[test]
fn ycbcr_rgb_roundtrip() {
    for (r, g, b) in [(0, 0, 0), (255, 255, 255), (255, 0, 0), (0, 255, 0), (0, 0, 255), (128, 64, 192)] {
        let (y, cb, cr) = rgb_to_ycbcr(r, g, b);
        let (r2, g2, b2) = ycbcr_to_rgb(y, cb, cr);
        assert!((r as i32 - r2 as i32).abs() <= 2, "r mismatch: {} vs {}", r, r2);
        assert!((g as i32 - g2 as i32).abs() <= 2, "g mismatch: {} vs {}", g, g2);
        assert!((b as i32 - b2 as i32).abs() <= 2, "b mismatch: {} vs {}", b, b2);
    }
}

#[test]
fn cmyk_rgb_roundtrip() {
    for (r, g, b) in [(0, 0, 0), (255, 255, 255), (255, 0, 0), (0, 255, 0), (0, 0, 255), (128, 128, 128)] {
        let (c, m, y, k) = rgb_to_cmyk(r, g, b);
        let (r2, g2, b2) = cmyk_to_rgb(c, m, y, k);
        assert!((r as i32 - r2 as i32).abs() <= 2, "r cmyk: {} vs {}", r, r2);
        assert!((g as i32 - g2 as i32).abs() <= 2, "g cmyk: {} vs {}", g, g2);
        assert!((b as i32 - b2 as i32).abs() <= 2, "b cmyk: {} vs {}", b, b2);
    }
}

#[test]
fn yuv420p_rgb_roundtrip() {
    for (r, g, b) in [(16, 16, 16), (235, 235, 235), (200, 50, 50), (50, 200, 50), (50, 50, 200)] {
        let (y, u, v) = rgb_to_yuv420p_pixel(r, g, b);
        let (r2, g2, b2) = yuv420p_to_rgb_pixel(y, u, v);
        assert!((r as i32 - r2 as i32).abs() <= 3, "r yuv: {} vs {}", r, r2);
        assert!((g as i32 - g2 as i32).abs() <= 3, "g yuv: {} vs {}", g, g2);
        assert!((b as i32 - b2 as i32).abs() <= 3, "b yuv: {} vs {}", b, b2);
    }
}

#[test]
fn image_layout_conversions() {
    let gray: Image<Gray> = Image::from_vec(1, 1, vec![100]).unwrap();
    let rgb = gray.to_rgb();
    assert_eq!(rgb.as_slice(), &[100, 100, 100]);
    let rgba = gray.to_rgba();
    assert_eq!(rgba.as_slice(), &[100, 100, 100, 255]);

    let rgb: Image<Rgb> = Image::from_vec(1, 1, vec![10, 20, 30]).unwrap();
    let rgba = rgb.to_rgba();
    assert_eq!(rgba.as_slice(), &[10, 20, 30, 255]);
    let rgb2 = rgba.to_rgb();
    assert_eq!(rgb2.as_slice(), &[10, 20, 30]);

    let la: Image<La> = Image::from_vec(1, 1, vec![120, 200]).unwrap();
    let la_gray = la.to_gray();
    assert_eq!(la_gray.as_slice(), &[120]);
    let la_rgba = la.to_rgba();
    assert_eq!(la_rgba.as_slice(), &[120, 120, 120, 200]);

    let cmyk: Image<Cmyk> = Image::from_vec(1, 1, vec![0, 255, 255, 0]).unwrap(); // Red in CMYK (C=0, M=255, Y=255, K=0)
    let cmyk_rgb = cmyk.to_rgb();
    assert_eq!(cmyk_rgb.as_slice(), &[255, 0, 0]);
}
