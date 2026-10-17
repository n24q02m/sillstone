//! Edge-branch tests for the bit reader that complement the
//! conformance suite: the oversized-width refusal, the unaligned
//! multi-byte general path, and the position accessor.

use sillstone_digest::BitReader;

/// A width over 64 is a named `BadValue`, refused before anything is
/// read, and the reader does not move.
#[test]
fn bits_over_64_is_bad_value() {
    let mut r = BitReader::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(
        r.bits(65),
        Err(sillstone_digest::Error::BadValue("bit width over 64"))
    );
    assert_eq!(r.bit_position(), 0);
}

/// `bit_position` reports the consumed bit count across reads and
/// alignment.
#[test]
fn bit_position_tracks_consumed_bits() {
    let mut r = BitReader::new(&[0xA6, 0x50]);
    assert_eq!(r.bit_position(), 0);
    r.bits(3).unwrap();
    assert_eq!(r.bit_position(), 3);
    r.bits(5).unwrap();
    assert_eq!(r.bit_position(), 8);
    r.bits(0).unwrap();
    assert_eq!(r.bit_position(), 8);
}

/// A field that starts mid-byte and spans into later bytes takes the
/// general assembly path with a non-zero head: the first stream bits
/// come from the low tail of the current byte, then whole bytes, then
/// the final partial byte.
#[test]
fn unaligned_spanning_field_assembles_from_head_and_bytes() {
    // Stream 1010_0110 0101_0000 0000_0000: after three bits (101) the
    // next twelve are 0_0110 0101_000 (positions 3..15).
    let mut r = BitReader::new(&[0xA6, 0x50, 0x00]);
    r.bits(3).unwrap();
    assert_eq!(r.bits(12).unwrap(), 0b0011_0010_1000);
    assert_eq!(r.bit_position(), 15);
}
