//! Edge cases of the TAR reader: hostile size fields, GNU and PAX extensions,
//! the ustar prefix rule, checksums and path-escape detection.

use sillstone_file::{is_path_traversal, tar_entries, tar_read};

/// Builds one 512-byte header with a valid checksum; `tweak` may overwrite any field first.
fn header(
    name: &str,
    size_field: &[u8],
    typeflag: u8,
    tweak: impl FnOnce(&mut [u8; 512]),
) -> [u8; 512] {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0001000\0");
    h[116..124].copy_from_slice(b"0001000\0");
    h[124..124 + size_field.len()].copy_from_slice(size_field);
    h[136..148].copy_from_slice(b"00000000000 ");
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    tweak(&mut h);
    seal(&mut h);
    h
}

fn seal(h: &mut [u8; 512]) {
    h[148..156].copy_from_slice(b"        ");
    let sum: usize = h.iter().map(|&b| b as usize).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
}

fn octal(n: usize) -> Vec<u8> {
    format!("{n:011o} ").into_bytes()
}

fn entry(name: &str, content: &[u8], typeflag: u8) -> Vec<u8> {
    let mut out = header(name, &octal(content.len()), typeflag, |_| {}).to_vec();
    out.extend_from_slice(content);
    out.resize(out.len().next_multiple_of(512), 0);
    out
}

fn finish(mut archive: Vec<u8>) -> Vec<u8> {
    archive.extend_from_slice(&[0u8; 1024]);
    archive
}

#[test]
fn oversized_binary_size_fields_are_refused_without_panicking() {
    // GNU base-256 size of usize::MAX / 2^k: the arithmetic on offset + size must not wrap.
    for fill in [0x7Fu8, 0xFF, 0x01] {
        let mut field = [fill; 12];
        field[0] = 0x80 | (fill & 0x7F);
        let mut archive = header("big", &field, b'0', |_| {}).to_vec();
        archive.extend_from_slice(&[0u8; 1024]);
        // Any outcome but a panic is acceptable; a size this large cannot be inside the slice.
        assert!(tar_entries(&archive, false).is_err(), "fill {fill:#x}");
    }
    // A size that fits in usize but not in the data: Truncated, not a slice panic.
    let mut field = [0u8; 12];
    field[0] = 0x80;
    field[4..12].copy_from_slice(&(u64::MAX - 700).to_be_bytes());
    let archive = finish(header("big", &field, b'0', |_| {}).to_vec());
    assert!(tar_entries(&archive, false).is_err());
}

#[test]
fn binary_size_field_that_overflows_usize_is_not_silently_truncated() {
    // 12 bytes of base-256 with a non-zero byte in the top positions cannot fit a 64-bit size.
    let mut field = [0u8; 12];
    field[0] = 0x80;
    field[1] = 0x01; // 2^88 once shifted in
    field[11] = 0x05;
    let mut archive = header("wrap", &field, b'0', |_| {}).to_vec();
    archive.extend_from_slice(&[0u8; 1024]);
    // Truncating to the low 64 bits would read this as a 5-byte file inside the slice.
    assert!(tar_entries(&archive, false).is_err());
}

#[test]
fn negative_base256_sizes_are_refused() {
    let mut field = [0xFFu8; 12];
    field[0] = 0xFF;
    let archive = finish(header("neg", &field, b'0', |_| {}).to_vec());
    assert!(tar_entries(&archive, false).is_err());
}

#[test]
fn small_binary_size_field_is_read() {
    let mut field = [0u8; 12];
    field[0] = 0x80;
    field[11] = 5;
    let mut archive = header("b256", &field, b'0', |_| {}).to_vec();
    archive.extend_from_slice(b"hello");
    archive.resize(1024, 0);
    let archive = finish(archive);
    let e = tar_entries(&archive, false).unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(tar_read(&archive, &e[0]).unwrap(), b"hello");
}

#[test]
fn truncated_data_and_bad_checksum_are_errors() {
    let good = finish(entry("a.txt", b"hello world", b'0'));
    assert_eq!(tar_entries(&good, false).unwrap().len(), 1);

    // Cut inside the entry content.
    assert!(tar_entries(&good[..512 + 4], false).is_err());
    // Header says 11 bytes but the archive ends right after the header.
    assert!(tar_entries(&good[..512], false).is_err());

    // Flip a header byte: checksum no longer matches.
    let mut bad = good.clone();
    bad[0] ^= 0x01;
    assert!(tar_entries(&bad, false).is_err());
    // Non-octal garbage in the checksum field.
    let mut bad = good.clone();
    bad[148..156].copy_from_slice(b"zzzzzzz\0");
    assert!(tar_entries(&bad, false).is_err());
    // Non-octal garbage in the size field (checksum fixed up afterwards).
    let archive = finish(header("x", b"12x45678901 ", b'0', |_| {}).to_vec());
    assert!(tar_entries(&archive, false).is_err());
}

#[test]
fn entries_end_at_two_zero_blocks_and_tolerate_a_lone_one() {
    let mut a = entry("one", b"1", b'0');
    a.extend_from_slice(&[0u8; 512]); // a single zero block, then more entries
    a.extend_from_slice(&entry("two", b"22", b'0'));
    let a = finish(a);
    let names: Vec<_> = tar_entries(&a, false)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["one", "two"]);

    // Anything after the terminator is ignored.
    let mut b = finish(entry("one", b"1", b'0'));
    b.extend_from_slice(&entry("ghost", b"g", b'0'));
    assert_eq!(tar_entries(&b, false).unwrap().len(), 1);

    // No terminator at all, and trailing bytes shorter than a block.
    let mut c = entry("one", b"1", b'0');
    c.extend_from_slice(&[7u8; 100]);
    assert_eq!(tar_entries(&c, false).unwrap().len(), 1);
    assert!(tar_entries(&[], false).unwrap().is_empty());
}

#[test]
fn ustar_prefix_joins_the_name_but_gnu_headers_do_not_use_it() {
    let ustar = header("file.txt", &octal(0), b'0', |h| {
        h[345..345 + 7].copy_from_slice(b"dir/sub");
    });
    let e = tar_entries(&finish(ustar.to_vec()), false).unwrap();
    assert_eq!(e[0].name, "dir/sub/file.txt");

    // GNU magic "ustar  \0": bytes 345.. hold atime/ctime, not a path prefix.
    let gnu = header("file.txt", &octal(0), b'0', |h| {
        h[257..265].copy_from_slice(b"ustar  \0");
        h[345..357].copy_from_slice(b"14123456701\0"); // atime
        h[357..369].copy_from_slice(b"14123456702\0"); // ctime
    });
    let e = tar_entries(&finish(gnu.to_vec()), false).unwrap();
    assert_eq!(e[0].name, "file.txt");

    // No magic at all (v7 tar): also no prefix.
    let v7 = header("old.txt", &octal(0), b'0', |h| {
        h[257..265].fill(0);
        h[345..352].copy_from_slice(b"garbage");
    });
    let e = tar_entries(&finish(v7.to_vec()), false).unwrap();
    assert_eq!(e[0].name, "old.txt");
}

#[test]
fn gnu_long_name_and_long_link_entries() {
    let long = "d/".repeat(80) + "file.txt";
    assert!(long.len() > 100);
    let mut name_payload = long.clone().into_bytes();
    name_payload.push(0);

    let mut a = entry("././@LongLink", &name_payload, b'L');
    // A long link target must not show up as an entry of its own.
    a.extend_from_slice(&entry("././@LongLink", b"target/of/the/link\0", b'K'));
    a.extend_from_slice(&entry("truncated-name", b"abc", b'0'));
    let a = finish(a);
    let e = tar_entries(&a, false).unwrap();
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!(e[0].name, long);
    assert_eq!(tar_read(&a, &e[0]).unwrap(), b"abc");
}

fn pax_record(key: &str, value: &str) -> String {
    // "<len> key=value\n" where len counts the whole record including its own digits.
    let body = format!(" {key}={value}\n");
    let mut len = body.len() + 1;
    while len.to_string().len() + body.len() != len {
        len = len.to_string().len() + body.len();
    }
    format!("{len}{body}")
}

#[test]
fn pax_path_and_size_overrides_apply_to_the_next_entry_only() {
    let long = "pax/".repeat(40) + "name.bin";
    let ext = pax_record("path", &long) + &pax_record("comment", "ignored");
    let mut a = entry("PaxHeader/x", ext.as_bytes(), b'x');
    a.extend_from_slice(&entry("short", b"payload", b'0'));
    a.extend_from_slice(&entry("plain", b"p", b'0'));
    let a = finish(a);
    let e = tar_entries(&a, false).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].name, long);
    assert_eq!(tar_read(&a, &e[0]).unwrap(), b"payload");
    assert_eq!(e[1].name, "plain");

    // A PAX size record replaces the (here deliberately wrong) header size.
    let ext = pax_record("size", "7");
    let mut b = entry("PaxHeader/x", ext.as_bytes(), b'x');
    let mut h = header("sized", &octal(0), b'0', |_| {}).to_vec();
    h.extend_from_slice(b"payload");
    h.resize(h.len().next_multiple_of(512), 0);
    b.extend_from_slice(&h);
    b.extend_from_slice(&entry("after", b"z", b'0'));
    let b = finish(b);
    let e = tar_entries(&b, false).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!((e[0].name.as_str(), e[0].size), ("sized", 7));
    assert_eq!(tar_read(&b, &e[0]).unwrap(), b"payload");
    assert_eq!(e[1].name, "after");
}

/// An entry whose ustar size field is `header_size` but whose data occupies `payload`.
fn entry_with_header_size(name: &str, header_size: usize, payload: &[u8]) -> Vec<u8> {
    let mut out = header(name, &octal(header_size), b'0', |_| {}).to_vec();
    out.extend_from_slice(payload);
    out.resize(out.len().next_multiple_of(512), 0);
    out
}

#[test]
fn pax_global_size_applies_to_every_following_entry_and_local_size_wins() {
    // POSIX.1-2001 pax: a `g` record is a default for all later entries, an `x` record
    // overrides it for the next entry only. Sizes in the ustar fields are deliberately wrong.
    let mut a = entry("PaxHeader/g", pax_record("size", "7").as_bytes(), b'g');
    a.extend_from_slice(&entry_with_header_size("first", 0, b"payload"));
    a.extend_from_slice(&entry_with_header_size("second", 0, b"PAYLOAD"));
    a.extend_from_slice(&entry(
        "PaxHeader/x",
        pax_record("size", "3").as_bytes(),
        b'x',
    ));
    a.extend_from_slice(&entry_with_header_size("local", 0, b"abc"));
    a.extend_from_slice(&entry_with_header_size("third", 0, b"payload"));
    // A later `g` replaces the earlier default.
    a.extend_from_slice(&entry(
        "PaxHeader/g",
        pax_record("size", "2").as_bytes(),
        b'g',
    ));
    a.extend_from_slice(&entry_with_header_size("fourth", 0, b"hi"));
    let a = finish(a);
    let e = tar_entries(&a, false).unwrap();
    let got: Vec<_> = e.iter().map(|x| (x.name.as_str(), x.size)).collect();
    assert_eq!(
        got,
        [
            ("first", 7),
            ("second", 7),
            ("local", 3),
            ("third", 7),
            ("fourth", 2)
        ]
    );
    assert_eq!(tar_read(&a, &e[0]).unwrap(), b"payload");
    assert_eq!(tar_read(&a, &e[1]).unwrap(), b"PAYLOAD");
    assert_eq!(tar_read(&a, &e[2]).unwrap(), b"abc");
    assert_eq!(tar_read(&a, &e[3]).unwrap(), b"payload");
    assert_eq!(tar_read(&a, &e[4]).unwrap(), b"hi");
}

#[test]
fn entries_before_a_pax_global_header_keep_their_own_size() {
    let mut a = entry("before", b"xyz", b'0');
    a.extend_from_slice(&entry(
        "PaxHeader/g",
        pax_record("size", "1").as_bytes(),
        b'g',
    ));
    a.extend_from_slice(&entry_with_header_size("after", 9, b"z"));
    let e = tar_entries(&finish(a), false).unwrap();
    assert_eq!((e[0].name.as_str(), e[0].size), ("before", 3));
    assert_eq!((e[1].name.as_str(), e[1].size), ("after", 1));
}

#[test]
fn pax_values_may_contain_newlines_and_malformed_records_are_ignored() {
    // The length prefix, not the line structure, delimits a record.
    let ext = pax_record("comment", "two\nlines path=evil") + &pax_record("path", "real/name.txt");
    let mut a = entry("PaxHeader/x", ext.as_bytes(), b'x');
    a.extend_from_slice(&entry("short", b"", b'0'));
    let e = tar_entries(&finish(a), false).unwrap();
    assert_eq!(e[0].name, "real/name.txt");

    for junk in [
        "",
        "notanumber path=x\n",
        "999 path=x\n",
        "3 path=x\n",
        "12 path\n",
        "\u{1}\u{2}",
    ] {
        let mut a = entry("PaxHeader/x", junk.as_bytes(), b'x');
        a.extend_from_slice(&entry("short", b"", b'0'));
        let e = tar_entries(&finish(a), false).unwrap();
        assert_eq!(e[0].name, "short", "junk {junk:?}");
    }
    // Invalid UTF-8 in the extended header is ignored, not an error.
    let mut a = entry("PaxHeader/x", &[0xFF, 0xFE, b' ', 0xC3], b'x');
    a.extend_from_slice(&entry("short", b"", b'0'));
    assert_eq!(tar_entries(&finish(a), false).unwrap()[0].name, "short");
}

#[test]
fn path_escapes_are_rejected_unless_allowed() {
    for name in [
        "../up",
        "a/../../b",
        "/abs",
        "\\abs",
        "..\\up",
        "C:/windows",
        "c:\\windows",
        "a/b/../../../c",
    ] {
        let a = finish(entry(name, b"x", b'0'));
        assert!(is_path_traversal(name), "{name}");
        assert!(tar_entries(&a, false).is_err(), "{name}");
        let allowed = tar_entries(&a, true).unwrap();
        assert_eq!(allowed.len(), 1, "{name}");
    }
    for name in ["a/b", "..a/b..", "a/./b", "dir/", "...", "x/C:/y", "1:b"] {
        assert!(!is_path_traversal(name), "{name}");
        let a = finish(entry(name, b"x", b'0'));
        assert!(tar_entries(&a, false).is_ok(), "{name}");
    }
    // Windows separators in stored names are normalised.
    let a = finish(entry("dir\\file.txt", b"x", b'0'));
    assert_eq!(tar_entries(&a, false).unwrap()[0].name, "dir/file.txt");
}

#[test]
fn a_long_name_cannot_smuggle_a_traversal_past_the_check() {
    let mut payload = b"../../etc/passwd".to_vec();
    payload.push(0);
    let mut a = entry("././@LongLink", &payload, b'L');
    a.extend_from_slice(&entry("harmless", b"x", b'0'));
    let a = finish(a);
    assert!(tar_entries(&a, false).is_err());
    assert_eq!(tar_entries(&a, true).unwrap()[0].name, "../../etc/passwd");
}

#[test]
fn directories_symlinks_and_signed_checksums() {
    let a = finish(
        [
            entry("dir/", b"", b'5'),
            header("link", &octal(0), b'2', |h| {
                h[157..165].copy_from_slice(b"target\0\0")
            })
            .to_vec(),
        ]
        .concat(),
    );
    let e = tar_entries(&a, false).unwrap();
    assert_eq!(
        e.iter()
            .map(|e| (e.name.as_str(), e.typeflag))
            .collect::<Vec<_>>(),
        [("dir/", b'5'), ("link", b'2')]
    );

    // Old tars summed header bytes as signed chars: a name with a high byte must still verify.
    let mut h = header("caf\u{e9}", &octal(0), b'0', |_| {});
    h[0..5].copy_from_slice(&[b'c', b'a', b'f', 0xE9, 0]);
    h[148..156].copy_from_slice(b"        ");
    let signed: isize = h.iter().map(|&b| b as i8 as isize).sum();
    h[148..156].copy_from_slice(format!("{signed:06o}\0 ").as_bytes());
    let a = finish(h.to_vec());
    assert_eq!(tar_entries(&a, false).unwrap().len(), 1);
}

#[test]
fn read_refuses_entries_that_do_not_fit_the_slice() {
    let a = finish(entry("a", b"hello", b'0'));
    let mut e = tar_entries(&a, false).unwrap().remove(0);
    assert_eq!(tar_read(&a, &e).unwrap(), b"hello");
    e.size = usize::MAX;
    assert!(tar_read(&a, &e).is_err()); // offset + size overflows
    e.size = a.len();
    assert!(tar_read(&a, &e).is_err()); // runs past the end
}
