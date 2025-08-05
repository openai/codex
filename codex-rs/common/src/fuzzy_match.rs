/// Simple case-insensitive subsequence matcher used for fuzzy filtering.
///
/// Returns the indices (character positions) of the matched characters in the
/// ORIGINAL `haystack` string and a score where smaller is better.
///
/// Unicode correctness: we perform the match on a lowercased copy of the
/// haystack and needle but maintain a mapping from each character in the
/// lowercased haystack back to the original character index in `haystack`.
/// This ensures the returned indices can be safely used with
/// `str::chars().enumerate()` consumers for highlighting, even when
/// lowercasing expands certain characters (e.g., ß → ss, İ → i̇).
pub fn fuzzy_match(haystack: &str, needle: &str) -> Option<(Vec<usize>, i32)> {
    if needle.is_empty() {
        return Some((Vec::new(), i32::MAX));
    }

    let mut lowered_chars: Vec<char> = Vec::new();
    let mut lowered_to_orig_char_idx: Vec<usize> = Vec::new();
    for (orig_idx, ch) in haystack.chars().enumerate() {
        for lc in ch.to_lowercase() {
            lowered_chars.push(lc);
            lowered_to_orig_char_idx.push(orig_idx);
        }
    }

    let lowered_needle: Vec<char> = needle.to_lowercase().chars().collect();

    let mut result_orig_indices: Vec<usize> = Vec::with_capacity(lowered_needle.len());
    let mut last_lower_pos: Option<usize> = None;
    let mut cur = 0usize;
    for &nc in lowered_needle.iter() {
        let mut found_at: Option<usize> = None;
        while cur < lowered_chars.len() {
            if lowered_chars[cur] == nc {
                found_at = Some(cur);
                cur += 1;
                break;
            }
            cur += 1;
        }
        let pos = found_at?;
        result_orig_indices.push(lowered_to_orig_char_idx[pos]);
        last_lower_pos = Some(pos);
    }

    let first_lower_pos = if result_orig_indices.is_empty() {
        0usize
    } else {
        let target_orig = result_orig_indices[0];
        lowered_to_orig_char_idx
            .iter()
            .position(|&oi| oi == target_orig)
            .unwrap_or(0)
    };
    let last_lower_pos = last_lower_pos.unwrap_or(first_lower_pos);
    let window =
        (last_lower_pos as i32 - first_lower_pos as i32 + 1) - (lowered_needle.len() as i32);
    let mut score = window.max(0);
    if first_lower_pos == 0 {
        score -= 100;
    }

    result_orig_indices.sort_unstable();
    result_orig_indices.dedup();
    Some((result_orig_indices, score))
}

/// Convenience wrapper to get only the indices for a fuzzy match.
pub fn fuzzy_indices(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    fuzzy_match(haystack, needle).map(|(mut idx, _)| {
        idx.sort_unstable();
        idx.dedup();
        idx
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_basic_indices() {
        let (idx, _score) = match fuzzy_match("hello", "hl") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        assert_eq!(idx, vec![0, 2]);
    }

    #[test]
    fn unicode_dotted_i_istanbul_highlighting() {
        let (idx, _score) = match fuzzy_match("İstanbul", "is") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn unicode_german_sharp_s_casefold() {
        assert!(fuzzy_match("straße", "strasse").is_none());
    }

    #[test]
    fn prefer_contiguous_match_over_spread() {
        // Contiguous match should receive a better (smaller) score than a
        // spread-out match because the window penalty is larger when
        // characters are farther apart.
        let (_idx_a, score_a) = fuzzy_match("abc", "abc").expect("expected match");
        let (_idx_b, score_b) = fuzzy_match("a-b-c", "abc").expect("expected match");
        assert!(
            score_a < score_b,
            "contiguous match should rank better: {score_a} < {score_b}"
        );
    }

    #[test]
    fn start_of_string_bonus_applies() {
        // Matches that begin at index 0 get a large bonus (-100), so
        // "file_name" should outrank "my_file_name" for the pattern "file".
        let (_idx_a, score_a) = fuzzy_match("file_name", "file").expect("expected match");
        let (_idx_b, score_b) = fuzzy_match("my_file_name", "file").expect("expected match");
        assert!(
            score_a < score_b,
            "start-of-string bonus should apply: {score_a} < {score_b}"
        );
    }

    #[test]
    fn empty_needle_matches_with_max_score_and_no_indices() {
        let (idx, score) = fuzzy_match("anything", "").expect("empty needle should match");
        assert!(idx.is_empty());
        assert_eq!(score, i32::MAX);
    }

    #[test]
    fn case_insensitive_matching_basic() {
        // Verify case-insensitivity: mixed-case needle should match and
        // indices should refer to original haystack character positions.
        let (idx, _score) = fuzzy_match("Hello", "heL").expect("expected match");
        assert_eq!(idx, vec![0, 1, 2]);
    }

    #[test]
    fn indices_are_deduped_for_multichar_lowercase_expansion() {
        // U+0130 LATIN CAPITAL LETTER I WITH DOT ABOVE lowercases to two code
        // points: 'i' + U+0307 COMBINING DOT ABOVE. When the needle matches
        // both of these lowered code points, the resulting original indices
        // would be [0, 0] (same original char). The implementation should
        // return unique, sorted indices.
        let needle = "\u{0069}\u{0307}"; // "i" + combining dot above
        let (idx, _score) = fuzzy_match("İ", needle).expect("expected match");
        assert_eq!(idx, vec![0], "indices should be unique and sorted");
    }
}
