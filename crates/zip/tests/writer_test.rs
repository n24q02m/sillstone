//! Test ZipWriter creating archives that ZipArchive reads.

use sillstone_zip::{ZipArchive, ZipWriter};

#[test]
fn test_zip_writer_roundtrip() {
    let mut writer = ZipWriter::new();
    writer
        .write_file("file1.txt", b"Hello from file1!")
        .expect("write file1");
    writer
        .write_file("folder/file2.bin", &[1, 2, 3, 4, 5, 42, 255])
        .expect("write file2");
    writer.write_file("empty.txt", b"").expect("write empty");

    let zip_bytes = writer.finish().expect("finish zip");
    assert!(zip_bytes.len() > 100);

    let archive = ZipArchive::new(&zip_bytes).expect("parse zip archive");
    let entries = archive.entries();
    assert_eq!(entries.len(), 3);

    assert_eq!(entries[0].name(), "file1.txt");
    assert_eq!(entries[0].uncompressed_size(), 17);
    assert_eq!(
        archive.extract(&entries[0]).unwrap().as_slice(),
        b"Hello from file1!"
    );

    assert_eq!(entries[1].name(), "folder/file2.bin");
    assert_eq!(entries[1].uncompressed_size(), 7);
    assert_eq!(
        archive.extract(&entries[1]).unwrap().as_slice(),
        &[1, 2, 3, 4, 5, 42, 255]
    );

    assert_eq!(entries[2].name(), "empty.txt");
    assert_eq!(entries[2].uncompressed_size(), 0);
    assert_eq!(archive.extract(&entries[2]).unwrap().as_slice(), b"");
}
