//! QOI against the reference encoder, plus hand-built streams for every
//! opcode and every malformed-input branch.
//!
//! `fixtures/qoi-vectors.txt` holds pixels and the reference encoder's byte
//! stream for 40 images (RGB and RGBA, flat runs on both sides of the
//! 62-pixel run limit, index hits, diff and luma deltas, raw pixels).

use sillstone_qoi::{QOI_MAGIC, QOI_PADDING, QoiHeader, decode, encode};

const VECTORS: &str = include_str!("fixtures/qoi-vectors.txt");

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

struct Case {
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8,
    pixels: Vec<u8>,
    stream: Vec<u8>,
}

fn cases() -> Vec<Case> {
    VECTORS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let mut c = l.split(' ');
            Case {
                width: c.next().unwrap().parse().unwrap(),
                height: c.next().unwrap().parse().unwrap(),
                channels: c.next().unwrap().parse().unwrap(),
                colorspace: c.next().unwrap().parse().unwrap(),
                pixels: unhex(c.next().unwrap()),
                stream: unhex(c.next().unwrap()),
            }
        })
        .collect()
}

/// A stream with a valid header and footer around `body`.
fn stream(width: u32, height: u32, channels: u8, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&QOI_MAGIC);
    v.extend_from_slice(&width.to_be_bytes());
    v.extend_from_slice(&height.to_be_bytes());
    v.push(channels);
    v.push(0);
    v.extend_from_slice(body);
    v.extend_from_slice(&QOI_PADDING);
    v
}

#[test]
fn encoder_output_round_trips_and_is_no_larger_than_the_reference() {
    // The reference port emits INDEX where the original qoi.h would emit a
    // one-pixel RUN; both are one byte, so streams are compared by meaning
    // (round trip) and by size, not by exact bytes.
    let all = cases();
    assert_eq!(all.len(), 40);
    for (n, c) in all.iter().enumerate() {
        let out = encode(c.width, c.height, c.channels, c.colorspace, &c.pixels).unwrap();
        assert!(
            out.len() <= c.stream.len(),
            "case {n}: larger than reference"
        );
        let (_, back) = decode(&out).unwrap();
        assert_eq!(back, c.pixels, "case {n} ({}x{})", c.width, c.height);
    }
}

#[test]
fn decoder_reads_every_reference_stream() {
    for (n, c) in cases().iter().enumerate() {
        let (header, pixels) = decode(&c.stream).unwrap();
        assert_eq!(
            header,
            QoiHeader {
                width: c.width,
                height: c.height,
                channels: c.channels,
                colorspace: c.colorspace,
            },
            "case {n}"
        );
        assert_eq!(pixels, c.pixels, "case {n}");
    }
}

#[test]
fn every_opcode_decodes_to_the_specified_pixels() {
    let hash_first = ((10 * 3 + 20 * 5 + 30 * 7 + 255 * 11) % 64) as u8;
    let body = [
        0xfe,
        10,
        20,
        30, // RGB  -> (10,20,30,255)
        0xff,
        1,
        2,
        3,
        4,                          // RGBA -> (1,2,3,4)
        hash_first,                 // INDEX -> (10,20,30,255)
        0x40 | (1 << 4) | (3 << 2), // DIFF dr=-1 dg=+1 db=-2
        0x80 | (32 + 5),            // LUMA dg=5
        (10 << 4) | 5,              //   dr-dg=+2, db-dg=-3
        0xc0 | 2,                   // RUN: this pixel and two more
    ];
    let (_, px) = decode(&stream(8, 1, 4, &body)).unwrap();
    let expect: [[u8; 4]; 8] = [
        [10, 20, 30, 255],
        [1, 2, 3, 4],
        [10, 20, 30, 255],
        [9, 21, 28, 255],
        [16, 26, 30, 255],
        [16, 26, 30, 255],
        [16, 26, 30, 255],
        [16, 26, 30, 255],
    ];
    assert_eq!(px.len(), 32);
    for (i, e) in expect.iter().enumerate() {
        assert_eq!(&px[i * 4..i * 4 + 4], e, "pixel {i}");
    }
}

#[test]
fn diff_arithmetic_wraps_modulo_256() {
    // First pixel is implicitly (0,0,0,255); DIFF -2 on each channel wraps.
    let body = [0x40];
    let (_, px) = decode(&stream(1, 1, 3, &body)).unwrap();
    assert_eq!(px, [254, 254, 254]);
}

#[test]
fn rgb_streams_ignore_alpha_and_keep_three_channels() {
    let (h, px) = decode(&stream(2, 1, 3, &[0xfe, 1, 2, 3, 0xc0])).unwrap();
    assert_eq!(h.channels, 3);
    assert_eq!(px, [1, 2, 3, 1, 2, 3]);
}

#[test]
fn rejects_short_input_bad_magic_and_bad_header_fields() {
    assert!(decode(&[]).is_err());
    assert!(decode(&[0u8; 21]).is_err());

    let mut bad_magic = stream(1, 1, 3, &[0xfe, 0, 0, 0]);
    bad_magic[0] = b'x';
    assert!(decode(&bad_magic).is_err());

    assert!(decode(&stream(0, 1, 3, &[])).is_err());
    assert!(decode(&stream(1, 0, 3, &[])).is_err());
    assert!(decode(&stream(1, 1, 2, &[0xfe, 0, 0, 0])).is_err());
    assert!(decode(&stream(1, 1, 5, &[0xfe, 0, 0, 0])).is_err());
}

#[test]
fn rejects_truncated_opcodes_and_incomplete_pixel_streams() {
    // Operand bytes missing before the footer.
    assert!(decode(&stream(1, 1, 3, &[0xfe, 1, 2])).is_err());
    assert!(decode(&stream(1, 1, 4, &[0xff, 1, 2, 3])).is_err());
    assert!(decode(&stream(1, 1, 3, &[0x80 | 32])).is_err());
    // Fewer pixels encoded than the header promises.
    assert!(decode(&stream(2, 2, 3, &[0xfe, 1, 2, 3])).is_err());
    assert!(decode(&stream(3, 1, 3, &[])).is_err());
}

#[test]
fn a_tiny_header_cannot_demand_a_giant_allocation() {
    // 22 bytes claiming a 100000 x 100000 RGB image (30 GB of pixels): the
    // stream cannot possibly contain that many pixels, so it must be
    // refused without reserving the buffer.
    let r = decode(&stream(100_000, 100_000, 3, &[]));
    assert!(r.is_err());
    let r = decode(&stream(u32::MAX, u32::MAX, 4, &[0xfe, 1, 2, 3]));
    assert!(r.is_err());
}

#[test]
fn a_maximal_run_stream_still_decodes() {
    // The densest legal encoding is one byte per 62 pixels.
    let (w, h) = (62 * 40, 1);
    let mut body = vec![0xfe, 5, 6, 7];
    body.push(0xc0 | 60); // first run: pixel 0 is the RGB, then 61 more
    for _ in 0..39 {
        body.push(0xc0 | 61);
    }
    let (_, px) = decode(&stream(w, h, 3, &body)).unwrap();
    assert_eq!(px.len(), (w * h * 3) as usize);
    assert!(px.chunks(3).all(|p| p == [5, 6, 7]));
}

#[test]
fn encoder_validates_its_arguments() {
    let px = [0u8; 12];
    assert!(encode(0, 1, 3, 0, &[]).is_err());
    assert!(encode(1, 0, 3, 0, &[]).is_err());
    assert!(encode(2, 2, 2, 0, &px).is_err());
    assert!(encode(2, 2, 5, 0, &px).is_err());
    assert!(encode(2, 2, 3, 0, &px[..11]).is_err());
    assert!(encode(2, 2, 3, 0, &[0u8; 13]).is_err());
    // Dimensions whose pixel count overflows must error rather than panic.
    assert!(encode(u32::MAX, u32::MAX, 4, 0, &px).is_err());
}

#[test]
fn colorspace_byte_survives_a_round_trip() {
    let pixels = [9u8, 8, 7, 6, 5, 4];
    for cs in [0u8, 1, 7] {
        let enc = encode(2, 1, 3, cs, &pixels).unwrap();
        let (h, px) = decode(&enc).unwrap();
        assert_eq!(h.colorspace, cs);
        assert_eq!(px, pixels);
    }
}

#[test]
fn long_runs_split_at_sixty_two_pixels() {
    for n in [61u32, 62, 63, 124, 125, 186] {
        let pixels = vec![200u8; (n * 3) as usize];
        let enc = encode(n, 1, 3, 0, &pixels).unwrap();
        let (_, back) = decode(&enc).unwrap();
        assert_eq!(back, pixels, "run of {n}");
    }
}
