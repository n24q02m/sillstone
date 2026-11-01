//! Pure Rust Brotli (RFC 7932) stream decoder, meta-block framing only.
//!
//! Implemented: the stream header (WBITS), meta-block framing, empty and
//! metadata meta-blocks, and uncompressed meta-blocks, all bounds-checked and
//! capped by [`Limits`].
//!
//! Not implemented: compressed meta-blocks (prefix codes, context modelling,
//! the static dictionary). A stream that contains one is refused with
//! [`Error::Unsupported`]; the decoder never guesses and never returns bytes
//! it did not decode. Tracked as an open item until the full decoder lands.

use alloc::vec::Vec;
use sillstone_digest::{Error, Result};
use crate::Limits;

/// Decompresses a Brotli stream according to RFC 7932, refusing compressed
/// meta-blocks with [`Error::Unsupported`].
pub fn decompress(input: &[u8], limits: Limits) -> Result<Vec<u8>> {
    if input.len() > limits.max_input {
        return Err(Error::too_large("brotli input", limits.max_input));
    }
    if input.is_empty() {
        return Err(Error::truncated("brotli stream", 1, 0));
    }

    let mut br = BitReader::new(input);
    let mut out = Vec::new();

    // RFC 7932 section 9.1: WBITS. The window size only bounds back-references,
    // which exist only in compressed meta-blocks, so it is validated, not used.
    if br.read_bits(1)? == 1 {
        let n = br.read_bits(3)?;
        if n == 0 {
            let m = br.read_bits(3)?;
            if m == 1 {
                return Err(Error::Unsupported("brotli large-window stream"));
            }
        }
    }

    loop {
        let is_last = br.read_bits(1)? == 1;
        if is_last && br.read_bits(1)? == 1 {
            // ISLASTEMPTY: the stream ends here.
            break;
        }

        // RFC 7932 section 9.2: MNIBBLES is 4, 5, 6 or (value 3) 0 = metadata.
        let mnibbles = match br.read_bits(2)? {
            0 => 4usize,
            1 => 5,
            2 => 6,
            _ => 0,
        };

        if mnibbles == 0 {
            if is_last {
                return Err(Error::BadValue("brotli last meta-block cannot be metadata"));
            }
            if br.read_bits(1)? != 0 {
                return Err(Error::BadValue("brotli reserved bit nonzero"));
            }
            let skip_bytes = br.read_bits(2)? as usize;
            let mut skip_len = 0usize;
            for i in 0..skip_bytes {
                let byte = br.read_bits(8)? as usize;
                if i + 1 == skip_bytes && skip_bytes > 1 && byte == 0 {
                    return Err(Error::BadValue("brotli metadata length not minimal"));
                }
                skip_len |= byte << (i * 8);
            }
            if skip_bytes > 0 {
                skip_len += 1;
            }
            br.align_to_byte()?;
            br.read_exact_bytes(skip_len)?;
            continue;
        }

        let mut mlen = 0usize;
        for i in 0..mnibbles {
            let nibble = br.read_bits(4)? as usize;
            if i + 1 == mnibbles && mnibbles > 4 && nibble == 0 {
                return Err(Error::BadValue("brotli meta-block length not minimal"));
            }
            mlen |= nibble << (i * 4);
        }
        mlen += 1;

        let is_uncompressed = !is_last && br.read_bits(1)? == 1;
        if !is_uncompressed {
            return Err(Error::Unsupported("brotli compressed meta-block"));
        }

        if out.len() + mlen > limits.max_output {
            return Err(Error::too_large("brotli output", limits.max_output));
        }
        br.align_to_byte()?;
        out.extend_from_slice(br.read_exact_bytes(mlen)?);
    }

    Ok(out)
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

    /// Discards the bits up to the next byte boundary; they must be zero.
    fn align_to_byte(&mut self) -> Result<()> {
        let remainder = self.bits_in_buf % 8;
        if self.bit_buf & ((1u64 << remainder) - 1) != 0 {
            return Err(Error::BadValue("brotli padding bits nonzero"));
        }
        self.bit_buf >>= remainder;
        self.bits_in_buf -= remainder;
        Ok(())
    }

    fn read_exact_bytes(&mut self, count: usize) -> Result<&'a [u8]> {
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
