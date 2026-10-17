//! Tests for Brotli and Zstandard decompression against golden test vectors.

use sillstone_inflate::{inflate_brotli, inflate_zstd, Limits};

#[test]
fn test_zstd_golden_frames() {
    // empty-block.zst
    let empty_frame = include_bytes!("fixtures/empty-block.zst");
    let decomp_empty = inflate_zstd(empty_frame, Limits::default()).expect("empty block decompresses");
    assert_eq!(decomp_empty, b"");

    // zeroSeq_2B.zst ("Hello World!\n")
    let zeroseq_frame = include_bytes!("fixtures/zeroSeq_2B.zst");
    let decomp_zeroseq = inflate_zstd(zeroseq_frame, Limits::default()).expect("zeroSeq_2B decompresses");
    assert_eq!(decomp_zeroseq, b"Hello World!\n");

    // rle-first-block.zst (1MB zeros)
    let rle_frame = include_bytes!("fixtures/rle-first-block.zst");
    let decomp_rle = inflate_zstd(rle_frame, Limits { max_output: 2 * 1024 * 1024, max_input: 1024 * 1024 })
        .expect("rle frame decompresses");
    assert_eq!(decomp_rle.len(), 1048576);
    assert!(decomp_rle.iter().all(|&b| b == 0));
}

#[test]
fn test_brotli_golden_frames() {
    // empty.compressed.00
    let empty_brotli = include_bytes!("fixtures/empty.compressed.00");
    let decomp = inflate_brotli(empty_brotli, Limits::default()).expect("empty brotli decompresses");
    assert_eq!(decomp, b"");
}
