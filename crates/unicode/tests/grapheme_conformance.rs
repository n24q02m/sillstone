//! UAX #29 Grapheme Conformance tests.

use sillstone_unicode::{char_category, char_name, graphemes, GeneralCategory};

#[test]
fn test_official_uax29_grapheme_break() {
    let test_data = include_str!("GraphemeBreakTest.txt");
    let mut total_cases = 0;
    let mut passed_cases = 0;

    for (line_no, line) in test_data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let test_part = line.split('#').next().unwrap().trim();
        let tokens: Vec<&str> = test_part.split_whitespace().collect();

        let mut expected_clusters: Vec<String> = Vec::new();
        let mut current_cluster = String::new();

        for token in tokens {
            if token == "÷" {
                if !current_cluster.is_empty() {
                    expected_clusters.push(current_cluster.clone());
                    current_cluster.clear();
                }
            } else if token == "×" {
                // boundary must not break
            } else {
                let cp = u32::from_str_radix(token, 16)
                    .unwrap_or_else(|e| panic!("line {}: invalid hex {}: {}", line_no + 1, token, e));
                let c = char::from_u32(cp)
                    .unwrap_or_else(|| panic!("line {}: invalid code point {:X}", line_no + 1, cp));
                current_cluster.push(c);
            }
        }

        total_cases += 1;
        let full_text: String = expected_clusters.concat();
        let actual_clusters: Vec<String> = graphemes(&full_text).map(|s| s.to_string()).collect();

        assert_eq!(
            actual_clusters,
            expected_clusters,
            "UAX#29 grapheme break mismatch on line {}:\nExpected: {:?}\nActual:   {:?}",
            line_no + 1,
            expected_clusters,
            actual_clusters
        );
        passed_cases += 1;
    }

    assert!(total_cases >= 1180, "Expected at least 1180 test cases, ran {}", total_cases);
    assert_eq!(total_cases, passed_cases, "All test cases must pass");
}

#[test]
fn test_char_category_and_name() {
    assert_eq!(char_category('A'), GeneralCategory::Lu);
    assert_eq!(char_category('a'), GeneralCategory::Ll);
    assert_eq!(char_category('0'), GeneralCategory::Nd);
    assert_eq!(char_category(' '), GeneralCategory::Zs);
    assert_eq!(char_category('\n'), GeneralCategory::Cc);
    assert_eq!(char_category('!'), GeneralCategory::Po);
    assert_eq!(char_category('가'), GeneralCategory::Lo);

    assert_eq!(char_name('가').as_deref(), Some("HANGUL SYLLABLE GA"));
    assert_eq!(char_name('한').as_deref(), Some("HANGUL SYLLABLE HAN"));
    assert_eq!(char_name('글').as_deref(), Some("HANGUL SYLLABLE GEUL"));
    assert_eq!(char_name('中').as_deref(), Some("CJK UNIFIED IDEOGRAPH-4E2D"));
    assert_eq!(char_name('A'), None);
}
