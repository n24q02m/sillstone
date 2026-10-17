//! xxHash64 (XXH64), the 64-bit non-cryptographic hash of the xxHash
//! reference (Cyan4973/xxHash, `xxhash.h`).
//!
//! Four accumulators over 32-byte stripes, a direct or seeded initial
//! state, and the avalanche mix at the end. Every multiply wraps and
//! every length is data-driven, so hostile input lengths cannot drive
//! an allocation. Vectors are the reference implementation's values
//! (the empty-input constants `0xEF46DB3751D8E999` / seed-1 form, the
//! short-input values cross-checked against the reference `xxhsum`
//! corpus); the corpus in `reference.json` spans the stripe, u64-tail,
//! u32-tail and byte-tail branches.

/// The PRIME64_1 constant.
const P1: u64 = 0x9E37_79B1_85EB_CA87;
/// The PRIME64_2 constant.
const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
/// The PRIME64_3 constant.
const P3: u64 = 0x1656_67B1_9E37_79F9;
/// The PRIME64_4 constant.
const P4: u64 = 0x85EB_CA77_C2B2_AE63;
/// The PRIME64_5 constant.
const P5: u64 = 0x27D4_EB2F_1656_67C5;

/// Computes the XXH64 hash of `data` under `seed`.
///
/// ```
/// use sillstone_digest::xxh64;
/// // The reference's empty-input constant at seed 0.
/// assert_eq!(xxh64(b"", 0), 0xEF46_DB37_51D8_E999);
/// assert_eq!(xxh64(b"abc", 0), 0x44BC_2CF5_AD77_0999);
/// ```
#[must_use]
pub fn xxh64(data: &[u8], seed: u64) -> u64 {
    let len = data.len();
    let mut chunks = data.chunks_exact(32);
    let mut h = if len >= 32 {
        let mut v1 = seed.wrapping_add(P1).wrapping_add(P2);
        let mut v2 = seed.wrapping_add(P2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(P1);
        for block in chunks.by_ref() {
            let block: [u8; 32] = block.try_into().expect("chunks_exact yields 32");
            v1 = round(
                v1,
                u64::from_le_bytes(block[0..8].try_into().expect("8 bytes")),
            );
            v2 = round(
                v2,
                u64::from_le_bytes(block[8..16].try_into().expect("8 bytes")),
            );
            v3 = round(
                v3,
                u64::from_le_bytes(block[16..24].try_into().expect("8 bytes")),
            );
            v4 = round(
                v4,
                u64::from_le_bytes(block[24..32].try_into().expect("8 bytes")),
            );
        }
        let mut h = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        for v in [v1, v2, v3, v4] {
            h ^= round(0, v);
            h = h.wrapping_mul(P1).wrapping_add(P4);
        }
        h
    } else {
        seed.wrapping_add(P5)
    };

    h = h.wrapping_add(len as u64);

    let rem = chunks.remainder();
    let mut rem = rem;
    while rem.len() >= 8 {
        let lane = u64::from_le_bytes(rem[..8].try_into().expect("8 bytes"));
        h ^= round(0, lane);
        h = h.rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        rem = &rem[8..];
    }
    if rem.len() >= 4 {
        let lane = u32::from_le_bytes(rem[..4].try_into().expect("4 bytes"));
        h ^= u64::from(lane).wrapping_mul(P1);
        h = h.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        rem = &rem[4..];
    }
    for &byte in rem {
        h ^= u64::from(byte).wrapping_mul(P5);
        h = h.rotate_left(11).wrapping_mul(P1);
    }

    avalanche(h)
}

/// One 64-bit lane merge: `rotl(acc + lane * P2, 31) * P1`.
fn round(acc: u64, lane: u64) -> u64 {
    acc.wrapping_add(lane.wrapping_mul(P2))
        .rotate_left(31)
        .wrapping_mul(P1)
}

/// The final avalanche mix.
fn avalanche(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^= h >> 32;
    h
}

#[cfg(test)]
mod tests {
    use super::xxh64;

    /// The reference's short-input values (seeded and unseeded), each
    /// exercising a different tail branch: 0 bytes, 1 byte, 3 bytes,
    /// 13 bytes.
    #[test]
    fn reference_short_inputs() {
        assert_eq!(xxh64(b"", 0), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxh64(b"", 1), 0xD5AF_BA13_36A3_BE4B);
        assert_eq!(xxh64(b"a", 0), 0xD24E_C4F1_A98C_6E5B);
        assert_eq!(xxh64(b"abc", 0), 0x44BC_2CF5_AD77_0999);
        assert_eq!(xxh64(b"Hello, world!", 0), 0xF583_36A7_8B6F_9476);
    }

    /// Stripe and mixed-tail lengths: 40 bytes (one stripe plus u64
    /// and u32 and byte tails, seeded), 33 and 15 bytes.
    #[test]
    fn stripe_and_tail_lengths() {
        assert_eq!(xxh64(&[b'p'; 40], 7), 0x7470_070D_6C48_70B7);
        let bytes33: Vec<u8> = (0..33).collect();
        assert_eq!(xxh64(&bytes33, 0), 0x0C53_5D1A_CAFB_8EAD);
        let bytes15: Vec<u8> = (0..15).collect();
        assert_eq!(xxh64(&bytes15, 1), 0xC60A_A959_76ED_0E4E);
    }
}
