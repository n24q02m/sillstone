//! Test for FLAC Vorbis comment and Picture parsing.

use sillstone_flac::metadata::{parse_picture, parse_vorbis_comment};

#[test]
fn test_vorbis_comment_parsing() {
    let mut data = Vec::new();
    let vendor = "reference libFLAC 1.4.3";
    data.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    data.extend_from_slice(vendor.as_bytes());

    let comments = ["TITLE=Track Title", "ARTIST=Artist Name", "ALBUM=Album Name"];
    data.extend_from_slice(&(comments.len() as u32).to_le_bytes());
    for c in comments {
        data.extend_from_slice(&(c.len() as u32).to_le_bytes());
        data.extend_from_slice(c.as_bytes());
    }

    let vc = parse_vorbis_comment(&data).expect("parse vorbis comment");
    assert_eq!(vc.vendor, vendor);
    assert_eq!(vc.comments.len(), 3);
    assert_eq!(vc.comments[0], ("TITLE".into(), "Track Title".into()));
    assert_eq!(vc.comments[1], ("ARTIST".into(), "Artist Name".into()));
    assert_eq!(vc.comments[2], ("ALBUM".into(), "Album Name".into()));
}

#[test]
fn test_picture_parsing() {
    let mut data = Vec::new();
    data.extend_from_slice(&3u32.to_be_bytes()); // cover front
    let mime = "image/png";
    data.extend_from_slice(&(mime.len() as u32).to_be_bytes());
    data.extend_from_slice(mime.as_bytes());

    let desc = "Front cover art";
    data.extend_from_slice(&(desc.len() as u32).to_be_bytes());
    data.extend_from_slice(desc.as_bytes());

    data.extend_from_slice(&640u32.to_be_bytes()); // width
    data.extend_from_slice(&480u32.to_be_bytes()); // height
    data.extend_from_slice(&24u32.to_be_bytes()); // depth
    data.extend_from_slice(&0u32.to_be_bytes()); // colors

    let raw_pic = vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4];
    data.extend_from_slice(&(raw_pic.len() as u32).to_be_bytes());
    data.extend_from_slice(&raw_pic);

    let pic = parse_picture(&data).expect("parse picture");
    assert_eq!(pic.picture_type, 3);
    assert_eq!(pic.mime_type, "image/png");
    assert_eq!(pic.description, "Front cover art");
    assert_eq!(pic.width, 640);
    assert_eq!(pic.height, 480);
    assert_eq!(pic.depth, 24);
    assert_eq!(pic.data, raw_pic);
}
