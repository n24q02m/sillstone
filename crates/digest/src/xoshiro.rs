//! A deterministic 256-bit PRNG: xoshiro256** (Blackman & Vigna
//! 2018/2019), the reference `xoshiro256starstar.c` 1.0.
//!
//! The all-purpose successor to the suite's [`crate::SplitMix64`]:
//! same determinism contract (seeded with a constant, never the
//! clock), a 256-bit state for callers that need longer periods. The
//! reference implementation's own seeding recommendation is carried
//! verbatim: a 64-bit seed expands through splitmix64 into the four
//! state words ("If you have a 64-bit seed, we suggest to seed a
//! splitmix64 generator and use its output to fill s").
//!
//! The reference vectors pin the first eight outputs of the seeds
//! `reference.json` records, cross-checked against an independent
//! transcription of the reference C.

use crate::SplitMix64;

/// A xoshiro256** generator (Blackman & Vigna 2018/2019).
///
/// ```
/// let mut rng = sillstone_digest::Xoshiro256StarStar::from_seed(0);
/// assert_eq!(rng.next_u64(), 0x99ec_5f36_cb75_f2b4);
/// ```
#[derive(Clone)]
pub struct Xoshiro256StarStar {
    state: [u64; 4],
}

impl Xoshiro256StarStar {
    /// Creates a generator by expanding `seed` through splitmix64
    /// (the reference implementation's recommended seeding for 64-bit
    /// seeds), so the all-zero state the algorithm forbids cannot
    /// arise.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        let state = [sm.next_u64(), sm.next_u64(), sm.next_u64(), sm.next_u64()];
        Xoshiro256StarStar { state }
    }

    /// Creates a generator from a raw 256-bit state.
    ///
    /// The xoshiro256** step function maps the all-zero state to
    /// itself, so a caller seeding directly must never pass four zero
    /// words; [`Xoshiro256StarStar::from_seed`] cannot produce one.
    #[must_use]
    pub const fn from_state(state: [u64; 4]) -> Self {
        Xoshiro256StarStar { state }
    }

    /// Produces the next 64-bit output and advances the state: the
    /// reference `rotl(s[1] * 5, 7) * 9` output scrambler over the
    /// `s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3];
    /// s[2] ^= t; s[3] = rotl(s[3], 45)` linear step.
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.state;
        let result = (s[1].wrapping_mul(5).rotate_left(7)).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Produces `n` pseudo-random bytes, filling `out` exactly, with
    /// the same little-endian word stream [`crate::SplitMix64::fill_bytes`]
    /// defines. Deterministic in `out.len()` and the seed alone.
    pub fn fill_bytes(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Xoshiro256StarStar;

    /// The first outputs of the seeds `reference.json` records,
    /// cross-checked against an independent transcription of the
    /// reference C (`prng.di.unimi.it/xoshiro256starstar.c`).
    #[test]
    fn reference_seed_sequences() {
        let mut rng = Xoshiro256StarStar::from_seed(0);
        let expected = [
            0x99ec_5f36_cb75_f2b4,
            0xbf6e_1f78_4956_452a,
            0x1a5f_849d_4933_e6e0,
            0x6aa5_94f1_262d_2d2c,
            0xbba5_ad4a_1f84_2e59,
            0xffef_8375_d9eb_caca,
            0x6c16_0dee_d2f5_4c98,
            0x8920_ad64_8fc3_0a3f,
        ];
        for &want in &expected {
            assert_eq!(rng.next_u64(), want);
        }

        let mut rng = Xoshiro256StarStar::from_seed(42);
        assert_eq!(rng.next_u64(), 0x1578_0b2e_0c2e_c716);
        assert_eq!(rng.next_u64(), 0x6104_d986_6d11_3a7e);

        let mut rng = Xoshiro256StarStar::from_seed(7);
        assert_eq!(rng.next_u64(), 0xb358_faf7_4ef9_765a);
        assert_eq!(rng.next_u64(), 0x475c_3d96_4f48_2cd2);
    }

    /// `fill_bytes` streams the same little-endian words `next_u64`
    /// produces, including a partial trailing word.
    #[test]
    fn fill_bytes_streams_le_words() {
        let mut rng = Xoshiro256StarStar::from_seed(42);
        let mut bytes = [0u8; 10];
        rng.fill_bytes(&mut bytes);
        let mut rng = Xoshiro256StarStar::from_seed(42);
        let first = rng.next_u64().to_le_bytes();
        let second = rng.next_u64().to_le_bytes();
        assert_eq!(&bytes[..8], &first);
        assert_eq!(&bytes[8..], &second[..2]);
    }

    /// A raw all-zero state is the caller's own error: the step
    /// function maps it to itself, while `from_seed` never produces
    /// one.
    #[test]
    fn from_state_is_raw() {
        let mut rng = Xoshiro256StarStar::from_state([0; 4]);
        assert_eq!(rng.next_u64(), 0);
        // from_seed(0) is not the zero state.
        let mut rng = Xoshiro256StarStar::from_seed(0);
        assert_ne!(rng.next_u64(), 0);
    }
}
