/// Simple case-insensitive subsequence matcher used for fuzzy filtering.
///
/// Returns the indices (positions) of the matched characters in `haystack`
/// and a score where smaller is better. Currently, indices are byte offsets
/// from `char_indices()` of a lowercased copy of `haystack`.
///
/// Note: For ASCII inputs these indices align with character positions. If
/// extended Unicode inputs are used, be mindful of byte vs char indices.
pub fn fuzzy_match(haystack: &str, needle: &str) -> Option<(Vec<usize>, i32)> {
    if needle.is_empty() {
        return Some((Vec::new(), i32::MAX));
    }
    let h_lower = haystack.to_lowercase();
    let n_lower = needle.to_lowercase();
    let mut indices: Vec<usize> = Vec::with_capacity(n_lower.len());
    let mut h_iter = h_lower.char_indices();
    let mut last_pos: Option<usize> = None;

    for ch in n_lower.chars() {
        let mut found = None;
        for (i, hc) in h_iter.by_ref() {
            if hc == ch {
                found = Some(i);
                break;
            }
        }
        if let Some(pos) = found {
            indices.push(pos);
            last_pos = Some(pos);
        } else {
            return None;
        }
    }

    // Score: window length minus needle length (tighter is better), with a bonus for prefix match.
    let first = *indices.first().unwrap_or(&0);
    let last = last_pos.unwrap_or(first);
    let window = (last as i32 - first as i32 + 1) - (n_lower.len() as i32);
    let mut score = window.max(0);
    if first == 0 {
        score -= 100; // strong bonus for prefix match
    }
    Some((indices, score))
}

/// Convenience wrapper to get only the indices for a fuzzy match.
pub fn fuzzy_indices(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    fuzzy_match(haystack, needle).map(|(idx, _)| idx)
}
