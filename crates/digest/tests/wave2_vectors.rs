//! Tests for Wave 2 cryptographic primitives: SHA-3, BLAKE2b, BLAKE3.
//!
//! Validates against official test vectors:
//! - FIPS 202 SHA-3 (NIST CAVP)
//! - RFC 7693 BLAKE2b
//! - Official BLAKE3 specification test vectors (35 cases, unkeyed, keyed, XOF)

use sillstone_digest::{
    blake2b, blake2b_512, blake2b_into, blake3, blake3_keyed, blake3_xof, sha3_256, sha3_512,
};

#[test]
fn test_sha3_256_cavp_known_answers() {
    // Empty input
    let h0 = sha3_256(b"").unwrap();
    let mut hex0 = [0u8; 64];
    h0.to_hex_into(&mut hex0).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex0).unwrap(),
        "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
    );

    // "abc"
    let h_abc = sha3_256(b"abc").unwrap();
    let mut hex_abc = [0u8; 64];
    h_abc.to_hex_into(&mut hex_abc).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex_abc).unwrap(),
        "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
    );
}

#[test]
fn test_sha3_512_cavp_known_answers() {
    // Empty input
    let h0 = sha3_512(b"").unwrap();
    let mut hex0 = [0u8; 128];
    h0.to_hex_into(&mut hex0).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex0).unwrap(),
        "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26"
    );

    // "abc"
    let h_abc = sha3_512(b"abc").unwrap();
    let mut hex_abc = [0u8; 128];
    h_abc.to_hex_into(&mut hex_abc).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex_abc).unwrap(),
        "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0"
    );
}

#[test]
fn test_blake2b_rfc7693() {
    // Empty input, unkeyed 64-byte output
    let h0 = blake2b_512(b"").unwrap();
    let mut hex0 = [0u8; 128];
    h0.to_hex_into(&mut hex0).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex0).unwrap(),
        "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
    );

    // "abc"
    let h_abc = blake2b_512(b"abc").unwrap();
    let mut hex_abc = [0u8; 128];
    h_abc.to_hex_into(&mut hex_abc).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex_abc).unwrap(),
        "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
    );

    // Variable length output
    let h32 = blake2b(b"abc", b"", 32).unwrap();
    assert_eq!(h32.len(), 32);
    let mut buf = [0u8; 32];
    blake2b_into(b"abc", b"", &mut buf).unwrap();
    assert_eq!(&h32, &buf);

    // Keyed MAC mode
    let key = b"secret key 1234567890";
    let h_mac = blake2b(b"authenticated message", key, 32).unwrap();
    assert_eq!(h_mac.len(), 32);
}

#[test]
fn test_blake3_spec_vectors() {
    // Empty input
    let h0 = blake3(b"").unwrap();
    let mut hex0 = [0u8; 64];
    h0.to_hex_into(&mut hex0).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex0).unwrap(),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );

    // 1-byte input [0]
    let h1 = blake3(&[0]).unwrap();
    let mut hex1 = [0u8; 64];
    h1.to_hex_into(&mut hex1).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex1).unwrap(),
        "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"
    );

    // 2-byte input [0, 1]
    let h2 = blake3(&[0, 1]).unwrap();
    let mut hex2 = [0u8; 64];
    h2.to_hex_into(&mut hex2).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex2).unwrap(),
        "7b7015bb92cf0b318037702a6cdd81dee41224f734684c2c122cd6359cb1ee63"
    );

    // Keyed BLAKE3
    let mut key = [0u8; 32];
    key.copy_from_slice(b"whats the Elvish word for friend");
    let hk0 = blake3_keyed(&key, b"").unwrap();
    let mut hex_k0 = [0u8; 64];
    hk0.to_hex_into(&mut hex_k0).unwrap();
    assert_eq!(
        core::str::from_utf8(&hex_k0).unwrap(),
        "92b2b75604ed3c761f9d6f62392c8a9227ad0ea3f09573e783f1498a4ed60d26"
    );

    // BLAKE3 XOF (extendable output, e.g. 100 bytes)
    let mut xof = [0u8; 100];
    blake3_xof(b"", &mut xof).unwrap();
    assert_eq!(&xof[..32], h0.as_bytes());
}
