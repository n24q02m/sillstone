//! Pure-Rust, zero-dependency implementation of the QOI (Quite OK Image) format.
//!
//! Provides fast, lossless image encoding and decoding compliant with the QOI specification.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use sillstone_digest::{Error, Result};

/// QOI magic header: `b"qoif"`.
pub const QOI_MAGIC: [u8; 4] = *b"qoif";
/// QOI 8-byte end padding marker.
pub const QOI_PADDING: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

const QOI_OP_INDEX: u8 = 0x00;
const QOI_OP_DIFF: u8 = 0x40;
const QOI_OP_LUMA: u8 = 0x80;
const QOI_OP_RUN: u8 = 0xc0;
const QOI_OP_RGB: u8 = 0xfe;
const QOI_OP_RGBA: u8 = 0xff;

const QOI_MASK_2: u8 = 0xc0;

/// Image metadata from a QOI header.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct QoiHeader {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of channels: 3 for RGB, 4 for RGBA.
    pub channels: u8,
    /// Colorspace flag: 0 for sRGB with linear alpha, 1 for all linear.
    pub colorspace: u8,
}

#[inline]
fn color_hash(r: u8, g: u8, b: u8, a: u8) -> usize {
    ((r as usize * 3) + (g as usize * 5) + (b as usize * 7) + (a as usize * 11)) % 64
}

/// Decodes raw QOI bytes into image metadata and interleaved pixel bytes.
///
/// Returns `(header, pixels)` where `pixels` has length `width * height * channels`.
pub fn decode(bytes: &[u8]) -> Result<(QoiHeader, Vec<u8>)> {
    if bytes.len() < 14 + 8 {
        return Err(Error::BadValue("QOI data too short for header and footer"));
    }

    if &bytes[0..4] != &QOI_MAGIC {
        return Err(Error::BadValue("invalid QOI magic bytes"));
    }

    let width = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let height = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let channels = bytes[12];
    let colorspace = bytes[13];

    if width == 0 || height == 0 {
        return Err(Error::BadValue("QOI zero dimensions"));
    }
    if channels != 3 && channels != 4 {
        return Err(Error::BadValue("QOI channels must be 3 or 4"));
    }

    let header = QoiHeader {
        width,
        height,
        channels,
        colorspace,
    };

    let total_pixels = match (width as usize).checked_mul(height as usize) {
        Some(p) => p,
        None => return Err(Error::BadValue("QOI dimension overflow")),
    };
    let total_bytes = match total_pixels.checked_mul(channels as usize) {
        Some(b) => b,
        None => return Err(Error::BadValue("QOI byte length overflow")),
    };

    let mut pixels = vec![0u8; total_bytes];
    let mut index = [[0u8; 4]; 64];

    let mut r = 0u8;
    let mut g = 0u8;
    let mut b = 0u8;
    let mut a = 255u8;

    let mut p_idx = 0usize;
    let mut run = 0usize;

    let chunks_len = bytes.len() - 8;
    let mut offset = 14;

    while p_idx < total_pixels && (run > 0 || offset < chunks_len) {
        if run > 0 {
            run -= 1;
        } else {
            let b1 = bytes[offset];
            offset += 1;

            if b1 == QOI_OP_RGB {
                if offset + 3 > chunks_len {
                    return Err(Error::BadValue("truncated QOI_OP_RGB"));
                }
                r = bytes[offset];
                g = bytes[offset + 1];
                b = bytes[offset + 2];
                offset += 3;
            } else if b1 == QOI_OP_RGBA {
                if offset + 4 > chunks_len {
                    return Err(Error::BadValue("truncated QOI_OP_RGBA"));
                }
                r = bytes[offset];
                g = bytes[offset + 1];
                b = bytes[offset + 2];
                a = bytes[offset + 3];
                offset += 4;
            } else if (b1 & QOI_MASK_2) == QOI_OP_INDEX {
                let idx = (b1 & 0x3f) as usize;
                let px = index[idx];
                r = px[0];
                g = px[1];
                b = px[2];
                a = px[3];
            } else if (b1 & QOI_MASK_2) == QOI_OP_DIFF {
                let dr = ((b1 >> 4) & 0x03).wrapping_sub(2);
                let dg = ((b1 >> 2) & 0x03).wrapping_sub(2);
                let db = (b1 & 0x03).wrapping_sub(2);
                r = r.wrapping_add(dr);
                g = g.wrapping_add(dg);
                b = b.wrapping_add(db);
            } else if (b1 & QOI_MASK_2) == QOI_OP_LUMA {
                if offset >= chunks_len {
                    return Err(Error::BadValue("truncated QOI_OP_LUMA"));
                }
                let b2 = bytes[offset];
                offset += 1;
                let dg = (b1 & 0x3f).wrapping_sub(32);
                let dr = ((b2 >> 4) & 0x0f).wrapping_sub(8).wrapping_add(dg);
                let db = (b2 & 0x0f).wrapping_sub(8).wrapping_add(dg);
                r = r.wrapping_add(dr);
                g = g.wrapping_add(dg);
                b = b.wrapping_add(db);
            } else if (b1 & QOI_MASK_2) == QOI_OP_RUN {
                run = (b1 & 0x3f) as usize;
            }

            let h = color_hash(r, g, b, a);
            index[h] = [r, g, b, a];
        }

        let out_offset = p_idx * channels as usize;
        pixels[out_offset] = r;
        pixels[out_offset + 1] = g;
        pixels[out_offset + 2] = b;
        if channels == 4 {
            pixels[out_offset + 3] = a;
        }

        p_idx += 1;
    }

    if p_idx != total_pixels {
        return Err(Error::BadValue("QOI incomplete pixel stream"));
    }

    Ok((header, pixels))
}

/// Encodes raw pixel bytes (RGB or RGBA) into the QOI format.
pub fn encode(
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8,
    pixels: &[u8],
) -> Result<Vec<u8>> {
    if width == 0 || height == 0 {
        return Err(Error::BadValue("dimensions must be non-zero"));
    }
    if channels != 3 && channels != 4 {
        return Err(Error::BadValue("channels must be 3 or 4"));
    }

    let total_pixels = match (width as usize).checked_mul(height as usize) {
        Some(p) => p,
        None => return Err(Error::BadValue("dimension overflow")),
    };
    if pixels.len() != total_pixels * channels as usize {
        return Err(Error::BadValue("pixel buffer length mismatch"));
    }

    // Maximum theoretical QOI size: 14 header + pixels * 5 + 8 padding
    let max_size = 14 + total_pixels * 5 + 8;
    let mut out = Vec::with_capacity(max_size.min(1024 * 1024));

    // Write header
    out.extend_from_slice(&QOI_MAGIC);
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.push(channels);
    out.push(colorspace);

    let mut index = [[0u8; 4]; 64];

    let mut prev_r = 0u8;
    let mut prev_g = 0u8;
    let mut prev_b = 0u8;
    let mut prev_a = 255u8;

    let mut run = 0u8;

    for i in 0..total_pixels {
        let p_offset = i * channels as usize;
        let r = pixels[p_offset];
        let g = pixels[p_offset + 1];
        let b = pixels[p_offset + 2];
        let a = if channels == 4 {
            pixels[p_offset + 3]
        } else {
            255
        };

        if r == prev_r && g == prev_g && b == prev_b && a == prev_a {
            run += 1;
            if run == 62 {
                out.push(QOI_OP_RUN | (run - 1));
                run = 0;
            }
        } else {
            if run > 0 {
                out.push(QOI_OP_RUN | (run - 1));
                run = 0;
            }

            let h = color_hash(r, g, b, a);
            if index[h] == [r, g, b, a] {
                out.push(QOI_OP_INDEX | (h as u8));
            } else {
                index[h] = [r, g, b, a];

                if a == prev_a {
                    let vr = r.wrapping_sub(prev_r) as i8;
                    let vg = g.wrapping_sub(prev_g) as i8;
                    let vb = b.wrapping_sub(prev_b) as i8;

                    let vg_r = vr.wrapping_sub(vg);
                    let vg_b = vb.wrapping_sub(vg);

                    if vr >= -2 && vr <= 1 && vg >= -2 && vg <= 1 && vb >= -2 && vb <= 1 {
                        out.push(
                            QOI_OP_DIFF
                                | (((vr + 2) as u8) << 4)
                                | (((vg + 2) as u8) << 2)
                                | ((vb + 2) as u8),
                        );
                    } else if vg >= -32 && vg <= 31 && vg_r >= -8 && vg_r <= 7 && vg_b >= -8 && vg_b <= 7
                    {
                        out.push(QOI_OP_LUMA | ((vg + 32) as u8));
                        out.push((((vg_r + 8) as u8) << 4) | ((vg_b + 8) as u8));
                    } else {
                        out.push(QOI_OP_RGB);
                        out.push(r);
                        out.push(g);
                        out.push(b);
                    }
                } else {
                    out.push(QOI_OP_RGBA);
                    out.push(r);
                    out.push(g);
                    out.push(b);
                    out.push(a);
                }
            }

            prev_r = r;
            prev_g = g;
            prev_b = b;
            prev_a = a;
        }
    }

    if run > 0 {
        out.push(QOI_OP_RUN | (run - 1));
    }

    // Write 8-byte end marker
    out.extend_from_slice(&QOI_PADDING);

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qoi_roundtrip_rgb() {
        let width = 2;
        let height = 2;
        let pixels = vec![
            255, 0, 0,
            0, 255, 0,
            0, 0, 255,
            255, 255, 255,
        ];
        let encoded = encode(width, height, 3, 0, &pixels).unwrap();
        let (header, decoded) = decode(&encoded).unwrap();
        assert_eq!(header.width, width);
        assert_eq!(header.height, height);
        assert_eq!(header.channels, 3);
        assert_eq!(decoded, pixels);
    }

    #[test]
    fn test_qoi_roundtrip_rgba() {
        let width = 3;
        let height = 1;
        let pixels = vec![
            255, 0, 0, 128,
            255, 0, 0, 128, // run
            10, 20, 30, 255,
        ];
        let encoded = encode(width, height, 4, 0, &pixels).unwrap();
        let (header, decoded) = decode(&encoded).unwrap();
        assert_eq!(header.width, width);
        assert_eq!(header.height, height);
        assert_eq!(header.channels, 4);
        assert_eq!(decoded, pixels);
    }
}
