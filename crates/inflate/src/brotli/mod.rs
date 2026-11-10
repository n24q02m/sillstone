//! Pure Rust Brotli (RFC 7932) stream decoder.
//!
//! Implemented: the whole of RFC 7932 - the stream header, empty, metadata,
//! uncompressed and compressed meta-blocks, block-type switching for
//! literals, insert-and-copy commands and distances, context modelling with
//! context maps, simple and complex prefix codes, the postfix/direct distance
//! parameters, the four-slot last-distance buffer, and the 122,784-byte static
//! dictionary with all 121 word transforms. Everything is capped by
//! [`Limits`].
//!
//! Not implemented: the large-window extension (WBITS above 24), which is not
//! part of RFC 7932. A stream that announces it is refused with
//! [`Error::Unsupported`]; the decoder never guesses and never returns bytes
//! it did not decode.
//!
//! The data set ends with the last meta-block, which may be an empty one
//! (`ISLASTEMPTY`), an ordinary one or a metadata one. Its padding bits must
//! be zero and no byte may follow it: trailing bytes are rejected with
//! [`Error::BadValue`] instead of being silently ignored, so a caller cannot
//! mistake a concatenation or a mis-framed buffer for a decoded stream.

mod bits;
mod dict;
mod prefix;
mod tables;

use crate::Limits;
use alloc::vec;
use alloc::vec::Vec;
use bits::{inverse_move_to_front, BitReader};
use prefix::{read_prefix_code, Huffman};
use sillstone_digest::{Error, Result};
use tables::{LUT0, LUT1, LUT2};

/// Number of symbols in the insert-and-copy alphabet.
const COMMAND_SYMBOLS: usize = 704;
/// Number of block count codes.
const BLOCK_COUNT_SYMBOLS: usize = 26;
/// Block count used when a category has a single block type.
const SINGLE_BLOCK_COUNT: u32 = 1 << 24;

const BLOCK_COUNT_BASE: [u32; BLOCK_COUNT_SYMBOLS] = [
    1, 5, 9, 13, 17, 25, 33, 41, 49, 65, 81, 97, 113, 145, 177, 209, 241, 305, 369, 497, 753,
    1265, 2289, 4337, 8433, 16625,
];
const BLOCK_COUNT_EXTRA: [u8; BLOCK_COUNT_SYMBOLS] = [
    2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 7, 8, 9, 10, 11, 12, 13, 24,
];
const INSERT_BASE: [u32; 24] = [
    0, 1, 2, 3, 4, 5, 6, 8, 10, 14, 18, 26, 34, 50, 66, 98, 130, 194, 322, 578, 1090, 2114, 6210,
    22594,
];
const INSERT_EXTRA: [u8; 24] = [
    0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 12, 14, 24,
];
const COPY_BASE: [u32; 24] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 18, 22, 30, 38, 54, 70, 102, 134, 198, 326, 582, 1094,
    2118,
];
const COPY_EXTRA: [u8; 24] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 24,
];

/// For each 64-code cell of the insert-and-copy alphabet: the first insert
/// length code, the first copy length code, and whether the distance is the
/// implicit "last distance" (RFC 7932 section 5).
const CELLS: [(u8, u8, bool); 11] = [
    (0, 0, true),
    (0, 8, true),
    (0, 0, false),
    (0, 8, false),
    (8, 0, false),
    (8, 8, false),
    (0, 16, false),
    (16, 0, false),
    (8, 16, false),
    (16, 8, false),
    (16, 16, false),
];

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

    // RFC 7932 section 9.1: WBITS.
    let wbits = if br.read_bits(1)? == 0 {
        16
    } else {
        let n = br.read_bits(3)?;
        if n != 0 {
            17 + n
        } else {
            match br.read_bits(3)? {
                0 => 17,
                1 => return Err(Error::Unsupported("brotli large-window stream")),
                m => 8 + m,
            }
        }
    };
    let window = (1usize << wbits) - 16;

    // The last four distances, most recent first (RFC 7932 section 4).
    let mut ring = [4i64, 11, 15, 16];

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
            // A metadata meta-block is allowed to be the last one: the stream
            // ends after its payload, as the reference decoder accepts.
            if is_last {
                break;
            }
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
        if is_uncompressed {
            if out.len() + mlen > limits.max_output {
                return Err(Error::too_large("brotli output", limits.max_output));
            }
            br.align_to_byte()?;
            out.extend_from_slice(br.read_exact_bytes(mlen)?);
        } else {
            decode_compressed(&mut br, &mut out, &limits, window, &mut ring, mlen)?;
        }

        if is_last {
            break;
        }
    }

    // The last meta-block ends the data set: its final byte is padded with
    // zeros and nothing may follow it (RFC 7932 section 9.2; the reference
    // decoder rejects a stream with trailing bytes).
    br.finish()?;
    Ok(out)
}

/// Tracks the current block type and the symbols left in the current block
/// for one of the three categories (literal, insert-and-copy, distance).
struct BlockSwitch {
    ntypes: usize,
    cur: usize,
    prev: usize,
    remaining: u32,
    type_code: Option<Huffman>,
    count_code: Option<Huffman>,
}

impl BlockSwitch {
    /// Reads a category's block type count and, when there is more than one
    /// type, its two prefix codes and the first block count.
    fn read(br: &mut BitReader<'_>) -> Result<Self> {
        let ntypes = read_var_len_u8(br)?;
        let mut bs = Self {
            ntypes,
            cur: 0,
            prev: 1,
            remaining: SINGLE_BLOCK_COUNT,
            type_code: None,
            count_code: None,
        };
        if ntypes >= 2 {
            bs.type_code = Some(read_prefix_code(br, ntypes + 2)?);
            bs.count_code = Some(read_prefix_code(br, BLOCK_COUNT_SYMBOLS)?);
            bs.remaining = bs.read_count(br)?;
        }
        Ok(bs)
    }

    fn read_count(&self, br: &mut BitReader<'_>) -> Result<u32> {
        let code = match &self.count_code {
            Some(c) => c,
            None => return Ok(SINGLE_BLOCK_COUNT),
        };
        let sym = code.decode(br)? as usize;
        Ok(BLOCK_COUNT_BASE[sym] + br.read_bits(BLOCK_COUNT_EXTRA[sym] as u32)?)
    }

    /// Accounts for one symbol of this category, switching block type first
    /// when the current block is used up.
    #[inline]
    fn tick(&mut self, br: &mut BitReader<'_>) -> Result<()> {
        if self.remaining == 0 {
            let code = match &self.type_code {
                Some(c) => c,
                None => {
                    self.remaining = SINGLE_BLOCK_COUNT;
                    return self.tick(br);
                }
            };
            let sym = code.decode(br)? as usize;
            let next = match sym {
                0 => self.prev,
                1 => (self.cur + 1) % self.ntypes,
                s => s - 2,
            };
            self.prev = self.cur;
            self.cur = next;
            self.remaining = self.read_count(br)?;
        }
        self.remaining -= 1;
        Ok(())
    }
}

/// NBLTYPES / NTREES style value: 1, or `(1 << n) + 1 + n extra bits`.
fn read_var_len_u8(br: &mut BitReader<'_>) -> Result<usize> {
    if br.read_bits(1)? == 0 {
        return Ok(1);
    }
    let n = br.read_bits(3)?;
    Ok((1usize << n) + 1 + br.read_bits(n)? as usize)
}

/// Reads a context map of `size` entries over `ntrees` trees (RFC 7932
/// section 7.3); with a single tree there is nothing in the stream.
fn read_context_map(br: &mut BitReader<'_>, ntrees: usize, size: usize) -> Result<Vec<u8>> {
    if ntrees < 2 {
        return Ok(vec![0; size]);
    }
    let rle_max = if br.read_bits(1)? == 1 {
        br.read_bits(4)? as usize + 1
    } else {
        0
    };
    let code = read_prefix_code(br, ntrees + rle_max)?;
    let mut map = Vec::with_capacity(size);
    while map.len() < size {
        let sym = code.decode(br)? as usize;
        if sym == 0 {
            map.push(0);
        } else if sym <= rle_max {
            let reps = (1usize << sym) + br.read_bits(sym as u32)? as usize;
            if map.len() + reps > size {
                return Err(Error::BadValue("brotli context map run overruns the map"));
            }
            map.resize(map.len() + reps, 0);
        } else {
            map.push((sym - rle_max) as u8);
        }
    }
    if br.read_bits(1)? == 1 {
        inverse_move_to_front(&mut map);
    }
    Ok(map)
}

/// Context ID for a literal given the two previous bytes (RFC 7932 section 7.1).
#[inline]
fn literal_context(mode: u8, p1: u8, p2: u8) -> usize {
    match mode {
        0 => (p1 & 0x3f) as usize,
        1 => (p1 >> 2) as usize,
        2 => (LUT0[p1 as usize] | LUT1[p2 as usize]) as usize,
        _ => ((LUT2[p1 as usize] << 3) | LUT2[p2 as usize]) as usize,
    }
}

/// Decodes one compressed meta-block of `mlen` bytes into `out`.
fn decode_compressed(
    br: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    limits: &Limits,
    window: usize,
    ring: &mut [i64; 4],
    mlen: usize,
) -> Result<()> {
    let mut lit_bs = BlockSwitch::read(br)?;
    let mut cmd_bs = BlockSwitch::read(br)?;
    let mut dist_bs = BlockSwitch::read(br)?;

    let npostfix = br.read_bits(2)?;
    let ndirect = (br.read_bits(4)? as usize) << npostfix;

    let mut modes = Vec::with_capacity(lit_bs.ntypes);
    for _ in 0..lit_bs.ntypes {
        modes.push(br.read_bits(2)? as u8);
    }

    let ntrees_lit = read_var_len_u8(br)?;
    let cmap_lit = read_context_map(br, ntrees_lit, 64 * lit_bs.ntypes)?;
    let ntrees_dist = read_var_len_u8(br)?;
    let cmap_dist = read_context_map(br, ntrees_dist, 4 * dist_bs.ntypes)?;

    let mut lit_codes = Vec::with_capacity(ntrees_lit);
    for _ in 0..ntrees_lit {
        lit_codes.push(read_prefix_code(br, 256)?);
    }
    let mut cmd_codes = Vec::with_capacity(cmd_bs.ntypes);
    for _ in 0..cmd_bs.ntypes {
        cmd_codes.push(read_prefix_code(br, COMMAND_SYMBOLS)?);
    }
    let dist_alphabet = 16 + ndirect + (48usize << npostfix);
    let mut dist_codes = Vec::with_capacity(ntrees_dist);
    for _ in 0..ntrees_dist {
        dist_codes.push(read_prefix_code(br, dist_alphabet)?);
    }

    let postfix_mask = (1usize << npostfix) - 1;
    let mut remaining = mlen;
    while remaining > 0 {
        cmd_bs.tick(br)?;
        let cmd = cmd_codes[cmd_bs.cur].decode(br)? as usize;
        let (ins_base, copy_base, implicit) = CELLS[cmd >> 6];
        let ins_code = ins_base as usize + ((cmd >> 3) & 7);
        let copy_code = copy_base as usize + (cmd & 7);
        let insert_len =
            (INSERT_BASE[ins_code] + br.read_bits(INSERT_EXTRA[ins_code] as u32)?) as usize;
        let copy_len = (COPY_BASE[copy_code] + br.read_bits(COPY_EXTRA[copy_code] as u32)?) as usize;

        if insert_len > remaining {
            return Err(Error::BadValue("brotli insert length overruns the meta-block"));
        }
        if out.len() + insert_len > limits.max_output {
            return Err(Error::too_large("brotli output", limits.max_output));
        }
        for _ in 0..insert_len {
            lit_bs.tick(br)?;
            let n = out.len();
            let p1 = if n >= 1 { out[n - 1] } else { 0 };
            let p2 = if n >= 2 { out[n - 2] } else { 0 };
            let ctx = literal_context(modes[lit_bs.cur], p1, p2);
            let tree = cmap_lit[64 * lit_bs.cur + ctx] as usize;
            out.push(lit_codes[tree].decode(br)? as u8);
        }
        remaining -= insert_len;
        if remaining == 0 {
            // The copy length of the final command is ignored.
            break;
        }

        // Distance: implicit last distance, or a coded one.
        let (distance, pushes) = if implicit {
            (ring[0], false)
        } else {
            dist_bs.tick(br)?;
            let ctx = match copy_len {
                2 => 0,
                3 => 1,
                4 => 2,
                _ => 3,
            };
            let tree = cmap_dist[4 * dist_bs.cur + ctx] as usize;
            let dcode = dist_codes[tree].decode(br)? as usize;
            let d = if dcode < 16 {
                special_distance(dcode, ring)
            } else if dcode < 16 + ndirect {
                (dcode - 15) as i64
            } else {
                let d = dcode - ndirect - 16;
                let ndistbits = 1 + (d >> (npostfix + 1)) as u32;
                let hcode = d >> npostfix;
                let lcode = d & postfix_mask;
                let offset = ((2 + (hcode & 1)) << ndistbits) - 4;
                let extra = br.read_bits(ndistbits)? as usize;
                (((offset + extra) << npostfix) + lcode + ndirect + 1) as i64
            };
            (d, dcode != 0)
        };
        if distance <= 0 {
            return Err(Error::BadValue("brotli distance is not positive"));
        }
        let distance = distance as usize;

        let max_distance = window.min(out.len());
        if distance <= max_distance {
            if copy_len > remaining {
                return Err(Error::BadValue("brotli copy length overruns the meta-block"));
            }
            if out.len() + copy_len > limits.max_output {
                return Err(Error::too_large("brotli output", limits.max_output));
            }
            if pushes {
                *ring = [distance as i64, ring[0], ring[1], ring[2]];
            }
            let start = out.len() - distance;
            if distance >= copy_len {
                out.extend_from_within(start..start + copy_len);
            } else {
                for i in 0..copy_len {
                    let b = out[start + i];
                    out.push(b);
                }
            }
            remaining -= copy_len;
        } else {
            // A static dictionary reference; it does not enter the ring.
            let word_id = distance - (max_distance + 1);
            let before = out.len();
            let made = dict::append_word(out, copy_len, word_id)?;
            if made > remaining {
                return Err(Error::BadValue("brotli dictionary word overruns the meta-block"));
            }
            if before + made > limits.max_output {
                return Err(Error::too_large("brotli output", limits.max_output));
            }
            remaining -= made;
        }
    }
    Ok(())
}

/// Distance codes 0..=15 (RFC 7932 section 4): the four last distances and
/// the last two adjusted by up to three.
fn special_distance(dcode: usize, ring: &[i64; 4]) -> i64 {
    match dcode {
        0..=3 => ring[dcode],
        4 => ring[0] - 1,
        5 => ring[0] + 1,
        6 => ring[0] - 2,
        7 => ring[0] + 2,
        8 => ring[0] - 3,
        9 => ring[0] + 3,
        10 => ring[1] - 1,
        11 => ring[1] + 1,
        12 => ring[1] - 2,
        13 => ring[1] + 2,
        14 => ring[1] - 3,
        _ => ring[1] + 3,
    }
}
