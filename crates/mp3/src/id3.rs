//! ID3 metadata parsing for MP3 streams: ID3v1 and ID3v2 (v2.3 and v2.4).
//!
//! Provides zero-copy / zero-dep extraction of text tags (TIT2, TPE1, TALB, etc.) and APIC pictures.

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

fn clean_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s = core::str::from_utf8(&bytes[..end]).unwrap_or("");
    String::from(s.trim())
}

/// Parses an ID3v2 tag from the beginning of `data`, if present.
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

    let mut pos = 10usize;
    // Skip extended header if flag bit 6 (0x40) is set
    if (flags & 0x40) != 0 {
        if pos + 4 > 10 + tag_size {
            return Err(Error::truncated("ID3v2 extended header", pos + 4, data.len()));
        }
        let ext_size = if major == 3 {
            u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize + 4
        } else {
            parse_synchsafe(&data[pos..pos + 4])? as usize
        };
        pos += ext_size;
    }

    let mut text_frames = Vec::new();
    let mut pictures = Vec::new();
    let end = 10 + tag_size;

    while pos + 10 <= end {
        // Frame ID (4 bytes)
        let id_bytes = &data[pos..pos + 4];
        if id_bytes[0] == 0 {
            break; // padding reached
        }
        let id = match core::str::from_utf8(id_bytes) {
            Ok(s) => s,
            Err(_) => break,
        };

        let frame_size = if major == 4 {
            parse_synchsafe(&data[pos + 4..pos + 8])? as usize
        } else {
            u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize
        };

        pos += 10;
        if pos + frame_size > end {
            break;
        }

        let body = &data[pos..pos + frame_size];
        pos += frame_size;

        if id.starts_with('T') && id != "TXXX" {
            if let Some(text) = decode_text_frame(body) {
                text_frames.push((String::from(id), text));
            }
        } else if id == "APIC" {
            if let Some(pic) = decode_apic_frame(body) {
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
    if body.is_empty() {
        return None;
    }
    let encoding = body[0];
    let payload = &body[1..];
    match encoding {
        0 => {
            // ISO-8859-1 (Latin-1)
            let s: String = payload.iter().map(|&b| b as char).collect();
            Some(String::from(s.trim_end_matches('\0')))
        }
        1 => {
            // UTF-16 with BOM
            decode_utf16_bom(payload)
        }
        2 => {
            // UTF-16BE without BOM
            decode_utf16_be(payload)
        }
        3 => {
            // UTF-8
            core::str::from_utf8(payload)
                .ok()
                .map(|s| String::from(s.trim_end_matches('\0')))
        }
        _ => None,
    }
}

fn decode_utf16_bom(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 {
        return None;
    }
    let be = match (bytes[0], bytes[1]) {
        (0xFE, 0xFF) => true,
        (0xFF, 0xFE) => false,
        _ => true,
    };
    let payload = &bytes[2..];
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
    let _encoding = body[0];
    let mut pos = 1usize;

    // MIME type (terminated by 0x00)
    let mime_end = body[pos..].iter().position(|&b| b == 0)?;
    let mime_type = core::str::from_utf8(&body[pos..pos + mime_end]).ok()?;
    pos += mime_end + 1;

    if pos >= body.len() {
        return None;
    }
    let picture_type = body[pos];
    pos += 1;

    // Description (terminated by null)
    let desc_end = body[pos..].iter().position(|&b| b == 0)?;
    let description = core::str::from_utf8(&body[pos..pos + desc_end]).unwrap_or("");
    pos += desc_end + 1;

    let data = body[pos..].to_vec();

    Some(Id3Picture {
        picture_type,
        mime_type: String::from(mime_type),
        description: String::from(description),
        data,
    })
}
