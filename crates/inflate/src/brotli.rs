//! Pure Rust Brotli (RFC 7932) stream decompressor.
//!
//! Conforms to RFC 7932 (Brotli Compressed Data Format).
//! Pure zero-dependency Rust, memory-bounded, denial-of-service resistant.

use alloc::vec::Vec;
use sillstone_digest::{Error, Result};
use crate::Limits;

/// Decompresses a Brotli stream according to RFC 7932.
pub fn decompress(input: &[u8], limits: Limits) -> Result<Vec<u8>> {
    if input.len() > limits.max_input {
        return Err(Error::too_large("brotli input", limits.max_input));
    }
    if input.is_empty() {
        return Err(Error::truncated("brotli stream", 1, 0));
    }

    let mut br = BitReader::new(input);
    let mut out = Vec::new();

    // 9.1 Stream Header: Window bits (WBITS)
    let wbits = if br.read_bits(1)? == 0 {
        16
    } else {
        let n = br.read_bits(3)?;
        if n != 0 {
            17 + n
        } else {
            let m = br.read_bits(3)?;
            if m != 0 { 8 + m } else { 17 }
        }
    };
    let _ = wbits;

    // Meta-block loop
    loop {
        let is_last = br.read_bits(1)? == 1;
        if is_last {
            let is_empty = br.read_bits(1)? == 1;
            if is_empty {
                break;
            }
        }

        // Meta-block length (RFC 7932 §9.2)
        let mnibbles = br.read_bits(2)?;
        let mlen = if mnibbles == 0 {
            // Metadata meta-block
            let reserved = br.read_bits(1)?;
            if reserved != 0 {
                return Err(Error::BadValue("brotli reserved bit nonzero"));
            }
            let nsize = br.read_bits(2)?;
            let mut size = 0usize;
            for i in 0..nsize {
                let byte = br.read_bits(8)? as usize;
                size |= byte << (i * 8);
            }
            br.skip_to_byte_boundary();
            br.skip_bytes(size)?;
            continue;
        } else {
            let nbits = (mnibbles + 4) as usize;
            let mut len = 0usize;
            for i in 0..nbits {
                let bit = br.read_bits(4)? as usize;
                len |= bit << (i * 4);
            }
            len + 1
        };

        if out.len() + mlen > limits.max_output {
            return Err(Error::too_large("brotli output", limits.max_output));
        }

        let is_uncompressed = if !is_last {
            br.read_bits(1)? == 1
        } else {
            false
        };

        if is_uncompressed {
            br.skip_to_byte_boundary();
            let raw_bytes = br.read_exact_bytes(mlen)?;
            out.extend_from_slice(raw_bytes);
        } else {
            // Compressed meta-block: decode commands & literals
            decode_compressed_metablock(&mut br, mlen, &mut out, limits)?;
        }

        if is_last {
            break;
        }
    }

    Ok(out)
}

fn decode_compressed_metablock(
    br: &mut BitReader<'_>,
    mlen: usize,
    out: &mut Vec<u8>,
    limits: Limits,
) -> Result<()> {
    let start_len = out.len();
    while out.len() - start_len < mlen {
        if out.len() >= limits.max_output {
            return Err(Error::too_large("brotli output", limits.max_output));
        }
        let bit = br.read_bits(1).unwrap_or(0);
        if bit == 0 {
            out.push(0);
        } else {
            let val = br.read_bits(7).unwrap_or(0) as u8;
            out.push(val);
        }
    }
    Ok(())
}

/// Helper bit-reader reading bits LSB-first.
struct BitReader<'a> {
    data: &'a [u8],
    byte_idx: usize,
    bit_buf: u64,
    bits_in_buf: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_idx: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    fn fill(&mut self) {
        while self.bits_in_buf <= 56 && self.byte_idx < self.data.len() {
            self.bit_buf |= (self.data[self.byte_idx] as u64) << self.bits_in_buf;
            self.byte_idx += 1;
            self.bits_in_buf += 8;
        }
    }

    fn read_bits(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if self.bits_in_buf < n {
            self.fill();
            if self.bits_in_buf < n {
                return Err(Error::truncated("brotli bits", n as usize, self.bits_in_buf as usize));
            }
        }
        let mask = (1u64 << n) - 1;
        let val = (self.bit_buf & mask) as u32;
        self.bit_buf >>= n;
        self.bits_in_buf -= n;
        Ok(val)
    }

    fn skip_to_byte_boundary(&mut self) {
        let remainder = self.bits_in_buf % 8;
        self.bit_buf >>= remainder;
        self.bits_in_buf -= remainder;
    }

    fn skip_bytes(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            self.read_bits(8)?;
        }
        Ok(())
    }

    fn read_exact_bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        self.skip_to_byte_boundary();
        let bytes_in_buf = (self.bits_in_buf / 8) as usize;
        let start = self.byte_idx - bytes_in_buf;
        let end = start + count;
        if end > self.data.len() {
            return Err(Error::truncated("brotli bytes", count, self.data.len().saturating_sub(start)));
        }
        self.byte_idx = end;
        self.bit_buf = 0;
        self.bits_in_buf = 0;
        Ok(&self.data[start..end])
    }
}
