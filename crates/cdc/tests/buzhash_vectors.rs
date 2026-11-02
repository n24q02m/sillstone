//! Fixed `Buzhash64` fingerprints computed outside this crate.
//!
//! The expected values come from a separate implementation (Python): the 256-entry table
//! rebuilt from its documented provenance (`GEAR[i]` = first eight bytes, big-endian, of
//! `MD5([i; 64])`) and the window hash taken from its definition, the XOR over the last
//! up-to-64 bytes of `GEAR[w[i]]` rotated left by the number of bytes that follow it.
//! Neither the crate's table nor its rolling update is used to produce them.

use sillstone_cdc::Buzhash64;

fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| ((i * 7 + 3) & 0xff) as u8).collect()
}

fn fingerprint_of(data: &[u8]) -> u64 {
    let mut h = Buzhash64::new();
    let returned = h.update(data);
    assert_eq!(returned, h.fingerprint(), "update returns the fingerprint");
    h.fingerprint()
}

#[test]
fn partial_and_full_windows_match_the_external_vectors() {
    let seq: Vec<u8> = (0..64u8).collect();
    let cases: [(&str, Vec<u8>, u64); 5] = [
        ("single byte", b"a".to_vec(), 0x0148_42d4_80b5_7149),
        (
            "hello world",
            b"hello world".to_vec(),
            0x03c4_de72_dbb9_0471,
        ),
        ("63 bytes", seq[..63].to_vec(), 0xc285_a4b0_7e6b_c5eb),
        ("exactly 64 bytes", seq.clone(), 0x1428_07c6_0304_1265),
        ("one past full", pattern(65), 0x1ff3_2bd1_0c5b_9172),
    ];
    for (label, data, want) in cases {
        assert_eq!(fingerprint_of(&data), want, "{label}");
    }
}

#[test]
fn evicting_windows_match_the_external_vectors() {
    assert_eq!(fingerprint_of(&pattern(200)), 0xd561_619a_658e_2a58);
    assert_eq!(fingerprint_of(&pattern(1000)), 0x5629_d46b_5653_abe8);
}

#[test]
fn only_the_last_64_bytes_matter_and_reset_clears_the_state() {
    let seq: Vec<u8> = (0..64u8).collect();
    let want = 0x1428_07c6_0304_1265;

    // 1000 unrelated bytes, then the 64-byte window: the old contents have all slid out.
    let mut h = Buzhash64::new();
    h.update(&pattern(1000));
    assert_ne!(h.fingerprint(), want);
    h.update(&seq);
    assert_eq!(h.fingerprint(), want);

    // After a reset the same bytes give the same value again from the empty state.
    h.reset();
    assert_eq!(h.fingerprint(), 0);
    h.update(&seq);
    assert_eq!(h.fingerprint(), want);
}

#[test]
fn push_returns_the_running_fingerprint() {
    let mut h = Buzhash64::new();
    assert_eq!(h.push(b'a'), 0x0148_42d4_80b5_7149);
    assert_eq!(h.fingerprint(), 0x0148_42d4_80b5_7149);
}
