//! The committed pHash reference vectors of this crate, re-expressed
//! over raw pixel-dump fixtures.
//!
//! The upstream `modhash` facade exercised its pHash vectors through
//! PNG fixtures (`lab/phash_oracle.py`); this crate does not decode
//! PNG, so each fixture is committed as the *decoded* pixel matrix the
//! oracle hashed: a flat, row-major, channel-interleaved sample dump
//! (little-endian for 16-bit) under `tests/fixtures/phash/*.raw`,
//! with `base_444.raw` byte-identical to the kit's own committed raw
//! dump. The expected values are the oracle's, unchanged.
//!
//! [`tools/gen-reference`](../../tools/gen-reference) regenerates
//! `reference.json` from these vectors with the Rust pipeline;
//! CI verifies the committed copy is current. The [`VECTORS`] table
//! itself pins the oracle output independently of that file.

use crate::phash::image_phash;
use crate::raster::{Gray, Image, Rgb, Rgba};

/// One raw pixel-dump vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawVector {
    /// Vector name as it appears in `reference.json`.
    pub name: &'static str,
    /// Fixture file under `tests/fixtures/`.
    pub file: &'static str,
    /// Sample layout of the dump.
    pub layout: RawLayout,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// The oracle's expected 64-bit pHash.
    pub expected: u64,
}

/// Sample layout of a raw dump: channel count and sample width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawLayout {
    /// One `u8` luma sample per pixel.
    Gray8,
    /// One big-valued `u16` luma sample per pixel, little-endian.
    Gray16,
    /// Three `u8` samples per pixel, `R G B`.
    Rgb8,
    /// Three `u16` samples per pixel, `R G B`, little-endian.
    Rgb16,
    /// Four `u8` samples per pixel, `R G B A` (alpha is ignored by the
    /// pHash luma step, exactly as upstream).
    Rgba8,
}

impl RawLayout {
    /// Channels per pixel.
    #[must_use]
    pub fn channels(self) -> usize {
        match self {
            RawLayout::Gray8 | RawLayout::Gray16 => 1,
            RawLayout::Rgb8 | RawLayout::Rgb16 => 3,
            RawLayout::Rgba8 => 4,
        }
    }

    /// Bytes per sample in the dump.
    #[must_use]
    pub fn sample_bytes(self) -> usize {
        match self {
            RawLayout::Gray8 | RawLayout::Rgb8 | RawLayout::Rgba8 => 1,
            RawLayout::Gray16 | RawLayout::Rgb16 => 2,
        }
    }
}

/// A decoded fixture image, in whichever layout the dump carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fixture {
    /// `Gray8` dump.
    Gray8(Image<Gray, u8>),
    /// `Gray16` dump.
    Gray16(Image<Gray, u16>),
    /// `Rgb8` dump.
    Rgb8(Image<Rgb, u8>),
    /// `Rgb16` dump.
    Rgb16(Image<Rgb, u16>),
    /// `Rgba8` dump.
    Rgba8(Image<Rgba, u8>),
}

impl Fixture {
    /// The 64-bit pHash of this fixture.
    ///
    /// # Errors
    ///
    /// Forwards [`image_phash`]'s refusals; unreachable for images
    /// built by [`load`].
    pub fn phash(&self) -> Result<u64, sillstone_digest::Error> {
        match self {
            Fixture::Gray8(img) => image_phash(img),
            Fixture::Gray16(img) => image_phash(img),
            Fixture::Rgb8(img) => image_phash(img),
            Fixture::Rgb16(img) => image_phash(img),
            Fixture::Rgba8(img) => image_phash(img),
        }
    }
}

/// The nine committed vectors: the upstream oracle's output, unchanged.
///
/// Order matters only for readability; [`reference_json`] sorts by
/// name when it serializes.
#[must_use]
pub fn vectors() -> &'static [RawVector] {
    VECTORS
}

const VECTORS: &[RawVector] = &[
    RawVector {
        name: "base_444",
        file: "phash/base_444.raw",
        layout: RawLayout::Rgb8,
        width: 32,
        height: 24,
        expected: 0x8d0a_3f41_1ee5_0f7a,
    },
    RawVector {
        name: "phash_adam7_37x29",
        file: "phash/phash_adam7_37x29.raw",
        layout: RawLayout::Rgb8,
        width: 37,
        height: 29,
        expected: 0x8b4b_d90b_f426_b4f0,
    },
    RawVector {
        name: "phash_gray16_36x28",
        file: "phash/phash_gray16_36x28.raw",
        layout: RawLayout::Gray16,
        width: 36,
        height: 28,
        expected: 0xd386_0405_6df8_c37f,
    },
    RawVector {
        name: "phash_gray8_40x32",
        file: "phash/phash_gray8_40x32.raw",
        layout: RawLayout::Gray8,
        width: 40,
        height: 32,
        expected: 0xd3c7_0628_4777_fc81,
    },
    RawVector {
        name: "phash_pal8_trns_52x44",
        file: "phash/phash_pal8_trns_52x44.raw",
        layout: RawLayout::Rgba8,
        width: 52,
        height: 44,
        expected: 0xf260_e731_eb4a_3ce0,
    },
    RawVector {
        name: "phash_rgb16_40x24",
        file: "phash/phash_rgb16_40x24.raw",
        layout: RawLayout::Rgb16,
        width: 40,
        height: 24,
        expected: 0xa82d_435d_3c7d_7059,
    },
    RawVector {
        name: "phash_rgb8_48x40",
        file: "phash/phash_rgb8_48x40.raw",
        layout: RawLayout::Rgb8,
        width: 48,
        height: 40,
        expected: 0xc21b_8695_27e1_a74f,
    },
    RawVector {
        name: "phash_rgba8_48x40",
        file: "phash/phash_rgba8_48x40.raw",
        layout: RawLayout::Rgba8,
        width: 48,
        height: 40,
        expected: 0x8433_4f9d_2c5a_f4a9,
    },
    RawVector {
        name: "phash_small_13x9",
        file: "phash/phash_small_13x9.raw",
        layout: RawLayout::Rgb8,
        width: 13,
        height: 9,
        expected: 0xaa27_a317_f85c_a316,
    },
];

/// Loads and decodes a fixture dump into an [`Image`].
///
/// The file is read relative to the crate manifest directory (the same
/// convention the conformance tests use), so both `cargo test` and
/// `cargo run --bin gen-reference` resolve it from a repository
/// checkout.
///
/// # Errors
///
/// [`sillstone_digest::Error::Truncated`] if the dump is shorter than
/// `width * height * channels` samples; [`Image::from_vec`]'s own
/// refusal for the (unreachable) oversized case.
pub fn load(v: &RawVector) -> Result<Fixture, sillstone_digest::Error> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(v.file);
    let bytes = std::fs::read(&path).map_err(|_| sillstone_digest::Error::Truncated {
        what: "raw pixel fixture",
        needed: 1,
        found: 0,
    })?;
    let need = u128::from(v.width)
        * u128::from(v.height)
        * (v.layout.channels() as u128)
        * (v.layout.sample_bytes() as u128);
    if u128::try_from(bytes.len()).unwrap_or(u128::MAX) < need {
        return Err(sillstone_digest::Error::Truncated {
            what: "raw pixel fixture",
            needed: usize::try_from(need).unwrap_or(usize::MAX),
            found: bytes.len(),
        });
    }
    let samples = usize::try_from(need / v.layout.sample_bytes() as u128).expect("need fits usize");
    let need = usize::try_from(need).expect("need fits usize after the length check");
    let img = match v.layout {
        RawLayout::Gray8 => {
            Fixture::Gray8(Image::from_vec(v.width, v.height, bytes[..need].to_vec())?)
        }
        RawLayout::Gray16 => {
            let mut out = Vec::with_capacity(samples);
            for chunk in bytes[..need].chunks_exact(2) {
                out.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
            Fixture::Gray16(Image::from_vec(v.width, v.height, out)?)
        }
        RawLayout::Rgb8 => {
            Fixture::Rgb8(Image::from_vec(v.width, v.height, bytes[..need].to_vec())?)
        }
        RawLayout::Rgb16 => {
            let mut out = Vec::with_capacity(samples);
            for chunk in bytes[..need].chunks_exact(2) {
                out.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
            Fixture::Rgb16(Image::from_vec(v.width, v.height, out)?)
        }
        RawLayout::Rgba8 => {
            Fixture::Rgba8(Image::from_vec(v.width, v.height, bytes[..need].to_vec())?)
        }
    };
    Ok(img)
}

/// Serializes the canonical `reference.json` bytes.
///
/// Deterministic: vectors sorted by name, two-space indentation, a
/// single trailing newline — byte-identical across regenerations.
#[must_use]
pub fn reference_json() -> String {
    let mut names: Vec<&str> = VECTORS.iter().map(|v| v.name).collect();
    names.sort_unstable();
    let mut out = String::from("{\n  \"schema\": 1,\n  \"vectors\": {\n");
    for (i, name) in names.iter().enumerate() {
        let v = VECTORS
            .iter()
            .find(|v| v.name == *name)
            .expect("sorted name");
        out.push_str(&format!("    \"{name}\": \"{:016x}\"", v.expected));
        out.push_str(if i + 1 == names.len() { "\n" } else { ",\n" });
    }
    out.push_str("  }\n}\n");
    out
}

/// Compares the committed `reference.json` against a fresh
/// regeneration, returning a diff summary when they diverge.
///
/// # Errors
///
/// A description naming the first divergence, or the read failure.
pub fn verify() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("reference.json");
    let committed = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    verify_str(&committed)
}

/// The comparison underlying [`verify`]: a committed render against a
/// fresh one.
///
/// # Errors
///
/// A description naming the first divergence.
pub fn verify_str(committed: &str) -> Result<(), String> {
    let fresh = reference_json();
    if committed == fresh {
        return Ok(());
    }
    Err(format!(
        "reference.json is stale: regenerate with `cargo run --bin gen-reference`\nexpected:\n{fresh}\ncommitted:\n{committed}"
    ))
}

#[cfg(test)]
mod tests {
    use super::{Fixture, RawLayout, RawVector, load, reference_json, vectors, verify_str};

    /// A missing fixture names the failure instead of panicking.
    #[test]
    fn missing_fixture_is_a_truncation_error() {
        let v = RawVector {
            name: "absent",
            file: "phash/does_not_exist.raw",
            layout: RawLayout::Gray8,
            width: 1,
            height: 1,
            expected: 0,
        };
        let err = load(&v).expect_err("absent fixture must fail");
        assert!(matches!(err, sillstone_digest::Error::Truncated { .. }));
    }

    /// The layout geometry helpers agree with their documented counts.
    #[test]
    fn layout_geometry() {
        assert_eq!(RawLayout::Gray8.channels(), 1);
        assert_eq!(RawLayout::Gray8.sample_bytes(), 1);
        assert_eq!(RawLayout::Gray16.channels(), 1);
        assert_eq!(RawLayout::Gray16.sample_bytes(), 2);
        assert_eq!(RawLayout::Rgb8.channels(), 3);
        assert_eq!(RawLayout::Rgb8.sample_bytes(), 1);
        assert_eq!(RawLayout::Rgb16.channels(), 3);
        assert_eq!(RawLayout::Rgb16.sample_bytes(), 2);
        assert_eq!(RawLayout::Rgba8.channels(), 4);
        assert_eq!(RawLayout::Rgba8.sample_bytes(), 1);
    }

    /// The comparison accepts the current render and rejects a stale
    /// one.
    #[test]
    fn verify_str_accepts_current_and_rejects_stale() {
        verify_str(&reference_json()).expect("current render verifies");
        let stale = reference_json().replace("8d0a3f411ee50f7a", "0000000000000000");
        assert!(verify_str(&stale).is_err());
    }

    /// Every vector decodes into the fixture layout its row declares.
    #[test]
    fn fixtures_decode_into_declared_layouts() {
        for v in vectors() {
            let img = load(v).unwrap_or_else(|e| panic!("{}: {e}", v.file));
            let matches = match &img {
                Fixture::Gray8(im) => {
                    im.width() == v.width && im.height() == v.height && v.layout == RawLayout::Gray8
                }
                Fixture::Gray16(im) => {
                    im.width() == v.width
                        && im.height() == v.height
                        && v.layout == RawLayout::Gray16
                }
                Fixture::Rgb8(im) => {
                    im.width() == v.width && im.height() == v.height && v.layout == RawLayout::Rgb8
                }
                Fixture::Rgb16(im) => {
                    im.width() == v.width && im.height() == v.height && v.layout == RawLayout::Rgb16
                }
                Fixture::Rgba8(im) => {
                    im.width() == v.width && im.height() == v.height && v.layout == RawLayout::Rgba8
                }
            };
            assert!(matches, "{} decodes into its declared layout", v.name);
        }
    }
}
