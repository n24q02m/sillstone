//! RFC 7693 BLAKE2b cryptographic hash and MAC.
//!
//! Provides [`blake2b`], [`blake2b_512`], and caller-buffered [`blake2b_into`].
//! Supports unkeyed hashing and keyed MAC mode with keys up to 64 bytes
//! and output lengths from 1 to 64 bytes.

use crate::digest::Digest;
use crate::error::{Error, Result};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

/// BLAKE2b initialization vector (RFC 7693 §2.6).
const IV: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

/// BLAKE2b message schedule permutation SIGMA (RFC 7693 §2.7).
const SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

#[inline(always)]
fn g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn compress(h: &mut [u64; 8], block: &[u8; 128], t: u128, is_last: bool) {
    let mut v = [0u64; 16];
    v[..8].copy_from_slice(h);
    v[8..].copy_from_slice(&IV);

    v[12] ^= t as u64;
    v[13] ^= (t >> 64) as u64;
    if is_last {
        v[14] = !v[14];
    }

    let mut m = [0u64; 16];
    for i in 0..16 {
        let chunk = &block[8 * i..8 * (i + 1)];
        let mut buf = [0u8; 8];
        buf.copy_from_slice(chunk);
        m[i] = u64::from_le_bytes(buf);
    }

    for sigma_round in &SIGMA {
        g(&mut v, 0, 4, 8, 12, m[sigma_round[0]], m[sigma_round[1]]);
        g(&mut v, 1, 5, 9, 13, m[sigma_round[2]], m[sigma_round[3]]);
        g(&mut v, 2, 6, 10, 14, m[sigma_round[4]], m[sigma_round[5]]);
        g(&mut v, 3, 7, 11, 15, m[sigma_round[6]], m[sigma_round[7]]);
        g(&mut v, 0, 5, 10, 15, m[sigma_round[8]], m[sigma_round[9]]);
        g(&mut v, 1, 6, 11, 12, m[sigma_round[10]], m[sigma_round[11]]);
        g(&mut v, 2, 7, 8, 13, m[sigma_round[12]], m[sigma_round[13]]);
        g(&mut v, 3, 4, 9, 14, m[sigma_round[14]], m[sigma_round[15]]);
    }

    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

/// Computes BLAKE2b over `data` with optional `key` into `out`.
///
/// Output length is determined by `out.len()` (must be between 1 and 64 bytes).
/// Key length must not exceed 64 bytes.
pub fn blake2b_into(data: &[u8], key: &[u8], out: &mut [u8]) -> Result<()> {
    let out_len = out.len();
    if out_len == 0 || out_len > 64 {
        return Err(Error::BadValue("blake2b output length must be 1..=64"));
    }
    if key.len() > 64 {
        return Err(Error::BadValue("blake2b key length must be 0..=64"));
    }

    let mut h = IV;
    // Parameter block: out_len | (key_len << 8) | (1 << 16) | (1 << 24)
    let p0 = (out_len as u64)
        | ((key.len() as u64) << 8)
        | (1u64 << 16)
        | (1u64 << 24);
    h[0] ^= p0;

    let mut t: u128 = 0;
    let mut block = [0u8; 128];

    if !key.is_empty() {
        block[..key.len()].copy_from_slice(key);
        if data.is_empty() {
            compress(&mut h, &block, 128, true);
        } else {
            compress(&mut h, &block, 128, false);
            t = 128;
            absorb_data(&mut h, data, t);
        }
    } else if data.is_empty() {
        compress(&mut h, &block, 0, true);
    } else {
        absorb_data(&mut h, data, t);
    }

    // Produce output bytes
    let mut full_out = [0u8; 64];
    for (i, word) in h.iter().enumerate() {
        full_out[8 * i..8 * (i + 1)].copy_from_slice(&word.to_le_bytes());
    }
    out.copy_from_slice(&full_out[..out_len]);

    Ok(())
}

fn absorb_data(h: &mut [u64; 8], data: &[u8], mut t: u128) {
    let mut offset = 0;
    let mut block = [0u8; 128];
    while data.len().saturating_sub(offset) > 128 {
        block.copy_from_slice(&data[offset..offset + 128]);
        t = t.wrapping_add(128);
        compress(h, &block, t, false);
        offset += 128;
    }
    let rem = &data[offset..];
    block = [0u8; 128];
    block[..rem.len()].copy_from_slice(rem);
    t = t.wrapping_add(rem.len() as u128);
    compress(h, &block, t, true);
}

/// Computes BLAKE2b over `data` with optional `key` and variable `out_len`.
///
/// # Errors
///
/// Returns [`Error::BadValue`] if `out_len` is not in `1..=64` or `key.len() > 64`.
pub fn blake2b(data: &[u8], key: &[u8], out_len: usize) -> Result<Vec<u8>> {
    if out_len == 0 || out_len > 64 {
        return Err(Error::BadValue("blake2b output length must be 1..=64"));
    }
    let mut out = vec![0u8; out_len];
    blake2b_into(data, key, &mut out)?;
    Ok(out)
}

/// Computes the unkeyed 512-bit BLAKE2b digest of `data` (RFC 7693).
///
/// # Examples
///
/// ```
/// let hash = sillstone_digest::blake2b_512(b"").unwrap();
/// assert_eq!(
///     hash.as_bytes()[..4],
///     [0x78, 0x6a, 0x02, 0xf7]
/// );
/// ```
pub fn blake2b_512(data: &[u8]) -> Result<Digest<64>> {
    let mut bytes = [0u8; 64];
    blake2b_into(data, b"", &mut bytes)?;
    Ok(Digest::from_bytes(bytes))
}
