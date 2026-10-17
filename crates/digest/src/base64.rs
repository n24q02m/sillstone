//! Base64 (RFC 4648), the standard `A-Za-z0-9+/` alphabet with `=`
//! padding.
//!
//! Both directions write into caller-owned buffers and never
//! allocate, matching the crate's hex encoding: [`base64_encode`]
//! needs `encoded_len(input.len())` bytes of output, [`base64_decode`]
//! at most `encoded.len() / 4 * 3`. Decoding is strict and canonical
//! (the RFC 4648 §3.5 recommendation): the input length must be a
//! multiple of four with correct padding, only alphabet characters may
//! appear, and the discarded trailing bits of the last character must
//! be zero.

use crate::error::{Error, Result};

/// The 64 RFC 4648 alphabet characters, indexed by 6-bit value.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The encoded length of `len` bytes: four characters per three-byte
/// group, padded.
///
/// ```
/// assert_eq!(sillstone_digest::base64::encoded_len(0), 0);
/// assert_eq!(sillstone_digest::base64::encoded_len(1), 4);
/// assert_eq!(sillstone_digest::base64::encoded_len(3), 4);
/// assert_eq!(sillstone_digest::base64::encoded_len(6), 8);
/// ```
#[must_use]
pub const fn encoded_len(len: usize) -> usize {
    len.div_ceil(3) * 4
}

/// Encodes `data` into `out` (RFC 4648 §4).
///
/// `out` must hold at least [`encoded_len`]`(data.len())` bytes;
/// exactly that many are written.
///
/// ```
/// use sillstone_digest::base64::{base64_encode, encoded_len};
/// let mut out = [0u8; encoded_len(6)];
/// base64_encode(b"foobar", &mut out).unwrap();
/// assert_eq!(&out, b"Zm9vYmFy");
/// ```
///
/// # Errors
///
/// [`crate::Error::TooLarge`] if `out` is shorter than the encoding
/// needs.
pub fn base64_encode(data: &[u8], out: &mut [u8]) -> Result<()> {
    let needed = encoded_len(data.len());
    if out.len() < needed {
        return Err(Error::too_large("base64 output", needed));
    }

    let mut chunks = data.chunks_exact(3);
    let mut cursor = 0usize;
    for chunk in chunks.by_ref() {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out[cursor] = ALPHABET[((n >> 18) & 0x3f) as usize];
        out[cursor + 1] = ALPHABET[((n >> 12) & 0x3f) as usize];
        out[cursor + 2] = ALPHABET[((n >> 6) & 0x3f) as usize];
        out[cursor + 3] = ALPHABET[(n & 0x3f) as usize];
        cursor += 4;
    }
    let rem = chunks.remainder();
    match rem.len() {
        0 => {}
        1 => {
            let n = u32::from(rem[0]) << 16;
            out[cursor] = ALPHABET[((n >> 18) & 0x3f) as usize];
            out[cursor + 1] = ALPHABET[((n >> 12) & 0x3f) as usize];
            out[cursor + 2] = b'=';
            out[cursor + 3] = b'=';
        }
        _ => {
            let n = (u32::from(rem[0]) << 16) | (u32::from(rem[1]) << 8);
            out[cursor] = ALPHABET[((n >> 18) & 0x3f) as usize];
            out[cursor + 1] = ALPHABET[((n >> 12) & 0x3f) as usize];
            out[cursor + 2] = ALPHABET[((n >> 6) & 0x3f) as usize];
            out[cursor + 3] = b'=';
        }
    }
    Ok(())
}

/// Decodes one RFC 4648 alphabet character, or [`Error::BadValue`].
fn sextet(byte: u8) -> Result<u32> {
    let value = match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return Err(Error::BadValue("base64 input")),
    };
    Ok(u32::from(value))
}

/// Decodes canonical padded base64 from `encoded` into `out`,
/// returning the number of bytes written (RFC 4648 §4).
///
/// `out` must hold at least `encoded.len() / 4 * 3` bytes; the
/// returned count is that minus the padding.
///
/// ```
/// use sillstone_digest::base64::base64_decode;
/// let mut out = [0u8; 6];
/// let n = base64_decode(b"Zm9vYmFy", &mut out).unwrap();
/// assert_eq!(&out[..n], b"foobar");
/// ```
///
/// # Errors
///
/// [`crate::Error::TooLarge`] if `out` is shorter than the worst-case
/// decode; [`crate::Error::BadValue`] for a non-canonical input (a
/// character outside the alphabet, a pad character in the body,
/// non-zero discarded bits, or a length that is not a multiple of
/// four).
pub fn base64_decode(encoded: &[u8], out: &mut [u8]) -> Result<usize> {
    if encoded.len() % 4 != 0 {
        return Err(Error::BadValue("base64 input"));
    }
    let worst = encoded.len() / 4 * 3;
    if out.len() < worst {
        return Err(Error::too_large("base64 output", worst));
    }
    // `=` may only close the input: one or two pad characters.
    let pad = match encoded {
        [.., b'=', b'='] => 2,
        [.., b'='] => 1,
        _ => 0,
    };
    let body = encoded.len() - pad;

    let mut written = 0usize;
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for (i, &byte) in encoded.iter().enumerate() {
        if byte == b'=' {
            // Only the counted suffix may carry pad characters.
            if i < body {
                return Err(Error::BadValue("base64 input"));
            }
            continue;
        }
        acc = (acc << 6) | sextet(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out[written] = ((acc >> bits) & 0xff) as u8;
            written += 1;
        }
    }
    // Canonical trailing bits: a padded group discards two bits per
    // pad character, and they must be zero (RFC 4648 3.5).
    if acc & ((1 << bits) - 1) != 0 {
        return Err(Error::BadValue("base64 input"));
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::{base64_decode, base64_encode, encoded_len};

    /// The RFC 4648 §10 test vectors, both directions.
    #[test]
    fn rfc_4648_s10_vectors() {
        for (plain, encoded) in [
            (&b""[..], &b""[..]),
            (b"f", b"Zg=="),
            (b"fo", b"Zm8="),
            (b"foo", b"Zm9v"),
            (b"foob", b"Zm9vYg=="),
            (b"fooba", b"Zm9vYmE="),
            (b"foobar", b"Zm9vYmFy"),
        ] {
            let mut out = [0u8; 16];
            base64_encode(plain, &mut out[..encoded_len(plain.len())]).unwrap();
            assert_eq!(&out[..encoded_len(plain.len())], encoded, "{plain:?}");
            let mut back = [0u8; 16];
            let n = base64_decode(encoded, &mut back).unwrap();
            assert_eq!(&back[..n], plain, "{encoded:?}");
        }
    }

    /// Binary round-trips across group boundaries, including every
    /// 6-bit high value in the last character.
    #[test]
    fn binary_round_trips() {
        for len in 0..64usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let mut out = [0u8; 128];
            base64_encode(&data, &mut out[..encoded_len(len)]).unwrap();
            let mut back = [0u8; 128];
            let n = base64_decode(&out[..encoded_len(len)], &mut back).unwrap();
            assert_eq!(&back[..n], &data[..], "len {len}");
        }
    }

    /// A too-small output buffer is [`crate::Error::TooLarge`].
    #[test]
    fn short_output_is_too_large() {
        let mut out = [0u8; 3];
        assert!(matches!(
            base64_encode(b"foobar", &mut out),
            Err(crate::Error::TooLarge { .. })
        ));
        assert!(matches!(
            base64_decode(b"Zm9vYmFy", &mut out),
            Err(crate::Error::TooLarge { .. })
        ));
    }

    /// Non-canonical inputs are refused: bad characters, inner pads,
    /// non-zero discarded bits, a bad length, a double pad.
    #[test]
    fn non_canonical_inputs_are_refused() {
        let mut out = [0u8; 16];
        // A character outside the alphabet.
        assert!(base64_decode(b"Zm9*YmFy", &mut out).is_err());
        // A pad character inside the body.
        assert!(base64_decode(b"Zm=vYmFy", &mut out).is_err());
        // Non-zero discarded bits ("Zm9vYmFy" with the last character
        // bumped into the padding zone).
        assert!(base64_decode(b"Zg+=", &mut out).is_err());
        // Length not a multiple of four.
        assert!(base64_decode(b"Zm9", &mut out).is_err());
        // Two pads claiming two leftover bytes for a 2-byte group.
        assert!(base64_decode(b"Zg==", &mut out).is_ok());
        // One pad for a group that needs two.
        assert!(base64_decode(b"Zg=", &mut out).is_err());
    }
}
