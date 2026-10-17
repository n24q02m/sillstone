//! Trend Micro Locality Sensitive Hash (TLSH) implementation.
//!
//! Provides fuzzy hashing and distance calculation conforming to the
//! official Trend Micro TLSH specification (128 buckets, 1-byte checksum, "T1" version).

use alloc::format;
use alloc::string::String;
use core::fmt;
use sillstone_digest::Error;

const BUCKET_SIZE: usize = 128;
const WINDOW_SIZE: usize = 5;

const V_TABLE: [u8; 256] = [
    1, 87, 49, 12, 176, 178, 102, 166, 121, 193, 6, 84, 249, 230, 44, 163, 14,
    197, 213, 181, 161, 85, 218, 80, 64, 239, 24, 226, 236, 142, 38, 200, 110,
    177, 104, 103, 141, 253, 255, 50, 77, 101, 81, 18, 45, 96, 31, 222, 25,
    107, 190, 70, 86, 237, 240, 34, 72, 242, 20, 214, 244, 227, 149, 235, 97,
    234, 57, 22, 60, 250, 82, 175, 208, 5, 127, 199, 111, 62, 135, 248, 174,
    169, 211, 58, 66, 154, 106, 195, 245, 171, 17, 187, 182, 179, 0, 243, 132,
    56, 148, 75, 128, 133, 158, 100, 130, 126, 91, 13, 153, 246, 216, 219,
    119, 68, 223, 78, 83, 88, 201, 99, 122, 11, 92, 32, 136, 114, 52, 10, 138,
    30, 48, 183, 156, 35, 61, 26, 143, 74, 251, 94, 129, 162, 63, 152, 170, 7,
    115, 167, 241, 206, 3, 150, 55, 59, 151, 220, 90, 53, 23, 131, 125, 173,
    15, 238, 79, 95, 89, 16, 105, 137, 225, 224, 217, 160, 37, 123, 118, 73,
    2, 157, 46, 116, 9, 145, 134, 228, 207, 212, 202, 215, 69, 229, 27, 188,
    67, 124, 168, 252, 42, 4, 29, 108, 21, 247, 19, 205, 39, 203, 233, 40,
    186, 147, 198, 192, 155, 33, 164, 191, 98, 204, 165, 180, 117, 76, 140,
    36, 210, 172, 41, 54, 159, 8, 185, 232, 113, 196, 231, 47, 146, 120, 51,
    65, 28, 144, 254, 221, 93, 189, 194, 139, 112, 43, 71, 109, 184, 209,
];

const TOPVAL: [usize; 170] = [
    1, 2, 3, 5, 7, 11, 17, 25, 38, 57, 86, 129, 194, 291, 437, 656, 854, 1110,
    1443, 1876, 2439, 3171, 3475, 3823, 4205, 4626, 5088, 5597, 6157, 6772,
    7450, 8195, 9014, 9916, 10907, 11998, 13198, 14518, 15970, 17567, 19323,
    21256, 23382, 25720, 28292, 31121, 34233, 37656, 41422, 45564, 50121,
    55133, 60646, 66711, 73382, 80721, 88793, 97672, 107439, 118183, 130002,
    143002, 157302, 173032, 190335, 209369, 230306, 253337, 278670, 306538,
    337191, 370911, 408002, 448802, 493682, 543050, 597356, 657091, 722800,
    795081, 874589, 962048, 1058252, 1164078, 1280486, 1408534, 1549388,
    1704327, 1874759, 2062236, 2268459, 2495305, 2744836, 3019320, 3321252,
    3653374, 4018711, 4420582, 4862641, 5348905, 5883796, 6472176, 7119394,
    7831333, 8614467, 9475909, 10423501, 11465851, 12612437, 13873681,
    15261050, 16787154, 18465870, 20312458, 22343706, 24578077, 27035886,
    29739474, 32713425, 35984770, 39583245, 43541573, 47895730, 52685306,
    57953837, 63749221, 70124148, 77136564, 84850228, 93335252, 102668779,
    112935659, 124229227, 136652151, 150317384, 165349128, 181884040,
    200072456, 220079703, 242087671, 266296456, 292926096, 322218735,
    354440623, 389884688, 428873168, 471760495, 518936559, 570830240,
    627913311, 690704607, 759775136, 835752671, 919327967, 1011260767,
    1112386880, 1223623232, 1345985727, 1480584256, 1628642751, 1791507135,
    1970657856, 2167723648, 2384496256, 2622945920, 2885240448, 3173764736,
    3491141248, 3840255616, 4224281216,
];

#[inline]
fn pearson(salt: u8, i: u8, j: u8, k: u8) -> u8 {
    let mut h = 0usize;
    h = V_TABLE[h ^ (salt as usize)] as usize;
    h = V_TABLE[h ^ (i as usize)] as usize;
    h = V_TABLE[h ^ (j as usize)] as usize;
    h = V_TABLE[h ^ (k as usize)] as usize;
    h as u8
}

fn l_capturing(len: usize) -> u8 {
    for (idx, &top) in TOPVAL.iter().enumerate() {
        if len <= top {
            return idx as u8;
        }
    }
    (TOPVAL.len() - 1) as u8
}

fn bit_pairs_diff(i: u8, j: u8) -> u32 {
    let mut diff = 0u32;
    let (mut x, mut y) = (i as i32, j as i32);
    for _ in 0..4 {
        let d = ((x % 4) - (y % 4)).abs();
        diff += if d == 3 { 6 } else { d as u32 };
        x /= 4;
        y /= 4;
    }
    diff
}

fn mod_diff(x: usize, y: usize, circ: usize) -> usize {
    let (dl, dr) = if x >= y {
        (x - y, y + circ - x)
    } else {
        (y - x, x + circ - y)
    };
    dl.min(dr)
}

fn hex_val(b: u8) -> Result<u8, Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(Error::BadValue("invalid hex digit in TLSH")),
    }
}

/// A parsed 70-hex-character TLSH hash string with "T1" prefix.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tlsh {
    checksum: u8,
    len_val: u8,
    q1_ratio: u8,
    q2_ratio: u8,
    codes: [u8; 32],
    repr: String,
}

impl Tlsh {
    /// Formats the TLSH hash as a 72-character string starting with "T1".
    pub fn as_str(&self) -> &str {
        &self.repr
    }

    /// Parses a 72-character TLSH hash string starting with "T1".
    pub fn parse(s: &str) -> Result<Self, Error> {
        if s.len() != 72 || !s.starts_with("T1") {
            return Err(Error::BadValue("invalid TLSH string format"));
        }
        let bytes = s.as_bytes();
        let cs_hi = hex_val(bytes[3])?;
        let cs_lo = hex_val(bytes[2])?;
        let checksum = (cs_hi << 4) | cs_lo;

        let l_hi = hex_val(bytes[5])?;
        let l_lo = hex_val(bytes[4])?;
        let len_val = (l_hi << 4) | l_lo;

        let q1_ratio = hex_val(bytes[6])?;
        let q2_ratio = hex_val(bytes[7])?;

        let mut codes = [0u8; 32];
        for i in 0..32 {
            let hi = hex_val(bytes[8 + 2 * i])?;
            let lo = hex_val(bytes[9 + 2 * i])?;
            codes[31 - i] = (hi << 4) | lo;
        }

        Ok(Tlsh {
            checksum,
            len_val,
            q1_ratio,
            q2_ratio,
            codes,
            repr: String::from(s),
        })
    }

    /// Computes the difference (distance) between two TLSH hashes.
    pub fn distance(&self, other: &Self) -> u32 {
        tlsh_distance(self, other)
    }
}

impl fmt::Display for Tlsh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.repr)
    }
}

/// Computes the TLSH hash of a byte slice.
///
/// Input must be at least 50 bytes in length.
pub fn tlsh(data: &[u8]) -> Result<Tlsh, Error> {
    if data.len() < 50 {
        return Err(Error::BadValue("input too short: minimum 50 bytes for TLSH"));
    }

    let mut buckets = [0u32; 256];
    let mut checksum: u8 = 0;
    let mut sw = [0u8; WINDOW_SIZE];

    let mut j0 = 0;
    let mut j1 = 4;
    let mut j2 = 3;
    let mut j3 = 2;
    let mut j4 = 1;

    for (fed_len, &item) in data.iter().enumerate() {
        sw[j0] = item;
        if fed_len >= 4 {
            checksum = pearson(0, sw[j0], sw[j1], checksum);
            buckets[pearson(2, sw[j0], sw[j1], sw[j2]) as usize] += 1;
            buckets[pearson(3, sw[j0], sw[j1], sw[j3]) as usize] += 1;
            buckets[pearson(5, sw[j0], sw[j2], sw[j3]) as usize] += 1;
            buckets[pearson(7, sw[j0], sw[j2], sw[j4]) as usize] += 1;
            buckets[pearson(11, sw[j0], sw[j1], sw[j4]) as usize] += 1;
            buckets[pearson(13, sw[j0], sw[j3], sw[j4]) as usize] += 1;
        }

        let tmp = j4;
        j4 = j3;
        j3 = j2;
        j2 = j1;
        j1 = j0;
        j0 = tmp;
    }

    // Determine quartiles over the first 128 buckets
    let mut sorted_b = [0u32; BUCKET_SIZE];
    sorted_b.copy_from_slice(&buckets[..BUCKET_SIZE]);
    sorted_b.sort_unstable();

    let q1 = sorted_b[31];
    let q2 = sorted_b[63];
    let q3 = sorted_b[95];

    if q3 == 0 {
        return Err(Error::BadValue("degenerate input for TLSH: q3 is zero"));
    }

    let mut codes = [0u8; 32];
    for (i, item) in codes.iter_mut().enumerate() {
        let mut h = 0u8;
        for j in 0..4 {
            let k = buckets[4 * i + j];
            if k > q3 {
                h += 3 << (j * 2);
            } else if k > q2 {
                h += 2 << (j * 2);
            } else if k > q1 {
                h += 1 << (j * 2);
            }
        }
        *item = h;
    }

    let len_val = l_capturing(data.len());
    let q1_ratio = (((q1 as f64 * 100.0) / (q3 as f64)) as usize % 16) as u8;
    let q2_ratio = (((q2 as f64 * 100.0) / (q3 as f64)) as usize % 16) as u8;
    let q_ratio = (q1_ratio << 4) | q2_ratio;

    // Formatting:
    // "T1" + hex_rev(checksum) + hex_rev(len_val) + hex(q_ratio) + codes[31..0] hex
    let cs_hex = format!("{:02X}", checksum);
    let cs_rev: String = cs_hex.chars().rev().collect();

    let len_hex = format!("{:02X}", len_val);
    let len_rev: String = len_hex.chars().rev().collect();

    let q_hex = format!("{:02X}", q_ratio);

    let mut codes_hex = String::with_capacity(64);
    for &code in codes.iter().rev() {
        use core::fmt::Write;
        let _ = write!(&mut codes_hex, "{:02X}", code);
    }

    let repr = format!("T1{}{}{}{}", cs_rev, len_rev, q_hex, codes_hex);

    Ok(Tlsh {
        checksum,
        len_val,
        q1_ratio,
        q2_ratio,
        codes,
        repr,
    })
}

/// Computes the distance score between two TLSH hashes (with length penalty).
pub fn tlsh_distance(a: &Tlsh, b: &Tlsh) -> u32 {
    let mut result: usize = 0;

    let l_diff = mod_diff(a.len_val as usize, b.len_val as usize, 256);
    if l_diff <= 1 {
        result += l_diff;
    } else {
        result += l_diff * 12;
    }

    let q1_diff = mod_diff(a.q1_ratio as usize, b.q1_ratio as usize, 16);
    if q1_diff <= 1 {
        result += q1_diff;
    } else {
        result += (q1_diff - 1) * 12;
    }

    let q2_diff = mod_diff(a.q2_ratio as usize, b.q2_ratio as usize, 16);
    if q2_diff <= 1 {
        result += q2_diff;
    } else {
        result += (q2_diff - 1) * 12;
    }

    if a.checksum != b.checksum {
        result += 1;
    }

    for (&ca, &cb) in a.codes.iter().zip(b.codes.iter()) {
        result += bit_pairs_diff(ca, cb) as usize;
    }

    result as u32
}
