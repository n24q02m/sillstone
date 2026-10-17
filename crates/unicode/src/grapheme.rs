//! Unicode Standard Annex #29: Unicode Text Segmentation — Grapheme Cluster Boundaries.
//!
//! Provides grapheme cluster segmentation over UTF-8 strings conforming strictly
//! to UAX #29 (Unicode 15.1.0) rules GB1 through GB14, including Indic Conjunct
//! Break rules (GB9c) and Extended Pictographic ZWJ sequences (GB11).

use crate::tables::{self, Gbp, GeneralCategory, InCb};

/// Returns the UAX #29 Grapheme_Cluster_Break property of a code point.
#[inline]
pub fn grapheme_cluster_break(cp: u32) -> Gbp {
    match tables::GB_TABLE.binary_search_by(|&(start, end, _)| {
        if cp < start {
            core::cmp::Ordering::Greater
        } else if cp > end {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Equal
        }
    }) {
        Ok(idx) => tables::GB_TABLE[idx].2,
        Err(_) => Gbp::Other,
    }
}

/// Returns whether a code point has the `Extended_Pictographic` property.
#[inline]
pub fn is_extended_pictographic(cp: u32) -> bool {
    tables::EXTPIC_TABLE
        .binary_search_by(|&(start, end)| {
            if cp < start {
                core::cmp::Ordering::Greater
            } else if cp > end {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Returns the `Indic_Conjunct_Break` (InCB) property of a code point.
#[inline]
pub fn indic_conjunct_break(cp: u32) -> InCb {
    match tables::INCB_TABLE.binary_search_by(|&(start, end, _)| {
        if cp < start {
            core::cmp::Ordering::Greater
        } else if cp > end {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Equal
        }
    }) {
        Ok(idx) => tables::INCB_TABLE[idx].2,
        Err(_) => InCb::None,
    }
}

/// Returns the Unicode General Category of a character.
#[inline]
pub fn char_category(c: char) -> GeneralCategory {
    let cp = c as u32;
    match tables::CAT_TABLE.binary_search_by(|&(start, end, _)| {
        if cp < start {
            core::cmp::Ordering::Greater
        } else if cp > end {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Equal
        }
    }) {
        Ok(idx) => tables::CAT_TABLE[idx].2,
        Err(_) => GeneralCategory::Cn,
    }
}

/// Returns an algorithmic or standard character name for common ranges.
pub fn char_name(c: char) -> Option<String> {
    let cp = c as u32;
    if (0xAC00..=0xD7A3).contains(&cp) {
        // Hangul syllable algorithmic name
        let s_index = cp - 0xAC00;
        let l_index = (s_index / 588) as usize;
        let v_index = ((s_index % 588) / 28) as usize;
        let t_index = (s_index % 28) as usize;

        const JAMO_L: &[&str] = &[
            "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K",
            "T", "P", "H",
        ];
        const JAMO_V: &[&str] = &[
            "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U",
            "WEO", "WE", "WI", "YU", "EU", "YI", "I",
        ];
        const JAMO_T: &[&str] = &[
            "", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP",
            "LH", "M", "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
        ];

        let mut s = String::from("HANGUL SYLLABLE ");
        s.push_str(JAMO_L[l_index]);
        s.push_str(JAMO_V[v_index]);
        s.push_str(JAMO_T[t_index]);
        return Some(s);
    }
    if (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
    {
        return Some(format!("CJK UNIFIED IDEOGRAPH-{cp:04X}"));
    }
    if (0x17000..=0x187F7).contains(&cp) {
        return Some(format!("TANGUT IDEOGRAPH-{cp:04X}"));
    }
    None
}

/// An iterator over the grapheme clusters of a string slice according to UAX #29.
#[derive(Clone, Debug)]
pub struct Graphemes<'a> {
    text: &'a str,
    cursor: usize,
}

impl<'a> Graphemes<'a> {
    /// Creates a new grapheme cluster iterator for `text`.
    pub fn new(text: &'a str) -> Self {
        Self { text, cursor: 0 }
    }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.text.len() {
            return None;
        }

        let remaining = &self.text[self.cursor..];
        let mut char_indices = remaining.char_indices();
        let (_, first_char) = char_indices.next()?;

        let prev_cp = first_char as u32;
        let mut prev_gb = grapheme_cluster_break(prev_cp);

        let mut ri_count: usize = if prev_gb == Gbp::Regional_Indicator {
            1
        } else {
            0
        };
        let mut extpic_state: bool = is_extended_pictographic(prev_cp);

        // Keep track of recent codepoints for rule GB9c
        // 16 is sufficient for any Indic conjunct sequence
        let mut incb_buf = [0u32; 16];
        let mut incb_len = 1;
        incb_buf[0] = prev_cp;

        let mut end_offset = remaining.len();

        for (byte_offset, next_char) in char_indices {
            let next_cp = next_char as u32;
            let next_gb = grapheme_cluster_break(next_cp);

            let mut brk = should_break(
                prev_gb,
                next_gb,
                ri_count,
                extpic_state,
                next_cp,
            );

            if brk && check_gb9c(&incb_buf[..incb_len], next_cp) {
                brk = false;
            }

            if brk {
                end_offset = byte_offset;
                break;
            }

            // Update state
            if next_gb == Gbp::Regional_Indicator {
                ri_count += 1;
            } else {
                ri_count = 0;
            }

            if is_extended_pictographic(next_cp) {
                extpic_state = true;
            } else if extpic_state {
                if next_gb != Gbp::Extend && next_gb != Gbp::ZWJ {
                    extpic_state = false;
                }
            }

            if incb_len < incb_buf.len() {
                incb_buf[incb_len] = next_cp;
                incb_len += 1;
            } else {
                incb_buf.copy_within(1.., 0);
                incb_buf[incb_len - 1] = next_cp;
            }

            prev_gb = next_gb;
        }

        let cluster = &remaining[..end_offset];
        self.cursor += end_offset;
        Some(cluster)
    }
}

#[inline]
fn should_break(
    p: Gbp,
    n: Gbp,
    ri_count: usize,
    in_extpic_seq: bool,
    next_cp: u32,
) -> bool {
    // GB3: CR x LF
    if p == Gbp::CR && n == Gbp::LF {
        return false;
    }
    // GB4: (Control | CR | LF) ÷
    if p == Gbp::Control || p == Gbp::CR || p == Gbp::LF {
        return true;
    }
    // GB5: ÷ (Control | CR | LF)
    if n == Gbp::Control || n == Gbp::CR || n == Gbp::LF {
        return true;
    }
    // GB6: L x (L | V | LV | LVT)
    if p == Gbp::L && (n == Gbp::L || n == Gbp::V || n == Gbp::LV || n == Gbp::LVT) {
        return false;
    }
    // GB7: (LV | V) x (V | T)
    if (p == Gbp::LV || p == Gbp::V) && (n == Gbp::V || n == Gbp::T) {
        return false;
    }
    // GB8: (LVT | T) x T
    if (p == Gbp::LVT || p == Gbp::T) && n == Gbp::T {
        return false;
    }
    // GB9: x (Extend | ZWJ)
    if n == Gbp::Extend || n == Gbp::ZWJ {
        return false;
    }
    // GB9a: x SpacingMark
    if n == Gbp::SpacingMark {
        return false;
    }
    // GB9b: Prepend x
    if p == Gbp::Prepend {
        return false;
    }
    // GB11: \p{Extended_Pictographic} Extend* ZWJ x \p{Extended_Pictographic}
    if in_extpic_seq && p == Gbp::ZWJ && is_extended_pictographic(next_cp) {
        return false;
    }
    // GB12/13: sot (RI RI)* RI x RI
    if p == Gbp::Regional_Indicator && n == Gbp::Regional_Indicator {
        if ri_count % 2 == 1 {
            return false;
        }
    }
    // GB999: Any ÷ Any
    true
}

#[inline]
fn check_gb9c(curr_cps: &[u32], next_cp: u32) -> bool {
    if indic_conjunct_break(next_cp) != InCb::Consonant {
        return false;
    }
    let mut saw_linker = false;
    for &cp in curr_cps.iter().rev() {
        match indic_conjunct_break(cp) {
            InCb::Linker => saw_linker = true,
            InCb::Extend => continue,
            InCb::Consonant => return saw_linker,
            InCb::None => return false,
        }
    }
    false
}

/// Returns an iterator over the grapheme clusters of `s`.
#[inline]
pub fn graphemes(s: &str) -> Graphemes<'_> {
    Graphemes::new(s)
}
