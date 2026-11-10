//! LSB-first bit reader shared by every Brotli parsing stage.

use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// Reads bits least-significant first, as RFC 7932 section 1.5.1 packs them.
pub(super) struct BitReader<'a> {
    data: &'a [u8],
    byte_idx: usize,
    bit_buf: u64,
    bits_in_buf: u32,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_idx: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    /// Tops the buffer up to at least 57 bits, or to the end of the input.
    #[inline]
    pub(super) fn fill(&mut self) {
        while self.bits_in_buf <= 56 && self.byte_idx < self.data.len() {
            self.bit_buf |= (self.data[self.byte_idx] as u64) << self.bits_in_buf;
            self.byte_idx += 1;
            self.bits_in_buf += 8;
        }
    }

    /// The next `n` (at most 32) bits without consuming them; bits past the
    /// end of the input read as zero. Pair with [`BitReader::consume`], which
    /// is where a short input is reported.
    #[inline]
    pub(super) fn peek(&mut self, n: u32) -> u32 {
        if self.bits_in_buf < n {
            self.fill();
        }
        (self.bit_buf & ((1u64 << n) - 1)) as u32
    }

    /// Drops `n` bits that [`BitReader::peek`] returned.
    #[inline]
    pub(super) fn consume(&mut self, n: u32) -> Result<()> {
        if self.bits_in_buf < n {
            return Err(Error::truncated(
                "brotli bits",
                n as usize,
                self.bits_in_buf as usize,
            ));
        }
        self.bit_buf >>= n;
        self.bits_in_buf -= n;
        Ok(())
    }

    /// Reads `n` (at most 32) bits as an unsigned integer.
    #[inline]
    pub(super) fn read_bits(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        let v = self.peek(n);
        self.consume(n)?;
        Ok(v)
    }

    /// Discards the bits up to the next byte boundary; they must be zero.
    pub(super) fn align_to_byte(&mut self) -> Result<()> {
        let remainder = self.bits_in_buf % 8;
        if self.bit_buf & ((1u64 << remainder) - 1) != 0 {
            return Err(Error::BadValue("brotli padding bits nonzero"));
        }
        self.bit_buf >>= remainder;
        self.bits_in_buf -= remainder;
        Ok(())
    }

    /// Takes `count` whole bytes; the reader must be byte aligned.
    pub(super) fn read_exact_bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        let bytes_in_buf = (self.bits_in_buf / 8) as usize;
        let start = self.byte_idx - bytes_in_buf;
        let end = start.saturating_add(count);
        if end > self.data.len() {
            return Err(Error::truncated(
                "brotli bytes",
                count,
                self.data.len().saturating_sub(start),
            ));
        }
        self.byte_idx = end;
        self.bit_buf = 0;
        self.bits_in_buf = 0;
        Ok(&self.data[start..end])
    }

    /// Ends the stream: the bits that pad the final byte must be zero and no
    /// byte may follow the last meta-block.
    pub(super) fn finish(&mut self) -> Result<()> {
        self.align_to_byte()?;
        if self.bits_in_buf != 0 || self.byte_idx != self.data.len() {
            return Err(Error::BadValue("trailing bytes after the brotli stream"));
        }
        Ok(())
    }
}

/// Inverse move-to-front over byte values, in place (RFC 7932 section 7.3).
pub(super) fn inverse_move_to_front(values: &mut [u8]) {
    let mut mtf: Vec<u8> = (0..=255u8).collect();
    for v in values.iter_mut() {
        let idx = *v as usize;
        let val = mtf[idx];
        *v = val;
        mtf.copy_within(0..idx, 1);
        mtf[0] = val;
    }
}
