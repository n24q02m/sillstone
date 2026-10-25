//! FIPS 202 SHA-3 (Keccak-f[1600]).
//!
//! Provides [`sha3_256`] and [`sha3_512`] with fixed rates of 1088 bits
//! (136 bytes) and 576 bits (72 bytes) respectively, using the multi-rate
//! padding rule `pad10*1` with domain separation byte `0x06`.

use crate::digest::Digest;
use crate::error::Result;

/// Keccak round constants for 24 rounds.
const RC: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];

/// One Keccak-f[1600] round, fully unrolled over named lanes (`a[x + 5 * y]`).
#[inline(always)]
fn round(a: &mut [u64; 25], rc: u64) {
    let c0 = a[0] ^ a[5] ^ a[10] ^ a[15] ^ a[20];
    let c1 = a[1] ^ a[6] ^ a[11] ^ a[16] ^ a[21];
    let c2 = a[2] ^ a[7] ^ a[12] ^ a[17] ^ a[22];
    let c3 = a[3] ^ a[8] ^ a[13] ^ a[18] ^ a[23];
    let c4 = a[4] ^ a[9] ^ a[14] ^ a[19] ^ a[24];
    let d0 = c4 ^ c1.rotate_left(1);
    let d1 = c0 ^ c2.rotate_left(1);
    let d2 = c1 ^ c3.rotate_left(1);
    let d3 = c2 ^ c4.rotate_left(1);
    let d4 = c3 ^ c0.rotate_left(1);
    let b0 = a[0] ^ d0;
    let b1 = (a[6] ^ d1).rotate_left(44);
    let b2 = (a[12] ^ d2).rotate_left(43);
    let b3 = (a[18] ^ d3).rotate_left(21);
    let b4 = (a[24] ^ d4).rotate_left(14);
    let b5 = (a[3] ^ d3).rotate_left(28);
    let b6 = (a[9] ^ d4).rotate_left(20);
    let b7 = (a[10] ^ d0).rotate_left(3);
    let b8 = (a[16] ^ d1).rotate_left(45);
    let b9 = (a[22] ^ d2).rotate_left(61);
    let b10 = (a[1] ^ d1).rotate_left(1);
    let b11 = (a[7] ^ d2).rotate_left(6);
    let b12 = (a[13] ^ d3).rotate_left(25);
    let b13 = (a[19] ^ d4).rotate_left(8);
    let b14 = (a[20] ^ d0).rotate_left(18);
    let b15 = (a[4] ^ d4).rotate_left(27);
    let b16 = (a[5] ^ d0).rotate_left(36);
    let b17 = (a[11] ^ d1).rotate_left(10);
    let b18 = (a[17] ^ d2).rotate_left(15);
    let b19 = (a[23] ^ d3).rotate_left(56);
    let b20 = (a[2] ^ d2).rotate_left(62);
    let b21 = (a[8] ^ d3).rotate_left(55);
    let b22 = (a[14] ^ d4).rotate_left(39);
    let b23 = (a[15] ^ d0).rotate_left(41);
    let b24 = (a[21] ^ d1).rotate_left(2);
    a[0] = b0 ^ (!b1 & b2) ^ rc;
    a[1] = b1 ^ (!b2 & b3);
    a[2] = b2 ^ (!b3 & b4);
    a[3] = b3 ^ (!b4 & b0);
    a[4] = b4 ^ (!b0 & b1);
    a[5] = b5 ^ (!b6 & b7);
    a[6] = b6 ^ (!b7 & b8);
    a[7] = b7 ^ (!b8 & b9);
    a[8] = b8 ^ (!b9 & b5);
    a[9] = b9 ^ (!b5 & b6);
    a[10] = b10 ^ (!b11 & b12);
    a[11] = b11 ^ (!b12 & b13);
    a[12] = b12 ^ (!b13 & b14);
    a[13] = b13 ^ (!b14 & b10);
    a[14] = b14 ^ (!b10 & b11);
    a[15] = b15 ^ (!b16 & b17);
    a[16] = b16 ^ (!b17 & b18);
    a[17] = b17 ^ (!b18 & b19);
    a[18] = b18 ^ (!b19 & b15);
    a[19] = b19 ^ (!b15 & b16);
    a[20] = b20 ^ (!b21 & b22);
    a[21] = b21 ^ (!b22 & b23);
    a[22] = b22 ^ (!b23 & b24);
    a[23] = b23 ^ (!b24 & b20);
    a[24] = b24 ^ (!b20 & b21);
}

/// Runs the 24-round Keccak-f[1600] permutation over the 5x5 state of
/// 64-bit lanes (`state[x + 5 * y]`).
#[allow(unsafe_code)]
fn keccak_p(state: &mut [u64; 25]) {
    #[cfg(all(feature = "std", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("bmi1") && std::is_x86_feature_detected!("bmi2") {
            // SAFETY: the CPU features the callee enables were just detected.
            unsafe { keccak_p_bmi(state) };
            return;
        }
    }
    keccak_p_generic(state);
}

#[inline(always)]
fn keccak_p_generic(state: &mut [u64; 25]) {
    for &rc in &RC {
        round(state, rc);
    }
}

/// The same permutation compiled with `andn` (BMI1) and flag-free `rorx`
/// rotates (BMI2); bit-identical to the generic build.
#[cfg(all(feature = "std", target_arch = "x86_64"))]
#[target_feature(enable = "bmi1,bmi2")]
#[allow(unsafe_code)]
unsafe fn keccak_p_bmi(state: &mut [u64; 25]) {
    keccak_p_generic(state);
}

/// Generic SHA-3 sponge function over arbitrary data with given rate and output length.
fn sha3_sponge<const N: usize>(data: &[u8], rate: usize) -> Digest<N> {
    let mut state = [0u64; 25];
    let rate_words = rate / 8;
    let mut offset = 0;

    // Absorb full rate-sized blocks
    while data.len().saturating_sub(offset) >= rate {
        for i in 0..rate_words {
            let chunk = &data[offset + 8 * i..offset + 8 * (i + 1)];
            let mut buf = [0u8; 8];
            buf.copy_from_slice(chunk);
            state[i] ^= u64::from_le_bytes(buf);
        }
        keccak_p(&mut state);
        offset += rate;
    }

    // Pad remaining block with 0x06 and multi-rate padding
    let rem = &data[offset..];
    let mut pad = [0u8; 144]; // Max rate for SHA-3 is 144 bytes (for SHA3-224)
    pad[..rem.len()].copy_from_slice(rem);
    pad[rem.len()] ^= 0x06;
    pad[rate - 1] ^= 0x80;

    for i in 0..rate_words {
        let chunk = &pad[8 * i..8 * (i + 1)];
        let mut buf = [0u8; 8];
        buf.copy_from_slice(chunk);
        state[i] ^= u64::from_le_bytes(buf);
    }
    keccak_p(&mut state);

    // Squeeze output bytes
    let mut out = [0u8; N];
    let mut written = 0;
    for &lane in &state {
        let bytes = lane.to_le_bytes();
        let take = (N - written).min(8);
        out[written..written + take].copy_from_slice(&bytes[..take]);
        written += take;
        if written == N {
            break;
        }
    }

    Digest::from_bytes(out)
}

/// Computes the 256-bit SHA-3 digest of `data` (FIPS 202).
///
/// Rate is 1088 bits (136 bytes), domain separation byte is `0x06`.
///
/// # Examples
///
/// ```
/// let hash = sillstone_digest::sha3_256(b"").unwrap();
/// assert_eq!(
///     hash.as_bytes()[..4],
///     [0xa7, 0xff, 0xc6, 0xf8]
/// );
/// ```
pub fn sha3_256(data: &[u8]) -> Result<Digest<32>> {
    Ok(sha3_sponge::<32>(data, 136))
}

/// Computes the 512-bit SHA-3 digest of `data` (FIPS 202).
///
/// Rate is 576 bits (72 bytes), domain separation byte is `0x06`.
///
/// # Examples
///
/// ```
/// let hash = sillstone_digest::sha3_512(b"").unwrap();
/// assert_eq!(
///     hash.as_bytes()[..4],
///     [0xa6, 0x9f, 0x73, 0xcc]
/// );
/// ```
pub fn sha3_512(data: &[u8]) -> Result<Digest<64>> {
    Ok(sha3_sponge::<64>(data, 72))
}
