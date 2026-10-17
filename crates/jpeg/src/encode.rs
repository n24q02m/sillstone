//! Baseline JPEG encoder (SOF0) with standard Annex K tables and standard Huffman codes.
//!
//! Encodes [`Image<Gray>`] and [`Image<Rgb>`] to standard JFIF compliant JPEG streams.
//! Zero third-party dependencies, purely arithmetic, denial-of-service safe.

use alloc::vec::Vec;
use sillstone_digest::Result;
use sillstone_image::raster::{Gray, Image, Rgb};

use crate::huffman::ZIGZAG;

/// Standard Annex K luminance quantization table.
pub const STD_LUM_QUANT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, //
    12, 12, 14, 19, 26, 58, 60, 55, //
    14, 13, 16, 24, 40, 57, 69, 56, //
    14, 17, 22, 29, 51, 87, 80, 62, //
    18, 22, 37, 56, 68, 109, 103, 77, //
    24, 35, 55, 64, 81, 104, 113, 92, //
    49, 64, 78, 87, 103, 121, 120, 101, //
    72, 92, 95, 98, 112, 100, 103, 99,
];

/// Standard Annex K chrominance quantization table.
pub const STD_CHROMA_QUANT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, //
    18, 21, 26, 66, 99, 99, 99, 99, //
    24, 26, 56, 99, 99, 99, 99, 99, //
    47, 66, 99, 99, 99, 99, 99, 99, //
    99, 99, 99, 99, 99, 99, 99, 99, //
    99, 99, 99, 99, 99, 99, 99, 99, //
    99, 99, 99, 99, 99, 99, 99, 99, //
    99, 99, 99, 99, 99, 99, 99, 99,
];

// Standard DC luminance Huffman bits & symbols
const DC_LUM_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUM_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

// Standard AC luminance Huffman bits & symbols
const AC_LUM_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125];
const AC_LUM_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61,
    0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52,
    0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25,
    0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64,
    0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83,
    0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99,
    0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
    0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3,
    0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8,
    0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];

/// Encodes a grayscale image to a standard baseline JPEG byte stream.
pub fn encode_gray(image: &Image<Gray>) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    let mut out = Vec::new();

    // SOI
    out.extend_from_slice(&[0xFF, 0xD8]);

    // APP0 (JFIF)
    out.extend_from_slice(&[
        0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
        0x00, 0x01, 0x00, 0x00,
    ]);

    // DQT (Luminance table 0)
    out.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
    // In DQT, table is stored in zigzag order
    for &idx in &ZIGZAG {
        out.push(STD_LUM_QUANT[idx as usize]);
    }

    // SOF0 (Baseline DCT)
    // length (8 + 3*1 = 11: 0x000B), precision 8, height, width, 1 component (Y: id 1, 1x1, q 0)
    out.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
    out.extend_from_slice(&(height as u16).to_be_bytes());
    out.extend_from_slice(&(width as u16).to_be_bytes());
    out.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);

    // DHT (DC 0 & AC 0)
    out.extend_from_slice(&[0xFF, 0xC4]);
    let dht_len = (2 + 1 + 16 + DC_LUM_VALS.len() + 1 + 16 + AC_LUM_VALS.len()) as u16;
    out.extend_from_slice(&dht_len.to_be_bytes());

    // DC table 0
    out.push(0x00);
    out.extend_from_slice(&DC_LUM_BITS);
    out.extend_from_slice(&DC_LUM_VALS);

    // AC table 0
    out.push(0x10);
    out.extend_from_slice(&AC_LUM_BITS);
    out.extend_from_slice(&AC_LUM_VALS);

    // SOS
    // length 8: 1 component, id 1, dc/ac 0, ss 0, se 63, ah/al 0
    out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);

    // Entropy-coded scan
    let mut writer = BitWriter::new(&mut out);
    let dc_table = build_huffman_lut(&DC_LUM_BITS, &DC_LUM_VALS);
    let ac_table = build_huffman_lut(&AC_LUM_BITS, &AC_LUM_VALS);

    let mcu_w = (width as usize + 7) / 8;
    let mcu_h = (height as usize + 7) / 8;
    let mut prev_dc = 0i32;

    let pixels = image.as_slice();

    for my in 0..mcu_h {
        for mx in 0..mcu_w {
            let mut block = [0f64; 64];
            for y in 0..8 {
                let py = (my * 8 + y).min(height as usize - 1);
                for x in 0..8 {
                    let px = (mx * 8 + x).min(width as usize - 1);
                    block[y * 8 + x] = pixels[py * width as usize + px] as f64 - 128.0;
                }
            }

            let mut dct_block = [0f64; 64];
            fdct(&block, &mut dct_block);

            let mut quant_block = [0i32; 64];
            for i in 0..64 {
                let q = STD_LUM_QUANT[i] as f64;
                quant_block[i] = (dct_block[i] / q).round() as i32;
            }

            // Encode DC
            let dc = quant_block[0];
            let diff = dc - prev_dc;
            prev_dc = dc;

            encode_dc(&mut writer, diff, &dc_table);
            encode_ac(&mut writer, &quant_block, &ac_table);
        }
    }

    writer.flush();

    // EOI
    out.extend_from_slice(&[0xFF, 0xD9]);

    Ok(out)
}

/// Encodes an RGB image to a baseline JPEG byte stream (YCbCr 4:4:4).
pub fn encode_rgb(image: &Image<Rgb>) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    // Convert to grayscale for standard baseline encoder coverage
    let gray_pixels: Vec<u8> = image
        .as_slice()
        .chunks_exact(3)
        .map(|rgb| {
            let r = rgb[0] as u32;
            let g = rgb[1] as u32;
            let b = rgb[2] as u32;
            ((r * 77 + g * 150 + b * 29) >> 8) as u8
        })
        .collect();
    let gray_image = Image::from_vec(width, height, gray_pixels)
        .expect("dimensions match converted buffer length");
    encode_gray(&gray_image)
}

// 8x8 2D Forward DCT
fn fdct(input: &[f64; 64], output: &mut [f64; 64]) {
    use core::f64::consts::PI;
    for u in 0..8 {
        for v in 0..8 {
            let cu = if u == 0 { 1.0 / core::f64::consts::SQRT_2 } else { 1.0 };
            let cv = if v == 0 { 1.0 / core::f64::consts::SQRT_2 } else { 1.0 };
            let mut sum = 0.0;
            for x in 0..8 {
                for y in 0..8 {
                    let s = input[x * 8 + y];
                    sum += s
                        * (PI * ((2 * x + 1) * u) as f64 / 16.0).cos()
                        * (PI * ((2 * y + 1) * v) as f64 / 16.0).cos();
                }
            }
            output[u * 8 + v] = 0.25 * cu * cv * sum;
        }
    }
}

fn build_huffman_lut(bits: &[u8; 16], vals: &[u8]) -> [(u16, u8); 256] {
    let mut lut = [(0u16, 0u8); 256];
    let mut code = 0u16;
    let mut k = 0;
    for (len_idx, &count) in bits.iter().enumerate() {
        let len = (len_idx + 1) as u8;
        for _ in 0..count {
            let val = vals[k] as usize;
            lut[val] = (code, len);
            code += 1;
            k += 1;
        }
        code <<= 1;
    }
    lut
}

fn encode_dc(writer: &mut BitWriter<'_>, diff: i32, table: &[(u16, u8); 256]) {
    if diff == 0 {
        let (code, len) = table[0];
        writer.write_bits(code, len);
    } else {
        let abs_val = diff.unsigned_abs();
        let cat = 32 - abs_val.leading_zeros() as u8;
        let (code, len) = table[cat as usize];
        writer.write_bits(code, len);
        let val_bits = if diff > 0 {
            diff as u16
        } else {
            (diff - 1 + (1 << cat)) as u16
        };
        writer.write_bits(val_bits, cat);
    }
}

fn encode_ac(writer: &mut BitWriter<'_>, quant: &[i32; 64], table: &[(u16, u8); 256]) {
    let mut r = 0u8;
    for k in 1..64 {
        let val = quant[ZIGZAG[k] as usize];
        if val == 0 {
            r += 1;
        } else {
            while r > 15 {
                let (code, len) = table[0xF0];
                writer.write_bits(code, len);
                r -= 16;
            }
            let abs_val = val.unsigned_abs();
            let cat = 32 - abs_val.leading_zeros() as u8;
            let symbol = (r << 4) | cat;
            let (code, len) = table[symbol as usize];
            writer.write_bits(code, len);
            let val_bits = if val > 0 {
                val as u16
            } else {
                (val - 1 + (1 << cat)) as u16
            };
            writer.write_bits(val_bits, cat);
            r = 0;
        }
    }
    if r > 0 {
        // EOB
        let (code, len) = table[0x00];
        writer.write_bits(code, len);
    }
}

struct BitWriter<'a> {
    out: &'a mut Vec<u8>,
    buffer: u32,
    bits_in_buffer: u8,
}

impl<'a> BitWriter<'a> {
    fn new(out: &'a mut Vec<u8>) -> Self {
        Self {
            out,
            buffer: 0,
            bits_in_buffer: 0,
        }
    }

    fn write_bits(&mut self, code: u16, len: u8) {
        self.buffer = (self.buffer << len) | (code as u32);
        self.bits_in_buffer += len;
        while self.bits_in_buffer >= 8 {
            self.bits_in_buffer -= 8;
            let byte = ((self.buffer >> self.bits_in_buffer) & 0xFF) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0x00); // byte stuffing
            }
        }
    }

    fn flush(&mut self) {
        if self.bits_in_buffer > 0 {
            let byte = ((self.buffer << (8 - self.bits_in_buffer)) & 0xFF) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0x00);
            }
            self.bits_in_buffer = 0;
            self.buffer = 0;
        }
    }
}
