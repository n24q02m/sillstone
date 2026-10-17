//! MurmurHash3 x64 128-bit (SMHasher reference, Austin Appleby).
//!
//! Two 64-bit lanes over 16-byte bodies, the reference's descending
//! tail switch and the fmix64 finalization, byte-identical to the
//! `MurmurHash3_x64_128` reference implementation. The 16 digest bytes
//! are the reference's out-buffer order: `h1` little-endian followed
//! by `h2` little-endian (the `memcpy` the reference writes its
//! result through), which is also the byte order `reference.json` and
//! every SDK test pins.
//!
//! Vectors: the SMHasher reference values (the empty input is
//! definitionally all-zero; the short inputs cross-checked against
//! faithful ports of the reference such as the Python `mmh3` package's
//! digest bytes).

/// The body-mixing constant c1.
const C1: u64 = 0x87c3_7b91_1142_53d5;

/// The body-mixing constant c2.
const C2: u64 = 0x4cf5_ad43_2745_937f;

/// Computes the 128-bit MurmurHash3 x64 digest of `data` under
/// `seed`.
///
/// ```
/// use sillstone_digest::murmur3_x64_128;
/// // The reference's empty-input digest is all-zero at every seed.
/// assert_eq!(
///     murmur3_x64_128(b"", 0).to_string(),
///     "00000000000000000000000000000000",
/// );
/// assert_eq!(
///     murmur3_x64_128(b"hello", 0).to_string(),
///     "029bbd41b3a7d8cb191dae486a901e5b",
/// );
/// ```
#[must_use]
pub fn murmur3_x64_128(data: &[u8], seed: u32) -> crate::Digest<16> {
    let mut h1 = u64::from(seed);
    let mut h2 = u64::from(seed);

    let body_len = data.len() - (data.len() & 15);
    for block in data[..body_len].chunks_exact(16) {
        let mut k1 = u64::from_le_bytes(block[0..8].try_into().expect("8 bytes"));
        let mut k2 = u64::from_le_bytes(block[8..16].try_into().expect("8 bytes"));

        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(31);
        k1 = k1.wrapping_mul(C2);
        h1 ^= k1;
        h1 = h1.rotate_left(27);
        h1 = h1.wrapping_add(h2);
        h1 = h1.wrapping_mul(5).wrapping_add(0x52dc_e729);

        k2 = k2.wrapping_mul(C2);
        k2 = k2.rotate_left(33);
        k2 = k2.wrapping_mul(C1);
        h2 ^= k2;
        h2 = h2.rotate_left(31);
        h2 = h2.wrapping_add(h1);
        h2 = h2.wrapping_mul(5).wrapping_add(0x3849_5ab5);
    }

    // The tail, equivalent to the reference's fall-through `switch` on
    // `len & 15`: bytes 8.. mix into k2 (c2 then c1), bytes 0..8 into
    // k1 (c1 then c2), each byte `i` shifted by `(i mod 8) * 8`.
    let tail = &data[body_len..];
    let tail_len = tail.len() & 15;
    let mut k1: u64 = 0;
    let mut k2: u64 = 0;
    for (i, &byte) in tail[..tail_len].iter().enumerate().rev() {
        let place = u64::from(byte) << ((i % 8) * 8);
        if i >= 8 {
            k2 ^= place;
        } else {
            k1 ^= place;
        }
    }
    if tail_len > 8 {
        k2 = k2.wrapping_mul(C2);
        k2 = k2.rotate_left(33);
        k2 = k2.wrapping_mul(C1);
        h2 ^= k2;
    }
    if tail_len > 0 {
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(31);
        k1 = k1.wrapping_mul(C2);
        h1 ^= k1;
    }

    h1 ^= data.len() as u64;
    h2 ^= data.len() as u64;
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    h1 = fmix64(h1);
    h2 = fmix64(h2);
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);

    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&h1.to_le_bytes());
    out[8..].copy_from_slice(&h2.to_le_bytes());
    crate::Digest::from_bytes(out)
}

/// The fmix64 finalizer.
fn fmix64(mut k: u64) -> u64 {
    k ^= k >> 33;
    k = k.wrapping_mul(0xff51_afd7_ed55_8ccd);
    k ^= k >> 33;
    k = k.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    k ^= k >> 33;
    k
}

#[cfg(test)]
mod tests {
    use super::murmur3_x64_128;

    /// The SMHasher reference values, seed 0 and seeded, across tail
    /// lengths 0..8, 9..15 and body-plus-tail inputs.
    #[test]
    fn reference_vectors() {
        assert_eq!(
            murmur3_x64_128(b"", 0).to_string(),
            "00000000000000000000000000000000"
        );
        assert_eq!(
            murmur3_x64_128(b"hello", 0).to_string(),
            "029bbd41b3a7d8cb191dae486a901e5b"
        );
        assert_eq!(
            murmur3_x64_128(b"abc", 0).to_string(),
            "6778ad3f3f3f96b4522dca264174a23b"
        );
        assert_eq!(
            murmur3_x64_128(b"foo", 0).to_string(),
            "6145f501578671e2877dba2be487af7e"
        );
        assert_eq!(
            murmur3_x64_128(b"foobar", 0).to_string(),
            "455ac81671aed2bdafd6f8bae055a274"
        );
        assert_eq!(
            murmur3_x64_128(b"The quick brown fox jumps over the lazy dog", 0).to_string(),
            "6c1b07bc7bbc4be347939ac4a93c437a"
        );
        let bytes33: Vec<u8> = (0..33).collect();
        assert_eq!(
            murmur3_x64_128(&bytes33, 1).to_string(),
            "df0c60f0aaff3b9221e7844dad044ab3"
        );
        let bytes17: Vec<u8> = (0..17).collect();
        assert_eq!(
            murmur3_x64_128(&bytes17, 42).to_string(),
            "fba12a339df8088f6cdf19cd98b42b59"
        );
    }
}
