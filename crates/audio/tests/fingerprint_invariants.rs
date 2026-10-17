//! Tests for audio fingerprint invariants: gain invariance and low noise stability.

use sillstone_audio::{fingerprint, resample_linear};

fn generate_synthetic_audio(seconds: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (seconds * sample_rate as f32) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    let freqs = [
        130.0, 220.0, 330.0, 440.0, 550.0, 660.0, 880.0, 1100.0, 1400.0, 1800.0, 2200.0,
        2800.0, 3500.0, 4400.0,
    ];
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let mut s = 0.0f32;
        for (idx, &f) in freqs.iter().enumerate() {
            let phase = idx as f32 * 0.7;
            s += 0.08 * (2.0 * core::f32::consts::PI * f * t + phase).sin();
        }
        samples.push(s);
    }
    samples
}

#[test]
fn test_audio_fingerprint_empty_short() {
    assert_eq!(fingerprint(&[], 44100).unwrap(), Vec::<u32>::new());
    let short = [0.1f32; 100];
    assert_eq!(fingerprint(&short, 44100).unwrap(), Vec::<u32>::new());
}

#[test]
fn test_audio_fingerprint_gain_invariance() {
    let samples = generate_synthetic_audio(2.0, 44100);
    let fp1 = fingerprint(&samples, 44100).expect("fingerprint 1");
    assert!(!fp1.is_empty(), "must produce sub-fingerprints for 2 seconds");

    // Scale by 0.3
    let scaled_down: Vec<f32> = samples.iter().map(|&s| s * 0.3).collect();
    let fp2 = fingerprint(&scaled_down, 44100).expect("fingerprint scaled down");
    assert_eq!(
        fp1, fp2,
        "Gain scaling must produce bit-exact identical sub-fingerprints"
    );

    // Scale by 2.5
    let scaled_up: Vec<f32> = samples.iter().map(|&s| s * 2.5).collect();
    let fp3 = fingerprint(&scaled_up, 44100).expect("fingerprint scaled up");
    assert_eq!(
        fp1, fp3,
        "Gain boost must produce bit-exact identical sub-fingerprints"
    );
}

#[test]
fn test_audio_fingerprint_noise_stability() {
    let samples = generate_synthetic_audio(2.0, 44100);
    let fp_clean = fingerprint(&samples, 44100).expect("clean");

    // Add zero-mean small noise: ~0.1% signal level
    let noisy: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let noise = 0.0005 * (((i * 101) % 97) as f32 / 48.5 - 1.0);
            s + noise
        })
        .collect();
    let fp_noisy = fingerprint(&noisy, 44100).expect("noisy");

    assert_eq!(fp_clean.len(), fp_noisy.len());
    let mut total_bit_diff = 0u32;
    for (&c, &n) in fp_clean.iter().zip(fp_noisy.iter()) {
        total_bit_diff += (c ^ n).count_ones();
    }
    let avg_diff = total_bit_diff as f64 / fp_clean.len() as f64;
    assert!(
        avg_diff < 3.0,
        "Average flipped bits ({avg_diff}) must be less than 3 out of 32"
    );
}

#[test]
fn test_audio_resample() {
    let original = [0.0f32, 1.0, 2.0, 3.0];
    let resampled = resample_linear(&original, 4, 8);
    assert_eq!(resampled.len(), 8);
    assert_eq!(resampled[0], 0.0);
}
