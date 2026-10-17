//! FLAC metadata blocks: VORBIS_COMMENT (RFC 9639 §8.5) and PICTURE (RFC 9639 §8.7).

use alloc::string::String;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// A parsed Vorbis comment metadata block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VorbisComment {
    /// Vendor string.
    pub vendor: String,
    /// Key-value comment pairs, in order.
    pub comments: Vec<(String, String)>,
}

/// A parsed Picture metadata block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// Picture type (e.g. 3 for cover front).
    pub picture_type: u32,
    /// MIME type string (e.g. "image/jpeg" or "image/png").
    pub mime_type: String,
    /// Description string.
    pub description: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Color depth in bits per pixel.
    pub depth: u32,
    /// Number of colors used (0 for non-indexed).
    pub colors: u32,
    /// Raw picture data bytes.
    pub data: Vec<u8>,
}

/// Parses a VORBIS_COMMENT metadata body.
pub fn parse_vorbis_comment(data: &[u8]) -> Result<VorbisComment> {
    if data.len() < 4 {
        return Err(Error::truncated("Vorbis comment vendor length", 4, data.len()));
    }
    let vendor_len = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    if data.len() < 4 + vendor_len + 4 {
        return Err(Error::truncated("Vorbis comment vendor string", 4 + vendor_len + 4, data.len()));
    }
    let vendor = core::str::from_utf8(&data[4..4 + vendor_len])
        .map_err(|_| Error::BadValue("invalid UTF-8 in Vorbis vendor string"))?;
    let mut pos = 4 + vendor_len;

    let count = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let mut comments = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        if pos + 4 > data.len() {
            return Err(Error::truncated("Vorbis comment length", pos + 4, data.len()));
        }
        let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + len > data.len() {
            return Err(Error::truncated("Vorbis comment body", pos + len, data.len()));
        }
        let comment_str = core::str::from_utf8(&data[pos..pos + len])
            .map_err(|_| Error::BadValue("invalid UTF-8 in Vorbis comment"))?;
        pos += len;

        if let Some((k, v)) = comment_str.split_once('=') {
            comments.push((String::from(k), String::from(v)));
        } else {
            comments.push((String::from(comment_str), String::new()));
        }
    }

    Ok(VorbisComment {
        vendor: String::from(vendor),
        comments,
    })
}

/// Parses a PICTURE metadata body.
pub fn parse_picture(data: &[u8]) -> Result<Picture> {
    if data.len() < 32 {
        return Err(Error::truncated("FLAC picture header", 32, data.len()));
    }
    let mut pos = 0;
    let picture_type = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    pos += 4;

    let mime_len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + mime_len > data.len() {
        return Err(Error::truncated("FLAC picture MIME", pos + mime_len, data.len()));
    }
    let mime_type = core::str::from_utf8(&data[pos..pos + mime_len])
        .map_err(|_| Error::BadValue("invalid ASCII/UTF-8 in FLAC picture MIME"))?;
    pos += mime_len;

    if pos + 4 > data.len() {
        return Err(Error::truncated("FLAC picture description length", pos + 4, data.len()));
    }
    let desc_len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + desc_len > data.len() {
        return Err(Error::truncated("FLAC picture description", pos + desc_len, data.len()));
    }
    let description = core::str::from_utf8(&data[pos..pos + desc_len])
        .map_err(|_| Error::BadValue("invalid UTF-8 in FLAC picture description"))?;
    pos += desc_len;

    if pos + 20 > data.len() {
        return Err(Error::truncated("FLAC picture geometry", pos + 20, data.len()));
    }
    let width = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
    let height = u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap());
    let depth = u32::from_be_bytes(data[pos + 8..pos + 12].try_into().unwrap());
    let colors = u32::from_be_bytes(data[pos + 12..pos + 16].try_into().unwrap());
    let data_len = u32::from_be_bytes(data[pos + 16..pos + 20].try_into().unwrap()) as usize;
    pos += 20;

    if pos + data_len > data.len() {
        return Err(Error::truncated("FLAC picture data", pos + data_len, data.len()));
    }
    let pic_data = data[pos..pos + data_len].to_vec();

    Ok(Picture {
        picture_type,
        mime_type: String::from(mime_type),
        description: String::from(description),
        width,
        height,
        depth,
        colors,
        data: pic_data,
    })
}
