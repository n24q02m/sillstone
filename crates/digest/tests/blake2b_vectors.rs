//! BLAKE2b across block boundaries, key lengths and output lengths.
//!
//! `fixtures/blake2b-vectors.txt` was generated with Python's `hashlib`
//! (data[i] = i % 251, key[i] = (i * 7 + 3) % 256), so every row is an
//! independent oracle for the dispatched (AVX2 or scalar) compression.

use sillstone_digest::{blake2b, blake2b_512, blake2b_into};

const VECTORS: &str = include_str!("fixtures/blake2b-vectors.txt");

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
fn matches_hashlib_for_every_length_key_and_output_size() {
    let mut rows = 0;
    for line in VECTORS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let cols: Vec<&str> = line.split(' ').collect();
        let data_len: usize = cols[0].parse().unwrap();
        let key_len: usize = cols[1].parse().unwrap();
        let out_len: usize = cols[2].parse().unwrap();
        let expected = unhex(cols[3]);
        let data: Vec<u8> = (0..data_len).map(|i| (i % 251) as u8).collect();
        let key: Vec<u8> = (0..key_len).map(|i| ((i * 7 + 3) % 256) as u8).collect();

        assert_eq!(
            blake2b(&data, &key, out_len).unwrap(),
            expected,
            "data {data_len} key {key_len} out {out_len}"
        );
        let mut into = vec![0u8; out_len];
        blake2b_into(&data, &key, &mut into).unwrap();
        assert_eq!(into, expected);
        if key_len == 0 && out_len == 64 {
            assert_eq!(blake2b_512(&data).unwrap().as_bytes()[..], expected[..]);
        }
        rows += 1;
    }
    assert_eq!(rows, 144);
}

#[test]
fn rejects_bad_output_and_key_lengths() {
    for out_len in [0usize, 65, 1000] {
        assert!(blake2b(b"x", b"", out_len).is_err(), "out_len {out_len}");
        let mut buf = vec![0u8; out_len];
        assert!(blake2b_into(b"x", b"", &mut buf).is_err());
    }
    assert!(blake2b(b"x", &[0u8; 65], 32).is_err());
    assert!(blake2b(b"x", &[0u8; 64], 32).is_ok());
}
