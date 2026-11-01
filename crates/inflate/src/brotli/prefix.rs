//! Prefix codes (RFC 7932 section 3): reading them from the stream and
//! decoding symbols with them.

use super::bits::BitReader;
use alloc::vec;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// Longest code length the format allows.
const MAX_LEN: usize = 15;
/// Codes up to this long resolve with one table lookup.
const FAST_BITS: u32 = 9;

/// A canonical prefix code. Codes up to [`FAST_BITS`] long decode through a
/// direct table indexed by the next bits of the stream; longer ones fall back
/// to the bit-by-bit canonical walk.
pub(super) struct Huffman {
    /// `(length << 16) | symbol`, or 0 when the code is longer than the table.
    fast: Vec<u32>,
    /// Number of codes of each length.
    count: [u16; MAX_LEN + 1],
    /// Symbols ordered by (code length, symbol value).
    symbols: Vec<u16>,
    /// The only symbol of a zero-bit code, when the code has exactly one.
    single: Option<u16>,
}

impl Huffman {
    /// Builds the canonical code of RFC 7932 section 3.2 from per-symbol
    /// lengths (0 = absent). A single present symbol yields a zero-bit code.
    /// The caller has already verified the lengths fill the code space.
    pub(super) fn from_lengths(lengths: &[u8]) -> Self {
        let mut count = [0u16; MAX_LEN + 1];
        for &l in lengths {
            if l != 0 {
                count[l as usize] += 1;
            }
        }
        let used: usize = count.iter().map(|&c| c as usize).sum();
        if used == 1 {
            let sym = lengths.iter().position(|&l| l != 0).unwrap_or(0);
            return Self {
                fast: Vec::new(),
                count,
                symbols: Vec::new(),
                single: Some(sym as u16),
            };
        }

        let mut offset = [0usize; MAX_LEN + 2];
        for l in 1..=MAX_LEN {
            offset[l + 1] = offset[l] + count[l] as usize;
        }
        let mut symbols = vec![0u16; used];
        let mut fill = offset;
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[fill[l as usize]] = sym as u16;
                fill[l as usize] += 1;
            }
        }

        let mut fast = vec![0u32; 1 << FAST_BITS];
        let mut code = 0u32;
        let mut idx = 0usize;
        for (len, &num) in count.iter().enumerate().skip(1) {
            for _ in 0..num {
                if len as u32 <= FAST_BITS {
                    let rev = code.reverse_bits() >> (32 - len as u32);
                    let entry = ((len as u32) << 16) | symbols[idx] as u32;
                    let mut i = rev as usize;
                    while i < fast.len() {
                        fast[i] = entry;
                        i += 1 << len;
                    }
                }
                code += 1;
                idx += 1;
            }
            code <<= 1;
        }

        Self {
            fast,
            count,
            symbols,
            single: None,
        }
    }

    /// Decodes one symbol.
    #[inline]
    pub(super) fn decode(&self, br: &mut BitReader<'_>) -> Result<u16> {
        if let Some(sym) = self.single {
            return Ok(sym);
        }
        let v = br.peek(MAX_LEN as u32);
        let entry = self.fast[(v & ((1 << FAST_BITS) - 1)) as usize];
        if entry != 0 {
            br.consume(entry >> 16)?;
            return Ok((entry & 0xffff) as u16);
        }
        // Canonical walk: `first` is the first code of the current length.
        let mut code = 0u32;
        let mut first = 0u32;
        let mut index = 0u32;
        for len in 1..=MAX_LEN {
            code |= (v >> (len - 1)) & 1;
            let count = self.count[len] as u32;
            if code < first + count {
                br.consume(len as u32)?;
                return Ok(self.symbols[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(Error::BadValue("brotli invalid prefix code"))
    }
}

/// Smallest number of bits that can represent every symbol of the alphabet.
fn alphabet_bits(alphabet_size: usize) -> u32 {
    usize::BITS - (alphabet_size - 1).leading_zeros()
}

/// Code length code order and the variable-length code for the lengths
/// themselves (RFC 7932 section 3.5), indexed by the next four stream bits.
const CODE_LENGTH_ORDER: [usize; 18] =
    [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const CL_PREFIX_LEN: [u8; 16] = [2, 2, 2, 3, 2, 2, 2, 4, 2, 2, 2, 3, 2, 2, 2, 4];
const CL_PREFIX_VAL: [u8; 16] = [0, 4, 3, 2, 0, 4, 3, 1, 0, 4, 3, 2, 0, 4, 3, 5];

/// Reads one prefix code over an alphabet of `alphabet_size` symbols.
pub(super) fn read_prefix_code(br: &mut BitReader<'_>, alphabet_size: usize) -> Result<Huffman> {
    let hskip = br.read_bits(2)? as usize;
    if hskip == 1 {
        read_simple(br, alphabet_size)
    } else {
        read_complex(br, alphabet_size, hskip)
    }
}

fn read_simple(br: &mut BitReader<'_>, alphabet_size: usize) -> Result<Huffman> {
    let nsym = br.read_bits(2)? as usize + 1;
    let abits = alphabet_bits(alphabet_size);
    let mut syms = [0usize; 4];
    for i in 0..nsym {
        let s = br.read_bits(abits)? as usize;
        if s >= alphabet_size {
            return Err(Error::BadValue(
                "brotli simple prefix code symbol out of range",
            ));
        }
        if syms[..i].contains(&s) {
            return Err(Error::BadValue(
                "brotli simple prefix code repeats a symbol",
            ));
        }
        syms[i] = s;
    }
    let select = if nsym == 4 { br.read_bits(1)? } else { 0 };
    let lens: &[u8] = match (nsym, select) {
        (1, _) => &[1],
        (2, _) => &[1, 1],
        (3, _) => &[1, 2, 2],
        (_, 0) => &[2, 2, 2, 2],
        _ => &[1, 2, 3, 3],
    };
    let mut lengths = vec![0u8; alphabet_size];
    for (s, &l) in syms.iter().zip(lens) {
        lengths[*s] = l;
    }
    Ok(Huffman::from_lengths(&lengths))
}

fn read_complex(br: &mut BitReader<'_>, alphabet_size: usize, hskip: usize) -> Result<Huffman> {
    // The code that describes the code lengths.
    let mut cl_lengths = [0u8; 18];
    let mut space: i32 = 32;
    let mut num_codes = 0;
    for &slot in &CODE_LENGTH_ORDER[hskip..] {
        let v = br.peek(4) as usize;
        br.consume(CL_PREFIX_LEN[v] as u32)?;
        let len = CL_PREFIX_VAL[v];
        cl_lengths[slot] = len;
        if len != 0 {
            space -= 32 >> len;
            num_codes += 1;
            if space <= 0 {
                break;
            }
        }
    }
    if num_codes != 1 && space != 0 {
        return Err(Error::BadValue("brotli code length code is not complete"));
    }
    let cl = Huffman::from_lengths(&cl_lengths);

    // The symbol lengths, with the repeat codes 16 and 17.
    let mut lengths = vec![0u8; alphabet_size];
    let mut symbol = 0usize;
    let mut prev_len = 8u8;
    let mut repeat = 0usize;
    let mut repeat_len = 0u8;
    let mut space: i32 = 32768;
    while symbol < alphabet_size && space > 0 {
        let code_len = cl.decode(br)? as u8;
        if code_len < 16 {
            repeat = 0;
            lengths[symbol] = code_len;
            symbol += 1;
            if code_len != 0 {
                prev_len = code_len;
                space -= 32768 >> code_len;
            }
        } else {
            let extra = if code_len == 16 { 2 } else { 3 };
            let new_len = if code_len == 16 { prev_len } else { 0 };
            if repeat_len != new_len {
                repeat = 0;
                repeat_len = new_len;
            }
            let old = repeat;
            if repeat > 0 {
                repeat = (repeat - 2) << extra;
            }
            repeat += br.read_bits(extra)? as usize + 3;
            let delta = repeat - old;
            if symbol + delta > alphabet_size {
                return Err(Error::BadValue(
                    "brotli code length repeat overruns the alphabet",
                ));
            }
            lengths[symbol..symbol + delta].fill(repeat_len);
            symbol += delta;
            if repeat_len != 0 {
                space -= (delta as i32) << (15 - repeat_len);
            }
        }
    }
    if space != 0 {
        return Err(Error::BadValue("brotli prefix code is not complete"));
    }
    Ok(Huffman::from_lengths(&lengths))
}
