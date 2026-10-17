//! Tests for TAR parsing, path traversal rejection, and DOCX/XLSX text extraction.

use sillstone_file::{docx_text, is_path_traversal, tar_entries, tar_read, xlsx_cells, xlsx_text};
use sillstone_zip::ZipWriter;

fn build_tar_archive(entries: &[(&str, &[u8], u8)]) -> Vec<u8> {
    let mut out = Vec::new();
    for &(name, content, typeflag) in entries {
        let mut header = [0u8; 512];
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // mode, uid, gid
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0001000\0");
        header[116..124].copy_from_slice(b"0001000\0");

        // size in octal (11 octal digits + null/space)
        let size_str = format!("{:011o} ", content.len());
        header[124..136].copy_from_slice(size_str.as_bytes());

        // mtime
        header[136..148].copy_from_slice(b"00000000000 ");

        // typeflag
        header[156] = typeflag;

        // magic and version (ustar)
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // checksum calculation: 8 spaces at 148..156
        for b in &mut header[148..156] {
            *b = b' ';
        }
        let chksum: usize = header.iter().map(|&b| b as usize).sum();
        let chksum_str = format!("{:06o}\0 ", chksum);
        header[148..156].copy_from_slice(chksum_str.as_bytes());

        out.extend_from_slice(&header);

        // Content plus padding to 512-byte block
        out.extend_from_slice(content);
        let remainder = content.len() % 512;
        if remainder != 0 {
            out.extend_from_slice(&vec![0u8; 512 - remainder]);
        }
    }

    // Two 512-byte zero blocks at EOF
    out.extend_from_slice(&[0u8; 1024]);
    out
}

#[test]
fn test_tar_path_traversal_detection() {
    assert!(is_path_traversal("../etc/passwd"));
    assert!(is_path_traversal("a/b/../../c"));
    assert!(is_path_traversal("/root/secret"));
    assert!(is_path_traversal("\\windows\\system32"));
    assert!(!is_path_traversal("valid/sub/path.txt"));
    assert!(!is_path_traversal("simple.txt"));
}

#[test]
fn test_tar_basic_read_and_security() {
    let content_a = b"Hello from file A";
    let content_b = b"File B payload";
    let tar = build_tar_archive(&[
        ("hello.txt", content_a, b'0'),
        ("nested/sub.txt", content_b, b'0'),
    ]);

    let parsed = tar_entries(&tar, false).expect("valid tar parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "hello.txt");
    assert_eq!(parsed[0].size, content_a.len());
    assert_eq!(tar_read(&tar, &parsed[0]).unwrap(), content_a);

    assert_eq!(parsed[1].name, "nested/sub.txt");
    assert_eq!(tar_read(&tar, &parsed[1]).unwrap(), content_b);

    // Traversal rejection test
    let bad_tar = build_tar_archive(&[
        ("../escaped.txt", b"malicious", b'0'),
    ]);
    assert!(
        tar_entries(&bad_tar, false).is_err(),
        "Must reject path traversal when allow_escape is false"
    );
    assert!(
        tar_entries(&bad_tar, true).is_ok(),
        "Must allow path traversal when allow_escape is true"
    );
}

#[test]
fn test_office_docx_extraction() {
    let mut writer = ZipWriter::new();
    let doc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p>
            <w:r>
                <w:t>Hello world from docx!</w:t>
            </w:r>
        </w:p>
        <w:p>
            <w:r>
                <w:t>Second paragraph.</w:t>
            </w:r>
        </w:p>
    </w:body>
</w:document>"#;

    writer.write_file("word/document.xml", doc_xml).unwrap();
    let zip_bytes = writer.finish().unwrap();

    let text = docx_text(&zip_bytes).expect("docx text extraction");
    assert!(text.contains("Hello world from docx!"));
    assert!(text.contains("Second paragraph."));
}

#[test]
fn test_office_xlsx_extraction() {
    let mut writer = ZipWriter::new();
    let shared_strings_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <si><t>Revenue</t></si>
    <si><t>Profit</t></si>
</sst>"#;

    let sheet_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <sheetData>
        <row>
            <c t="s"><v>0</v></c>
            <c><v>1000</v></c>
        </row>
        <row>
            <c t="s"><v>1</v></c>
            <c><v>250</v></c>
        </row>
    </sheetData>
</worksheet>"#;

    writer.write_file("xl/sharedStrings.xml", shared_strings_xml).unwrap();
    writer.write_file("xl/worksheets/sheet1.xml", sheet_xml).unwrap();
    let zip_bytes = writer.finish().unwrap();

    let cells = xlsx_cells(&zip_bytes).expect("xlsx cells extraction");
    assert_eq!(cells, vec!["Revenue", "1000", "Profit", "250"]);

    let text = xlsx_text(&zip_bytes).expect("xlsx text");
    assert_eq!(text, "Revenue 1000 Profit 250");
}
