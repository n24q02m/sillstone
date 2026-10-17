//! TAR archive parsing with ustar, PAX, and GNU long-name support,
//! plus security enforcement against path traversal (`..` or absolute paths).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

const BLOCK_SIZE: usize = 512;

/// A parsed TAR entry header and location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TarEntry {
    /// Normalized path name of the entry.
    pub name: String,
    /// Size of the entry content in bytes.
    pub size: usize,
    /// Typeflag byte ('0' = normal, '5' = directory, '2' = symlink, etc.).
    pub typeflag: u8,
    /// Byte offset in the TAR slice where the file content begins.
    pub data_offset: usize,
}

/// Checks whether a path contains directory traversal sequences or absolute references.
pub fn is_path_traversal(path: &str) -> bool {
    let p = path.replace('\\', "/");
    if p.starts_with('/') {
        return true;
    }
    for comp in p.split('/') {
        if comp == ".." {
            return true;
        }
    }
    false
}

fn parse_octal(bytes: &[u8]) -> Option<usize> {
    // Strip trailing nulls and spaces
    let mut end = bytes.len();
    while end > 0 && (bytes[end - 1] == 0 || bytes[end - 1] == b' ') {
        end -= 1;
    }
    let mut start = 0;
    while start < end && bytes[start] == b' ' {
        start += 1;
    }
    if start >= end {
        return Some(0);
    }

    // GNU binary extension: if high bit of first byte is set
    if (bytes[0] & 0x80) != 0 {
        let mut val = (bytes[0] & 0x7F) as usize;
        for &b in &bytes[1..end] {
            val = (val << 8) | (b as usize);
        }
        return Some(val);
    }

    let s = core::str::from_utf8(&bytes[start..end]).ok()?;
    usize::from_str_radix(s.trim(), 8).ok()
}

fn verify_checksum(header: &[u8]) -> bool {
    if header.len() < BLOCK_SIZE {
        return false;
    }
    let chksum_field = &header[148..156];
    let Some(expected) = parse_octal(chksum_field) else {
        return false;
    };

    let mut unsigned_sum: usize = 0;
    let mut signed_sum: isize = 0;

    for (i, &b) in header[..BLOCK_SIZE].iter().enumerate() {
        let byte_val = if (148..156).contains(&i) {
            b' '
        } else {
            b
        };
        unsigned_sum += byte_val as usize;
        signed_sum += byte_val as i8 as isize;
    }

    unsigned_sum == expected || (signed_sum >= 0 && signed_sum as usize == expected)
}

fn parse_c_string(bytes: &[u8]) -> &str {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..len]).unwrap_or("")
}

/// Parses all file entries from a TAR archive byte slice.
///
/// If `allow_escape` is false, paths with `..` or leading `/` are rejected with `Error::BadValue`.
pub fn entries(data: &[u8], allow_escape: bool) -> Result<Vec<TarEntry>> {
    let mut result = Vec::new();
    let mut pos = 0;
    let mut next_long_name: Option<String> = None;

    while pos + BLOCK_SIZE <= data.len() {
        let header = &data[pos..pos + BLOCK_SIZE];

        // Check for two consecutive zero blocks (EOF)
        if header.iter().all(|&b| b == 0) {
            let next_pos = pos + BLOCK_SIZE;
            if next_pos + BLOCK_SIZE <= data.len() && data[next_pos..next_pos + BLOCK_SIZE].iter().all(|&b| b == 0) {
                break;
            }
            pos += BLOCK_SIZE;
            continue;
        }

        if !verify_checksum(header) {
            return Err(Error::BadValue("invalid tar header checksum"));
        }

        let raw_name = parse_c_string(&header[0..100]);
        let size = parse_octal(&header[124..136])
            .ok_or(Error::BadValue("invalid tar entry size"))?;
        let typeflag = header[156];

        let prefix = if &header[257..262] == b"ustar" {
            parse_c_string(&header[345..500])
        } else {
            ""
        };

        let data_offset = pos + BLOCK_SIZE;
        let content_blocks = size.div_ceil(BLOCK_SIZE);
        let next_pos = data_offset
            .checked_add(content_blocks * BLOCK_SIZE)
            .ok_or(Error::BadValue("tar archive offset overflow"))?;

        if data_offset + size > data.len() {
            return Err(Error::truncated("tar entry data", data_offset + size, data.len()));
        }

        match typeflag {
            // GNU long name
            b'L' => {
                let name_bytes = &data[data_offset..data_offset + size];
                let long_name = parse_c_string(name_bytes);
                next_long_name = Some(long_name.to_string());
            }
            // PAX extended header
            b'x' | b'g' => {
                let pax_data = &data[data_offset..data_offset + size];
                if let Ok(pax_str) = core::str::from_utf8(pax_data) {
                    for line in pax_str.lines() {
                        if let Some(space_idx) = line.find(' ') {
                            let rec = &line[space_idx + 1..];
                            if let Some(eq_idx) = rec.find('=') {
                                let key = &rec[..eq_idx];
                                let val = &rec[eq_idx + 1..];
                                if key == "path" {
                                    next_long_name = Some(val.to_string());
                                }
                            }
                        }
                    }
                }
            }
            // Regular file or directory or symlink
            _ => {
                let mut name = if let Some(n) = next_long_name.take() {
                    n
                } else if !prefix.is_empty() {
                    format!("{prefix}/{raw_name}")
                } else {
                    raw_name.to_string()
                };

                // Normalize Windows backslashes
                name = name.replace('\\', "/");

                if !allow_escape && is_path_traversal(&name) {
                    return Err(Error::BadValue("tar path traversal rejected"));
                }

                result.push(TarEntry {
                    name,
                    size,
                    typeflag,
                    data_offset,
                });
            }
        }

        pos = next_pos;
    }

    Ok(result)
}

/// Reads the raw content bytes for a given TAR entry.
pub fn read<'a>(data: &'a [u8], entry: &TarEntry) -> Result<&'a [u8]> {
    let end = entry
        .data_offset
        .checked_add(entry.size)
        .ok_or(Error::BadValue("tar entry size overflow"))?;
    data.get(entry.data_offset..end)
        .ok_or_else(|| Error::truncated("tar entry data", end, data.len()))
}
