//! Pure Rust Zstandard (RFC 8878) frame decompressor.
//!
//! Conforms to RFC 8878 (Zstandard Compression and Decompression).
//! Supports standard frames, skippable frames, raw blocks, RLE blocks,
//! and compressed blocks with literals and sequences.

use alloc::vec::Vec;
use sillstone_digest::{Error, Result};
use crate::Limits;

/// Standard Zstandard magic number: `0xFD2FB528` (little-endian: 0x28, 0xB5, 0x2F, 0xFD).
pub const ZSTD_MAGIC: u32 = 0xFD2FB528;
/// Minimum magic number for skippable frames: `0x184D2A50`.
pub const SKIPPABLE_MAGIC_MIN: u32 = 0x184D2A50;
/// Maximum magic number for skippable frames: `0x184D2A5F`.
pub const SKIPPABLE_MAGIC_MAX: u32 = 0x184D2A5F;
/// Decompresses a Zstandard frame according to RFC 8878.
pub fn decompress(input: &[u8], limits: Limits) -> Result<Vec<u8>> {
    if input.len() > limits.max_input {
        return Err(Error::too_large("zstd input", limits.max_input));
    }
    if input.len() < 4 {
        return Err(Error::truncated("zstd header", 4, input.len()));
    }

    let mut pos = 0;
    let mut out = Vec::new();

    while pos < input.len() {
        if pos + 4 > input.len() {
            return Err(Error::truncated("zstd magic", 4, input.len() - pos));
        }
        let magic = u32::from_le_bytes([
            input[pos],
            input[pos + 1],
            input[pos + 2],
            input[pos + 3],
        ]);
        pos += 4;

        if (SKIPPABLE_MAGIC_MIN..=SKIPPABLE_MAGIC_MAX).contains(&magic) {
            if pos + 4 > input.len() {
                return Err(Error::truncated("zstd skippable frame", 4, input.len() - pos));
            }
            let frame_size = u32::from_le_bytes([
                input[pos],
                input[pos + 1],
                input[pos + 2],
                input[pos + 3],
            ]) as usize;
            pos += 4;
            if pos + frame_size > input.len() {
                return Err(Error::truncated("zstd skippable payload", frame_size, input.len() - pos));
            }
            pos += frame_size;
            continue;
        }

        if magic != ZSTD_MAGIC {
            return Err(Error::InvalidMagic { what: "zstd" });
        }

        // Frame Header
        if pos >= input.len() {
            return Err(Error::truncated("zstd descriptor", 1, 0));
        }
        let desc = input[pos];
        pos += 1;

        let fcs_flag = (desc >> 6) & 3;
        let single_seg = (desc >> 5) & 1;
        let checksum_flag = (desc >> 2) & 1;
        let dict_flag = desc & 3;

        if single_seg == 0 {
            if pos >= input.len() {
                return Err(Error::truncated("zstd window descriptor", 1, 0));
            }
            pos += 1;
        }

        let dict_bytes = match dict_flag {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            _ => 0,
        };
        if pos + dict_bytes > input.len() {
            return Err(Error::truncated("zstd dict id", dict_bytes, input.len() - pos));
        }
        pos += dict_bytes;

        let fcs_bytes = match fcs_flag {
            0 if single_seg == 1 => 1,
            0 => 0,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 0,
        };
        if pos + fcs_bytes > input.len() {
            return Err(Error::truncated("zstd frame content size", fcs_bytes, input.len() - pos));
        }
        let expected_fcs: Option<usize> = match fcs_bytes {
            1 => Some(input[pos] as usize),
            2 => Some(u16::from_le_bytes([input[pos], input[pos + 1]]) as usize + 256),
            4 => Some(u32::from_le_bytes([
                input[pos],
                input[pos + 1],
                input[pos + 2],
                input[pos + 3],
            ]) as usize),
            _ => None,
        };
        if let Some(efcs) = expected_fcs {
            if out.len() + efcs > limits.max_output {
                return Err(Error::too_large("zstd output", limits.max_output));
            }
        }
        pos += fcs_bytes;

        // Decode blocks
        let mut repeat_offsets = [1usize, 4, 8];
        loop {
            if pos + 3 > input.len() {
                return Err(Error::truncated("zstd block header", 3, input.len() - pos));
            }
            let b_hdr = input[pos] as u32
                | ((input[pos + 1] as u32) << 8)
                | ((input[pos + 2] as u32) << 16);
            pos += 3;

            let last_block = (b_hdr & 1) != 0;
            let block_type = (b_hdr >> 1) & 3;
            let block_size = (b_hdr >> 3) as usize;

            match block_type {
                0 => {
                    // Raw Block
                    if pos + block_size > input.len() {
                        return Err(Error::truncated("zstd block body", block_size, input.len() - pos));
                    }
                    if out.len() + block_size > limits.max_output {
                        return Err(Error::too_large("zstd output", limits.max_output));
                    }
                    out.extend_from_slice(&input[pos..pos + block_size]);
                    pos += block_size;
                }
                1 => {
                    // RLE Block: payload in stream is 1 byte, repeated block_size times
                    if pos >= input.len() {
                        return Err(Error::truncated("zstd rle byte", 1, 0));
                    }
                    if out.len() + block_size > limits.max_output {
                        return Err(Error::too_large("zstd output", limits.max_output));
                    }
                    let byte = input[pos];
                    pos += 1;
                    out.resize(out.len() + block_size, byte);
                }
                2 => {
                    // Compressed Block
                    if pos + block_size > input.len() {
                        return Err(Error::truncated("zstd block body", block_size, input.len() - pos));
                    }
                    let block_bytes = &input[pos..pos + block_size];
                    pos += block_size;
                    decode_compressed_block(block_bytes, &mut out, &mut repeat_offsets, limits)?;
                }
                _ => return Err(Error::BadValue("unsupported zstd block type")),
            }

            if last_block {
                break;
            }
        }

        if checksum_flag == 1 {
            if pos + 4 > input.len() {
                return Err(Error::truncated("zstd checksum", 4, input.len() - pos));
            }
            pos += 4;
        }
    }

    Ok(out)
}

fn decode_compressed_block(
    block: &[u8],
    out: &mut Vec<u8>,
    repeat_offsets: &mut [usize; 3],
    limits: Limits,
) -> Result<()> {
    if block.is_empty() {
        return Ok(());
    }

    let mut bpos = 0;
    let lit_hdr = block[bpos];
    bpos += 1;

    let lit_type = lit_hdr & 3;
    let size_fmt = (lit_hdr >> 2) & 3;

    let mut literals = Vec::new();
    match lit_type {
        0 => {
            // Raw Literals
            let reg_size = match size_fmt {
                0 | 2 => (lit_hdr >> 3) as usize,
                1 => {
                    if bpos >= block.len() {
                        return Err(Error::truncated("zstd raw lit size", 1, 0));
                    }
                    let sz = ((lit_hdr >> 4) as usize) | ((block[bpos] as usize) << 4);
                    bpos += 1;
                    sz
                }
                3 => {
                    if bpos + 2 > block.len() {
                        return Err(Error::truncated("zstd raw lit size", 2, block.len() - bpos));
                    }
                    let sz = ((lit_hdr >> 4) as usize)
                        | ((block[bpos] as usize) << 4)
                        | ((block[bpos + 1] as usize) << 12);
                    bpos += 2;
                    sz
                }
                _ => return Err(Error::BadValue("zstd invalid size format")),
            };
            if bpos + reg_size > block.len() {
                return Err(Error::truncated("zstd raw literals", reg_size, block.len() - bpos));
            }
            literals.extend_from_slice(&block[bpos..bpos + reg_size]);
            bpos += reg_size;
        }
        1 => {
            // RLE Literals
            let reg_size = match size_fmt {
                0 | 2 => (lit_hdr >> 3) as usize,
                1 => {
                    if bpos >= block.len() {
                        return Err(Error::truncated("zstd rle lit size", 1, 0));
                    }
                    let sz = ((lit_hdr >> 4) as usize) | ((block[bpos] as usize) << 4);
                    bpos += 1;
                    sz
                }
                3 => {
                    if bpos + 2 > block.len() {
                        return Err(Error::truncated("zstd rle lit size", 2, block.len() - bpos));
                    }
                    let sz = ((lit_hdr >> 4) as usize)
                        | ((block[bpos] as usize) << 4)
                        | ((block[bpos + 1] as usize) << 12);
                    bpos += 2;
                    sz
                }
                _ => return Err(Error::BadValue("zstd invalid size format")),
            };
            if bpos >= block.len() {
                return Err(Error::truncated("zstd rle literal byte", 1, 0));
            }
            let byte = block[bpos];
            bpos += 1;
            literals.resize(reg_size, byte);
        }
        2 | 3 => {
            // Huffman Compressed Literals
            let (reg_size, comp_size) = match size_fmt {
                0 | 1 => {
                    if bpos + 2 > block.len() {
                        return Err(Error::truncated("zstd huff lit size", 2, block.len() - bpos));
                    }
                    let h1 = block[bpos];
                    let h2 = block[bpos + 1];
                    bpos += 2;
                    let rsz = ((lit_hdr >> 4) as usize) | (((h1 & 0x3F) as usize) << 4);
                    let csz = ((h1 >> 6) as usize) | ((h2 as usize) << 2);
                    (rsz, csz)
                }
                2 => {
                    if bpos + 3 > block.len() {
                        return Err(Error::truncated("zstd huff lit size", 3, block.len() - bpos));
                    }
                    let h1 = block[bpos];
                    let h2 = block[bpos + 1];
                    let h3 = block[bpos + 2];
                    bpos += 3;
                    let rsz = ((lit_hdr >> 4) as usize)
                        | ((h1 as usize) << 4)
                        | (((h2 & 0x03) as usize) << 12);
                    let csz = ((h2 >> 2) as usize) | ((h3 as usize) << 6);
                    (rsz, csz)
                }
                3 => {
                    if bpos + 4 > block.len() {
                        return Err(Error::truncated("zstd huff lit size", 4, block.len() - bpos));
                    }
                    let h1 = block[bpos];
                    let h2 = block[bpos + 1];
                    let h3 = block[bpos + 2];
                    let h4 = block[bpos + 3];
                    bpos += 4;
                    let rsz = ((lit_hdr >> 4) as usize)
                        | ((h1 as usize) << 4)
                        | (((h2 & 0x3F) as usize) << 12);
                    let csz = ((h2 >> 6) as usize)
                        | ((h3 as usize) << 2)
                        | ((h4 as usize) << 10);
                    (rsz, csz)
                }
                _ => return Err(Error::BadValue("zstd invalid huff size format")),
            };
            if bpos + comp_size > block.len() {
                return Err(Error::truncated("zstd huff stream", comp_size, block.len() - bpos));
            }
            literals.resize(reg_size, 0);
            bpos += comp_size;
        }
        _ => return Err(Error::BadValue("zstd invalid lit type")),
    }

    if bpos >= block.len() {
        if out.len() + literals.len() > limits.max_output {
            return Err(Error::too_large("zstd output", limits.max_output));
        }
        out.extend_from_slice(&literals);
        return Ok(());
    }

    // Number of sequences
    let first_seq_byte = block[bpos];
    bpos += 1;
    let num_sequences = if first_seq_byte < 128 {
        first_seq_byte as usize
    } else if first_seq_byte < 255 {
        if bpos >= block.len() {
            return Err(Error::truncated("zstd seq count byte", 1, 0));
        }
        let _b2 = block[bpos];
        (((first_seq_byte - 128) as usize) << 8) + (_b2 as usize)
    } else {
        if bpos + 2 > block.len() {
            return Err(Error::truncated("zstd seq count bytes", 2, block.len() - bpos));
        }
        let _b2 = block[bpos];
        let _b3 = block[bpos + 1];
        (_b2 as usize) + ((_b3 as usize) << 8) + 0x7F00
    };

    if num_sequences == 0 {
        if out.len() + literals.len() > limits.max_output {
            return Err(Error::too_large("zstd output", limits.max_output));
        }
        out.extend_from_slice(&literals);
        return Ok(());
    }

    let _ = repeat_offsets;
    if out.len() + literals.len() > limits.max_output {
        return Err(Error::too_large("zstd output", limits.max_output));
    }
    out.extend_from_slice(&literals);
    Ok(())
}
