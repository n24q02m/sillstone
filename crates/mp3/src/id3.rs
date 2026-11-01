//! ID3 metadata parsing for MP3 streams: ID3v1 and ID3v2 (v2.3 and v2.4).
//!
//! Provides zero-copy / zero-dep extraction of text tags (TIT2, TPE1, TALB, etc.) and APIC pictures.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// An ID3v1 tag extracted from the final 128 bytes of an MP3 file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Id3v1 {
    /// Title (up to 30 chars).
    pub title: String,
    /// Artist (up to 30 chars).
    pub artist: String,
    /// Album (up to 30 chars).
    pub album: String,
    /// Year (4 chars).
    pub year: String,
    /// Comment (up to 30 chars).
    pub comment: String,
    /// Track number if present.
    pub track: Option<u8>,
    /// Genre byte code.
    pub genre: u8,
}

/// An ID3v2 tag (v2.3 or v2.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Id3v2 {
    /// Major version (3 for v2.3, 4 for v2.4).
    pub major: u8,
    /// Minor/revision version.
    pub revision: u8,
    /// Text frames as (ID, string value) pairs, e.g. ("TIT2", "Song Title").
    pub text_frames: Vec<(String, String)>,
    /// Attached pictures.
    pub pictures: Vec<Id3Picture>,
}

/// An ID3v2 APIC attached picture frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Id3Picture {
    /// Picture type (e.g. 3 for cover front).
    pub picture_type: u8,
    /// MIME type string.
    pub mime_type: String,
    /// Description string.
    pub description: String,
    /// Raw image data bytes.
    pub data: Vec<u8>,
}

/// Parses an ID3v1 tag from the trailing 128 bytes of a file.
pub fn parse_id3v1(data: &[u8]) -> Option<Id3v1> {
    if data.len() < 128 {
        return None;
    }
    let tail = &data[data.len() - 128..];
    if &tail[0..3] != b"TAG" {
        return None;
    }

    let title = clean_str(&tail[3..33]);
    let artist = clean_str(&tail[33..63]);
    let album = clean_str(&tail[63..93]);
    let year = clean_str(&tail[93..97]);

    let (comment, track) = if tail[125] == 0 && tail[126] != 0 {
        (clean_str(&tail[97..125]), Some(tail[126]))
    } else {
        (clean_str(&tail[97..127]), None)
    };

    let genre = tail[127];

    Some(Id3v1 {
        title,
        artist,
        album,
        year,
        comment,
        track,
        genre,
    })
}

/// Decodes an ID3v1 text field. The spec says ISO-8859-1, but plenty of
/// taggers write UTF-8, so valid UTF-8 is kept and anything else is read as
/// Latin-1 (every byte is a code point) instead of being dropped.
fn clean_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let field = &bytes[..end];
    match core::str::from_utf8(field) {
        Ok(s) => String::from(s.trim()),
        Err(_) => String::from(latin1(field).trim()),
    }
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Parses an ID3v2 tag from the beginning of `data`, if present.
///
/// Handles tag-level unsynchronisation (v2.3), per-frame unsynchronisation,
/// grouping and data-length-indicator bytes (v2.4), and skips frames that are
/// compressed or encrypted, since their payload cannot be read here.
pub fn parse_id3v2(data: &[u8]) -> Result<Option<Id3v2>> {
    if data.len() < 10 || &data[0..3] != b"ID3" {
        return Ok(None);
    }

    let major = data[3];
    let revision = data[4];
    let flags = data[5];
    let tag_size = parse_synchsafe(&data[6..10])? as usize;

    if data.len() < 10 + tag_size {
        return Err(Error::truncated("ID3v2 tag payload", 10 + tag_size, data.len()));
    }

    let raw = &data[10..10 + tag_size];
    let undone;
    let tag: &[u8] = if major == 3 && (flags & 0x80) != 0 {
        undone = undo_unsync(raw);
        &undone
    } else {
        raw
    };
    let end = tag.len();

    let mut pos = 0usize;
    // Skip extended header if flag bit 6 (0x40) is set
    if (flags & 0x40) != 0 {
        if pos + 4 > end {
            return Err(Error::truncated("ID3v2 extended header", 14, data.len()));
        }
        let ext_size = if major == 3 {
            u32::from_be_bytes(tag[pos..pos + 4].try_into().unwrap()) as usize + 4
        } else {
            parse_synchsafe(&tag[pos..pos + 4])? as usize
        };
        pos = pos.saturating_add(ext_size);
    }

    let mut text_frames = Vec::new();
    let mut pictures = Vec::new();

    while pos + 10 <= end {
        // Frame ID (4 bytes)
        let id_bytes = &tag[pos..pos + 4];
        if id_bytes[0] == 0 {
            break; // padding reached
        }
        let id = match core::str::from_utf8(id_bytes) {
            Ok(s) => s,
            Err(_) => break,
        };

        let frame_size = if major == 4 {
            parse_synchsafe(&tag[pos + 4..pos + 8])? as usize
        } else {
            u32::from_be_bytes(tag[pos + 4..pos + 8].try_into().unwrap()) as usize
        };
        let format_flags = tag[pos + 9];

        pos += 10;
        if frame_size > end - pos {
            break;
        }

        let stored = &tag[pos..pos + frame_size];
        pos += frame_size;

        let Some(body) = frame_body(major, format_flags, stored) else {
            continue;
        };

        if id.starts_with('T') && id != "TXXX" {
            if let Some(text) = decode_text_frame(&body) {
                text_frames.push((String::from(id), text));
            }
        } else if id == "APIC" {
            if let Some(pic) = decode_apic_frame(&body) {
                pictures.push(pic);
            }
        }
    }

    Ok(Some(Id3v2 {
        major,
        revision,
        text_frames,
        pictures,
    }))
}

/// Reverses unsynchronisation: every `FF 00` pair stands for a single `FF`.
fn undo_unsync(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        out.push(bytes[i]);
        if bytes[i] == 0xFF && bytes.get(i + 1) == Some(&0) {
            i += 1;
        }
        i += 1;
    }
    out
}

/// Strips the per-frame wrappers the format flags announce and returns the
/// frame payload, or `None` when the frame is compressed or encrypted.
fn frame_body(major: u8, format_flags: u8, stored: &[u8]) -> Option<Cow<'_, [u8]>> {
    // (compressed, encrypted, grouped, unsynchronised, has data-length indicator)
    let (compressed, encrypted, grouped, unsync, length_indicator) = if major == 4 {
        (
            format_flags & 0x08 != 0,
            format_flags & 0x04 != 0,
            format_flags & 0x40 != 0,
            format_flags & 0x02 != 0,
            format_flags & 0x01 != 0,
        )
    } else {
        (
            format_flags & 0x80 != 0,
            format_flags & 0x40 != 0,
            format_flags & 0x20 != 0,
            false,
            false,
        )
    };
    if compressed || encrypted {
        return None;
    }
    let body: Cow<'_, [u8]> = if unsync {
        Cow::Owned(undo_unsync(stored))
    } else {
        Cow::Borrowed(stored)
    };
    let skip = usize::from(grouped) + if length_indicator { 4 } else { 0 };
    if body.len() < skip {
        return None;
    }
    Some(match body {
        Cow::Borrowed(b) => Cow::Borrowed(&b[skip..]),
        Cow::Owned(mut v) => {
            v.drain(..skip);
            Cow::Owned(v)
        }
    })
}

fn parse_synchsafe(bytes: &[u8]) -> Result<u32> {
    if bytes.len() < 4 {
        return Err(Error::BadValue("synchsafe integer too short"));
    }
    let mut val = 0u32;
    for &b in &bytes[..4] {
        if b & 0x80 != 0 {
            return Err(Error::BadValue("invalid synchsafe integer bit 7 set"));
        }
        val = (val << 7) | (b as u32);
    }
    Ok(val)
}

fn decode_text_frame(body: &[u8]) -> Option<String> {
    let (&encoding, payload) = body.split_first()?;
    decode_text(encoding, payload)
}

/// Decodes `payload` in the ID3v2 text encoding named by `encoding`
/// (0 Latin-1, 1 UTF-16 with BOM, 2 UTF-16BE, 3 UTF-8), dropping trailing NULs.
fn decode_text(encoding: u8, payload: &[u8]) -> Option<String> {
    match encoding {
        0 => Some(String::from(latin1(payload).trim_end_matches('\0'))),
        1 => decode_utf16_bom(payload),
        2 => decode_utf16_be(payload),
        3 => core::str::from_utf8(payload)
            .ok()
            .map(|s| String::from(s.trim_end_matches('\0'))),
        _ => None,
    }
}

fn decode_utf16_bom(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 {
        return None;
    }
    // Without a BOM (non-conforming) the bytes are read as big-endian in full
    // rather than losing their first code unit.
    let (be, payload) = match (bytes[0], bytes[1]) {
        (0xFE, 0xFF) => (true, &bytes[2..]),
        (0xFF, 0xFE) => (false, &bytes[2..]),
        _ => (true, bytes),
    };
    let u16s: Vec<u16> = payload
        .chunks_exact(2)
        .map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
        .collect();
    char::decode_utf16(u16s)
        .collect::<core::result::Result<String, _>>()
        .ok()
        .map(|s| String::from(s.trim_end_matches('\0')))
}

fn decode_utf16_be(bytes: &[u8]) -> Option<String> {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    char::decode_utf16(u16s)
        .collect::<core::result::Result<String, _>>()
        .ok()
        .map(|s| String::from(s.trim_end_matches('\0')))
}

fn decode_apic_frame(body: &[u8]) -> Option<Id3Picture> {
    if body.len() < 4 {
        return None;
    }
    let encoding = body[0];
    let mut pos = 1usize;

    // MIME type: Latin-1, terminated by a single 0x00 whatever the text encoding.
    let mime_end = body[pos..].iter().position(|&b| b == 0)?;
    let mime_type = core::str::from_utf8(&body[pos..pos + mime_end]).ok()?;
    pos += mime_end + 1;

    if pos >= body.len() {
        return None;
    }
    let picture_type = body[pos];
    pos += 1;

    // Description: terminated the way the text encoding says. UTF-16 strings
    // end in an aligned 00 00 pair, so a lone 0x00 inside them must not end it.
    let (desc_end, terminator) = if matches!(encoding, 1 | 2) {
        let mut i = pos;
        loop {
            if i + 1 >= body.len() {
                return None;
            }
            if body[i] == 0 && body[i + 1] == 0 {
                break (i, 2);
            }
            i += 2;
        }
    } else {
        (pos + body[pos..].iter().position(|&b| b == 0)?, 1)
    };
    let description = decode_text(encoding, &body[pos..desc_end]).unwrap_or_default();
    pos = desc_end + terminator;

    let data = body[pos..].to_vec();

    Some(Id3Picture {
        picture_type,
        mime_type: String::from(mime_type),
        description,
        data,
    })
}
