//! Minimal standard ZIP archive writer (PKZIP APPNOTE.TXT).
//!
//! Supports writing uncompressed (method 0, stored) entries and building
//! valid central directory and EOCD structures. Fully round-trips with [`crate::ZipArchive`].

use alloc::string::String;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result, crc32};

struct EntryRecord {
    name: String,
    crc: u32,
    size: u32,
    local_header_offset: u32,
}

/// A sequential ZIP archive writer.
pub struct ZipWriter {
    data: Vec<u8>,
    entries: Vec<EntryRecord>,
}

impl Default for ZipWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipWriter {
    /// Creates an empty ZIP writer.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Writes one file entry with uncompressed (stored, method 0) bytes.
    pub fn write_file(&mut self, name: &str, content: &[u8]) -> Result<()> {
        if name.len() > u16::MAX as usize {
            return Err(Error::BadValue("ZIP entry name too long"));
        }
        if content.len() > u32::MAX as usize {
            return Err(Error::too_large("ZIP entry payload", u32::MAX as usize));
        }

        let local_header_offset = self.data.len() as u32;
        let crc = crc32(content);
        let size = content.len() as u32;

        // Local file header signature: 0x04034b50
        self.data.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        // Version needed to extract: 20 (2.0)
        self.data.extend_from_slice(&20u16.to_le_bytes());
        // General purpose bit flag: 0
        self.data.extend_from_slice(&0u16.to_le_bytes());
        // Compression method: 0 (stored)
        self.data.extend_from_slice(&0u16.to_le_bytes());
        // File mod time & date: 0
        self.data.extend_from_slice(&0u16.to_le_bytes());
        self.data.extend_from_slice(&0u16.to_le_bytes());
        // CRC-32
        self.data.extend_from_slice(&crc.to_le_bytes());
        // Compressed size
        self.data.extend_from_slice(&size.to_le_bytes());
        // Uncompressed size
        self.data.extend_from_slice(&size.to_le_bytes());
        // File name length
        self.data.extend_from_slice(&(name.len() as u16).to_le_bytes());
        // Extra field length: 0
        self.data.extend_from_slice(&0u16.to_le_bytes());
        // File name
        self.data.extend_from_slice(name.as_bytes());
        // File data
        self.data.extend_from_slice(content);

        self.entries.push(EntryRecord {
            name: String::from(name),
            crc,
            size,
            local_header_offset,
        });

        Ok(())
    }

    /// Finalizes the ZIP archive: writes Central Directory and EOCD, returning raw ZIP bytes.
    pub fn finish(mut self) -> Result<Vec<u8>> {
        let cd_offset = self.data.len() as u32;
        let mut cd_size = 0u32;

        for entry in &self.entries {
            let cd_start = self.data.len();
            // Central directory header signature: 0x02014b50
            self.data.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            // Version made by: 20
            self.data.extend_from_slice(&20u16.to_le_bytes());
            // Version needed to extract: 20
            self.data.extend_from_slice(&20u16.to_le_bytes());
            // General purpose bit flag: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // Compression method: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // File mod time & date: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // CRC-32
            self.data.extend_from_slice(&entry.crc.to_le_bytes());
            // Compressed size
            self.data.extend_from_slice(&entry.size.to_le_bytes());
            // Uncompressed size
            self.data.extend_from_slice(&entry.size.to_le_bytes());
            // File name length
            self.data
                .extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            // Extra field length: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // Comment length: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // Disk number start: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // Internal file attributes: 0
            self.data.extend_from_slice(&0u16.to_le_bytes());
            // External file attributes: 0
            self.data.extend_from_slice(&0u32.to_le_bytes());
            // Relative offset of local header
            self.data
                .extend_from_slice(&entry.local_header_offset.to_le_bytes());
            // File name
            self.data.extend_from_slice(entry.name.as_bytes());

            cd_size += (self.data.len() - cd_start) as u32;
        }

        // End of central directory record signature: 0x06054b50
        self.data.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        // Disk number: 0
        self.data.extend_from_slice(&0u16.to_le_bytes());
        // Disk where central directory starts: 0
        self.data.extend_from_slice(&0u16.to_le_bytes());
        // Number of central directory records on this disk
        self.data
            .extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        // Total number of central directory records
        self.data
            .extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        // Size of central directory
        self.data.extend_from_slice(&cd_size.to_le_bytes());
        // Offset of central directory
        self.data.extend_from_slice(&cd_offset.to_le_bytes());
        // Comment length: 0
        self.data.extend_from_slice(&0u16.to_le_bytes());

        Ok(self.data)
    }
}
