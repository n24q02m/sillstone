//! RFC 2104 HMAC over the suite's SHA-1, SHA-256 and SHA-512 cores.
//!
//! HMAC xor-pads a (possibly pre-hashed) key block with `ipad` and
//! `opad` and wraps the inner hash around the message and the inner
//! digest. Both pad blocks are exactly one hash block wide, so the
//! inner pass is "compress the pad block, absorb the message, pad" and
//! the outer pass "compress the pad block, pad over the inner digest" —
//! the [`crate::sha256::absorb`]/[`finish`](crate::sha256::finish) and
//! the SHA-512 counterparts carry it with no allocation at any message
//! length.
//!
//! Test vectors: RFC 2202 for HMAC-SHA-1 and RFC 4231 for
//! HMAC-SHA-256/512 (the RFC 2104 appendix itself only tables the MD5
//! construction); every vector is reproduced in the module tests, the
//! conformance suite and `reference.json`.

use crate::Digest;
use crate::error::Result;

/// The 64-byte block size of SHA-1 and SHA-256 (RFC 2104 `B`).
const BLOCK64: usize = 64;

/// The 128-byte block size of SHA-512.
const BLOCK128: usize = 128;

/// The RFC 2104 inner pad byte.
const IPAD: u8 = 0x36;

/// The RFC 2104 outer pad byte.
const OPAD: u8 = 0x5c;

/// Builds the two 64-byte pad blocks for `key`, hashing it first when
/// it is longer than the block (RFC 2104 step 1: `K ⊕ ipad` with `K`
/// the hash of the over-long key). `hash` produces the `D`-byte
/// digest copied into the zeroed key block.
fn pad64<const D: usize>(
    key: &[u8],
    hash: impl FnOnce(&[u8]) -> Result<[u8; D]>,
) -> Result<([u8; 64], [u8; 64])> {
    let mut k = [0u8; BLOCK64];
    if key.len() > BLOCK64 {
        k[..D].copy_from_slice(&hash(key)?);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [IPAD; BLOCK64];
    let mut opad = [OPAD; BLOCK64];
    for (i, &byte) in k.iter().enumerate() {
        ipad[i] ^= byte;
        opad[i] ^= byte;
    }
    Ok((ipad, opad))
}

/// The 128-byte-block counterpart of [`pad64`] for SHA-512 keys;
/// SHA-512 cannot refuse a key, so this is infallible.
fn pad128(key: &[u8]) -> ([u8; BLOCK128], [u8; BLOCK128]) {
    let mut k = [0u8; BLOCK128];
    if key.len() > BLOCK128 {
        k[..64].copy_from_slice(crate::sha512(key).as_bytes());
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [IPAD; BLOCK128];
    let mut opad = [OPAD; BLOCK128];
    for (i, &byte) in k.iter().enumerate() {
        ipad[i] ^= byte;
        opad[i] ^= byte;
    }
    (ipad, opad)
}

/// Computes the HMAC-SHA-256 of `data` under `key` (RFC 2104; vectors
/// RFC 4231).
///
/// ```
/// use sillstone_digest::hmac_sha256;
/// // RFC 4231 test case 2: the "Jefe" vector.
/// assert_eq!(
///     hmac_sha256(b"Jefe", b"what do ya want for nothing?")
///         .unwrap()
///         .to_string(),
///     "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
/// );
/// ```
///
/// # Errors
///
/// [`crate::Error::TooLarge`] if `key` is longer than the SHA-256
/// padding format can express (unreachable for any real key).
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Digest<32>> {
    let (ipad, opad) = pad64(key, |k| Ok(*crate::sha256(k)?.as_bytes()))?;

    let mut h = crate::sha256::H0;
    let _ = crate::sha256::absorb(&mut h, &ipad);
    let rem = crate::sha256::absorb(&mut h, data);
    let inner = crate::sha256::finish(&h, rem, (BLOCK64 + data.len()) as u64);
    let mut inner_bytes = [0u8; 32];
    for (word, chunk) in inner.iter().zip(inner_bytes.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }

    let mut h = crate::sha256::H0;
    let _ = crate::sha256::absorb(&mut h, &opad);
    let words = crate::sha256::finish(&h, &inner_bytes, (BLOCK64 + 32) as u64);

    let mut out = [0u8; 32];
    for (word, chunk) in words.iter().zip(out.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    Ok(Digest::from_bytes(out))
}

/// Computes the HMAC-SHA-1 of `data` under `key` (RFC 2104; vectors
/// RFC 2202).
///
/// ```
/// use sillstone_digest::hmac_sha1;
/// // RFC 2202 test case 2: the "Jefe" vector.
/// assert_eq!(
///     hmac_sha1(b"Jefe", b"what do ya want for nothing?")
///         .unwrap()
///         .to_string(),
///     "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79",
/// );
/// ```
///
/// # Errors
///
/// [`crate::Error::TooLarge`] if `key` is longer than the SHA-1
/// padding format can express (unreachable for any real key).
pub fn hmac_sha1(key: &[u8], data: &[u8]) -> Result<Digest<20>> {
    let (ipad, opad) = pad64(key, |k| {
        let mut out = [0u8; 20];
        out.copy_from_slice(crate::sha1(k)?.as_bytes());
        Ok(out)
    })?;

    let mut h = crate::sha1::H0;
    let _ = crate::sha1::absorb(&mut h, &ipad);
    let rem = crate::sha1::absorb(&mut h, data);
    let inner = crate::sha1::finish(&h, rem, (BLOCK64 + data.len()) as u64);
    let mut inner_bytes = [0u8; 20];
    for (word, chunk) in inner.iter().zip(inner_bytes.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }

    let mut h = crate::sha1::H0;
    let _ = crate::sha1::absorb(&mut h, &opad);
    let words = crate::sha1::finish(&h, &inner_bytes, (BLOCK64 + 20) as u64);

    let mut out = [0u8; 20];
    for (word, chunk) in words.iter().zip(out.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    Ok(Digest::from_bytes(out))
}

/// Computes the HMAC-SHA-512 of `data` under `key` (RFC 2104; vectors
/// RFC 4231).
///
/// SHA-512 cannot refuse a key, so this is infallible.
///
/// ```
/// use sillstone_digest::hmac_sha512;
/// // RFC 4231 test case 2: the "Jefe" vector.
/// assert_eq!(
///     hmac_sha512(b"Jefe", b"what do ya want for nothing?").to_string(),
///     concat!(
///         "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea250554",
///         "9758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737",
///     ),
/// );
/// ```
#[must_use]
pub fn hmac_sha512(key: &[u8], data: &[u8]) -> Digest<64> {
    let (ipad, opad) = pad128(key);

    let mut h = crate::sha512::H0;
    let _ = crate::sha512::absorb(&mut h, &ipad);
    let rem = crate::sha512::absorb(&mut h, data);
    let inner = crate::sha512::finish512(&h, rem, (BLOCK128 + data.len()) as u64);
    let mut inner_bytes = [0u8; 64];
    for (word, chunk) in inner.iter().zip(inner_bytes.chunks_exact_mut(8)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }

    let mut h = crate::sha512::H0;
    let _ = crate::sha512::absorb(&mut h, &opad);
    let words = crate::sha512::finish512(&h, &inner_bytes, (BLOCK128 + 64) as u64);

    let mut out = [0u8; 64];
    for (word, chunk) in words.iter().zip(out.chunks_exact_mut(8)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    Digest::from_bytes(out)
}

#[cfg(test)]
mod tests {
    use super::{hmac_sha1, hmac_sha256, hmac_sha512};

    /// RFC 2202 HMAC-SHA-1: test cases 1, 2 and 6 (case 6 drives the
    /// over-long-key path, hashing the 80-byte key first).
    #[test]
    fn rfc_2202_hmac_sha1_vectors() {
        let key = [0x0b_u8; 20];
        assert_eq!(
            hmac_sha1(&key, b"Hi There").unwrap().to_string(),
            "b617318655057264e28bc0b6fb378c8ef146be00"
        );
        assert_eq!(
            hmac_sha1(b"Jefe", b"what do ya want for nothing?")
                .unwrap()
                .to_string(),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
        let long_key = [0xaa_u8; 80];
        assert_eq!(
            hmac_sha1(
                &long_key,
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )
            .unwrap()
            .to_string(),
            "aa4ae5e15272d00e95705637ce8a3b55ed402112"
        );
    }

    /// RFC 4231 HMAC-SHA-256: test cases 1, 2 and 7 (case 7 drives the
    /// over-long-key path with a 131-byte key).
    #[test]
    fn rfc_4231_hmac_sha256_vectors() {
        let key = [0x0b_u8; 20];
        assert_eq!(
            hmac_sha256(&key, b"Hi There").unwrap().to_string(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hmac_sha256(b"Jefe", b"what do ya want for nothing?")
                .unwrap()
                .to_string(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let long_key = [0xaa_u8; 131];
        assert_eq!(
            hmac_sha256(
                &long_key,
                b"This is a test using a larger than block-size key and a \
                  larger than block-size data. The key needs to be hashed \
                  before being used by the HMAC algorithm.",
            )
            .unwrap()
            .to_string(),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
        );
    }

    /// RFC 4231 HMAC-SHA-512: test cases 1, 2 and 7.
    #[test]
    fn rfc_4231_hmac_sha512_vectors() {
        let key = [0x0b_u8; 20];
        assert_eq!(
            hmac_sha512(&key, b"Hi There").to_string(),
            concat!(
                "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cd",
                "edaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854",
            )
        );
        assert_eq!(
            hmac_sha512(b"Jefe", b"what do ya want for nothing?").to_string(),
            concat!(
                "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea250554",
                "9758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737",
            )
        );
        let long_key = [0xaa_u8; 131];
        assert_eq!(
            hmac_sha512(
                &long_key,
                b"This is a test using a larger than block-size key and a \
                  larger than block-size data. The key needs to be hashed \
                  before being used by the HMAC algorithm.",
            )
            .to_string(),
            concat!(
                "e37b6a775dc87dbaa4dfa9f96e5e3ffddebd71f8867289865df5a32d20cdc944",
                "b6022cac3c4982b10d5eeb55c3e4de15134676fb6de0446065c97440fa8c6a58",
            )
        );
    }

    /// An empty key and an empty message are both legal inputs; the
    /// digests are the RFC 2104 construction over a zeroed key block
    /// (values recomputed against the Python `hmac` module during
    /// generation).
    #[test]
    fn empty_key_and_message() {
        assert_eq!(
            hmac_sha1(b"", b"").unwrap().to_string(),
            "fbdb1d1b18aa6c08324b7d64b71fb76370690e1d"
        );
        assert_eq!(
            hmac_sha256(b"", b"").unwrap().to_string(),
            "b613679a0814d9ec772f95d778c35fc5ff1697c493715653c6c712144292c5ad"
        );
        assert_eq!(
            hmac_sha512(b"", b"").to_string(),
            concat!(
                "b936cee86c9f87aa5d3c6f2e84cb5a4239a5fe50480a6ec66b70ab5b1f4ac673",
                "0c6c515421b327ec1d69402e53dfb49ad7381eb067b338fd7b0cb22247225d47",
            )
        );
    }

    /// A key exactly one block wide takes the no-hash path; a key one
    /// byte over takes the hash path — both land where the reference
    /// construction does (values recomputed against the Python
    /// `hmac` module during generation).
    #[test]
    fn key_block_boundaries() {
        assert_eq!(
            hmac_sha256(&[0x5a_u8; 64], b"pith").unwrap().to_string(),
            "ce01adb54159b9c7c722ea928b1dca33b866e4147e924a7e0da34d08bf04731e"
        );
        assert_eq!(
            hmac_sha256(&[0x5a_u8; 65], b"pith").unwrap().to_string(),
            "d46d5cc612e1c3570701177e2850acb85add57dbede0dcfcbb472e0e83f5736d"
        );
        assert_eq!(
            hmac_sha512(&[0x5a_u8; 128], b"pith").to_string(),
            concat!(
                "4bc48fec9e29b700f99a4909d60a02728d59b5f198f31ef0b6463c672504f1bd",
                "55758af5b6523d62dd734996c0fa9fdd6d1dca163a5c066de9816f311892ecf1",
            )
        );
        assert_eq!(
            hmac_sha512(&[0x5a_u8; 129], b"pith").to_string(),
            concat!(
                "fb7360804ba4788fdbef6656dabc7d8c96612ee8861fd6165d890cff3da864e8",
                "45b31d639f9bde9e08dd7755d0b8834f1a8de372d18190146afc40f7d394222a",
            )
        );
        assert_eq!(
            hmac_sha1(&[0x5a_u8; 64], b"pith").unwrap().to_string(),
            "14fd693305d0a13e6fa3940e8b6296323837fe4d"
        );
        assert_eq!(
            hmac_sha1(&[0x5a_u8; 65], b"pith").unwrap().to_string(),
            "03ca862849e9348f89ef6fac14dd68f43c7ea391"
        );
    }
}
