//! BLAKE3 cryptographic hash, keyed hash, and extendable-output function (XOF).
//!
//! Complies with the BLAKE3 specification. Pure Rust, zero external dependencies,
//! no dynamic allocation (stack-bounded Merkle tree supports inputs up to 2^64 bytes).

use crate::digest::Digest;
use crate::error::Result;

/// BLAKE3 IV (same as SHA-256 H0).
pub(crate) const IV: [u32; 8] = [
    0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a,
    0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
];

/// BLAKE3 permutation of message words.
const PERMUTE_INDICES: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

// Domain separation flags
const CHUNK_START: u32 = 1;
const CHUNK_END: u32 = 2;
const PARENT: u32 = 4;
const ROOT: u32 = 8;
const KEYED_HASH: u32 = 16;

#[inline(always)]
fn rot_r(x: u32, n: u32) -> u32 {
    x.rotate_right(n)
}

#[inline(always)]
fn g(v: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = rot_r(v[d] ^ v[a], 16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = rot_r(v[b] ^ v[c], 12);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = rot_r(v[d] ^ v[a], 8);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = rot_r(v[b] ^ v[c], 7);
}

fn permute(m: &mut [u32; 16]) {
    let mut permuted = [0u32; 16];
    for (i, &p) in PERMUTE_INDICES.iter().enumerate() {
        permuted[i] = m[p];
    }
    *m = permuted;
}

fn compress(
    cv: &[u32; 8],
    block: &[u32; 16],
    block_len: u32,
    counter: u64,
    flags: u32,
) -> [u32; 16] {
    let mut v = [
        cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7],
        IV[0], IV[1], IV[2], IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];

    let mut m = *block;
    for _ in 0..7 {
        g(&mut v, 0, 4, 8, 12, m[0], m[1]);
        g(&mut v, 1, 5, 9, 13, m[2], m[3]);
        g(&mut v, 2, 6, 10, 14, m[4], m[5]);
        g(&mut v, 3, 7, 11, 15, m[6], m[7]);
        g(&mut v, 0, 5, 10, 15, m[8], m[9]);
        g(&mut v, 1, 6, 11, 12, m[10], m[11]);
        g(&mut v, 2, 7, 8, 13, m[12], m[13]);
        g(&mut v, 3, 4, 9, 14, m[14], m[15]);
        permute(&mut m);
    }

    let mut out = [0u32; 16];
    for i in 0..8 {
        out[i] = v[i] ^ v[i + 8];
        out[i + 8] = v[i + 8] ^ cv[i];
    }
    out
}

fn parent_cv(left_cv: &[u32; 8], right_cv: &[u32; 8], key: &[u32; 8], flags: u32) -> [u32; 8] {
    let mut block = [0u32; 16];
    block[..8].copy_from_slice(left_cv);
    block[8..].copy_from_slice(right_cv);
    let out16 = compress(key, &block, 64, 0, flags | PARENT);
    let mut cv = [0u32; 8];
    cv.copy_from_slice(&out16[..8]);
    cv
}

fn parent_output(
    left_cv: &[u32; 8],
    right_cv: &[u32; 8],
    key: &[u32; 8],
    flags: u32,
    counter: u64,
) -> [u32; 16] {
    let mut block = [0u32; 16];
    block[..8].copy_from_slice(left_cv);
    block[8..].copy_from_slice(right_cv);
    compress(key, &block, 64, counter, flags | PARENT | ROOT)
}

/// BLAKE3 streaming hasher state.
pub struct Hasher {
    key: [u32; 8],
    flags: u32,
    cv_stack: [[u32; 8]; 54],
    cv_stack_len: usize,
    chunk_buffer: [u8; 1024],
    chunk_len: usize,
    chunk_counter: u64,
}

impl Hasher {
    /// Constructs a new hasher with the default BLAKE3 key and flags.
    pub fn new() -> Self {
        Self::with_key_and_flags(IV, 0)
    }

    /// Constructs a new keyed hasher with the provided 32-byte key.
    pub fn new_keyed(key_bytes: &[u8; 32]) -> Self {
        let mut key = [0u32; 8];
        for (i, word) in key.iter_mut().enumerate() {
            let chunk = &key_bytes[4 * i..4 * (i + 1)];
            *word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        Self::with_key_and_flags(key, KEYED_HASH)
    }

    fn with_key_and_flags(key: [u32; 8], flags: u32) -> Self {
        Self {
            key,
            flags,
            cv_stack: [[0u32; 8]; 54],
            cv_stack_len: 0,
            chunk_buffer: [0u8; 1024],
            chunk_len: 0,
            chunk_counter: 0,
        }
    }

    /// Absorbs input bytes into the hasher.
    pub fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.chunk_len == 1024 {
                let cv = self.chunk_cv(self.chunk_len, self.chunk_counter);
                self.push_stack(cv);
                self.chunk_len = 0;
                self.chunk_counter += 1;
            }
            let take = (1024 - self.chunk_len).min(input.len());
            self.chunk_buffer[self.chunk_len..self.chunk_len + take]
                .copy_from_slice(&input[..take]);
            self.chunk_len += take;
            input = &input[take..];
        }
    }

    fn push_stack(&mut self, mut cv: [u32; 8]) {
        let mut total_chunks = self.chunk_counter + 1;
        while (total_chunks & 1) == 0 {
            if self.cv_stack_len > 0 {
                self.cv_stack_len -= 1;
                let left = self.cv_stack[self.cv_stack_len];
                cv = parent_cv(&left, &cv, &self.key, self.flags);
            }
            total_chunks >>= 1;
        }
        if self.cv_stack_len < self.cv_stack.len() {
            self.cv_stack[self.cv_stack_len] = cv;
            self.cv_stack_len += 1;
        }
    }

    fn chunk_cv(&self, buf_len: usize, counter: u64) -> [u32; 8] {
        let mut cv = self.key;
        let num_blocks = if buf_len == 0 { 1 } else { (buf_len + 63) / 64 };
        for i in 0..num_blocks {
            let start = i * 64;
            let end = (start + 64).min(buf_len);
            let mut block_bytes = [0u8; 64];
            if end > start {
                block_bytes[..end - start].copy_from_slice(&self.chunk_buffer[start..end]);
            }
            let mut block = [0u32; 16];
            for w in 0..16 {
                let chunk = &block_bytes[4 * w..4 * (w + 1)];
                block[w] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            }
            let mut flags = self.flags;
            if i == 0 {
                flags |= CHUNK_START;
            }
            if i == num_blocks - 1 {
                flags |= CHUNK_END;
            }
            let block_len = (end - start) as u32;
            let out16 = compress(&cv, &block, block_len, counter, flags);
            cv.copy_from_slice(&out16[..8]);
        }
        cv
    }

    /// Squeezes output bytes into `out` using BLAKE3 XOF.
    pub fn finalize_into(&self, out: &mut [u8]) {
        if self.cv_stack_len == 0 {
            // Single chunk root
            let mut cv = self.key;
            let buf_len = self.chunk_len;
            let num_blocks = if buf_len == 0 { 1 } else { (buf_len + 63) / 64 };
            for i in 0..num_blocks.saturating_sub(1) {
                let start = i * 64;
                let end = (start + 64).min(buf_len);
                let mut block_bytes = [0u8; 64];
                if end > start {
                    block_bytes[..end - start].copy_from_slice(&self.chunk_buffer[start..end]);
                }
                let mut block = [0u32; 16];
                for w in 0..16 {
                    let chunk = &block_bytes[4 * w..4 * (w + 1)];
                    block[w] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                }
                let mut flags = self.flags;
                if i == 0 {
                    flags |= CHUNK_START;
                }
                let block_len = (end - start) as u32;
                let out16 = compress(&cv, &block, block_len, 0, flags);
                cv.copy_from_slice(&out16[..8]);
            }

            let start = (num_blocks - 1) * 64;
            let end = buf_len;
            let mut block_bytes = [0u8; 64];
            if end > start {
                block_bytes[..end - start].copy_from_slice(&self.chunk_buffer[start..end]);
            }
            let mut block = [0u32; 16];
            for w in 0..16 {
                let chunk = &block_bytes[4 * w..4 * (w + 1)];
                block[w] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            }
            let mut flags = self.flags | CHUNK_END | ROOT;
            if num_blocks == 1 {
                flags |= CHUNK_START;
            }
            let block_len = (end.saturating_sub(start)) as u32;

            let mut written = 0;
            let mut counter = 0u64;
            while written < out.len() {
                let out16 = compress(&cv, &block, block_len, counter, flags);
                let mut chunk_out = [0u8; 64];
                for (w, word) in out16.iter().enumerate() {
                    chunk_out[4 * w..4 * (w + 1)].copy_from_slice(&word.to_le_bytes());
                }
                let take = (out.len() - written).min(64);
                out[written..written + take].copy_from_slice(&chunk_out[..take]);
                written += take;
                counter += 1;
            }
            return;
        }

        // Multi-chunk tree: reduce stack
        let mut cv = self.chunk_cv(self.chunk_len, self.chunk_counter);
        for i in (1..self.cv_stack_len).rev() {
            let left = self.cv_stack[i];
            cv = parent_cv(&left, &cv, &self.key, self.flags);
        }

        let left = self.cv_stack[0];
        let mut written = 0;
        let mut counter = 0u64;
        while written < out.len() {
            let out16 = parent_output(&left, &cv, &self.key, self.flags, counter);
            let mut chunk_out = [0u8; 64];
            for (w, word) in out16.iter().enumerate() {
                chunk_out[4 * w..4 * (w + 1)].copy_from_slice(&word.to_le_bytes());
            }
            let take = (out.len() - written).min(64);
            out[written..written + take].copy_from_slice(&chunk_out[..take]);
            written += take;
            counter += 1;
        }
    }

    /// Squeezes the standard 32-byte BLAKE3 digest.
    pub fn finalize(&self) -> Digest<32> {
        let mut bytes = [0u8; 32];
        self.finalize_into(&mut bytes);
        Digest::from_bytes(bytes)
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes the default 32-byte BLAKE3 digest of `data`.
///
/// # Examples
///
/// ```
/// let hash = sillstone_digest::blake3(b"").unwrap();
/// assert_eq!(
///     hash.as_bytes()[..4],
///     [0xaf, 0x13, 0x49, 0xb9]
/// );
/// ```
pub fn blake3(data: &[u8]) -> Result<Digest<32>> {
    let mut hasher = Hasher::new();
    hasher.update(data);
    Ok(hasher.finalize())
}

/// Computes a 32-byte keyed BLAKE3 MAC of `data` using `key`.
pub fn blake3_keyed(key: &[u8; 32], data: &[u8]) -> Result<Digest<32>> {
    let mut hasher = Hasher::new_keyed(key);
    hasher.update(data);
    Ok(hasher.finalize())
}

/// Computes extendable output (XOF) of arbitrary length into `out`.
pub fn blake3_xof(data: &[u8], out: &mut [u8]) -> Result<()> {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize_into(out);
    Ok(())
}
