//! Unit test for baseline JPEG encoding and decode round-trip.

use sillstone_image::raster::{Gray, Image, Rgb};
use sillstone_jpeg::{decode, encode_gray, encode_rgb};

#[test]
fn test_encode_gray_roundtrip() {
    let width = 16;
    let height = 16;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push(((x * 16 + y * 8) % 256) as u8);
        }
    }
    let img = Image::<Gray>::from_vec(width, height, pixels).unwrap();
    let jpeg_bytes = encode_gray(&img).expect("encode gray succeeds");

    assert!(jpeg_bytes.len() > 100);
    assert_eq!(&jpeg_bytes[0..2], &[0xFF, 0xD8]);
    assert_eq!(&jpeg_bytes[jpeg_bytes.len() - 2..], &[0xFF, 0xD9]);

    let decoded = decode(&jpeg_bytes).expect("decode baseline jpeg succeeds");
    assert_eq!(decoded.width(), width);
    assert_eq!(decoded.height(), height);
}

#[test]
fn test_encode_rgb_roundtrip() {
    let width = 16;
    let height = 16;
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push((x * 16) as u8);
            pixels.push((y * 16) as u8);
            pixels.push(128);
        }
    }
    let img = Image::<Rgb>::from_vec(width, height, pixels).unwrap();
    let jpeg_bytes = encode_rgb(&img).expect("encode rgb succeeds");

    assert!(jpeg_bytes.len() > 100);
    assert_eq!(&jpeg_bytes[0..2], &[0xFF, 0xD8]);
    assert_eq!(&jpeg_bytes[jpeg_bytes.len() - 2..], &[0xFF, 0xD9]);

    let decoded = decode(&jpeg_bytes).expect("decode baseline rgb jpeg succeeds");
    assert_eq!(decoded.width(), width);
    assert_eq!(decoded.height(), height);
}
