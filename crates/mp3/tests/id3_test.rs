//! Tests for ID3v1 and ID3v2 parsing in MP3.

use sillstone_mp3::{parse_id3v1, parse_id3v2};

#[test]
fn test_id3v1_parsing() {
    let mut data = vec![0u8; 1000]; // audio data
    let mut tag = [0u8; 128];
    tag[0..3].copy_from_slice(b"TAG");
    let title = b"Test Song Title";
    tag[3..3 + title.len()].copy_from_slice(title);
    let artist = b"Test Artist";
    tag[33..33 + artist.len()].copy_from_slice(artist);
    let album = b"Test Album";
    tag[63..63 + album.len()].copy_from_slice(album);
    let year = b"2026";
    tag[93..97].copy_from_slice(year);
    tag[125] = 0;
    tag[126] = 7; // track 7
    tag[127] = 12; // genre
    data.extend_from_slice(&tag);

    let id3 = parse_id3v1(&data).expect("parse id3v1");
    assert_eq!(id3.title, "Test Song Title");
    assert_eq!(id3.artist, "Test Artist");
    assert_eq!(id3.album, "Test Album");
    assert_eq!(id3.year, "2026");
    assert_eq!(id3.track, Some(7));
    assert_eq!(id3.genre, 12);
}

#[test]
fn test_id3v2_parsing() {
    let mut data = Vec::new();
    data.extend_from_slice(b"ID3");
    data.push(3); // v2.3
    data.push(0); // rev 0
    data.push(0); // flags

    // Build frames
    let mut frames = Vec::new();
    // TIT2 frame: Song Name
    frames.extend_from_slice(b"TIT2");
    let tit2_payload = b"\x03My Song Name"; // UTF-8
    frames.extend_from_slice(&(tit2_payload.len() as u32).to_be_bytes());
    frames.extend_from_slice(&[0, 0]); // flags
    frames.extend_from_slice(tit2_payload);

    // TPE1 frame: My Artist Name
    frames.extend_from_slice(b"TPE1");
    let tpe1_payload = b"\x03My Artist Name";
    frames.extend_from_slice(&(tpe1_payload.len() as u32).to_be_bytes());
    frames.extend_from_slice(&[0, 0]);
    frames.extend_from_slice(tpe1_payload);

    // APIC frame
    frames.extend_from_slice(b"APIC");
    let mut apic_payload = Vec::new();
    apic_payload.push(0); // latin-1
    apic_payload.extend_from_slice(b"image/jpeg\0");
    apic_payload.push(3); // cover front
    apic_payload.extend_from_slice(b"Cover\0");
    apic_payload.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]);
    frames.extend_from_slice(&(apic_payload.len() as u32).to_be_bytes());
    frames.extend_from_slice(&[0, 0]);
    frames.extend_from_slice(&apic_payload);

    // synchsafe size of frames
    let sz = frames.len() as u32;
    let ss = [
        ((sz >> 21) & 0x7F) as u8,
        ((sz >> 14) & 0x7F) as u8,
        ((sz >> 7) & 0x7F) as u8,
        (sz & 0x7F) as u8,
    ];
    data.extend_from_slice(&ss);
    data.extend_from_slice(&frames);

    let tag = parse_id3v2(&data).expect("parse id3v2").expect("tag found");
    assert_eq!(tag.major, 3);
    assert_eq!(tag.text_frames.len(), 2);
    assert_eq!(tag.text_frames[0], ("TIT2".into(), "My Song Name".into()));
    assert_eq!(tag.text_frames[1], ("TPE1".into(), "My Artist Name".into()));
    assert_eq!(tag.pictures.len(), 1);
    assert_eq!(tag.pictures[0].picture_type, 3);
    assert_eq!(tag.pictures[0].mime_type, "image/jpeg");
    assert_eq!(tag.pictures[0].description, "Cover");
    assert_eq!(tag.pictures[0].data, &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]);
}
