//! Huffman-compressed literals (RFC 8878 section 4.2).

use super::bits::BackBits;
use super::fse;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// Longest code the format allows.
const MAX_BITS: u32 = 11;

#[derive(Clone, Debug)]
pub(super) struct HufTable {
    max_bits: u32,
    /// Indexed by the next `max_bits` bits of the stream: (symbol, code length).
    entries: Vec<(u8, u8)>,
}

impl HufTable {
    /// Builds the table from the explicitly stored weights; the weight of the
    /// last symbol is implied by the requirement that the weights fill a
    /// power of two.
    fn from_weights(stored: &[u8]) -> Result<HufTable> {
        if stored.len() > 255 {
            return Err(Error::BadValue("zstd huffman too many weights"));
        }
        let mut sum = 0u32;
        for &w in stored {
            if w as u32 > MAX_BITS {
                return Err(Error::BadValue("zstd huffman weight too large"));
            }
            if w > 0 {
                sum += 1 << (w - 1);
            }
        }
        if sum == 0 {
            return Err(Error::BadValue("zstd huffman empty weights"));
        }
        let max_bits = 32 - sum.leading_zeros();
        if max_bits > MAX_BITS {
            return Err(Error::BadValue("zstd huffman code too long"));
        }
        let rest = (1u32 << max_bits) - sum;
        if rest == 0 || rest & (rest - 1) != 0 {
            return Err(Error::BadValue("zstd huffman weights not a power of two"));
        }
        let mut weights = stored.to_vec();
        weights.push(rest.trailing_zeros() as u8 + 1);

        let mut entries = alloc::vec![(0u8, 0u8); 1usize << max_bits];
        let mut pos = 0usize;
        for w in 1..=max_bits {
            let nb = (max_bits + 1 - w) as u8;
            for (sym, &sw) in weights.iter().enumerate() {
                if sw as u32 == w {
                    let run = 1usize << (w - 1);
                    for e in &mut entries[pos..pos + run] {
                        *e = (sym as u8, nb);
                    }
                    pos += run;
                }
            }
        }
        debug_assert_eq!(pos, entries.len());
        Ok(HufTable { max_bits, entries })
    }

    /// Reads a tree description from the start of `data`; returns the table
    /// and the bytes the description occupies.
    pub(super) fn read(data: &[u8]) -> Result<(HufTable, usize)> {
        let Some(&header) = data.first() else {
            return Err(Error::truncated("zstd huffman header", 1, 0));
        };
        if header >= 128 {
            let n = header as usize - 127;
            let bytes = n.div_ceil(2);
            if 1 + bytes > data.len() {
                return Err(Error::truncated(
                    "zstd huffman weights",
                    1 + bytes,
                    data.len(),
                ));
            }
            let mut weights = Vec::with_capacity(n);
            for i in 0..n {
                let b = data[1 + i / 2];
                weights.push(if i % 2 == 0 { b >> 4 } else { b & 15 });
            }
            Ok((HufTable::from_weights(&weights)?, 1 + bytes))
        } else {
            let size = header as usize;
            if size == 0 {
                return Err(Error::BadValue("zstd huffman empty description"));
            }
            if 1 + size > data.len() {
                return Err(Error::truncated(
                    "zstd huffman weights",
                    1 + size,
                    data.len(),
                ));
            }
            let weights = fse_weights(&data[1..1 + size])?;
            Ok((HufTable::from_weights(&weights)?, 1 + size))
        }
    }

    /// Decodes exactly `count` symbols from one backward stream, which must be
    /// consumed to the last bit.
    pub(super) fn decode_stream(&self, data: &[u8], count: usize, out: &mut Vec<u8>) -> Result<()> {
        let mut br = BackBits::new(data)?;
        for _ in 0..count {
            let (sym, nb) = self.entries[br.peek(self.max_bits) as usize];
            br.consume(nb as u32);
            out.push(sym);
        }
        if !br.is_exhausted() {
            return Err(Error::BadValue("zstd huffman stream not consumed exactly"));
        }
        Ok(())
    }
}

/// Weights stored as an FSE table followed by a backward stream carrying two
/// interleaved states.
fn fse_weights(data: &[u8]) -> Result<Vec<u8>> {
    let (table, used) = fse::read_table(data, 6, 12)?;
    let mut br = BackBits::new(&data[used..])?;
    let mut s1 = br.read(table.log) as usize;
    let mut s2 = br.read(table.log) as usize;
    if br.overrun() {
        return Err(Error::BadValue("zstd huffman weight stream too short"));
    }
    let step = |state: &mut usize, br: &mut BackBits| -> u8 {
        let e = table.entries[*state];
        *state = e.baseline as usize + br.read(e.nb_bits as u32) as usize;
        e.symbol
    };
    let mut weights = Vec::new();
    loop {
        weights.push(step(&mut s1, &mut br));
        if br.overrun() {
            weights.push(table.entries[s2].symbol);
            break;
        }
        weights.push(step(&mut s2, &mut br));
        if br.overrun() {
            weights.push(table.entries[s1].symbol);
            break;
        }
        if weights.len() > 255 {
            return Err(Error::BadValue("zstd huffman too many weights"));
        }
    }
    Ok(weights)
}
