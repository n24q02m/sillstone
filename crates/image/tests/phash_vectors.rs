//! Conformance for the 64-bit pHash: the nine oracle vectors, ported
//! from the upstream `modhash` facade's `tests/phash_vectors.rs` and
//! re-expressed over raw pixel-dump fixtures (this crate does not
//! decode PNG; each dump is the exact pixel matrix the upstream oracle
//! hashed — see `tests/fixtures/phash/PROVENANCE.md`).
//!
//! Three pins per vector: the oracle's committed expected value (this
//! file, unchanged from upstream), the freshly computed pHash, and the
//! committed `reference.json` (which CI also re-derives via
//! `gen-reference verify`).

use sillstone_image::reference;

/// (vector name, expected 64-bit pHash, width, height) — the upstream
/// oracle output, byte-identical to the monorepo's committed table.
#[rustfmt::skip]
const PHASH_VECTORS: &[(&str, u64, u32, u32)] = &[
    ("base_444",             0x8d0a3f411ee50f7a, 32, 24),
    ("phash_adam7_37x29",    0x8b4bd90bf426b4f0, 37, 29),
    ("phash_gray16_36x28",   0xd38604056df8c37f, 36, 28),
    ("phash_gray8_40x32",    0xd3c706284777fc81, 40, 32),
    ("phash_pal8_trns_52x44", 0xf260e731eb4a3ce0, 52, 44),
    ("phash_rgb16_40x24",    0xa82d435d3c7d7059, 40, 24),
    ("phash_rgb8_48x40",     0xc21b869527e1a74f, 48, 40),
    ("phash_rgba8_48x40",    0x84334f9d2c5af4a9, 48, 40),
    ("phash_small_13x9",     0xaa27a317f85ca316, 13, 9),
];

/// The vector table matches the upstream oracle row for row.
#[test]
fn table_matches_upstream_oracle_output() {
    assert_eq!(reference::vectors().len(), PHASH_VECTORS.len());
    for (name, expected, width, height) in PHASH_VECTORS {
        let v = reference::vectors()
            .iter()
            .find(|v| v.name == *name)
            .unwrap_or_else(|| panic!("vector {name} missing from reference table"));
        assert_eq!(v.expected, *expected, "{name}: expected pHash drift");
        assert_eq!(v.width, *width, "{name}: width drift");
        assert_eq!(v.height, *height, "{name}: height drift");
    }
}

/// Every vector's fixture loads, and the pipeline reproduces the
/// oracle's pHash exactly.
#[test]
fn phash_reproduces_oracle_vectors() {
    for v in reference::vectors() {
        let img = reference::load(v).unwrap_or_else(|e| panic!("{}: {e}", v.file));
        let got = img.phash().unwrap_or_else(|e| panic!("{}: {e}", v.file));
        assert_eq!(got, v.expected, "{}: pHash drift", v.file);
    }
}

/// The committed reference.json carries exactly the oracle values.
#[test]
fn reference_json_matches_oracle_table() {
    let json = reference::reference_json();
    for (name, expected, _, _) in PHASH_VECTORS {
        let hex = format!("\"{name}\": \"{expected:016x}\"");
        assert!(json.contains(&hex), "reference.json missing {hex}");
    }
    // And the committed copy is current (same check CI runs).
    reference::verify().expect("reference.json must be current");
}

/// A dump whose declared dimensions exceed its bytes is refused, never
/// silently hashed.
#[test]
fn truncated_fixture_is_refused() {
    let mut v = *reference::vectors().first().expect("non-empty table");
    v.width = u32::MAX;
    v.height = u32::MAX;
    let err = reference::load(&v).expect_err("oversized dimensions must fail");
    assert!(matches!(err, sillstone_digest::Error::Truncated { .. }));
}
