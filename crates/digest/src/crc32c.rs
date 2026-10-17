//! The Castagnoli CRC-32C (RFC 3720 Appendix B.4, the iSCSI CRC;
//! also RFC 4960 Appendix B for SCTP).
//!
//! The reflected 0x1EDC6F41 polynomial, initial value 0xFFFFFFFF and
//! final xor 0xFFFFFFFF. The same table shape as [`crate::crc32`]:
//! the 256-entry table is built by the compiler at compile time, so a
//! call costs one lookup per byte and nothing else.

/// The reflected CRC-32C polynomial 0x82F63B78 (Castagnoli).
const CRC32C_POLY: u32 = 0x82F6_3B78;

/// The 256-entry processing table, built by the compiler at compile
/// time so a call costs one lookup per byte and nothing else.
const CRC32C_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                CRC32C_POLY ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// Computes the CRC-32C (Castagnoli) of `data` (RFC 3720 B.4).
///
/// ```
/// use sillstone_digest::crc32c;
/// // The catalogued check value of "123456789".
/// assert_eq!(crc32c(b"123456789"), 0xE306_9283);
/// // The RFC 3720 B.4 vectors: 32 bytes of zeroes and of ones.
/// assert_eq!(crc32c(&[0x00; 32]), 0x8A91_36AA);
/// assert_eq!(crc32c(&[0xff; 32]), 0x62A8_AB43);
/// ```
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc = CRC32C_TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::crc32c;

    /// The four RFC 3720 B.4 vectors and the catalogued check value.
    #[test]
    fn rfc_3720_b4_vectors() {
        assert_eq!(crc32c(&[0x00; 32]), 0x8a91_36aa);
        assert_eq!(crc32c(&[0xff; 32]), 0x62a8_ab43);
        let increasing: Vec<u8> = (0..=0x1f).collect();
        assert_eq!(crc32c(&increasing), 0x46dd_794e);
        let decreasing: Vec<u8> = (0..=0x1f).rev().collect();
        assert_eq!(crc32c(&decreasing), 0x113f_db5c);
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        assert_eq!(crc32c(b""), 0);
    }
}
