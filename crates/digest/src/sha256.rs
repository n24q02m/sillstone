//! FIPS 180-2 SHA-256.
//!
//! One `compress` step over the 64 round constants, invoked per 64-byte
//! block. The message schedule lives in a fixed `[u32; 64]`, so hostile
//! input lengths cannot drive an allocation.

use crate::error::Result;

/// The SHA-256 initial hash values (FIPS 180-2 5.3.3).
pub(crate) const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The 64 round constants (FIPS 180-2 4.2.2), the fractional parts of the
/// cube roots of the first 64 primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The 64-bit length suffix is appended big-endian, so the message length in
/// bits must fit: the FIPS padding shape cannot express a longer message.
const MAX_INPUT_BYTES: u64 = (u64::MAX >> 3) - 71;

/// Computes the SHA-256 digest of `data` (FIPS 180-2).
///
/// # Errors
///
/// [`crate::Error::TooLarge`] if `data` is longer than the padding format
/// can express.
pub fn sha256(data: &[u8]) -> Result<crate::Digest<32>> {
    if data.len() as u64 > MAX_INPUT_BYTES {
        return Err(crate::Error::too_large("sha256 input", usize::MAX));
    }

    let mut h = H0;
    let rem = absorb(&mut h, data);
    let h = finish(&h, rem, data.len() as u64);

    let mut out = [0u8; 32];
    for (word, chunk) in h.iter().zip(out.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    Ok(crate::Digest::from_bytes(out))
}

/// Absorbs `data`'s full 64-byte blocks into `h`, returning the
/// sub-block remainder (0..64 bytes) that [`finish`] pads.
///
/// Shared with SHA-1 (the same 64-byte block shape and padding); the
/// slice boundary between calls must fall on a block edge, which the
/// callers guarantee by always absorbing from a block-aligned state.
pub(crate) fn absorb<'a>(h: &mut [u32; 8], data: &'a [u8]) -> &'a [u8] {
    let (blocks, rem) = data.split_at(data.len() & !63);
    compress_blocks(h, blocks);
    rem
}

/// Compresses every 64-byte block of `blocks` (whose length is a
/// multiple of 64) into `h`, on the CPU's SHA extensions when present.
#[allow(unsafe_code)]
pub(crate) fn compress_blocks(h: &mut [u32; 8], blocks: &[u8]) {
    debug_assert!(blocks.len() % 64 == 0);
    #[cfg(all(feature = "std", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sha")
            && std::is_x86_feature_detected!("sse2")
            && std::is_x86_feature_detected!("ssse3")
            && std::is_x86_feature_detected!("sse4.1")
        {
            // SAFETY: the CPU features the callee enables were just detected.
            unsafe { sha_ni::compress(h, blocks) };
            return;
        }
    }
    #[cfg(all(feature = "std", target_arch = "aarch64"))]
    {
        if std::arch::is_aarch64_feature_detected!("sha2") {
            // SAFETY: the CPU feature the callee enables was just detected.
            unsafe { arm_sha2::compress(h, blocks) };
            return;
        }
    }
    for block in blocks.chunks_exact(64) {
        compress(h, block.try_into().expect("chunks_exact yields 64"));
    }
}

/// Pads the `rem` tail (0x80, zeros, the 64-bit big-endian bit length
/// of the whole `total_len`-byte message) into one or two final
/// blocks, compresses them and returns the finalized state.
///
/// Shared with SHA-1. A remainder that leaves fewer than 9 bytes takes
/// a second block, which the loop over full empty blocks handles
/// naturally.
pub(crate) fn finish(h: &[u32; 8], rem: &[u8], total_len: u64) -> [u32; 8] {
    let mut h = *h;
    let bit_len = total_len << 3;
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let pad_len = if rem.len() + 9 <= 64 { 64 } else { 128 };
    tail[pad_len - 8..pad_len].copy_from_slice(&bit_len.to_be_bytes());
    compress_blocks(&mut h, &tail[..pad_len]);
    h
}

/// One round with the working variables passed by name, so eight
/// consecutive rounds rotate the roles instead of moving the values.
macro_rules! round {
    ($a:ident, $b:ident, $c:ident, $d:ident, $e:ident, $f:ident, $g:ident, $h:ident, $k:expr, $w:expr) => {
        let t1 = $h
            .wrapping_add($e.rotate_right(6) ^ $e.rotate_right(11) ^ $e.rotate_right(25))
            .wrapping_add($g ^ ($e & ($f ^ $g)))
            .wrapping_add($k)
            .wrapping_add($w);
        let t2 = ($a.rotate_right(2) ^ $a.rotate_right(13) ^ $a.rotate_right(22))
            .wrapping_add(($a & $b) | ($c & ($a | $b)));
        $d = $d.wrapping_add(t1);
        $h = t1.wrapping_add(t2);
    };
}

/// One compression step: 64 rounds over a rolling 16-word schedule.
pub(crate) fn compress(h: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 16];
    for (word, chunk) in w.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes(chunk.try_into().expect("chunks_exact yields 4"));
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    for base in (0..64).step_by(16) {
        if base > 0 {
            for i in 0..16 {
                let w15 = w[(i + 1) & 15];
                let w2 = w[(i + 14) & 15];
                let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
                let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
                w[i] = w[i]
                    .wrapping_add(s0)
                    .wrapping_add(w[(i + 9) & 15])
                    .wrapping_add(s1);
            }
        }
        for i in (0..16).step_by(8) {
            let k = &K[base + i..base + i + 8];
            round!(a, b, c, d, e, f, g, hh, k[0], w[i]);
            round!(hh, a, b, c, d, e, f, g, k[1], w[i + 1]);
            round!(g, hh, a, b, c, d, e, f, k[2], w[i + 2]);
            round!(f, g, hh, a, b, c, d, e, k[3], w[i + 3]);
            round!(e, f, g, hh, a, b, c, d, k[4], w[i + 4]);
            round!(d, e, f, g, hh, a, b, c, k[5], w[i + 5]);
            round!(c, d, e, f, g, hh, a, b, k[6], w[i + 6]);
            round!(b, c, d, e, f, g, hh, a, k[7], w[i + 7]);
        }
    }
    for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *slot = slot.wrapping_add(value);
    }
}

/// The x86-64 SHA extensions kernel (`sha256rnds2`, `sha256msg1/2`).
#[cfg(all(feature = "std", target_arch = "x86_64"))]
#[allow(unsafe_code, unused_unsafe)]
mod sha_ni {
    use super::K;
    use core::arch::x86_64::*;

    /// # Safety
    ///
    /// The CPU must support SHA, SSE2, SSSE3 and SSE4.1; `blocks.len()`
    /// must be a multiple of 64.
    #[target_feature(enable = "sha,sse2,ssse3,sse4.1")]
    pub(super) unsafe fn compress(state: &mut [u32; 8], blocks: &[u8]) {
        unsafe {
            let mask = _mm_set_epi64x(0x0c0d_0e0f_0809_0a0b, 0x0405_0607_0001_0203);
            let dcba = _mm_loadu_si128(state.as_ptr().cast());
            let hgfe = _mm_loadu_si128(state.as_ptr().add(4).cast());
            let cdab = _mm_shuffle_epi32(dcba, 0xB1);
            let efgh = _mm_shuffle_epi32(hgfe, 0x1B);
            let mut s0 = _mm_alignr_epi8(cdab, efgh, 8); // ABEF
            let mut s1 = _mm_blend_epi16(efgh, cdab, 0xF0); // CDGH

            for block in blocks.chunks_exact(64) {
                let (save0, save1) = (s0, s1);
                let p = block.as_ptr();
                let mut m = [
                    _mm_shuffle_epi8(_mm_loadu_si128(p.cast()), mask),
                    _mm_shuffle_epi8(_mm_loadu_si128(p.add(16).cast()), mask),
                    _mm_shuffle_epi8(_mm_loadu_si128(p.add(32).cast()), mask),
                    _mm_shuffle_epi8(_mm_loadu_si128(p.add(48).cast()), mask),
                ];
                // Sixteen groups of four rounds, written out so every index is
                // a constant: the schedule words then stay in registers.
                macro_rules! group {
                    ($i:literal) => {{
                        if $i >= 4 {
                            let w4 = m[$i % 4];
                            let w3 = m[($i + 1) % 4];
                            let w2 = m[($i + 2) % 4];
                            let w1 = m[($i + 3) % 4];
                            let t = _mm_add_epi32(
                                _mm_sha256msg1_epu32(w4, w3),
                                _mm_alignr_epi8(w1, w2, 4),
                            );
                            m[$i % 4] = _mm_sha256msg2_epu32(t, w1);
                        }
                        let wk = _mm_add_epi32(
                            m[$i % 4],
                            _mm_loadu_si128(K.as_ptr().add(4 * $i).cast()),
                        );
                        s1 = _mm_sha256rnds2_epu32(s1, s0, wk);
                        s0 = _mm_sha256rnds2_epu32(s0, s1, _mm_shuffle_epi32(wk, 0x0E));
                    }};
                }
                group!(0);
                group!(1);
                group!(2);
                group!(3);
                group!(4);
                group!(5);
                group!(6);
                group!(7);
                group!(8);
                group!(9);
                group!(10);
                group!(11);
                group!(12);
                group!(13);
                group!(14);
                group!(15);
                s0 = _mm_add_epi32(s0, save0);
                s1 = _mm_add_epi32(s1, save1);
            }

            let feba = _mm_shuffle_epi32(s0, 0x1B);
            let dchg = _mm_shuffle_epi32(s1, 0xB1);
            let dcba = _mm_blend_epi16(feba, dchg, 0xF0);
            let hgfe = _mm_alignr_epi8(dchg, feba, 8);
            _mm_storeu_si128(state.as_mut_ptr().cast(), dcba);
            _mm_storeu_si128(state.as_mut_ptr().add(4).cast(), hgfe);
        }
    }
}

/// The ARMv8 cryptographic-extension kernel (`sha256h`, `sha256su0/1`).
#[cfg(all(feature = "std", target_arch = "aarch64"))]
#[allow(unsafe_code, unused_unsafe)]
mod arm_sha2 {
    use super::K;
    use core::arch::aarch64::*;

    /// # Safety
    ///
    /// The CPU must support the `sha2` extension; `blocks.len()` must be a
    /// multiple of 64.
    #[target_feature(enable = "sha2")]
    pub(super) unsafe fn compress(state: &mut [u32; 8], blocks: &[u8]) {
        unsafe {
            let mut s0 = vld1q_u32(state.as_ptr());
            let mut s1 = vld1q_u32(state.as_ptr().add(4));
            for block in blocks.chunks_exact(64) {
                let (save0, save1) = (s0, s1);
                let p = block.as_ptr();
                let mut m = [
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(p))),
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(p.add(16)))),
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(p.add(32)))),
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(p.add(48)))),
                ];
                macro_rules! group {
                    ($i:literal) => {{
                        if $i >= 4 {
                            m[$i % 4] = vsha256su1q_u32(
                                vsha256su0q_u32(m[$i % 4], m[($i + 1) % 4]),
                                m[($i + 2) % 4],
                                m[($i + 3) % 4],
                            );
                        }
                        let wk = vaddq_u32(m[$i % 4], vld1q_u32(K.as_ptr().add(4 * $i)));
                        let prev = s0;
                        s0 = vsha256hq_u32(s0, s1, wk);
                        s1 = vsha256h2q_u32(s1, prev, wk);
                    }};
                }
                group!(0);
                group!(1);
                group!(2);
                group!(3);
                group!(4);
                group!(5);
                group!(6);
                group!(7);
                group!(8);
                group!(9);
                group!(10);
                group!(11);
                group!(12);
                group!(13);
                group!(14);
                group!(15);
                s0 = vaddq_u32(s0, save0);
                s1 = vaddq_u32(s1, save1);
            }
            vst1q_u32(state.as_mut_ptr(), s0);
            vst1q_u32(state.as_mut_ptr().add(4), s1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dispatched kernel (SHA extensions where the CPU has them) must
    /// agree with the portable scalar compression on every block count.
    #[test]
    fn dispatched_kernel_matches_scalar() {
        let data: Vec<u8> = (0..64 * 9u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        for n in 0..=9 {
            let blocks = &data[..64 * n];
            let mut fast = H0;
            compress_blocks(&mut fast, blocks);
            let mut slow = H0;
            for block in blocks.chunks_exact(64) {
                compress(&mut slow, block.try_into().unwrap());
            }
            assert_eq!(fast, slow, "{n} blocks");
        }
    }
}
