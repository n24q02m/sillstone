//! Edge cases of the ID3v1 / ID3v2 parsers: every text encoding, the APIC
//! description terminator per encoding, unsynchronisation, the v2.4 frame
//! format flags, extended headers and the malformed-input refusals.

use sillstone_mp3::{parse_id3v1, parse_id3v2};

fn synchsafe(n: usize) -> [u8; 4] {
    [
        ((n >> 21) & 0x7F) as u8,
        ((n >> 14) & 0x7F) as u8,
        ((n >> 7) & 0x7F) as u8,
        (n & 0x7F) as u8,
    ]
}

fn frame(major: u8, id: &[u8; 4], format_flags: u8, body: &[u8]) -> Vec<u8> {
    let mut f = id.to_vec();
    if major == 4 {
        f.extend_from_slice(&synchsafe(body.len()));
    } else {
        f.extend_from_slice(&(body.len() as u32).to_be_bytes());
    }
    f.push(0);
    f.push(format_flags);
    f.extend_from_slice(body);
    f
}

fn tag(major: u8, flags: u8, body: &[u8]) -> Vec<u8> {
    let mut t = b"ID3".to_vec();
    t.push(major);
    t.push(0);
    t.push(flags);
    t.extend_from_slice(&synchsafe(body.len()));
    t.extend_from_slice(body);
    t
}

fn text(encoding: u8, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![encoding];
    b.extend_from_slice(payload);
    b
}

fn utf16(s: &str, le: bool, bom: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
    }
    for u in s.encode_utf16() {
        out.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
    }
    out
}

fn first_text(t: &sillstone_mp3::Id3v2, id: &str) -> Option<String> {
    t.text_frames
        .iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v.clone())
}

fn v1_tag(fill: impl FnOnce(&mut [u8; 128])) -> Vec<u8> {
    let mut data = vec![0u8; 200];
    let mut t = [0u8; 128];
    t[0..3].copy_from_slice(b"TAG");
    fill(&mut t);
    data.extend_from_slice(&t);
    data
}

#[test]
fn id3v1_reads_latin1_when_the_field_is_not_utf8() {
    let data = v1_tag(|t| {
        // "Café" in ISO-8859-1, "Zoë" and "Ñandú" likewise.
        t[3..7].copy_from_slice(&[b'C', b'a', b'f', 0xE9]);
        t[33..36].copy_from_slice(&[b'Z', b'o', 0xEB]);
        t[63..68].copy_from_slice(&[0xD1, b'a', b'n', b'd', 0xFA]);
    });
    let v1 = parse_id3v1(&data).unwrap();
    assert_eq!(v1.title, "Caf\u{e9}");
    assert_eq!(v1.artist, "Zo\u{eb}");
    assert_eq!(v1.album, "\u{d1}and\u{fa}");
}

#[test]
fn id3v1_keeps_utf8_fields_and_trims_padding() {
    let data = v1_tag(|t| {
        let title = "Caf\u{e9}  ".as_bytes();
        t[3..3 + title.len()].copy_from_slice(title);
    });
    assert_eq!(parse_id3v1(&data).unwrap().title, "Caf\u{e9}");
}

#[test]
fn id3v1_comment_runs_to_byte_126_without_a_track() {
    // Byte 125 non-zero: no v1.1 track, the comment owns 97..127.
    let data = v1_tag(|t| {
        t[97..127].copy_from_slice(&[b'c'; 30]);
        t[127] = 255;
    });
    let v1 = parse_id3v1(&data).unwrap();
    assert_eq!(v1.comment, "c".repeat(30));
    assert_eq!(v1.track, None);
    assert_eq!(v1.genre, 255);

    // Byte 125 zero and 126 zero: v1.0 with a short comment.
    let data = v1_tag(|t| t[97..100].copy_from_slice(b"abc"));
    let v1 = parse_id3v1(&data).unwrap();
    assert_eq!((v1.comment.as_str(), v1.track), ("abc", None));
}

#[test]
fn id3v1_is_absent_without_a_tag_or_enough_bytes() {
    assert!(parse_id3v1(&[0u8; 127]).is_none());
    assert!(parse_id3v1(&[0u8; 300]).is_none());
    assert!(parse_id3v1(&[]).is_none());
}

#[test]
fn text_frames_decode_every_encoding() {
    let frames = [
        frame(3, b"TIT2", 0, &text(0, &[b'C', b'a', b'f', 0xE9, 0])),
        frame(
            3,
            b"TPE1",
            0,
            &text(1, &utf16("Zo\u{eb} \u{4e16}", true, true)),
        ),
        frame(3, b"TALB", 0, &text(1, &utf16("Alb\u{fc}m", false, true))),
        frame(3, b"TCON", 0, &text(2, &utf16("Jazz", false, false))),
        frame(3, b"TYER", 0, &text(3, "20\u{32}6\0".as_bytes())),
        // Surrogate pair (U+1F3B5) in UTF-16LE.
        frame(3, b"TPE2", 0, &text(1, &utf16("\u{1f3b5}", true, true))),
    ]
    .concat();
    let t = parse_id3v2(&tag(3, 0, &frames)).unwrap().unwrap();
    assert_eq!(first_text(&t, "TIT2").unwrap(), "Caf\u{e9}");
    assert_eq!(first_text(&t, "TPE1").unwrap(), "Zo\u{eb} \u{4e16}");
    assert_eq!(first_text(&t, "TALB").unwrap(), "Alb\u{fc}m");
    assert_eq!(first_text(&t, "TCON").unwrap(), "Jazz");
    assert_eq!(first_text(&t, "TYER").unwrap(), "2026");
    assert_eq!(first_text(&t, "TPE2").unwrap(), "\u{1f3b5}");
}

#[test]
fn utf16_without_a_bom_keeps_its_first_code_unit() {
    let frames = frame(3, b"TIT2", 0, &text(1, &utf16("Hi", false, false)));
    let t = parse_id3v2(&tag(3, 0, &frames)).unwrap().unwrap();
    assert_eq!(first_text(&t, "TIT2").unwrap(), "Hi");
}

#[test]
fn unreadable_text_frames_are_dropped_not_fatal() {
    let frames = [
        frame(3, b"TIT2", 0, &[]),                                 // no body at all
        frame(3, b"TPE1", 0, &text(9, b"x")),                      // unknown encoding
        frame(3, b"TALB", 0, &text(3, &[0xFF, 0xFE, 0xFD])),       // invalid UTF-8
        frame(3, b"TCON", 0, &text(1, &[0xFF])),                   // UTF-16 shorter than a BOM
        frame(3, b"TYER", 0, &text(1, &[0xFF, 0xFE, 0x00, 0xD8])), // lone surrogate
        frame(3, b"TRCK", 0, &text(2, &[0xDC, 0x00])),             // lone low surrogate (BE)
        frame(3, b"TXXX", 0, &text(3, b"user\0defined")),          // user text is not a plain frame
        frame(3, b"COMM", 0, &text(3, b"eng\0\0a comment")),       // other frames ignored
        frame(3, b"TLEN", 0, &text(3, b"1234")),                   // still parsed afterwards
    ]
    .concat();
    let t = parse_id3v2(&tag(3, 0, &frames)).unwrap().unwrap();
    assert_eq!(
        t.text_frames,
        vec![("TLEN".to_string(), "1234".to_string())]
    );
    assert!(t.pictures.is_empty());
}

fn apic(
    encoding: u8,
    mime: &[u8],
    ptype: u8,
    desc: &[u8],
    terminator: &[u8],
    data: &[u8],
) -> Vec<u8> {
    let mut b = vec![encoding];
    b.extend_from_slice(mime);
    b.push(0);
    b.push(ptype);
    b.extend_from_slice(desc);
    b.extend_from_slice(terminator);
    b.extend_from_slice(data);
    b
}

const PIXELS: [u8; 8] = [0x89, b'P', b'N', b'G', 0, 0, 7, 0];

#[test]
fn apic_description_ends_where_its_encoding_says() {
    let cases: Vec<(u8, Vec<u8>, &[u8], &str)> = vec![
        (0, b"Front".to_vec(), &[0], "Front"),
        (0, vec![b'C', 0xE9], &[0], "C\u{e9}"),
        (3, "Cov\u{e9}r".as_bytes().to_vec(), &[0], "Cov\u{e9}r"),
        // UTF-16 text contains 0x00 bytes of its own: the aligned 00 00 ends it.
        (1, utf16("Cover", true, true), &[0, 0], "Cover"),
        (1, utf16("Cover", false, true), &[0, 0], "Cover"),
        (2, utf16("Cover", false, false), &[0, 0], "Cover"),
        // Empty descriptions, with and without a BOM.
        (1, vec![0xFF, 0xFE], &[0, 0], ""),
        (1, vec![], &[0, 0], ""),
        (2, vec![], &[0, 0], ""),
        (0, vec![], &[0], ""),
    ];
    for (enc, desc, term, want) in cases {
        let frames = frame(
            3,
            b"APIC",
            0,
            &apic(enc, b"image/png", 3, &desc, term, &PIXELS),
        );
        let t = parse_id3v2(&tag(3, 0, &frames)).unwrap().unwrap();
        assert_eq!(t.pictures.len(), 1, "encoding {enc} {want:?}");
        let p = &t.pictures[0];
        assert_eq!(p.description, want, "encoding {enc}");
        assert_eq!(p.mime_type, "image/png");
        assert_eq!(p.picture_type, 3);
        assert_eq!(
            p.data, PIXELS,
            "picture bytes must start right after the terminator (encoding {enc})"
        );
    }
}

#[test]
fn malformed_apic_frames_yield_no_picture() {
    let bodies: Vec<Vec<u8>> = vec![
        vec![0, b'i', 0],                                          // shorter than 4
        vec![0, b'i', b'm', b'g'],                                 // no MIME terminator
        vec![0, 0xFF, 0xFE, 0, 3, 0],                              // MIME not UTF-8
        vec![0, b'a', 0, 3][..4].to_vec(),                         // ends before the description
        vec![0, b'a', 0], // MIME terminator is the last byte (len 3)
        [vec![0, b'a', 0, 3], b"no terminator".to_vec()].concat(), // description never ends
        [vec![1, b'a', 0, 3], utf16("x", true, true)].concat(), // UTF-16 description never ends
        vec![1, b'a', 0, 3, 0xFF], // odd single byte after the type
    ];
    for body in bodies {
        let frames = frame(3, b"APIC", 0, &body);
        let t = parse_id3v2(&tag(3, 0, &frames)).unwrap().unwrap();
        assert!(t.pictures.is_empty(), "body {body:02x?}");
    }
}

#[test]
fn v23_tag_level_unsynchronisation_is_reversed() {
    // A Latin-1 title ending in 0xFF; on disk the encoder wrote FF 00.
    let plain = frame(3, b"TIT2", 0, &text(0, &[b'a', 0xFF, b'b', 0xFF]));
    let mut stored = Vec::new();
    for &b in &plain {
        stored.push(b);
        if b == 0xFF {
            stored.push(0);
        }
    }
    assert!(stored.len() > plain.len());
    let t = parse_id3v2(&tag(3, 0x80, &stored)).unwrap().unwrap();
    assert_eq!(first_text(&t, "TIT2").unwrap(), "a\u{ff}b\u{ff}");

    // Without the flag the same bytes are taken literally.
    let literal = parse_id3v2(&tag(3, 0, &plain)).unwrap().unwrap();
    assert_eq!(first_text(&literal, "TIT2").unwrap(), "a\u{ff}b\u{ff}");
}

#[test]
fn v24_frame_flags_are_honoured() {
    // data-length indicator (0x01): 4 synchsafe bytes of original size lead the body.
    let mut dli = synchsafe(6).to_vec();
    dli.extend_from_slice(&text(3, b"Song"));
    let f_dli = frame(4, b"TIT2", 0x01, &dli);

    // unsynchronisation (0x02) over a UTF-8 title that contains U+00FF -> C3 BF (no FF),
    // so use Latin-1 0xFF followed by 0x00 padding byte that unsync would escape.
    let raw = text(0, &[0xFF, b'x']);
    let mut escaped = Vec::new();
    for &b in &raw {
        escaped.push(b);
        if b == 0xFF {
            escaped.push(0);
        }
    }
    let f_unsync = frame(4, b"TPE1", 0x02, &escaped);

    // grouping identity (0x40): one leading group byte.
    let mut grouped = vec![0x2A];
    grouped.extend_from_slice(&text(3, b"Album"));
    let f_group = frame(4, b"TALB", 0x40, &grouped);

    // grouping + data length + unsync together.
    let mut all = vec![0x07];
    all.extend_from_slice(&synchsafe(3));
    all.extend_from_slice(&text(0, &[0xFF]));
    let mut all_escaped = Vec::new();
    for &b in &all {
        all_escaped.push(b);
        if b == 0xFF {
            all_escaped.push(0);
        }
    }
    let f_all = frame(4, b"TCON", 0x40 | 0x02 | 0x01, &all_escaped);

    // compressed (0x08) and encrypted (0x04) frames cannot be read: skipped.
    let f_comp = frame(4, b"TYER", 0x08, &text(3, b"2020"));
    let f_enc = frame(4, b"TRCK", 0x04, &text(3, b"1/2"));
    let f_ok = frame(4, b"TLEN", 0, &text(3, b"99"));

    let frames = [f_dli, f_unsync, f_group, f_all, f_comp, f_enc, f_ok].concat();
    let t = parse_id3v2(&tag(4, 0, &frames)).unwrap().unwrap();
    assert_eq!(first_text(&t, "TIT2").unwrap(), "Song");
    assert_eq!(first_text(&t, "TPE1").unwrap(), "\u{ff}x");
    assert_eq!(first_text(&t, "TALB").unwrap(), "Album");
    assert_eq!(first_text(&t, "TCON").unwrap(), "\u{ff}");
    assert_eq!(first_text(&t, "TLEN").unwrap(), "99");
    assert!(first_text(&t, "TYER").is_none());
    assert!(first_text(&t, "TRCK").is_none());
}

#[test]
fn v23_frame_flags_skip_compressed_and_encrypted_and_strip_grouping() {
    let mut grouped = vec![0x01];
    grouped.extend_from_slice(&text(3, b"Grouped"));
    let frames = [
        frame(3, b"TIT2", 0x80, &text(3, b"zlib")),
        frame(3, b"TPE1", 0x40, &text(3, b"enc")),
        frame(3, b"TALB", 0x20, &grouped),
        // Grouping flag on a frame too short to hold the group byte.
        frame(3, b"TCON", 0x20, &[]),
        frame(3, b"TLEN", 0, &text(3, b"7")),
    ]
    .concat();
    let t = parse_id3v2(&tag(3, 0, &frames)).unwrap().unwrap();
    assert_eq!(
        t.text_frames,
        vec![
            ("TALB".to_string(), "Grouped".to_string()),
            ("TLEN".to_string(), "7".to_string())
        ]
    );
}

#[test]
fn extended_headers_are_skipped_in_both_versions() {
    // v2.3: size field (6, excludes itself) + 6 bytes.
    let mut body23 = 6u32.to_be_bytes().to_vec();
    body23.extend_from_slice(&[0; 6]);
    body23.extend_from_slice(&frame(3, b"TIT2", 0, &text(3, b"v23")));
    let t = parse_id3v2(&tag(3, 0x40, &body23)).unwrap().unwrap();
    assert_eq!(first_text(&t, "TIT2").unwrap(), "v23");

    // v2.4: synchsafe size (6, includes itself) + 2 more bytes.
    let mut body24 = synchsafe(6).to_vec();
    body24.extend_from_slice(&[0; 2]);
    body24.extend_from_slice(&frame(4, b"TIT2", 0, &text(3, b"v24")));
    let t = parse_id3v2(&tag(4, 0x40, &body24)).unwrap().unwrap();
    assert_eq!(first_text(&t, "TIT2").unwrap(), "v24");

    // An extended header larger than the tag leaves no frames, not a panic.
    let mut huge = 0x7FFF_FFFFu32.to_be_bytes().to_vec();
    huge.extend_from_slice(&[0; 8]);
    let t = parse_id3v2(&tag(3, 0x40, &huge)).unwrap().unwrap();
    assert!(t.text_frames.is_empty());
    let t = parse_id3v2(&tag(4, 0x40, &[0x7F, 0x7F, 0x7F, 0x7F, 0, 0, 0, 0]))
        .unwrap()
        .unwrap();
    assert!(t.text_frames.is_empty());
}

#[test]
fn malformed_tags_are_refused_or_ignored() {
    // Not an ID3 tag, or too short to hold a header.
    assert!(parse_id3v2(b"RIFF....12345").unwrap().is_none());
    assert!(parse_id3v2(b"ID3\x03\x00\x00").unwrap().is_none());
    assert!(parse_id3v2(&[]).unwrap().is_none());

    // Declared size beyond the available bytes.
    let mut t = tag(3, 0, &[0; 20]);
    t.truncate(15);
    assert!(parse_id3v2(&t).is_err());

    // Tag size with bit 7 set in a size byte is not synchsafe.
    let mut t = tag(3, 0, &[0; 4]);
    t[7] = 0x80;
    assert!(parse_id3v2(&t).is_err());

    // Extended header flag set but no room for its size field.
    assert!(parse_id3v2(&tag(3, 0x40, &[0, 0])).is_err());

    // v2.4 frame whose size is not synchsafe.
    let mut f = b"TIT2".to_vec();
    f.extend_from_slice(&[0x80, 0, 0, 1, 0, 0, 3]);
    f.resize(12, 0);
    assert!(parse_id3v2(&tag(4, 0, &f)).is_err());
}

#[test]
fn frame_walking_stops_at_padding_junk_and_oversized_frames() {
    let good = frame(3, b"TIT2", 0, &text(3, b"kept"));

    // Zero padding after the last frame.
    let padded = [good.clone(), vec![0; 40]].concat();
    let t = parse_id3v2(&tag(3, 0, &padded)).unwrap().unwrap();
    assert_eq!(t.text_frames.len(), 1);

    // A non-UTF-8 frame id ends the walk.
    let mut junk = good.clone();
    junk.extend_from_slice(&[0xFF, 0xFE, 0xFD, 0xFC, 0, 0, 0, 1, 0, 0, 3]);
    let t = parse_id3v2(&tag(3, 0, &junk)).unwrap().unwrap();
    assert_eq!(t.text_frames.len(), 1);

    // A frame claiming more bytes than the tag holds ends the walk, even at u32::MAX.
    for size in [200u32, u32::MAX] {
        let mut big = good.clone();
        big.extend_from_slice(b"TPE1");
        big.extend_from_slice(&size.to_be_bytes());
        big.extend_from_slice(&[0, 0, 3, b'x']);
        let t = parse_id3v2(&tag(3, 0, &big)).unwrap().unwrap();
        assert_eq!(t.text_frames.len(), 1);
    }

    // Fewer than 10 bytes after the frames: nothing to read.
    let tail = [good, vec![b'T', b'I', b'T', b'2', 0, 0, 0, 0]].concat();
    let t = parse_id3v2(&tag(3, 0, &tail)).unwrap().unwrap();
    assert_eq!(t.text_frames.len(), 1);
    assert_eq!((t.major, t.revision), (3, 0));
}
