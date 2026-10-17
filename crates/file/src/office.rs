//! Office Open XML text and cell extraction for DOCX and XLSX documents.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};
use sillstone_zip::{XmlEvent, XmlReader, ZipArchive};

/// Extracts plain text from a WordprocessingML (`.docx`) file.
///
/// Reads `word/document.xml` from the ZIP container and extracts all text runs
/// (`<w:t>`), separating paragraphs (`<w:p>`) with newlines.
pub fn docx_text(bytes: &[u8]) -> Result<String> {
    let archive = ZipArchive::new(bytes)?;
    let doc_xml_bytes = archive.extract_by_name("word/document.xml")?;
    let doc_xml_str = core::str::from_utf8(&doc_xml_bytes)
        .map_err(|_| Error::BadValue("word/document.xml is not valid UTF-8"))?;

    let mut reader = XmlReader::new(doc_xml_str);
    let mut out = String::new();
    let mut inside_t = false;

    while let Some(event) = reader.next_event()? {
        match event {
            XmlEvent::Start { name, .. } => {
                if name == "w:t" {
                    inside_t = true;
                }
            }
            XmlEvent::End { name } => {
                if name == "w:t" {
                    inside_t = false;
                } else if name == "w:p" && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            XmlEvent::Text(txt) => {
                if inside_t {
                    out.push_str(&txt);
                }
            }
        }
    }

    Ok(out)
}

/// Extracts all cell text values from an SpreadsheetML (`.xlsx`) workbook.
///
/// Resolves shared strings from `xl/sharedStrings.xml` and reads cell records
/// across all worksheets (`xl/worksheets/sheet*.xml`).
pub fn xlsx_cells(bytes: &[u8]) -> Result<Vec<String>> {
    let archive = ZipArchive::new(bytes)?;

    // 1. Read shared strings if present
    let mut shared_strings = Vec::new();
    if let Ok(ss_bytes) = archive.extract_by_name("xl/sharedStrings.xml") {
        if let Ok(ss_str) = core::str::from_utf8(&ss_bytes) {
            let mut reader = XmlReader::new(ss_str);
            let mut inside_t = false;
            let mut current_str = String::new();

            while let Some(event) = reader.next_event()? {
                match event {
                    XmlEvent::Start { name, .. } => {
                        if name == "t" {
                            inside_t = true;
                        }
                    }
                    XmlEvent::End { name } => {
                        if name == "t" {
                            inside_t = false;
                        } else if name == "si" {
                            shared_strings.push(core::mem::take(&mut current_str));
                        }
                    }
                    XmlEvent::Text(txt) => {
                        if inside_t {
                            current_str.push_str(&txt);
                        }
                    }
                }
            }
        }
    }

    // 2. Iterate through worksheets in order
    let mut cells = Vec::new();
    for entry in archive.entries() {
        let name = entry.name();
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            let sheet_bytes = archive.extract(entry)?;
            let Ok(sheet_str) = core::str::from_utf8(&sheet_bytes) else {
                continue;
            };

            let mut reader = XmlReader::new(sheet_str);
            let mut is_shared_string = false;
            let mut inside_v = false;
            let mut inside_inline_t = false;
            let mut val_str = String::new();

            while let Some(event) = reader.next_event()? {
                match event {
                    XmlEvent::Start { name, attrs } => {
                        if name == "c" {
                            is_shared_string = attrs.iter().any(|(k, v)| *k == "t" && v == "s");
                        } else if name == "v" {
                            inside_v = true;
                            val_str.clear();
                        } else if name == "t" {
                            inside_inline_t = true;
                        }
                    }
                    XmlEvent::End { name } => {
                        if name == "v" {
                            inside_v = false;
                            if is_shared_string {
                                if let Ok(idx) = val_str.trim().parse::<usize>() {
                                    if let Some(ss) = shared_strings.get(idx) {
                                        cells.push(ss.clone());
                                    }
                                }
                            } else if !val_str.is_empty() {
                                cells.push(core::mem::take(&mut val_str));
                            }
                        } else if name == "t" {
                            inside_inline_t = false;
                        }
                    }
                    XmlEvent::Text(txt) => {
                        if inside_v {
                            val_str.push_str(&txt);
                        } else if inside_inline_t {
                            cells.push(txt.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(cells)
}

/// Formats the extracted cell values of an `.xlsx` workbook as plain text.
pub fn xlsx_text(bytes: &[u8]) -> Result<String> {
    let cells = xlsx_cells(bytes)?;
    Ok(cells.join(" "))
}
