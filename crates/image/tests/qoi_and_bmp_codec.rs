//! Tests for BMP and QOI image encoding and decoding roundtrips.

use sillstone_image::{decode_bmp, encode_bmp_rgb, encode_bmp_rgba};
use sillstone_image::qoi_codec::{qoi_decode_rgb, qoi_decode_rgba, qoi_encode_rgb, qoi_encode_rgba};
use sillstone_image::raster::{Image, Rgb, Rgba};

#[test]
fn test_bmp_rgb_roundtrip() {
    let width = 3;
    let height = 2;
    let pixels = vec![
        10, 20, 30, 40, 50, 60, 70, 80, 90,
        100, 110, 120, 130, 140, 150, 160, 170, 180,
    ];
    let img: Image<Rgb, u8> = Image::from_vec(width, height, pixels).unwrap();
    let encoded = encode_bmp_rgb(&img).unwrap();
    let decoded = decode_bmp(&encoded).unwrap();

    assert_eq!((decoded.width(), decoded.height()), (width, height));
    // decode_bmp returns Rgba with alpha = 255
    let expected_rgba: Vec<u8> = img
        .as_slice()
        .chunks_exact(3)
        .flat_map(|c| [c[0], c[1], c[2], 255])
        .collect();
    assert_eq!(decoded.as_slice(), expected_rgba.as_slice());
}

#[test]
fn test_bmp_rgba_roundtrip() {
    let width = 2;
    let height = 2;
    let pixels = vec![
        1, 2, 3, 255, 4, 5, 6, 255,
        7, 8, 9, 255, 10, 11, 12, 255,
    ];
    let img: Image<Rgba, u8> = Image::from_vec(width, height, pixels).unwrap();
    let encoded = encode_bmp_rgba(&img).unwrap();
    let decoded = decode_bmp(&encoded).unwrap();

    assert_eq!((decoded.width(), decoded.height()), (width, height));
    assert_eq!(decoded.as_slice(), img.as_slice());
}

#[test]
fn test_qoi_rgb_and_rgba_roundtrip() {
    let width = 4;
    let height = 3;
    let rgb_pixels = vec![55u8; (width * height * 3) as usize];
    let img_rgb: Image<Rgb, u8> = Image::from_vec(width, height, rgb_pixels).unwrap();
    let qoi_bytes = qoi_encode_rgb(&img_rgb).unwrap();
    let decoded_rgb = qoi_decode_rgb(&qoi_bytes).unwrap();
    assert_eq!(decoded_rgb.as_slice(), img_rgb.as_slice());

    let rgba_pixels = vec![77u8; (width * height * 4) as usize];
    let img_rgba: Image<Rgba, u8> = Image::from_vec(width, height, rgba_pixels).unwrap();
    let qoi_rgba_bytes = qoi_encode_rgba(&img_rgba).unwrap();
    let decoded_rgba = qoi_decode_rgba(&qoi_rgba_bytes).unwrap();
    assert_eq!(decoded_rgba.as_slice(), img_rgba.as_slice());
}
