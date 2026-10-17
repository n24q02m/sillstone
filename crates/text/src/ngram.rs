//! String n-gram tokenization and Jaccard similarity.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

/// Extracts character n-grams from a string.
///
/// Returns an empty vector if `n == 0` or if the string has fewer than `n` characters.
pub fn ngrams(s: &str, n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < n {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(chars.len() - n + 1);
    for window in chars.windows(n) {
        result.push(window.iter().collect::<String>());
    }
    result
}

/// Computes the Jaccard similarity index between two n-gram collections:
///
/// `J(A, B) = |A ∩ B| / |A ∪ B|`
///
/// Returns `1.0` if both collections are empty, or `0.0` if only one is empty.
pub fn ngram_jaccard(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let set_a: BTreeSet<&String> = a.iter().collect();
    let set_b: BTreeSet<&String> = b.iter().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}
