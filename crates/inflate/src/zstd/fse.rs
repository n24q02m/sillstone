//! Finite State Entropy tables (RFC 8878 section 4.1).

use super::bits::FwdBits;
use alloc::vec;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

#[derive(Copy, Clone, Debug)]
pub(super) struct FseEntry {
    pub symbol: u8,
    pub nb_bits: u8,
    pub baseline: u16,
}

#[derive(Clone, Debug)]
pub(super) struct FseTable {
    pub log: u32,
    pub entries: Vec<FseEntry>,
}

/// Reads a table description from the start of `data`. Returns the table and
/// the number of bytes the description occupies.
pub(super) fn read_table(
    data: &[u8],
    max_log: u32,
    max_symbols: usize,
) -> Result<(FseTable, usize)> {
    let mut br = FwdBits::new(data);
    let log = br.read(4)? + 5;
    if log > max_log {
        return Err(Error::BadValue("zstd fse accuracy log too large"));
    }
    let mut remaining: i32 = (1 << log) + 1;
    let mut probs: Vec<i16> = Vec::new();
    let mut prev_zero = false;
    while remaining > 1 {
        if prev_zero {
            // A zero probability is followed by a run length of further zeros,
            // in 2-bit groups where 3 means "three more and keep reading".
            loop {
                let r = br.read(2)?;
                probs.resize(probs.len() + r as usize, 0);
                if probs.len() > max_symbols {
                    return Err(Error::BadValue("zstd fse too many symbols"));
                }
                if r != 3 {
                    break;
                }
            }
        }
        let nb = 32 - (remaining as u32).leading_zeros();
        let threshold = 1i32 << (nb - 1);
        let max = 2 * threshold - 1 - remaining;
        let lo = br.read(nb - 1)? as i32;
        let value = if lo < max {
            lo
        } else {
            let v = lo | ((br.read(1)? as i32) << (nb - 1));
            if v >= threshold { v - max } else { v }
        };
        let prob = value - 1;
        remaining -= prob.abs();
        if remaining < 1 {
            return Err(Error::BadValue("zstd fse probabilities exceed table size"));
        }
        probs.push(prob as i16);
        if probs.len() > max_symbols {
            return Err(Error::BadValue("zstd fse too many symbols"));
        }
        prev_zero = prob == 0;
    }
    if remaining != 1 {
        return Err(Error::BadValue(
            "zstd fse probabilities do not fill the table",
        ));
    }
    let table = build(&probs, log)?;
    Ok((table, br.bytes_consumed()))
}

/// Builds the decoding table for normalised counts `probs` (-1 meaning
/// "less than one") at accuracy `log`.
pub(super) fn build(probs: &[i16], log: u32) -> Result<FseTable> {
    let size = 1usize << log;
    let mut entries = vec![
        FseEntry {
            symbol: 0,
            nb_bits: 0,
            baseline: 0,
        };
        size
    ];
    let mut next = vec![0u32; probs.len()];
    let mut high = size;
    for (s, &p) in probs.iter().enumerate() {
        if p == -1 {
            if high == 0 {
                return Err(Error::BadValue("zstd fse table overfull"));
            }
            high -= 1;
            entries[high].symbol = s as u8;
            next[s] = 1;
        } else if p >= 0 {
            next[s] = p as u32;
        } else {
            return Err(Error::BadValue("zstd fse invalid probability"));
        }
    }
    let step = (size >> 1) + (size >> 3) + 3;
    let mask = size - 1;
    let mut pos = 0usize;
    for (s, &p) in probs.iter().enumerate() {
        for _ in 0..p.max(0) {
            entries[pos].symbol = s as u8;
            pos = (pos + step) & mask;
            while pos >= high {
                pos = (pos + step) & mask;
            }
        }
    }
    if pos != 0 {
        return Err(Error::BadValue("zstd fse spread did not close"));
    }
    for e in entries.iter_mut() {
        let s = e.symbol as usize;
        let n = next[s];
        next[s] += 1;
        let nb = log - (31 - n.leading_zeros());
        e.nb_bits = nb as u8;
        e.baseline = ((n << nb) - size as u32) as u16;
    }
    Ok(FseTable { log, entries })
}

/// A table of one symbol that costs zero bits: the RLE mode of a sequence
/// symbol-compression-modes byte.
pub(super) fn rle(symbol: u8) -> FseTable {
    FseTable {
        log: 0,
        entries: vec![FseEntry {
            symbol,
            nb_bits: 0,
            baseline: 0,
        }],
    }
}
