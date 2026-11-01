//! BLAKE3 against the official test-vector inputs (`input[i] = i % 251`),
//! covering every tree shape: single block, multi block, chunk boundaries,
//! and deep Merkle stacks up to 102,400 bytes (100 chunks).
//!
//! `fixtures/blake3-vectors.txt` holds the 131-byte XOF output of the plain
//! and keyed modes for each length, generated with the reference
//! implementation (the first three rows equal the published vectors).

use sillstone_digest::{Blake3Hasher, blake3, blake3_keyed, blake3_xof};

const KEY: &[u8; 32] = b"whats the Elvish word for friend";
const VECTORS: &str = include_str!("fixtures/blake3-vectors.txt");

fn input(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

struct Row {
    len: usize,
    hash: Vec<u8>,
    keyed: Vec<u8>,
}

fn rows() -> Vec<Row> {
    VECTORS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let mut cols = l.split(' ');
            let len = cols.next().unwrap().parse().unwrap();
            let hash = unhex(cols.next().unwrap());
            let keyed = unhex(cols.next().unwrap());
            assert_eq!((hash.len(), keyed.len()), (131, 131));
            Row { len, hash, keyed }
        })
        .collect()
}

#[test]
fn fixture_covers_every_tree_shape() {
    let lens: Vec<usize> = rows().iter().map(|r| r.len).collect();
    assert_eq!(lens.len(), 42);
    assert!(lens.contains(&0) && lens.contains(&1024) && lens.contains(&102_400));
}

#[test]
fn one_shot_hash_and_xof_match_the_reference() {
    for row in rows() {
        let data = input(row.len);
        let digest = blake3(&data).unwrap();
        assert_eq!(
            digest.as_bytes()[..],
            row.hash[..32],
            "hash len {}",
            row.len
        );

        let mut xof = vec![0u8; 131];
        blake3_xof(&data, &mut xof).unwrap();
        assert_eq!(xof, row.hash, "xof len {}", row.len);
    }
}

#[test]
fn keyed_hash_and_keyed_xof_match_the_reference() {
    for row in rows() {
        let data = input(row.len);
        let digest = blake3_keyed(KEY, &data).unwrap();
        assert_eq!(
            digest.as_bytes()[..],
            row.keyed[..32],
            "keyed len {}",
            row.len
        );

        let mut hasher = Blake3Hasher::new_keyed(KEY);
        hasher.update(&data);
        let mut xof = vec![0u8; 131];
        hasher.finalize_into(&mut xof);
        assert_eq!(xof, row.keyed, "keyed xof len {}", row.len);
    }
}

#[test]
fn streaming_in_any_split_equals_one_shot() {
    // Steps chosen to straddle block (64) and chunk (1024) boundaries.
    for row in rows().iter().filter(|r| r.len > 0) {
        let data = input(row.len);
        for step in [1usize, 3, 63, 64, 65, 1000, 1024, 1025, 4096] {
            let mut hasher = Blake3Hasher::new();
            for piece in data.chunks(step) {
                hasher.update(piece);
            }
            assert_eq!(
                hasher.finalize().as_bytes()[..],
                row.hash[..32],
                "len {} step {step}",
                row.len
            );
        }
    }
}

#[test]
fn finalize_does_not_consume_state_and_default_matches_new() {
    let data = input(3000);
    let mut hasher = Blake3Hasher::default();
    hasher.update(&data[..1500]);
    let _midway = hasher.finalize();
    hasher.update(&data[1500..]);
    assert_eq!(hasher.finalize(), blake3(&data).unwrap());
    // Finalizing twice yields the same digest.
    assert_eq!(hasher.finalize(), hasher.finalize());
}

#[test]
fn xof_prefix_property_and_short_outputs() {
    let data = input(2500);
    let mut long = vec![0u8; 200];
    blake3_xof(&data, &mut long).unwrap();
    for n in [0usize, 1, 31, 32, 33, 63, 64, 65, 129] {
        let mut short = vec![0u8; n];
        blake3_xof(&data, &mut short).unwrap();
        assert_eq!(short[..], long[..n], "prefix {n}");
    }
}
