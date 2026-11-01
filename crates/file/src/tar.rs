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
    // A drive designator ("C:" or "C:foo") escapes the extraction root on Windows.
    if let [drive, b':', ..] = p.as_bytes() {
        if drive.is_ascii_alphabetic() {
            return true;
        }
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

    // GNU binary extension: if the high bit of the first byte is set the field
    // is a big-endian base-256 number. 0xFF marks a negative value, and a value
    // that does not fit `usize` is refused rather than wrapped.
    if (bytes[0] & 0x80) != 0 {
        if bytes[0] == 0xFF {
            return None;
        }
        let mut val = (bytes[0] & 0x7F) as usize;
        for &b in &bytes[1..] {
            val = val.checked_mul(256)?.checked_add(b as usize)?;
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

/// Extended-header overrides that apply to the next entry.
#[derive(Default)]
struct Pax {
    path: Option<String>,
    size: Option<usize>,
}

/// Reads PAX records (`"<len> key=value\n"`). The decimal length covers the whole
/// record, so a value may itself contain newlines. Parsing stops at the first record
/// that is malformed or not valid UTF-8; what was read before it still applies.
fn parse_pax(data: &[u8], out: &mut Pax) {
    let mut pos = 0;
    while pos < data.len() {
        let rest = &data[pos..];
        let Some(space) = rest.iter().position(|&b| b == b' ') else {
            return;
        };
        let Some(len) = core::str::from_utf8(&rest[..space])
            .ok()
            .and_then(|digits| digits.parse::<usize>().ok())
        else {
            return;
        };
        // The record must hold at least one byte after "<digits> " and fit the data.
        if len <= space + 1 || len > rest.len() {
            return;
        }
        let Ok(record) = core::str::from_utf8(&rest[space + 1..len]) else {
            return;
        };
        let record = record.strip_suffix('\n').unwrap_or(record);
        if let Some((key, value)) = record.split_once('=') {
            match key {
                "path" => out.path = Some(value.to_string()),
                "size" => {
                    if let Ok(n) = value.parse::<usize>() {
                        out.size = Some(n);
                    }
                }
                _ => {}
            }
        }
        pos += len;
    }
}

/// Parses all file entries from a TAR archive byte slice.
///
/// If `allow_escape` is false, paths with `..` or leading `/` are rejected with `Error::BadValue`.
pub fn entries(data: &[u8], allow_escape: bool) -> Result<Vec<TarEntry>> {
    let mut result = Vec::new();
    let mut pos = 0;
    let mut next_long_name: Option<String> = None;
    // `x` records override the next entry only; `g` records are defaults for every entry
    // after them, and a local `x` record wins over a global one (POSIX.1-2001 pax).
    let mut next_size: Option<usize> = None;
    let mut global_size: Option<usize> = None;

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
        let header_size = parse_octal(&header[124..136])
            .ok_or(Error::BadValue("invalid tar entry size"))?;
        let typeflag = header[156];
        // A PAX `size` replaces the header field: the local (`x`) value for the entry that
        // follows it, otherwise the global (`g`) value for every entry after the `g` header.
        let size = if matches!(typeflag, b'L' | b'K' | b'x' | b'g') {
            header_size
        } else {
            next_size.take().or(global_size).unwrap_or(header_size)
        };

        // The ustar `prefix` field exists only in POSIX headers ("ustar\0" + "00");
        // GNU headers ("ustar  \0") keep atime/ctime in those bytes.
        let prefix = if &header[257..263] == b"ustar\0" {
            parse_c_string(&header[345..500])
        } else {
            ""
        };

        let data_offset = pos + BLOCK_SIZE;
        let data_end = data_offset
            .checked_add(size)
            .ok_or(Error::BadValue("tar entry size overflow"))?;
        if data_end > data.len() {
            return Err(Error::truncated("tar entry data", data_end, data.len()));
        }
        let next_pos = size
            .div_ceil(BLOCK_SIZE)
            .checked_mul(BLOCK_SIZE)
            .and_then(|padded| data_offset.checked_add(padded))
            .ok_or(Error::BadValue("tar archive offset overflow"))?;

        match typeflag {
            // GNU long link target: not an entry, and nothing here uses link targets.
            b'K' => {}
            // GNU long name
            b'L' => {
                let name_bytes = &data[data_offset..data_end];
                let long_name = parse_c_string(name_bytes);
                next_long_name = Some(long_name.to_string());
            }
            // PAX extended header
            b'x' | b'g' => {
                let mut pax = Pax::default();
                parse_pax(&data[data_offset..data_end], &mut pax);
                if pax.path.is_some() {
                    next_long_name = pax.path;
                }
                if pax.size.is_some() {
                    if typeflag == b'g' {
                        global_size = pax.size;
                    } else {
                        next_size = pax.size;
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
