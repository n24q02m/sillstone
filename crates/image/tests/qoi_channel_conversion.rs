//! `qoi_decode_rgb` / `qoi_decode_rgba` when the stream's channel count differs from the
//! requested layout.
//!
//! The streams are assembled by hand from the QOI specification (header, then
//! `QOI_OP_RGB` / `QOI_OP_RGBA` / `QOI_OP_RUN` / `QOI_OP_DIFF` / `QOI_OP_LUMA` chunks, then
//! the 8-byte end marker), so they do not depend on this crate's encoder. The expected
//! pixels were also decoded with Pillow's QOI plugin (Pillow 12.3.0) and match.

use sillstone_image::qoi_codec::{qoi_decode_rgb, qoi_decode_rgba};

const END: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

fn header(width: u32, height: u32, channels: u8) -> Vec<u8> {
    let mut v = b"qoif".to_vec();
    v.extend_from_slice(&width.to_be_bytes());
    v.extend_from_slice(&height.to_be_bytes());
    v.push(channels);
    v.push(0); // sRGB with linear alpha
    v
}

/// 3x2, three channels. Pixels: (10,20,30), (40,50,60), a run of two repeats of (40,50,60),
/// a DIFF of (+1,-1,0) giving (41,49,60), then a LUMA with dg=+5, dr-dg=+2, db-dg=-2 giving
/// (48,54,63).
fn rgb_stream() -> Vec<u8> {
    let mut v = header(3, 2, 3);
    v.extend_from_slice(&[0xFE, 10, 20, 30]);
    v.extend_from_slice(&[0xFE, 40, 50, 60]);
    v.push(0xC1); // QOI_OP_RUN, length 2
    v.push(0x76); // QOI_OP_DIFF
    v.extend_from_slice(&[0xA5, 0xA6]); // QOI_OP_LUMA
    v.extend_from_slice(&END);
    v
}

const RGB_PIXELS: [[u8; 3]; 6] = [
    [10, 20, 30],
    [40, 50, 60],
    [40, 50, 60],
    [40, 50, 60],
    [41, 49, 60],
    [48, 54, 63],
];

/// 2x2, four channels, with alpha values 200, 100, 0 and a run repeating the last pixel.
fn rgba_stream() -> Vec<u8> {
    let mut v = header(2, 2, 4);
    v.extend_from_slice(&[0xFF, 1, 2, 3, 200]);
    v.extend_from_slice(&[0xFF, 4, 5, 6, 100]);
    v.extend_from_slice(&[0xFF, 7, 8, 9, 0]);
    v.push(0xC0); // QOI_OP_RUN, length 1
    v.extend_from_slice(&END);
    v
}

const RGBA_PIXELS: [[u8; 4]; 4] = [[1, 2, 3, 200], [4, 5, 6, 100], [7, 8, 9, 0], [7, 8, 9, 0]];

#[test]
fn rgb_stream_decodes_as_rgb() {
    let img = qoi_decode_rgb(&rgb_stream()).unwrap();
    assert_eq!((img.width(), img.height()), (3, 2));
    let expected: Vec<u8> = RGB_PIXELS.iter().flatten().copied().collect();
    assert_eq!(img.as_slice(), expected.as_slice());
}

#[test]
fn rgb_stream_decoded_as_rgba_gets_opaque_alpha() {
    let img = qoi_decode_rgba(&rgb_stream()).unwrap();
    assert_eq!((img.width(), img.height()), (3, 2));
    let expected: Vec<u8> = RGB_PIXELS
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    assert_eq!(img.as_slice(), expected.as_slice());
}

#[test]
fn rgba_stream_decodes_as_rgba() {
    let img = qoi_decode_rgba(&rgba_stream()).unwrap();
    assert_eq!((img.width(), img.height()), (2, 2));
    let expected: Vec<u8> = RGBA_PIXELS.iter().flatten().copied().collect();
    assert_eq!(img.as_slice(), expected.as_slice());
}

#[test]
fn rgba_stream_decoded_as_rgb_drops_alpha() {
    let img = qoi_decode_rgb(&rgba_stream()).unwrap();
    assert_eq!((img.width(), img.height()), (2, 2));
    let expected: Vec<u8> = RGBA_PIXELS
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    assert_eq!(img.as_slice(), expected.as_slice());
}

#[test]
fn malformed_streams_are_rejected_by_both_decoders() {
    let good = rgb_stream();
    // Too short for header + end marker.
    assert!(qoi_decode_rgb(&good[..10]).is_err());
    assert!(qoi_decode_rgba(&good[..10]).is_err());
    // Wrong magic.
    let mut bad_magic = good.clone();
    bad_magic[0] = b'x';
    assert!(qoi_decode_rgb(&bad_magic).is_err());
    assert!(qoi_decode_rgba(&bad_magic).is_err());
}
