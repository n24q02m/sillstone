//! Framing, refusal and bounds tests for the Brotli and Zstandard decoders.
//!
//! The Zstandard decoder is a complete RFC 8878 decoder minus dictionaries;
//! the tests here pin its framing rules and error paths (the round-trip
//! evidence against the reference encoder lives in `zstd_oracle.rs`). The
//! Brotli decoder still implements only framing, uncompressed meta-blocks and
//! metadata, and must refuse compressed meta-blocks with `Error::Unsupported`
//! rather than decode them to guessed bytes. The fixtures were produced by the
//! reference encoders (python `zstandard` / `brotli`); `random400.bin` is
//! incompressible, so both encoders emit it as raw / uncompressed blocks.

use sillstone_digest::Error;
use sillstone_inflate::{inflate_brotli, inflate_zstd, Limits};

const RANDOM: &[u8] = include_bytes!("fixtures/random400.bin");
const RANDOM_ZST: &[u8] = include_bytes!("fixtures/random400-checksum.zst");
const RANDOM_ZST_NO_CK: &[u8] = include_bytes!("fixtures/random400-nochecksum.zst");
const RANDOM_BR: &[u8] = include_bytes!("fixtures/random400.br");
const TEXT_ZST: &[u8] = include_bytes!("fixtures/text-repeat.zst");
const TEXT_BR: &[u8] = include_bytes!("fixtures/text-repeat.br");
const RLE_ZST: &[u8] = include_bytes!("fixtures/rle-first-block.zst");

fn cap(max_output: usize) -> Limits {
    Limits {
        max_output,
        max_input: 1 << 20,
    }
}

/// A zstd frame with a 1 KiB window and no declared content size, holding
/// one last compressed block `block`.
fn zstd_compressed_block_frame(block: &[u8]) -> Vec<u8> {
    let hdr = ((block.len() as u32) << 3) | (2 << 1) | 1;
    let mut f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00];
    f.extend_from_slice(&hdr.to_le_bytes()[..3]);
    f.extend_from_slice(block);
    f
}

#[test]
fn zstd_raw_block_roundtrips_with_and_without_checksum() {
    assert_eq!(inflate_zstd(RANDOM_ZST, Limits::default()).unwrap(), RANDOM);
    assert_eq!(
        inflate_zstd(RANDOM_ZST_NO_CK, Limits::default()).unwrap(),
        RANDOM
    );
}

#[test]
fn zstd_checksum_mismatch_is_rejected() {
    let mut bad = RANDOM_ZST.to_vec();
    let last = bad.len() - 1;
    bad[last] ^= 0xFF;
    assert!(matches!(
        inflate_zstd(&bad, Limits::default()),
        Err(Error::BadValue("zstd content checksum mismatch"))
    ));
    // A flipped payload byte is caught by the same checksum.
    let mut bad = RANDOM_ZST.to_vec();
    bad[40] ^= 0x01;
    assert!(inflate_zstd(&bad, Limits::default()).is_err());
}

#[test]
fn zstd_every_truncation_is_an_error_never_output() {
    for cut in 0..RANDOM_ZST.len() {
        let r = inflate_zstd(&RANDOM_ZST[..cut], Limits::default());
        assert!(r.is_err(), "prefix of {cut} bytes must not decode");
    }
}

#[test]
fn zstd_compressed_frame_decodes_to_the_original() {
    let text = include_bytes!("fixtures/text-repeat.bin");
    assert_eq!(inflate_zstd(TEXT_ZST, Limits::default()).unwrap(), text);
}

#[test]
fn zstd_skippable_frame_then_data_frame() {
    let mut stream = vec![0x50, 0x2A, 0x4D, 0x18, 3, 0, 0, 0, 9, 9, 9];
    stream.extend_from_slice(RANDOM_ZST_NO_CK);
    assert_eq!(inflate_zstd(&stream, Limits::default()).unwrap(), RANDOM);
    // Skippable payload shorter than declared.
    assert!(matches!(
        inflate_zstd(&[0x50, 0x2A, 0x4D, 0x18, 9, 0, 0, 0, 1], Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Skippable header cut off after the magic.
    assert!(matches!(
        inflate_zstd(&[0x50, 0x2A, 0x4D, 0x18, 1, 0], Limits::default()),
        Err(Error::Truncated { .. })
    ));
}

#[test]
fn zstd_header_errors() {
    assert!(matches!(
        inflate_zstd(&[], Limits::default()),
        Err(Error::Truncated { .. })
    ));
    assert!(matches!(
        inflate_zstd(&[1, 2, 3, 4, 5], Limits::default()),
        Err(Error::InvalidMagic { .. })
    ));
    // Magic only.
    assert!(matches!(
        inflate_zstd(&[0x28, 0xB5, 0x2F, 0xFD], Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Three stray bytes after a complete frame.
    let mut s = RANDOM_ZST_NO_CK.to_vec();
    s.extend_from_slice(&[0x28, 0xB5, 0x2F]);
    assert!(matches!(
        inflate_zstd(&s, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Dictionary id present: refused.
    assert!(matches!(
        inflate_zstd(
            &[0x28, 0xB5, 0x2F, 0xFD, 0x21, 5, 1, 0, 0, 0],
            Limits::default()
        ),
        Err(Error::Unsupported("zstd dictionary"))
    ));
    // Input larger than max_input.
    let tiny = Limits {
        max_output: 1 << 20,
        max_input: 8,
    };
    assert!(matches!(
        inflate_zstd(RANDOM_ZST, tiny),
        Err(Error::TooLarge { .. })
    ));
}

#[test]
fn zstd_output_limit_is_enforced() {
    assert!(matches!(
        inflate_zstd(RANDOM_ZST, cap(100)),
        Err(Error::TooLarge { .. })
    ));
    // The RLE block honours the cap too.
    assert!(matches!(
        inflate_zstd(RLE_ZST, cap(1000)),
        Err(Error::TooLarge { .. })
    ));
}

#[test]
fn zstd_literal_only_compressed_blocks() {
    // Raw literals "hi!", zero sequences.
    let f = zstd_compressed_block_frame(&[0x18, b'h', b'i', b'!', 0x00]);
    assert_eq!(inflate_zstd(&f, Limits::default()).unwrap(), b"hi!");
    // Sequence-count byte omitted: the section is mandatory.
    let f = zstd_compressed_block_frame(&[0x18, b'h', b'i', b'!']);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // RLE literals: 4 x 'a'.
    let f = zstd_compressed_block_frame(&[0x21, b'a', 0x00]);
    assert_eq!(inflate_zstd(&f, Limits::default()).unwrap(), b"aaaa");
    // Raw literals, 12-bit size format: size 40 = low nibble 8, high byte 2.
    let mut blk = vec![0x04 | (8u8 << 4), 2];
    blk.extend(std::iter::repeat_n(7u8, 40));
    blk.push(0);
    let f = zstd_compressed_block_frame(&blk);
    assert_eq!(inflate_zstd(&f, Limits::default()).unwrap(), vec![7u8; 40]);
    // Raw literals, 20-bit size format (size_fmt 3): size 40.
    let mut blk = vec![0x0C | (8u8 << 4), 2, 0];
    blk.extend(std::iter::repeat_n(9u8, 40));
    blk.push(0);
    let f = zstd_compressed_block_frame(&blk);
    assert_eq!(inflate_zstd(&f, Limits::default()).unwrap(), vec![9u8; 40]);
    // RLE literals through the 12-bit and 20-bit size formats.
    let f = zstd_compressed_block_frame(&[0x05 | (8u8 << 4), 2, b'z', 0]);
    assert_eq!(inflate_zstd(&f, Limits::default()).unwrap(), vec![b'z'; 40]);
    let f = zstd_compressed_block_frame(&[0x0D | (8u8 << 4), 2, 0, b'y', 0]);
    assert_eq!(inflate_zstd(&f, Limits::default()).unwrap(), vec![b'y'; 40]);
    // A compressed block cannot be empty: the literals header is mandatory.
    let f = zstd_compressed_block_frame(&[]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Literal-only blocks honour the output cap.
    let f = zstd_compressed_block_frame(&[0x18, b'h', b'i', b'!', 0x00]);
    assert!(matches!(
        inflate_zstd(&f, cap(2)),
        Err(Error::TooLarge { .. })
    ));
}

#[test]
fn zstd_malformed_entropy_sections_are_rejected() {
    // One sequence announced but no symbol compression modes byte.
    let f = zstd_compressed_block_frame(&[0x18, b'h', b'i', b'!', 0x01]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Huffman-compressed literals with an empty tree description.
    let f = zstd_compressed_block_frame(&[0x02, 0x00, 0x00, 0x00]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Treeless literals before any tree was sent.
    let f = zstd_compressed_block_frame(&[0x03, 0x00, 0x00, 0x00, 0x00]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::BadValue("zstd treeless literals without a previous table"))
    ));
    // Repeat mode for all three tables in the first block of a frame.
    let f = zstd_compressed_block_frame(&[0x00, 0x01, 0xFC, 0x01]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::BadValue("zstd repeat table without a previous table"))
    ));
    // Reserved bits of the compression modes byte.
    let f = zstd_compressed_block_frame(&[0x00, 0x01, 0x01, 0x01]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::BadValue("zstd reserved compression mode bits"))
    ));
    // Literals-only block with trailing garbage after the zero count.
    let f = zstd_compressed_block_frame(&[0x00, 0x00, 0x00]);
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::BadValue(_))
    ));
    // Declared content size disagrees with what the blocks produce.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, 4, 0x19, 0, 0, b'h', b'i', b'!'];
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::BadValue("zstd frame content size mismatch"))
    ));
}

#[test]
fn zstd_literal_section_truncation_errors() {
    for blk in [
        &[0x18, b'h'][..],                      // raw literals cut short
        &[0x21][..],                            // RLE literal byte missing
        &[0x04][..],                            // size_fmt 1 needs a second header byte
        &[0x0C][..],                            // size_fmt 3 needs two more bytes
        &[0x25][..],                            // RLE size_fmt 1 needs a second byte
        &[0x2D][..],                            // RLE size_fmt 3 needs two more bytes
        &[0x02][..],                            // Huffman header cut short
        &[0x18, b'h', b'i', b'!', 0x80][..],    // 2-byte sequence count cut short
        &[0x18, b'h', b'i', b'!', 0xFF, 1][..], // 3-byte sequence count cut short
    ] {
        let f = zstd_compressed_block_frame(blk);
        assert!(
            inflate_zstd(&f, Limits::default()).is_err(),
            "block {blk:02x?}"
        );
    }
}

#[test]
fn zstd_block_header_errors() {
    // Reserved block type 3.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x07, 0, 0];
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::BadValue("reserved zstd block type"))
    ));
    // Block header cut off.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x01];
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Raw block declares more than is present.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x41, 0, 0, 1];
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // RLE block missing its byte.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x0B, 0, 0];
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Compressed block declares more than is present.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x55, 0, 0, 1];
    assert!(matches!(
        inflate_zstd(&f, Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // Frame content size larger than the output cap.
    let f = vec![0x28, 0xB5, 0x2F, 0xFD, 0xA0, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(matches!(
        inflate_zstd(&f, cap(1000)),
        Err(Error::TooLarge { .. })
    ));
    // Window descriptor / content size / dictionary fields missing.
    assert!(inflate_zstd(&[0x28, 0xB5, 0x2F, 0xFD, 0x00], Limits::default()).is_err());
    assert!(inflate_zstd(&[0x28, 0xB5, 0x2F, 0xFD, 0x60, 1], Limits::default()).is_err());
}

#[test]
fn brotli_uncompressed_metablock_roundtrips() {
    assert_eq!(inflate_brotli(RANDOM_BR, Limits::default()).unwrap(), RANDOM);
}

#[test]
fn brotli_compressed_metablocks_are_refused_not_guessed() {
    assert!(matches!(
        inflate_brotli(TEXT_BR, Limits::default()),
        Err(Error::Unsupported("brotli compressed meta-block"))
    ));
}

#[test]
fn brotli_every_truncation_is_an_error_never_output() {
    for cut in 0..RANDOM_BR.len() {
        assert!(
            inflate_brotli(&RANDOM_BR[..cut], Limits::default()).is_err(),
            "prefix of {cut} bytes must not decode"
        );
    }
}

#[test]
fn brotli_limits_and_empty_input() {
    assert!(matches!(
        inflate_brotli(&[], Limits::default()),
        Err(Error::Truncated { .. })
    ));
    let tiny = Limits {
        max_output: 1 << 20,
        max_input: 4,
    };
    assert!(matches!(
        inflate_brotli(RANDOM_BR, tiny),
        Err(Error::TooLarge { .. })
    ));
    assert!(matches!(
        inflate_brotli(RANDOM_BR, cap(10)),
        Err(Error::TooLarge { .. })
    ));
}

#[test]
fn brotli_window_bits_forms_and_large_window_refusal() {
    // WBITS=16 (one 0 bit), ISLAST=1, ISLASTEMPTY=1.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (1, 1), (1, 1)] {
        b.push(v, n);
    }
    assert_eq!(inflate_brotli(&b.finish(), Limits::default()).unwrap(), b"");
    // WBITS = 17 + n form.
    let mut b = Bits::default();
    for (v, n) in [(1, 1), (1, 3), (1, 1), (1, 1)] {
        b.push(v, n);
    }
    assert_eq!(inflate_brotli(&b.finish(), Limits::default()).unwrap(), b"");
    // WBITS = 8 + m form.
    let mut b = Bits::default();
    for (v, n) in [(1, 1), (0, 3), (2, 3), (1, 1), (1, 1)] {
        b.push(v, n);
    }
    assert_eq!(inflate_brotli(&b.finish(), Limits::default()).unwrap(), b"");
    // WBITS = 17 (n = 0, m = 0).
    let mut b = Bits::default();
    for (v, n) in [(1, 1), (0, 3), (0, 3), (1, 1), (1, 1)] {
        b.push(v, n);
    }
    assert_eq!(inflate_brotli(&b.finish(), Limits::default()).unwrap(), b"");
    // Large-window marker (m = 1) is refused.
    let mut b = Bits::default();
    for (v, n) in [(1, 1), (0, 3), (1, 3)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::Unsupported("brotli large-window stream"))
    ));
}

#[test]
fn brotli_metadata_metablock_is_skipped() {
    // ISLAST=0, MNIBBLES=3 (metadata), reserved 0, MSKIPBYTES=1,
    // MSKIPLEN-1 = 2 -> skip 3 bytes, then a last-empty meta-block.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (3, 2), (0, 1), (1, 2), (2, 8)] {
        b.push(v, n);
    }
    b.align();
    b.bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
    b.push(1, 1);
    b.push(1, 1);
    assert_eq!(inflate_brotli(&b.finish(), Limits::default()).unwrap(), b"");

    // MSKIPBYTES = 0: nothing to skip.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (3, 2), (0, 1), (0, 2)] {
        b.push(v, n);
    }
    b.align();
    b.push(1, 1);
    b.push(1, 1);
    assert_eq!(inflate_brotli(&b.finish(), Limits::default()).unwrap(), b"");
}

#[test]
fn brotli_malformed_framing_is_rejected() {
    // Metadata with the reserved bit set.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (3, 2), (1, 1)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::BadValue("brotli reserved bit nonzero"))
    ));
    // ISLAST meta-block that is metadata.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (1, 1), (0, 1), (3, 2)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::BadValue("brotli last meta-block cannot be metadata"))
    ));
    // Metadata length whose most-significant byte is zero (not minimal).
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (3, 2), (0, 1), (2, 2), (5, 8), (0, 8)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::BadValue("brotli metadata length not minimal"))
    ));
    // Metadata padding bits nonzero.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (3, 2), (0, 1), (0, 2), (1, 1)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::BadValue("brotli padding bits nonzero"))
    ));
    // Metadata payload longer than the stream.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (3, 2), (0, 1), (1, 2), (200, 8)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::Truncated { .. })
    ));
    // 5-nibble MLEN whose top nibble is zero is not minimal.
    let mut b = Bits::default();
    for (v, n) in [(0, 1), (0, 1), (1, 2), (1, 4), (0, 4), (0, 4), (0, 4), (0, 4)] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::BadValue("brotli meta-block length not minimal"))
    ));
    // Uncompressed meta-block with a nonzero padding bit.
    let mut b = Bits::default();
    for (v, n) in [
        (0, 1),
        (0, 1),
        (0, 2),
        (1, 4),
        (0, 4),
        (0, 4),
        (0, 4),
        (1, 1),
        (1, 1),
    ] {
        b.push(v, n);
    }
    assert!(matches!(
        inflate_brotli(&b.finish(), Limits::default()),
        Err(Error::BadValue("brotli padding bits nonzero"))
    ));
}

/// LSB-first bit packer for hand-built Brotli headers.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    used: usize,
}

impl Bits {
    fn push(&mut self, value: u32, n: u32) {
        for i in 0..n {
            if self.used % 8 == 0 {
                self.bytes.push(0);
            }
            let bit = ((value >> i) & 1) as u8;
            let last = self.bytes.len() - 1;
            self.bytes[last] |= bit << (self.used % 8);
            self.used += 1;
        }
    }

    fn align(&mut self) {
        self.used = self.bytes.len() * 8;
    }

    fn finish(mut self) -> Vec<u8> {
        self.align();
        self.bytes
    }
}
