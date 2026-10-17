//! FIPS 180-4 SHA-1.
//!
//! One `compress` step over the 80 round constants, invoked per 64-byte
//! block. The message schedule lives in a fixed `[u32; 80]`, so hostile
//! input lengths cannot drive an allocation. The block absorption and
//! the padding are the SHA-256 shape (the same 64-byte block, the same
//! 64-bit big-endian length suffix), so [`absorb`] and [`finish`] are
//! shared with `sha256`.
//!
//! SHA-1 is collision-broken (SHAttered 2017) and must not be used for
//! signatures; the suite carries it because on-disk records and
//! third-party feeds still name it.

use crate::error::Result;

/// The SHA-1 initial hash values (FIPS 180-4 5.3.1).
pub(crate) const H0: [u32; 5] = [
    0x6745_2301,
    0xefcd_ab89,
    0x98ba_dcfe,
    0x1032_5476,
    0xc3d2_e1f0,
];

/// The four round constants (FIPS 180-4 4.2.1), one per 20-round
/// group: the fractional parts of the square roots of 2, 3, 5 and 10.
const K: [u32; 4] = [0x5a82_7999, 0x6ed9_eba1, 0x8f1b_bcdc, 0xca62_c1d6];

/// The 64-bit length suffix is appended big-endian, so the message
/// length in bits must fit: the FIPS padding shape cannot express a
/// longer message.
const MAX_INPUT_BYTES: u64 = (u64::MAX >> 3) - 71;

/// Computes the SHA-1 digest of `data` (FIPS 180-4).
///
/// ```
/// assert_eq!(
///     sillstone_digest::sha1(b"abc").unwrap().to_string(),
///     "a9993e364706816aba3e25717850c26c9cd0d89d",
/// );
/// ```
///
/// # Errors
///
/// [`crate::Error::TooLarge`] if `data` is longer than the padding format
/// can express.
pub fn sha1(data: &[u8]) -> Result<crate::Digest<20>> {
    if data.len() as u64 > MAX_INPUT_BYTES {
        return Err(crate::Error::too_large("sha1 input", usize::MAX));
    }

    let mut h = H0;
    let rem = absorb(&mut h, data);
    let h = finish(&h, rem, data.len() as u64);

    let mut out = [0u8; 20];
    for (word, chunk) in h.iter().zip(out.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    Ok(crate::Digest::from_bytes(out))
}

/// Absorbs `data`'s full 64-byte blocks into `h`, returning the
/// sub-block remainder (0..64 bytes) that [`finish`] pads. The
/// SHA-256 block shape with the 5-word SHA-1 state; the slice
/// boundary between calls must fall on a block edge, which the
/// callers guarantee by always absorbing from a block-aligned state.
pub(crate) fn absorb<'a>(h: &mut [u32; 5], data: &'a [u8]) -> &'a [u8] {
    let mut chunks = data.chunks_exact(64);
    for block in chunks.by_ref() {
        compress(h, block.try_into().expect("chunks_exact yields 64"));
    }
    chunks.remainder()
}

/// Pads the `rem` tail (0x80, zeros, the 64-bit big-endian bit length
/// of the whole `total_len`-byte message) into one or two final
/// blocks, compresses them and returns the finalized state.
pub(crate) fn finish(h: &[u32; 5], rem: &[u8], total_len: u64) -> [u32; 5] {
    let mut h = *h;
    let bit_len = total_len << 3;
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let pad_len = if rem.len() + 9 <= 64 { 64 } else { 128 };
    tail[pad_len - 8..pad_len].copy_from_slice(&bit_len.to_be_bytes());
    for block in tail[..pad_len].chunks_exact(64) {
        compress(&mut h, block.try_into().expect("chunks_exact yields 64"));
    }
    h
}

/// One compression step: 80 rounds over the message schedule for
/// `block`.
fn compress(h: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (word, chunk) in w.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes(chunk.try_into().expect("chunks_exact yields 4"));
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *h;
    for (i, &word) in w.iter().enumerate() {
        let (f, k) = match i {
            0..=19 => ((b & c) | (!b & d), K[0]),
            20..=39 => (b ^ c ^ d, K[1]),
            40..=59 => ((b & c) | (b & d) | (c & d), K[2]),
            _ => (b ^ c ^ d, K[3]),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(word);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }
    for (slot, value) in h.iter_mut().zip([a, b, c, d, e]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::sha1;

    /// The FIPS 180-4 example vectors (RFC 3174 7.3 carries the same
    /// digests).
    #[test]
    fn fips_180_4_vectors() {
        assert_eq!(
            sha1(b"").unwrap().to_string(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(
            sha1(b"abc").unwrap().to_string(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")
                .unwrap()
                .to_string(),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    /// One and two padding blocks: the FIPS two-block "a x 1 000 000"
    /// vector and the 55/56-byte remainder boundary.
    #[test]
    fn padding_boundaries() {
        assert_eq!(
            sha1(&[b'a'; 1_000_000]).unwrap().to_string(),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
        assert_eq!(
            sha1(&[b'x'; 55]).unwrap().to_string(),
            "cef734ba81a024479e09eb5a75b6ddae62e6abf1"
        );
        assert_eq!(
            sha1(&[b'x'; 56]).unwrap().to_string(),
            "901305367c259952f4e7af8323f480d59f81335b"
        );
        assert_eq!(
            sha1(&[b'x'; 64]).unwrap().to_string(),
            "bb2fa3ee7afb9f54c6dfb5d021f14b1ffe40c163"
        );
        assert_eq!(
            sha1(&[b'x'; 65]).unwrap().to_string(),
            "78c741ddc482e4cdf8c474a0876347a0905b6233"
        );
    }
}
