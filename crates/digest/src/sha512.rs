//! FIPS 180-4 SHA-512.
//!
//! One `compress` step over the 80 round constants, invoked per
//! 128-byte block. The message schedule lives in a fixed `[u64; 80]`,
//! so hostile input lengths cannot drive an allocation. The padding is
//! the SHA-256 shape scaled to the wider state: zeros, then a 128-bit
//! big-endian length suffix.
//!
//! The round constants are the fractional parts of the cube roots of
//! the first 80 primes, and the initial state the fractional parts of
//! the square roots of the first 8 primes (FIPS 180-4 4.2.3 / 5.3.5);
//! both tables below are those definitions carried verbatim.

use crate::Digest;

/// The SHA-512 initial hash values (FIPS 180-4 5.3.5).
pub(crate) const H0: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

/// The 80 round constants (FIPS 180-4 4.2.3).
const K: [u64; 80] = [
    0x428a_2f98_d728_ae22,
    0x7137_4491_23ef_65cd,
    0xb5c0_fbcf_ec4d_3b2f,
    0xe9b5_dba5_8189_dbbc,
    0x3956_c25b_f348_b538,
    0x59f1_11f1_b605_d019,
    0x923f_82a4_af19_4f9b,
    0xab1c_5ed5_da6d_8118,
    0xd807_aa98_a303_0242,
    0x1283_5b01_4570_6fbe,
    0x2431_85be_4ee4_b28c,
    0x550c_7dc3_d5ff_b4e2,
    0x72be_5d74_f27b_896f,
    0x80de_b1fe_3b16_96b1,
    0x9bdc_06a7_25c7_1235,
    0xc19b_f174_cf69_2694,
    0xe49b_69c1_9ef1_4ad2,
    0xefbe_4786_384f_25e3,
    0x0fc1_9dc6_8b8c_d5b5,
    0x240c_a1cc_77ac_9c65,
    0x2de9_2c6f_592b_0275,
    0x4a74_84aa_6ea6_e483,
    0x5cb0_a9dc_bd41_fbd4,
    0x76f9_88da_8311_53b5,
    0x983e_5152_ee66_dfab,
    0xa831_c66d_2db4_3210,
    0xb003_27c8_98fb_213f,
    0xbf59_7fc7_beef_0ee4,
    0xc6e0_0bf3_3da8_8fc2,
    0xd5a7_9147_930a_a725,
    0x06ca_6351_e003_826f,
    0x1429_2967_0a0e_6e70,
    0x27b7_0a85_46d2_2ffc,
    0x2e1b_2138_5c26_c926,
    0x4d2c_6dfc_5ac4_2aed,
    0x5338_0d13_9d95_b3df,
    0x650a_7354_8baf_63de,
    0x766a_0abb_3c77_b2a8,
    0x81c2_c92e_47ed_aee6,
    0x9272_2c85_1482_353b,
    0xa2bf_e8a1_4cf1_0364,
    0xa81a_664b_bc42_3001,
    0xc24b_8b70_d0f8_9791,
    0xc76c_51a3_0654_be30,
    0xd192_e819_d6ef_5218,
    0xd699_0624_5565_a910,
    0xf40e_3585_5771_202a,
    0x106a_a070_32bb_d1b8,
    0x19a4_c116_b8d2_d0c8,
    0x1e37_6c08_5141_ab53,
    0x2748_774c_df8e_eb99,
    0x34b0_bcb5_e19b_48a8,
    0x391c_0cb3_c5c9_5a63,
    0x4ed8_aa4a_e341_8acb,
    0x5b9c_ca4f_7763_e373,
    0x682e_6ff3_d6b2_b8a3,
    0x748f_82ee_5def_b2fc,
    0x78a5_636f_4317_2f60,
    0x84c8_7814_a1f0_ab72,
    0x8cc7_0208_1a64_39ec,
    0x90be_fffa_2363_1e28,
    0xa450_6ceb_de82_bde9,
    0xbef9_a3f7_b2c6_7915,
    0xc671_78f2_e372_532b,
    0xca27_3ece_ea26_619c,
    0xd186_b8c7_21c0_c207,
    0xeada_7dd6_cde0_eb1e,
    0xf57d_4f7f_ee6e_d178,
    0x06f0_67aa_7217_6fba,
    0x0a63_7dc5_a2c8_98a6,
    0x113f_9804_bef9_0dae,
    0x1b71_0b35_131c_471b,
    0x28db_77f5_2304_7d84,
    0x32ca_ab7b_40c7_2493,
    0x3c9e_be0a_15c9_bebc,
    0x431d_67c4_9c10_0d4c,
    0x4cc5_d4be_cb3e_42b6,
    0x597f_299c_fc65_7e2a,
    0x5fcb_6fab_3ad6_faec,
    0x6c44_198c_4a47_5817,
];

/// Computes the SHA-512 digest of `data` (FIPS 180-4).
///
/// The 128-bit length suffix can express any byte length a real
/// machine can hand over, so unlike [`crate::sha256`] this cannot
/// refuse an input: the function is infallible.
///
/// ```
/// assert_eq!(
///     sillstone_digest::sha512(b"abc").to_string(),
///     "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
///      2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
///         .chars()
///         .filter(|c| *c != ' ')
///         .collect::<String>(),
/// );
/// ```
#[must_use]
pub fn sha512(data: &[u8]) -> Digest<64> {
    let mut h = H0;
    let mut chunks = data.chunks_exact(128);
    for block in chunks.by_ref() {
        compress(&mut h, block.try_into().expect("chunks_exact yields 128"));
    }

    // Padding: 0x80, zeros, then the 128-bit big-endian bit length,
    // filling one or two final blocks. A remainder that leaves fewer
    // than 17 bytes takes a second block, which the loop over full
    // empty blocks handles naturally.
    let rem = chunks.remainder();
    let bit_len = (data.len() as u128) << 3;
    let mut tail = [0u8; 256];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let pad_len = if rem.len() + 17 <= 128 { 128 } else { 256 };
    tail[pad_len - 16..pad_len].copy_from_slice(&bit_len.to_be_bytes());
    for block in tail[..pad_len].chunks_exact(128) {
        compress(&mut h, block.try_into().expect("chunks_exact yields 128"));
    }

    let mut out = [0u8; 64];
    for (word, chunk) in h.iter().zip(out.chunks_exact_mut(8)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    Digest::from_bytes(out)
}

/// Absorbs `data`'s full 128-byte blocks into `h`, returning the
/// sub-block remainder for [`finish512`]. Only the remainder state
/// carries between calls: callers absorb from a block-aligned state.
pub(crate) fn absorb<'a>(h: &mut [u64; 8], data: &'a [u8]) -> &'a [u8] {
    let mut chunks = data.chunks_exact(128);
    for block in chunks.by_ref() {
        compress(h, block.try_into().expect("chunks_exact yields 128"));
    }
    chunks.remainder()
}

/// Pads the `rem` tail with the 128-bit big-endian bit length of the
/// whole `total_len`-byte message, compresses the final block(s) and
/// returns the finalized state. The HMAC layer finishes
/// block-aligned-prefix messages this way.
pub(crate) fn finish512(h: &[u64; 8], rem: &[u8], total_len: u64) -> [u64; 8] {
    let mut h = *h;
    let bit_len = u128::from(total_len) << 3;
    let mut tail = [0u8; 256];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let pad_len = if rem.len() + 17 <= 128 { 128 } else { 256 };
    tail[pad_len - 16..pad_len].copy_from_slice(&bit_len.to_be_bytes());
    for block in tail[..pad_len].chunks_exact(128) {
        compress(&mut h, block.try_into().expect("chunks_exact yields 128"));
    }
    h
}

/// One compression step: 80 rounds over the message schedule for
/// `block`.
fn compress(h: &mut [u64; 8], block: &[u8; 128]) {
    let mut w = [0u64; 80];
    for (word, chunk) in w.iter_mut().zip(block.chunks_exact(8)) {
        *word = u64::from_be_bytes(chunk.try_into().expect("chunks_exact yields 8"));
    }
    for i in 16..80 {
        let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
        let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    for (i, &word) in w.iter().enumerate() {
        let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
        let ch = (e & f) ^ (!e & g);
        let t1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(word);
        let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::{absorb, finish512, sha512};

    /// The FIPS 180-4 example vectors, including the empty message and
    /// the two-block 56-byte input.
    #[test]
    fn fips_180_4_vectors() {
        assert_eq!(
            sha512(b"").to_string(),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
                .chars()
                .filter(|c| *c != ' ')
                .collect::<String>()
        );
        assert_eq!(
            sha512(b"abc").to_string(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
                .chars()
                .filter(|c| *c != ' ')
                .collect::<String>()
        );
        assert_eq!(
            sha512(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq").to_string(),
            "204a8fc6dda82f0a0ced7beb8e08a41657c16ef468b228a8279be331a703c335\
             96fd15c13b1b07f9aa1d3bea57789ca031ad85c7a71dd70354ec631238ca3445"
                .chars()
                .filter(|c| *c != ' ')
                .collect::<String>()
        );
        assert_eq!(
            sha512(&[b'a'; 1_000_000]).to_string(),
            "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973eb\
             de0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b"
                .chars()
                .filter(|c| *c != ' ')
                .collect::<String>()
        );
    }

    /// One and two padding blocks around the 111/112-byte remainder
    /// boundary.
    #[test]
    fn padding_boundaries() {
        assert_eq!(
            sha512(&[b'x'; 111]).to_string(),
            "9a2a120825c2319867758ec277924f6faa254968bf752046dacdd948d8ad299b\
             10359fd04bfd7d3810b5fa1b16a294236138baff981cbb85248478053ac4d3dd"
                .chars()
                .filter(|c| *c != ' ')
                .collect::<String>()
        );
        assert_eq!(
            sha512(&[b'x'; 112]).to_string(),
            "a3722b515ef40c910f2419f6e0da8ca51d410114ce6272faae64045f9e9f630e\
             7fa8dd5a3243c9860b899d148c3da4bc0f9e07454542604d030bb55531fe0d5b"
                .chars()
                .filter(|c| *c != ' ')
                .collect::<String>()
        );
    }

    /// The absorb/finish pair used by the HMAC layer reproduces the
    /// one-shot digest for a block-aligned prefix plus a tail.
    #[test]
    fn absorb_finish_matches_one_shot() {
        for len in [0usize, 1, 111, 112, 127, 128, 129, 200] {
            let data = vec![b'p'; len];
            let mut h = super::H0;
            let rem = absorb(&mut h, &data);
            let h = finish512(&h, rem, len as u64);
            let mut out = [0u8; 64];
            for (word, chunk) in h.iter().zip(out.chunks_exact_mut(8)) {
                chunk.copy_from_slice(&word.to_be_bytes());
            }
            assert_eq!(out, *sha512(&data).as_bytes(), "len {len}");
        }
    }
}
