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

/// Keccak rotation offsets `RHO[x][y]`.
const RHO: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

/// Runs the 24-round Keccak-f[1600] permutation over the 5x5 state of 64-bit lanes.
fn keccak_p(state: &mut [u64; 25]) {
    let mut a = [[0u64; 5]; 5];
    for x in 0..5 {
        for y in 0..5 {
            a[x][y] = state[x + 5 * y];
        }
    }

    for &rc in &RC {
        // Theta step
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = a[x][0] ^ a[x][1] ^ a[x][2] ^ a[x][3] ^ a[x][4];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..5 {
            for item in &mut a[x] {
                *item ^= d[x];
            }
        }

        // Rho and Pi steps
        let mut b = [[0u64; 5]; 5];
        for x in 0..5 {
            for y in 0..5 {
                b[y][(2 * x + 3 * y) % 5] = a[x][y].rotate_left(RHO[x][y]);
            }
        }

        // Chi step
        for x in 0..5 {
            for y in 0..5 {
                a[x][y] = b[x][y] ^ ((!b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]);
            }
        }

        // Iota step
        a[0][0] ^= rc;
    }

    for x in 0..5 {
        for y in 0..5 {
            state[x + 5 * y] = a[x][y];
        }
    }
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
