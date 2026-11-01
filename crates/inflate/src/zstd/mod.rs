//! Pure Rust Zstandard (RFC 8878) frame decoder.
//!
//! Implemented: standard and skippable frames, raw, RLE and compressed
//! blocks, raw, RLE and Huffman-compressed literals (one and four streams,
//! treeless reuse), FSE-coded sequences with the predefined, RLE, compressed
//! and repeat table modes, the three repeat offsets, and the frame content
//! checksum (low 32 bits of XXH64). Everything is capped by [`Limits`] and the
//! frame's own window and block-size rules.
//!
//! Not implemented: dictionaries. A frame that names a nonzero dictionary id
//! is refused with [`Error::Unsupported`]; the decoder never guesses and never
//! returns bytes it did not decode.

mod bits;
mod fse;
mod huf;

use crate::Limits;
use alloc::vec::Vec;
use bits::BackBits;
use fse::FseTable;
use huf::HufTable;
use sillstone_digest::{Error, Result, xxh64};

/// Standard Zstandard magic number: `0xFD2FB528` (little-endian: 0x28, 0xB5, 0x2F, 0xFD).
pub const ZSTD_MAGIC: u32 = 0xFD2FB528;
/// Minimum magic number for skippable frames: `0x184D2A50`.
pub const SKIPPABLE_MAGIC_MIN: u32 = 0x184D2A50;
/// Maximum magic number for skippable frames: `0x184D2A5F`.
pub const SKIPPABLE_MAGIC_MAX: u32 = 0x184D2A5F;

/// Largest block the format allows regardless of window size.
const BLOCK_SIZE_MAX: usize = 128 * 1024;

const LL_BASE: [u32; 36] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32, 40, 48, 64,
    128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
];
const LL_BITS: [u8; 36] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11,
    12, 13, 14, 15, 16,
];
const ML_BASE: [u32; 53] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32, 33, 34, 35, 37, 39, 41, 43, 47, 51, 59, 67, 83, 99, 131, 259, 515, 1027,
    2051, 4099, 8195, 16387, 32771, 65539,
];
const ML_BITS: [u8; 53] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
];

const LL_DEFAULT: [i16; 36] = [
    4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1,
    -1, -1, -1, -1,
];
const ML_DEFAULT: [i16; 53] = [
    1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1,
];
const OF_DEFAULT: [i16; 29] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1,
];

/// Entropy state that carries from one block to the next within a frame.
#[derive(Default)]
struct Carry {
    huf: Option<HufTable>,
    ll: Option<FseTable>,
    of: Option<FseTable>,
    ml: Option<FseTable>,
    rep: [usize; 3],
}

struct Frame {
    /// Where this frame's output begins inside the shared output vector.
    start: usize,
    window: u64,
    block_max: usize,
    carry: Carry,
}

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
        let magic =
            u32::from_le_bytes([input[pos], input[pos + 1], input[pos + 2], input[pos + 3]]);
        pos += 4;

        if (SKIPPABLE_MAGIC_MIN..=SKIPPABLE_MAGIC_MAX).contains(&magic) {
            if pos + 4 > input.len() {
                return Err(Error::truncated(
                    "zstd skippable frame",
                    4,
                    input.len() - pos,
                ));
            }
            let frame_size =
                u32::from_le_bytes([input[pos], input[pos + 1], input[pos + 2], input[pos + 3]])
                    as usize;
            pos += 4;
            if frame_size > input.len() - pos {
                return Err(Error::truncated(
                    "zstd skippable payload",
                    frame_size,
                    input.len() - pos,
                ));
            }
            pos += frame_size;
            continue;
        }

        if magic != ZSTD_MAGIC {
            return Err(Error::InvalidMagic { what: "zstd" });
        }
        pos += decode_frame(&input[pos..], &mut out, limits)?;
    }

    Ok(out)
}

/// Decodes one standard frame (after its magic number); returns the bytes it
/// occupies.
fn decode_frame(input: &[u8], out: &mut Vec<u8>, limits: Limits) -> Result<usize> {
    let mut pos = 0;
    let Some(&desc) = input.first() else {
        return Err(Error::truncated("zstd descriptor", 1, 0));
    };
    pos += 1;

    let fcs_flag = desc >> 6;
    let single_seg = (desc >> 5) & 1 == 1;
    if desc & 0x08 != 0 {
        return Err(Error::BadValue("zstd reserved descriptor bit"));
    }
    let has_checksum = (desc >> 2) & 1 == 1;
    let dict_bytes = [0usize, 1, 2, 4][(desc & 3) as usize];

    let mut window_desc = 0u8;
    if !single_seg {
        let Some(&b) = input.get(pos) else {
            return Err(Error::truncated("zstd window descriptor", 1, 0));
        };
        window_desc = b;
        pos += 1;
    }

    if pos + dict_bytes > input.len() {
        return Err(Error::truncated(
            "zstd dict id",
            dict_bytes,
            input.len() - pos,
        ));
    }
    let mut dict_id = 0u32;
    for (i, &b) in input[pos..pos + dict_bytes].iter().enumerate() {
        dict_id |= (b as u32) << (8 * i);
    }
    pos += dict_bytes;
    if dict_id != 0 {
        return Err(Error::Unsupported("zstd dictionary"));
    }

    let fcs_bytes = match fcs_flag {
        0 if single_seg => 1,
        0 => 0,
        1 => 2,
        2 => 4,
        _ => 8,
    };
    if pos + fcs_bytes > input.len() {
        return Err(Error::truncated(
            "zstd frame content size",
            fcs_bytes,
            input.len() - pos,
        ));
    }
    let mut fcs: Option<u64> = None;
    if fcs_bytes > 0 {
        let mut v = 0u64;
        for (i, &b) in input[pos..pos + fcs_bytes].iter().enumerate() {
            v |= (b as u64) << (8 * i);
        }
        if fcs_bytes == 2 {
            v += 256;
        }
        fcs = Some(v);
    }
    pos += fcs_bytes;

    let window = if single_seg {
        fcs.unwrap_or(0)
    } else {
        let exp = (window_desc >> 3) as u32;
        let mantissa = (window_desc & 7) as u64;
        let base = 1u64 << (10 + exp);
        base + (base >> 3) * mantissa
    };
    let block_max = window.min(BLOCK_SIZE_MAX as u64) as usize;

    if let Some(n) = fcs {
        if n > (limits.max_output - out.len().min(limits.max_output)) as u64 {
            return Err(Error::too_large("zstd output", limits.max_output));
        }
    }

    let mut frame = Frame {
        start: out.len(),
        window,
        block_max,
        carry: Carry {
            rep: [1, 4, 8],
            ..Carry::default()
        },
    };

    loop {
        if pos + 3 > input.len() {
            return Err(Error::truncated("zstd block header", 3, input.len() - pos));
        }
        let hdr = input[pos] as u32 | (input[pos + 1] as u32) << 8 | (input[pos + 2] as u32) << 16;
        pos += 3;

        let last = hdr & 1 == 1;
        let kind = (hdr >> 1) & 3;
        let size = (hdr >> 3) as usize;

        match kind {
            0 => {
                if size > frame.block_max {
                    return Err(Error::BadValue("zstd block larger than the window allows"));
                }
                if size > input.len() - pos {
                    return Err(Error::truncated("zstd block body", size, input.len() - pos));
                }
                if out.len() + size > limits.max_output {
                    return Err(Error::too_large("zstd output", limits.max_output));
                }
                out.extend_from_slice(&input[pos..pos + size]);
                pos += size;
            }
            1 => {
                if size > frame.block_max {
                    return Err(Error::BadValue("zstd block larger than the window allows"));
                }
                let Some(&byte) = input.get(pos) else {
                    return Err(Error::truncated("zstd rle byte", 1, 0));
                };
                if out.len() + size > limits.max_output {
                    return Err(Error::too_large("zstd output", limits.max_output));
                }
                pos += 1;
                out.resize(out.len() + size, byte);
            }
            2 => {
                if size > frame.block_max {
                    return Err(Error::BadValue("zstd block larger than the window allows"));
                }
                if size > input.len() - pos {
                    return Err(Error::truncated("zstd block body", size, input.len() - pos));
                }
                decode_compressed_block(&input[pos..pos + size], out, &mut frame, limits)?;
                pos += size;
            }
            _ => return Err(Error::BadValue("reserved zstd block type")),
        }

        if last {
            break;
        }
    }

    let produced = (out.len() - frame.start) as u64;
    if let Some(n) = fcs {
        if produced != n {
            return Err(Error::BadValue("zstd frame content size mismatch"));
        }
    }

    if has_checksum {
        if pos + 4 > input.len() {
            return Err(Error::truncated("zstd checksum", 4, input.len() - pos));
        }
        let stored =
            u32::from_le_bytes([input[pos], input[pos + 1], input[pos + 2], input[pos + 3]]);
        let computed = xxh64(&out[frame.start..], 0) as u32;
        if stored != computed {
            return Err(Error::BadValue("zstd content checksum mismatch"));
        }
        pos += 4;
    }
    Ok(pos)
}

/// Splits a compressed block into its literals and sequences and executes it.
fn decode_compressed_block(
    block: &[u8],
    out: &mut Vec<u8>,
    frame: &mut Frame,
    limits: Limits,
) -> Result<()> {
    let (literals, used) = parse_literals(block, &mut frame.carry.huf, frame.block_max)?;
    decode_sequences(&block[used..], &literals, out, frame, limits)
}

/// Parses the literals section; returns the literals and the section length.
fn parse_literals(
    block: &[u8],
    huf_slot: &mut Option<HufTable>,
    block_max: usize,
) -> Result<(Vec<u8>, usize)> {
    let Some(&b0) = block.first() else {
        return Err(Error::truncated("zstd literals header", 1, 0));
    };
    let kind = b0 & 3;
    let sf = (b0 >> 2) & 3;
    let le = |n: usize| -> Result<u32> {
        if block.len() < n {
            return Err(Error::truncated("zstd literals header", n, block.len()));
        }
        Ok(block[..n]
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, &b)| a | (b as u32) << (8 * i)))
    };

    if kind < 2 {
        let (hdr, size) = match sf {
            0 | 2 => (1, (b0 >> 3) as usize),
            1 => (2, (le(2)? >> 4) as usize),
            _ => (3, (le(3)? >> 4) as usize),
        };
        if size > block_max {
            return Err(Error::BadValue(
                "zstd literals larger than the block allows",
            ));
        }
        if kind == 0 {
            if hdr + size > block.len() {
                return Err(Error::truncated(
                    "zstd raw literals",
                    hdr + size,
                    block.len(),
                ));
            }
            return Ok((block[hdr..hdr + size].to_vec(), hdr + size));
        }
        let Some(&byte) = block.get(hdr) else {
            return Err(Error::truncated("zstd rle literal", hdr + 1, block.len()));
        };
        return Ok((alloc::vec![byte; size], hdr + 1));
    }

    // Compressed (2) or treeless (3): sizes are 10, 10, 14 or 18 bits each.
    let (hdr, bits, streams) = match sf {
        0 => (3, 10, 1),
        1 => (3, 10, 4),
        2 => (4, 14, 4),
        _ => (5, 18, 4),
    };
    let v = if hdr == 5 {
        // Needs 40 bits: assemble in u64.
        if block.len() < 5 {
            return Err(Error::truncated("zstd literals header", 5, block.len()));
        }
        block[..5]
            .iter()
            .enumerate()
            .fold(0u64, |a, (i, &b)| a | (b as u64) << (8 * i))
    } else {
        le(hdr)? as u64
    };
    let mask = (1u64 << bits) - 1;
    let regen = ((v >> 4) & mask) as usize;
    let comp = ((v >> (4 + bits)) & mask) as usize;
    if regen > block_max {
        return Err(Error::BadValue(
            "zstd literals larger than the block allows",
        ));
    }
    if hdr + comp > block.len() {
        return Err(Error::truncated(
            "zstd compressed literals",
            hdr + comp,
            block.len(),
        ));
    }
    let data = &block[hdr..hdr + comp];

    let tree_len = if kind == 2 {
        let (table, n) = HufTable::read(data)?;
        *huf_slot = Some(table);
        n
    } else {
        0
    };
    let Some(table) = huf_slot.as_ref() else {
        return Err(Error::BadValue(
            "zstd treeless literals without a previous table",
        ));
    };
    let stream = &data[tree_len..];

    let mut literals = Vec::with_capacity(regen);
    if streams == 1 {
        table.decode_stream(stream, regen, &mut literals)?;
    } else {
        if stream.len() < 6 {
            return Err(Error::truncated("zstd jump table", 6, stream.len()));
        }
        let s1 = u16::from_le_bytes([stream[0], stream[1]]) as usize;
        let s2 = u16::from_le_bytes([stream[2], stream[3]]) as usize;
        let s3 = u16::from_le_bytes([stream[4], stream[5]]) as usize;
        let body = &stream[6..];
        if s1 + s2 + s3 >= body.len() {
            return Err(Error::BadValue(
                "zstd huffman stream sizes exceed the section",
            ));
        }
        let seg = regen.div_ceil(4);
        if 3 * seg > regen {
            return Err(Error::BadValue("zstd four-stream literals too short"));
        }
        let (a, rest) = body.split_at(s1);
        let (b, rest) = rest.split_at(s2);
        let (c, d) = rest.split_at(s3);
        table.decode_stream(a, seg, &mut literals)?;
        table.decode_stream(b, seg, &mut literals)?;
        table.decode_stream(c, seg, &mut literals)?;
        table.decode_stream(d, regen - 3 * seg, &mut literals)?;
    }
    Ok((literals, hdr + comp))
}

/// Which of the three sequence symbols a table codes.
#[derive(Copy, Clone)]
enum Kind {
    LitLen,
    Offset,
    MatchLen,
}

impl Kind {
    fn max_log(self) -> u32 {
        match self {
            Kind::LitLen | Kind::MatchLen => 9,
            Kind::Offset => 8,
        }
    }

    fn max_symbols(self) -> usize {
        match self {
            Kind::LitLen => 36,
            Kind::Offset => 32,
            Kind::MatchLen => 53,
        }
    }

    fn predefined(self) -> Result<FseTable> {
        match self {
            Kind::LitLen => fse::build(&LL_DEFAULT, 6),
            Kind::Offset => fse::build(&OF_DEFAULT, 5),
            Kind::MatchLen => fse::build(&ML_DEFAULT, 6),
        }
    }
}

/// Resolves one table per the symbol compression mode; returns the bytes read.
fn load_table(kind: Kind, mode: u8, data: &[u8], slot: &mut Option<FseTable>) -> Result<usize> {
    match mode {
        0 => {
            *slot = Some(kind.predefined()?);
            Ok(0)
        }
        1 => {
            let Some(&sym) = data.first() else {
                return Err(Error::truncated("zstd rle symbol", 1, 0));
            };
            if sym as usize >= kind.max_symbols() {
                return Err(Error::BadValue("zstd rle symbol out of range"));
            }
            *slot = Some(fse::rle(sym));
            Ok(1)
        }
        2 => {
            let (table, n) = fse::read_table(data, kind.max_log(), kind.max_symbols())?;
            *slot = Some(table);
            Ok(n)
        }
        _ => {
            if slot.is_none() {
                return Err(Error::BadValue(
                    "zstd repeat table without a previous table",
                ));
            }
            Ok(0)
        }
    }
}

fn decode_sequences(
    sec: &[u8],
    literals: &[u8],
    out: &mut Vec<u8>,
    frame: &mut Frame,
    limits: Limits,
) -> Result<()> {
    let Some(&c0) = sec.first() else {
        return Err(Error::truncated("zstd sequences header", 1, 0));
    };
    let mut p = 1usize;
    let nb_seq = if c0 < 128 {
        c0 as usize
    } else if c0 < 255 {
        let Some(&c1) = sec.get(p) else {
            return Err(Error::truncated("zstd sequence count", 2, 1));
        };
        p += 1;
        ((c0 as usize - 128) << 8) + c1 as usize
    } else {
        if sec.len() < 3 {
            return Err(Error::truncated("zstd sequence count", 3, sec.len()));
        }
        p += 2;
        sec[1] as usize + ((sec[2] as usize) << 8) + 0x7F00
    };

    let block_start = out.len();
    if nb_seq == 0 {
        if p != sec.len() {
            return Err(Error::BadValue(
                "zstd trailing bytes after literals-only block",
            ));
        }
        return append_literals(out, literals, block_start, frame.block_max, limits);
    }

    let Some(&modes) = sec.get(p) else {
        return Err(Error::truncated("zstd symbol compression modes", 1, 0));
    };
    p += 1;
    if modes & 3 != 0 {
        return Err(Error::BadValue("zstd reserved compression mode bits"));
    }
    let carry = &mut frame.carry;
    p += load_table(Kind::LitLen, modes >> 6, &sec[p..], &mut carry.ll)?;
    p += load_table(Kind::Offset, (modes >> 4) & 3, &sec[p..], &mut carry.of)?;
    p += load_table(Kind::MatchLen, (modes >> 2) & 3, &sec[p..], &mut carry.ml)?;

    let (Some(llt), Some(oft), Some(mlt)) = (&carry.ll, &carry.of, &carry.ml) else {
        return Err(Error::BadValue("zstd missing sequence table"));
    };
    let mut br = BackBits::new(&sec[p..])?;
    let mut ls = br.read(llt.log) as usize;
    let mut os = br.read(oft.log) as usize;
    let mut ms = br.read(mlt.log) as usize;
    if br.overrun() {
        return Err(Error::BadValue("zstd sequence stream too short"));
    }

    let mut lit_pos = 0usize;
    let mut rep = carry.rep;
    for i in 0..nb_seq {
        let ll_code = llt.entries[ls].symbol as usize;
        let of_code = oft.entries[os].symbol as u32;
        let ml_code = mlt.entries[ms].symbol as usize;

        let of_value = (1u64 << of_code) + br.read(of_code) as u64;
        let ml = (ML_BASE[ml_code] + br.read(ML_BITS[ml_code] as u32)) as usize;
        let ll = (LL_BASE[ll_code] + br.read(LL_BITS[ll_code] as u32)) as usize;

        let offset = if of_value > 3 {
            let o = (of_value - 3) as usize;
            rep = [o, rep[0], rep[1]];
            o
        } else {
            // With no literals the repeat indices shift by one.
            let idx = of_value as usize - 1 + usize::from(ll == 0);
            let o = match idx {
                0 => rep[0],
                1 => rep[1],
                2 => rep[2],
                _ => rep[0].wrapping_sub(1),
            };
            if o == 0 {
                return Err(Error::BadValue("zstd zero offset"));
            }
            match idx {
                0 => {}
                1 => rep = [o, rep[0], rep[2]],
                _ => rep = [o, rep[0], rep[1]],
            }
            o
        };

        if ll > literals.len() - lit_pos {
            return Err(Error::BadValue("zstd sequence overruns the literals"));
        }
        let produced = out.len() - block_start;
        if produced + ll + ml > frame.block_max {
            return Err(Error::BadValue(
                "zstd block regenerates more than the block allows",
            ));
        }
        if out.len() + ll + ml > limits.max_output {
            return Err(Error::too_large("zstd output", limits.max_output));
        }
        out.extend_from_slice(&literals[lit_pos..lit_pos + ll]);
        lit_pos += ll;

        let history = out.len() - frame.start;
        if offset > history || offset as u64 > frame.window {
            return Err(Error::BadValue("zstd match offset beyond the window"));
        }
        let from = out.len() - offset;
        if offset >= ml {
            out.extend_from_within(from..from + ml);
        } else {
            for k in 0..ml {
                let b = out[from + k];
                out.push(b);
            }
        }

        if i + 1 < nb_seq {
            let e = llt.entries[ls];
            ls = e.baseline as usize + br.read(e.nb_bits as u32) as usize;
            let e = mlt.entries[ms];
            ms = e.baseline as usize + br.read(e.nb_bits as u32) as usize;
            let e = oft.entries[os];
            os = e.baseline as usize + br.read(e.nb_bits as u32) as usize;
        }
    }
    if !br.is_exhausted() {
        return Err(Error::BadValue("zstd sequence stream not consumed exactly"));
    }
    carry.rep = rep;
    append_literals(
        out,
        &literals[lit_pos..],
        block_start,
        frame.block_max,
        limits,
    )
}

fn append_literals(
    out: &mut Vec<u8>,
    lits: &[u8],
    block_start: usize,
    block_max: usize,
    limits: Limits,
) -> Result<()> {
    if out.len() - block_start + lits.len() > block_max {
        return Err(Error::BadValue(
            "zstd block regenerates more than the block allows",
        ));
    }
    if out.len() + lits.len() > limits.max_output {
        return Err(Error::too_large("zstd output", limits.max_output));
    }
    out.extend_from_slice(lits);
    Ok(())
}
