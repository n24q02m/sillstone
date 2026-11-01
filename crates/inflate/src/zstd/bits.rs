//! Bit readers for Zstandard: a forward LSB-first reader for FSE table
//! descriptions and a backward reader for the entropy-coded streams.

use sillstone_digest::{Error, Result};

/// Reads `n` (at most 32) bits whose lowest bit sits at LSB-first bit index
/// `start` of `data`. Bytes past the end read as zero.
fn bits_at(data: &[u8], start: usize, n: u32) -> u32 {
    debug_assert!(n <= 32);
    if n == 0 {
        return 0;
    }
    let first = start / 8;
    let mut word = 0u64;
    for k in 0..8 {
        if let Some(&b) = data.get(first + k) {
            word |= (b as u64) << (8 * k);
        }
    }
    ((word >> (start % 8)) & ((1u64 << n) - 1)) as u32
}

/// Forward reader: bits are taken from the least significant end of each
/// byte, bytes in order. Used for FSE table descriptions.
pub(super) struct FwdBits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> FwdBits<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        FwdBits { data, pos: 0 }
    }

    /// Reads `n` bits, failing if fewer than `n` remain.
    pub(super) fn read(&mut self, n: u32) -> Result<u32> {
        let total = self.data.len() * 8;
        if self.pos + n as usize > total {
            return Err(Error::truncated(
                "zstd fse table",
                (self.pos + n as usize).div_ceil(8),
                self.data.len(),
            ));
        }
        let v = bits_at(self.data, self.pos, n);
        self.pos += n as usize;
        Ok(v)
    }

    /// Whole bytes consumed so far, rounding a partial byte up.
    pub(super) fn bytes_consumed(&self) -> usize {
        self.pos.div_ceil(8)
    }
}

/// Backward reader. The last byte of the stream carries a sentinel: its
/// highest set bit marks the first bit to be read (just below it); reading
/// proceeds towards the start of the stream, most significant bit first.
pub(super) struct BackBits<'a> {
    data: &'a [u8],
    /// Unread bits. Negative once a read has run past the start.
    remaining: isize,
}

impl<'a> BackBits<'a> {
    pub(super) fn new(data: &'a [u8]) -> Result<Self> {
        let Some(&last) = data.last() else {
            return Err(Error::truncated("zstd bitstream", 1, 0));
        };
        if last == 0 {
            return Err(Error::BadValue("zstd bitstream missing end mark"));
        }
        let high = 7 - last.leading_zeros() as usize;
        Ok(BackBits {
            data,
            remaining: ((data.len() - 1) * 8 + high) as isize,
        })
    }

    /// Looks at the next `n` (at most 32) bits without consuming them. Bits
    /// before the start of the stream read as zero.
    pub(super) fn peek(&self, n: u32) -> u32 {
        if n == 0 || self.remaining <= 0 {
            return 0;
        }
        let rem = self.remaining as usize;
        if rem >= n as usize {
            bits_at(self.data, rem - n as usize, n)
        } else {
            bits_at(self.data, 0, rem as u32) << (n as usize - rem)
        }
    }

    pub(super) fn consume(&mut self, n: u32) {
        self.remaining -= n as isize;
    }

    pub(super) fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.consume(n);
        v
    }

    /// True when a read went past the start of the stream.
    pub(super) fn overrun(&self) -> bool {
        self.remaining < 0
    }

    /// True when every bit has been consumed, no more and no fewer.
    pub(super) fn is_exhausted(&self) -> bool {
        self.remaining == 0
    }
}
