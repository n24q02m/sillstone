//! Conformance vectors for the computational primitives. Every value
//! here is a published test vector (FIPS 180-2, RFC 1950, the PNG
//! spec, the FNV reference, the splitmix64 paper) or arithmetic that
//! pins a bit-order convention the codec crates build on.

use sillstone_digest::{BitReader, SplitMix64, adler32, crc32, fnv1a64, sha256};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[test]
fn sha256_fips_180_2_vectors() {
    // The three FIPS 180-2 appendix B vectors plus the empty string.
    let cases: [(&[u8], &str); 4] = [
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            &[b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
        (
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
    ];
    for (input, want) in cases {
        let got = hex(sha256(input).expect("within size limit").as_bytes());
        assert_eq!(got, want, "sha256({:?}...)", &input[..input.len().min(12)]);
    }
}

#[test]
fn sha256_digests_across_block_and_tail_boundaries() {
    // Inputs around the 55/56/64/65-byte padding boundaries: each exercises
    // a different tail-block path, and the one-million-'a' FIPS vector above
    // pins the multi-block path.
    let reference: Vec<(usize, String)> = (0..=66)
        .map(|n| (n, hex(sha256(&vec![b'x'; n]).unwrap().as_bytes())))
        .collect();
    // Self-consistency alone is vacuous, so pin three boundary lengths
    // against python hashlib values computed out of band.
    let pinned: [(usize, &str); 3] = [
        (
            55,
            "d5e285683cd4efc02d021a5c62014694958901005d6f71e89e0989fac77e4072",
        ),
        (
            56,
            "04c26261370ee7541549d16dee320c723e3fd14671e66a099afe0a377c16888e",
        ),
        (
            64,
            "7ce100971f64e7001e8fe5a51973ecdfe1ced42befe7ee8d5fd6219506b5393c",
        ),
    ];
    for (n, want) in pinned {
        let (_, got) = &reference[n];
        assert_eq!(got, want, "sha256 boundary pin at {n} bytes");
    }
}

#[test]
fn crc32_png_spec_vectors() {
    assert_eq!(crc32(b""), 0x0000_0000);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(
        crc32(b"The quick brown fox jumps over the lazy dog"),
        0x414F_A339
    );
}

#[test]
fn adler32_rfc1950_vectors() {
    assert_eq!(adler32(b""), 0x0000_0001);
    assert_eq!(adler32(b"a"), 0x0062_0062);
    assert_eq!(adler32(b"abc"), 0x024D_0127);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    // Longer than NMAX (5552): the modulo-per-chunk path must agree with
    // the byte-at-a-time reference for the same input.
    let long = vec![0xABu8; 12_000];
    let mut slow_a: u32 = 1;
    let mut slow_b: u32 = 0;
    for &byte in &long {
        slow_a = (slow_a + u32::from(byte)) % 65_521;
        slow_b = (slow_b + slow_a) % 65_521;
    }
    assert_eq!(adler32(&long), (slow_b << 16) | slow_a);
}

#[test]
fn fnv1a64_reference_vectors() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn splitmix64_paper_sequence() {
    let mut rng = SplitMix64::new(0);
    assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
    assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    // Same seed, same sequence: the property tests across the workspace
    // depend on this being reproducible on every machine.
    let mut a = SplitMix64::new(42);
    let mut b = SplitMix64::new(42);
    for _ in 0..16 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
    // fill_bytes chunks a stream of words little-endian and fills exactly.
    let mut bytes = [0u8; 10];
    SplitMix64::new(7).fill_bytes(&mut bytes);
    let mut words = SplitMix64::new(7);
    let w0 = words.next_u64().to_le_bytes();
    assert_eq!(&bytes[..8], &w0);
    let w1 = words.next_u64().to_le_bytes();
    assert_eq!(&bytes[8..], &w1[..2]);
}

#[test]
fn bitreader_msb_stream_fields() {
    // 0xA6 = 0b1010_0110: first 3 stream bits are 101, next 5 are 00110.
    let mut r = BitReader::new(&[0xA6, 0x50]);
    assert_eq!(r.bits(3).unwrap(), 0b101);
    assert_eq!(r.bits(5).unwrap(), 0b0_0110);
    assert!(r.is_aligned());
    // A field may span the byte boundary: remaining 0x50 high bits plus
    // the next byte's top bits reassemble MSB-first across bytes.
    let mut r = BitReader::new(&[0xA6, 0x50]);
    assert_eq!(r.bits(12).unwrap(), 0xA65);
    // MSB order over the whole first byte reads back the byte itself.
    let mut r = BitReader::new(&[0xA6, 0x50]);
    assert_eq!(r.bits(8).unwrap(), 0xA6);
}

#[test]
fn bitreader_truncation_is_error_not_panic() {
    let mut r = BitReader::new(&[0xFF]);
    assert!(r.bits(9).is_err());
    // Exhausted exactly: 8 bits then zero remain.
    assert_eq!(r.bits(8).unwrap(), 0xFF);
    assert_eq!(r.remaining_bits(), 0);
    assert!(r.bits(1).is_err());
}

#[test]
fn bitreader_byte_align_and_byte() {
    let mut r = BitReader::new(&[0x00, 0xAB]);
    assert_eq!(r.bits(3).unwrap(), 0);
    assert!(!r.is_aligned());
    r.byte_align();
    assert!(r.is_aligned());
    assert_eq!(r.byte().unwrap(), 0xAB);
    assert!(r.byte().is_err());
    // Unaligned byte() is refused, not rounded.
    let mut r = BitReader::new(&[0xFF, 0x00]);
    r.bits(1).unwrap();
    assert!(r.byte().is_err());
}

#[test]
fn bitreader_zero_width_read_is_zero_at_any_position() {
    // Zero-width reads are legal no-ops (pre-fast-path semantics) at
    // byte-aligned AND mid-byte positions; they must not shift the stream.
    let mut r = BitReader::new(&[0xA6, 0x50]);
    assert_eq!(r.bits(0).unwrap(), 0);
    assert_eq!(r.bits(3).unwrap(), 0b101);
    assert_eq!(r.bits(0).unwrap(), 0);
    assert_eq!(r.bits(5).unwrap(), 0b0_0110);
    // Position unchanged by the interleaved zero-width reads.
    let mut r = BitReader::new(&[0xA6, 0x50]);
    r.bits(0).unwrap();
    assert_eq!(r.bits(12).unwrap(), 0xA65);
    // Exhausted stream: zero-width reads stay legal no-ops.
    let mut r = BitReader::new(&[0xFF]);
    assert_eq!(r.bits(8).unwrap(), 0xFF);
    assert_eq!(r.remaining_bits(), 0);
    assert_eq!(r.bits(0).unwrap(), 0);
}

// ----------------------------------------------------------------- sha1

/// The four FIPS 180-4 appendix vectors for SHA-1 (the "abc" chain,
/// the nopnopq chain, the one-million-'a' stress and the empty
/// string).
#[test]
fn sha1_fips_180_4_vectors() {
    let cases: [(&[u8], &str); 4] = [
        (b"abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1",
        ),
        (
            &[b'a'; 1_000_000],
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f",
        ),
        (b"", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
    ];
    for (input, want) in cases {
        let got = hex(sillstone_digest::sha1(input)
            .expect("within size limit")
            .as_bytes());
        assert_eq!(got, want, "sha1({:?}...)", &input[..input.len().min(12)]);
    }
}

// --------------------------------------------------------------- sha512

/// The four FIPS 180-4 appendix vectors for SHA-512.
#[test]
fn sha512_fips_180_4_vectors() {
    let cases: [(&[u8], &str); 4] = [
        (
            b"abc",
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "204a8fc6dda82f0a0ced7beb8e08a41657c16ef468b228a8279be331a703c33596fd15c13b1b07f9aa1d3bea57789ca031ad85c7a71dd70354ec631238ca3445",
        ),
        (
            &[b'a'; 1_000_000],
            "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973ebde0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b",
        ),
        (
            b"",
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
        ),
    ];
    for (input, want) in cases {
        let got = hex(sillstone_digest::sha512(input).as_bytes());
        assert_eq!(got, want, "sha512({:?}...)", &input[..input.len().min(12)]);
    }
}

// ----------------------------------------------------------------- hmac

/// RFC 2104 test cases 1 and 2 for HMAC-SHA-1, plus the RFC 2202
/// test-case-6 over-long key.
#[test]
fn hmac_sha1_rfc_2104_vectors() {
    let cases: [(&[u8], &[u8], &str); 3] = [
        (
            &[0x0b; 20],
            b"Hi There",
            "b617318655057264e28bc0b6fb378c8ef146be00",
        ),
        (
            b"Jefe",
            b"what do ya want for nothing?",
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79",
        ),
        (
            &[0xaa; 80],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
            "aa4ae5e15272d00e95705637ce8a3b55ed402112",
        ),
    ];
    for (key, data, want) in cases {
        let got = hex(sillstone_digest::hmac_sha1(key, data)
            .expect("legal inputs")
            .as_bytes());
        assert_eq!(got, want, "hmac-sha1(key[..], data={:?})", data);
    }
}

/// RFC 2104 test cases 1 and 2, plus RFC 4231 test case 6 (the
/// 131-byte key), for HMAC-SHA-256.
#[test]
fn hmac_sha256_rfc_2104_vectors() {
    let cases: [(&[u8], &[u8], &str); 3] = [
        (
            &[0x0b; 20],
            b"Hi There",
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        ),
        (
            b"Jefe",
            b"what do ya want for nothing?",
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        ),
        (
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        ),
    ];
    for (key, data, want) in cases {
        let got = hex(sillstone_digest::hmac_sha256(key, data)
            .expect("legal inputs")
            .as_bytes());
        assert_eq!(got, want, "hmac-sha256(key[..], data={:?})", data);
    }
}

/// RFC 2104 test cases 1 and 2, plus RFC 4231 test case 6, for
/// HMAC-SHA-512.
#[test]
fn hmac_sha512_rfc_2104_vectors() {
    let cases: [(&[u8], &[u8], &str); 3] = [
        (
            &[0x0b; 20],
            b"Hi There",
            "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854",
        ),
        (
            b"Jefe",
            b"what do ya want for nothing?",
            "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737",
        ),
        (
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
            "80b24263c7c1a3ebb71493c1dd7be8b49b46d1f41b4aeec1121b013783f8f3526b56d037e05f2598bd0fd2215d6a1e5295e64f73f63f0aec8b915a985d786598",
        ),
    ];
    for (key, data, want) in cases {
        let got = hex(sillstone_digest::hmac_sha512(key, data).as_bytes());
        assert_eq!(got, want, "hmac-sha512(key[..], data={:?})", data);
    }
}

// ---------------------------------------------------------------- xxh64

/// The xxHash reference values spanning every branch of the algorithm:
/// the empty-input constants, the byte/word/doubleword tails and the
/// 32-byte stripe path.
#[test]
fn xxh64_reference_vectors() {
    let cases: [(u64, &[u8], u64); 8] = [
        (0, b"", 0xEF46_DB37_51D8_E999),
        (1, b"", 0xD5AF_BA13_36A3_BE4B),
        (0, b"abc", 0x44BC_2CF5_AD77_0999),
        (0, b"Hello, world!", 0xF583_36A7_8B6F_9476),
        (
            0,
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23,
            ],
            0xDD14_A029_2632_BE9B,
        ),
        (
            7,
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30,
            ],
            0x0BDB_BCAE_AD6C_6E56,
        ),
        (0, &(0..32u8).collect::<Vec<u8>>(), 0xCBF5_9C51_16FF_32B4),
        (0, &(0..33u8).collect::<Vec<u8>>(), 0x0C53_5D1A_CAFB_8EAD),
    ];
    for (seed, data, want) in cases {
        assert_eq!(
            sillstone_digest::xxh64(data, seed),
            want,
            "xxh64(seed={seed}, len={})",
            data.len()
        );
    }
}

// -------------------------------------------------------------- murmur3

/// The SMHasher reference vectors for MurmurHash3 x64 128.
#[test]
fn murmur3_x64_128_smhasher_vectors() {
    let cases: [(u32, &[u8], [u8; 16]); 7] = [
        (
            0,
            b"",
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        ),
        (
            1,
            b"",
            [
                0xb5, 0x5c, 0xff, 0x6e, 0xe5, 0xab, 0x10, 0x46, 0x83, 0x35, 0xf8, 0x78, 0xaa, 0x2d,
                0x62, 0x51,
            ],
        ),
        (
            0,
            b"hello",
            [
                0x02, 0x9b, 0xbd, 0x41, 0xb3, 0xa7, 0xd8, 0xcb, 0x19, 0x1d, 0xae, 0x48, 0x6a, 0x90,
                0x1e, 0x5b,
            ],
        ),
        (
            0,
            &(0..32u8).collect::<Vec<u8>>(),
            [
                0x0f, 0x50, 0x2f, 0xb6, 0x22, 0x90, 0x6d, 0xc6, 0x51, 0x11, 0xc3, 0x34, 0x6e, 0x0a,
                0x05, 0x1c,
            ],
        ),
        (
            0,
            &(0..100u8).collect::<Vec<u8>>(),
            [
                0xca, 0x51, 0x40, 0xc1, 0x99, 0x99, 0x6f, 0xb0, 0x99, 0x07, 0x34, 0xc8, 0x93, 0x6d,
                0xbd, 0x0f,
            ],
        ),
        (
            42,
            &(0..64u8).collect::<Vec<u8>>(),
            [
                0x05, 0x2d, 0xa1, 0x84, 0xef, 0xe7, 0xad, 0xf1, 0x4d, 0xab, 0x0e, 0xe6, 0x14, 0xb8,
                0xb6, 0x5a,
            ],
        ),
        (
            42,
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            [
                0x00, 0x42, 0xf3, 0x36, 0x15, 0x3c, 0xcb, 0x24, 0xf5, 0xad, 0x81, 0x17, 0x57, 0x4c,
                0x6b, 0xa2,
            ],
        ),
    ];
    for (seed, data, want) in cases {
        assert_eq!(
            *sillstone_digest::murmur3_x64_128(data, seed).as_bytes(),
            want,
            "murmur3(seed={seed}, len={})",
            data.len()
        );
    }
}

// --------------------------------------------------------------- crc32c

/// The RFC 3720 B.4 vectors: the four 32-byte patterns, the iSCSI
/// Read(10) PDU and the RFC 4960 check value of "123456789".
#[test]
fn crc32c_rfc_3720_b4_vectors() {
    let mut pdu = Vec::new();
    pdu.extend_from_slice(&[0x01, 0xc0, 0x00, 0x00]);
    pdu.extend_from_slice(&[0x00; 12]);
    pdu.extend_from_slice(&[0x14, 0x00, 0x00, 0x00]);
    pdu.extend_from_slice(&[0x00, 0x00, 0x04, 0x00]);
    pdu.extend_from_slice(&[0x00, 0x00, 0x00, 0x14]);
    pdu.extend_from_slice(&[0x00, 0x00, 0x00, 0x18]);
    pdu.extend_from_slice(&[0x28, 0x00, 0x00, 0x00]);
    pdu.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    pdu.extend_from_slice(&[0x02, 0x00, 0x00, 0x00]);
    pdu.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    assert_eq!(pdu.len(), 48);
    assert_eq!(sillstone_digest::crc32c(&[0x00; 32]), 0x8a91_36aa);
    assert_eq!(sillstone_digest::crc32c(&[0xff; 32]), 0x62a8_ab43);
    assert_eq!(
        sillstone_digest::crc32c(&(0..=0x1fu8).collect::<Vec<u8>>()),
        0x46dd_794e
    );
    assert_eq!(
        sillstone_digest::crc32c(&(0..=0x1fu8).rev().collect::<Vec<u8>>()),
        0x113f_db5c
    );
    assert_eq!(sillstone_digest::crc32c(&pdu), 0xd996_3a56);
    assert_eq!(sillstone_digest::crc32c(b"123456789"), 0xe306_9283);
    assert_eq!(sillstone_digest::crc32c(b""), 0);
}

// --------------------------------------------------------------- base64

/// The RFC 4648 section 10 vectors, encode and decode.
#[test]
fn base64_rfc_4648_vectors() {
    let cases: [(&[u8], &str); 7] = [
        (b"", ""),
        (b"f", "Zg=="),
        (b"fo", "Zm8="),
        (b"foo", "Zm9v"),
        (b"foob", "Zm9vYg=="),
        (b"fooba", "Zm9vYmE="),
        (b"foobar", "Zm9vYmFy"),
    ];
    for (raw, encoded) in cases {
        let enc_len = raw.len().div_ceil(3) * 4;
        let mut buf = [0u8; 12];
        sillstone_digest::base64_encode(raw, &mut buf[..enc_len]).expect("buffer fits");
        assert_eq!(&buf[..enc_len], encoded.as_bytes(), "{raw:?}");
        let mut dec = [0u8; 6];
        let n = sillstone_digest::base64_decode(encoded.as_bytes(), &mut dec).expect("canonical input");
        assert_eq!(&dec[..n], raw, "{encoded}");
    }
}

// ----------------------------------------------------------- xoshiro256

/// The reference implementation's output sequence (splitmix64 seeding)
/// for seeds 0, 1 and 42, four outputs each.
#[test]
fn xoshiro256_starstar_reference_vectors() {
    let cases: [(u64, [u64; 4]); 3] = [
        (
            0,
            [
                0x99ec_5f36_cb75_f2b4,
                0xbf6e_1f78_4956_452a,
                0x1a5f_849d_4933_e6e0,
                0x6aa5_94f1_262d_2d2c,
            ],
        ),
        (
            1,
            [
                0xb3f2_af6d_0fc7_10c5,
                0x853b_5596_4736_4cea,
                0x92f8_9756_082a_4514,
                0x642e_1c7b_c266_a3a7,
            ],
        ),
        (
            42,
            [
                0x1578_0b2e_0c2e_c716,
                0x6104_d986_6d11_3a7e,
                0xae17_5332_39e4_99a1,
                0xecb8_ad47_03b3_60a1,
            ],
        ),
    ];
    for (seed, want) in cases {
        let mut rng = sillstone_digest::Xoshiro256StarStar::from_seed(seed);
        for w in want {
            assert_eq!(rng.next_u64(), w, "xoshiro256** seed={seed}");
        }
    }
}
