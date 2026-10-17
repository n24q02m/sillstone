//! Chromaprint-style audio fingerprinting.
//!
//! Extracts a sequence of 32-bit sub-fingerprints using windowed FFT, logarithmic
//! frequency bands, and differential spectral/temporal quantization.
//!
//! Invariants:
//! - Gain invariance: scaling samples by any positive constant produces the exact same fingerprint.
//! - Low noise invariance: small additive noise preserves the vast majority of sub-fingerprint bits.

use alloc::vec::Vec;
use core::f32::consts::PI;
use sillstone_math::fft_real;

use crate::{Error, Result};

/// Target analysis sample rate (11 025 Hz).
pub const FINGERPRINT_SAMPLE_RATE: u32 = 11_025;

/// Analysis window size in samples (≈ 185.7 ms at 11 025 Hz).
pub const FINGERPRINT_WINDOW: usize = 2048;

/// Hop size in samples (≈ 62 ms at 11 025 Hz).
pub const FINGERPRINT_HOP: usize = 682;

/// Number of logarithmic frequency bands.
pub const NUM_BANDS: usize = 16;

/// Resamples an input slice of f32 audio samples from `source_rate` to `target_rate`
/// using linear interpolation.
pub fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = source_rate as f64 / target_rate as f64;
    let out_len = ((samples.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let s0 = samples[idx.min(samples.len() - 1)];
        let s1 = samples[(idx + 1).min(samples.len() - 1)];
        out.push(s0 + frac * (s1 - s0));
    }

    out
}

/// Computes the chromaprint-style audio fingerprint for a mono f32 PCM audio buffer.
///
/// Returns a sequence of 32-bit sub-fingerprints, one for each analysis frame after the first.
pub fn fingerprint(samples: &[f32], sample_rate: u32) -> Result<Vec<u32>> {
    if sample_rate == 0 {
        return Err(Error::BadValue("sample_rate must be non-zero"));
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    // 1. Resample to 11025 Hz
    let resampled = if sample_rate != FINGERPRINT_SAMPLE_RATE {
        resample_linear(samples, sample_rate, FINGERPRINT_SAMPLE_RATE)
    } else {
        samples.to_vec()
    };

    if resampled.len() < FINGERPRINT_WINDOW {
        return Ok(Vec::new());
    }

    // 2. Hann window table
    let mut hann = [0.0f64; FINGERPRINT_WINDOW];
    for (i, slot) in hann.iter_mut().enumerate() {
        *slot = 0.5 * (1.0 - (2.0 * PI as f64 * i as f64 / (FINGERPRINT_WINDOW - 1) as f64).cos());
    }

    // 3. Define 16 logarithmic frequency bands between 100 Hz and 5000 Hz
    // In bin space: bin_hz = 11025 / 2048 ≈ 5.3833 Hz per bin
    let min_freq = 100.0f64;
    let max_freq = 5000.0f64;
    let bin_hz = FINGERPRINT_SAMPLE_RATE as f64 / FINGERPRINT_WINDOW as f64;

    let mut band_limits = [0usize; NUM_BANDS + 1];
    for i in 0..=NUM_BANDS {
        let freq = min_freq * (max_freq / min_freq).powf(i as f64 / NUM_BANDS as f64);
        let bin = (freq / bin_hz).round() as usize;
        band_limits[i] = bin.max(1).min(FINGERPRINT_WINDOW / 2);
    }

    // 4. Compute spectral band energies for each frame
    let num_frames = (resampled.len() - FINGERPRINT_WINDOW) / FINGERPRINT_HOP + 1;
    let mut frame_energies = Vec::with_capacity(num_frames);

    let mut windowed = [0.0f64; FINGERPRINT_WINDOW];
    for frame_idx in 0..num_frames {
        let start = frame_idx * FINGERPRINT_HOP;
        for j in 0..FINGERPRINT_WINDOW {
            windowed[j] = resampled[start + j] as f64 * hann[j];
        }

        let spectrum = fft_real(&windowed);
        let mut bands = [0.0f64; NUM_BANDS];

        for b in 0..NUM_BANDS {
            let b_start = band_limits[b];
            let b_end = band_limits[b + 1].max(b_start + 1);
            let mut energy = 0.0f64;
            for &c in &spectrum[b_start..b_end] {
                energy += c.norm_sq();
            }
            bands[b] = energy.ln_1p();
        }
        frame_energies.push(bands);
    }

    if frame_energies.len() < 2 {
        return Ok(Vec::new());
    }

    // 5. Quantize 16 spatial (intra-frame) and 16 temporal (inter-frame) differences into 32 bits
    let mut sub_fps = Vec::with_capacity(frame_energies.len() - 1);

    for t in 1..frame_energies.len() {
        let curr = &frame_energies[t];
        let prev = &frame_energies[t - 1];

        let mut bits = 0u32;

        // Bits 0..16: spatial differences curr[i] > curr[(i+1)%16]
        for i in 0..NUM_BANDS {
            let next_i = (i + 1) % NUM_BANDS;
            if curr[i] > curr[next_i] {
                bits |= 1 << i;
            }
        }

        // Bits 16..32: temporal differences curr[i] > prev[i]
        for i in 0..NUM_BANDS {
            if curr[i] > prev[i] {
                bits |= 1 << (16 + i);
            }
        }

        sub_fps.push(bits);
    }

    Ok(sub_fps)
}
