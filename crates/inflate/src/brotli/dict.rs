//! The static dictionary and its word transforms (RFC 7932 section 8 and
//! appendices A and B).

use super::tables::TRANSFORMS;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// The 122,784 bytes of RFC 7932 Appendix A, byte-identical to the
/// `dictionary.bin` of google/brotli v1.1.0 (see the repository `NOTICE`).
pub(super) static DICTIONARY: &[u8] = include_bytes!("dictionary.bin");

/// log2 of the number of dictionary words of each length (RFC 7932 section 8).
const NDBITS: [u8; 25] = [
    0, 0, 0, 0, 10, 10, 11, 11, 10, 10, 10, 10, 10, 9, 9, 8, 7, 7, 8, 7, 7, 6, 6, 5, 5,
];

/// Byte offset of the first word of each length.
const fn offsets() -> [usize; 26] {
    let mut off = [0usize; 26];
    let mut len = 0;
    while len < 25 {
        let words = if len < 4 { 0 } else { 1usize << NDBITS[len] };
        off[len + 1] = off[len] + len * words;
        len += 1;
    }
    off
}
const OFFSETS: [usize; 26] = offsets();

/// Appends the dictionary word selected by `word_id` for a copy of `len`
/// bytes, run through its transform, and reports how many bytes it added.
pub(super) fn append_word(out: &mut Vec<u8>, len: usize, word_id: usize) -> Result<usize> {
    if !(4..=24).contains(&len) {
        return Err(Error::BadValue(
            "brotli dictionary word length out of range",
        ));
    }
    let bits = NDBITS[len] as u32;
    let index = word_id & ((1usize << bits) - 1);
    let transform_id = word_id >> bits;
    if transform_id >= TRANSFORMS.len() {
        return Err(Error::BadValue("brotli dictionary transform out of range"));
    }
    let start = OFFSETS[len] + index * len;
    let word = &DICTIONARY[start..start + len];
    let (prefix, kind, suffix) = TRANSFORMS[transform_id];

    let before = out.len();
    out.extend_from_slice(prefix);
    let body = out.len();
    match kind {
        0 => out.extend_from_slice(word),
        1 | 2 => {
            out.extend_from_slice(word);
            let end = out.len();
            let mut i = body;
            while i < end {
                i += ferment(&mut out[i..end]);
                if kind == 1 {
                    break;
                }
            }
        }
        3..=11 => {
            let skip = (kind - 2) as usize;
            if skip < len {
                out.extend_from_slice(&word[skip..]);
            }
        }
        _ => {
            let skip = (kind - 11) as usize;
            if skip < len {
                out.extend_from_slice(&word[..len - skip]);
            }
        }
    }
    out.extend_from_slice(suffix);
    Ok(out.len() - before)
}

/// The RFC's "overly simplified" uppercase model for UTF-8; returns the
/// number of bytes the character at the start of `p` spans. A character cut
/// off by the end of the word leaves the missing continuation bytes alone.
fn ferment(p: &mut [u8]) -> usize {
    let b = p[0];
    if b < 0xc0 {
        if b.is_ascii_lowercase() {
            p[0] ^= 32;
        }
        1
    } else if b < 0xe0 {
        if let Some(c) = p.get_mut(1) {
            *c ^= 32;
        }
        2
    } else {
        if let Some(c) = p.get_mut(2) {
            *c ^= 5;
        }
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_cover_the_whole_dictionary() {
        assert_eq!(OFFSETS[25], DICTIONARY.len());
    }
}
