//! Video scene cut detection using chi-square histogram difference and adaptive thresholding.

use alloc::vec::Vec;
use sillstone_h264::Frame;

/// Parameters for scene cut detection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCutParams {
    /// Base chi-square distance threshold (in range 0.0..=1.0).
    pub threshold: f64,
    /// Minimum frame distance between consecutive cuts.
    pub min_scene_len: usize,
}

impl Default for SceneCutParams {
    fn default() -> Self {
        Self {
            threshold: 0.35,
            min_scene_len: 2,
        }
    }
}

/// Computes a normalized 16-bin luma histogram from the frame's Y plane.
fn luma_histogram(frame: &Frame) -> [f64; 16] {
    let mut hist = [0.0f64; 16];
    if frame.y.is_empty() {
        return hist;
    }
    for &val in &frame.y {
        let bin = (val >> 4) as usize;
        hist[bin.min(15)] += 1.0;
    }
    let total = frame.y.len() as f64;
    for slot in &mut hist {
        *slot /= total;
    }
    hist
}

/// Computes the Chi-Square histogram distance between two normalized histograms:
///
/// `χ²(P, Q) = 0.5 * Σ (P_i - Q_i)² / (P_i + Q_i + ε)`
fn chi_square_distance(p: &[f64; 16], q: &[f64; 16]) -> f64 {
    let mut dist = 0.0;
    for i in 0..16 {
        let sum = p[i] + q[i];
        if sum > 1e-9 {
            let diff = p[i] - q[i];
            dist += (diff * diff) / sum;
        }
    }
    dist * 0.5
}

/// Detects scene cut frame indices in an ordered sequence of video frames.
///
/// Returns the list of 0-based frame indices where a new scene begins.
/// Returns an empty list for 0 or 1 frames.
pub fn scene_cuts(frames: &[Frame], params: SceneCutParams) -> Vec<usize> {
    if frames.len() <= 1 {
        return Vec::new();
    }

    let histograms: Vec<[f64; 16]> = frames.iter().map(luma_histogram).collect();
    let mut diffs = Vec::with_capacity(frames.len() - 1);

    for i in 1..frames.len() {
        diffs.push(chi_square_distance(&histograms[i - 1], &histograms[i]));
    }

    let count = diffs.len() as f64;
    let mean: f64 = diffs.iter().sum::<f64>() / count;
    let variance: f64 = diffs
        .iter()
        .map(|&d| {
            let diff = d - mean;
            diff * diff
        })
        .sum::<f64>()
        / count;
    let std_dev = variance.sqrt();

    let mut cuts = Vec::new();
    let mut last_cut: Option<usize> = None;

    for (i, &diff) in diffs.iter().enumerate() {
        let frame_idx = i + 1;
        let is_adaptive_cut = diff >= params.threshold && diff > (mean + 1.5 * std_dev);
        let is_hard_cut = diff >= params.threshold * 1.5;

        if is_adaptive_cut || is_hard_cut {
            if let Some(prev) = last_cut {
                if frame_idx - prev < params.min_scene_len {
                    continue;
                }
            }
            cuts.push(frame_idx);
            last_cut = Some(frame_idx);
        }
    }

    cuts
}
