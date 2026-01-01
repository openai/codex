//! Ultrathink keyword detection.

use regex::Regex;
use std::sync::LazyLock;

/// Regex to detect "ultrathink" keyword (case-insensitive, word boundary).
static ULTRATHINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bultrathink\b").expect("valid regex"));

/// Detect if the message contains the "ultrathink" keyword.
///
/// The detection is case-insensitive and requires word boundaries,
/// matching "ultrathink", "ULTRATHINK", "Ultrathink", etc.
pub fn detect_ultrathink(message: &str) -> bool {
    ULTRATHINK_REGEX.is_match(message)
}

/// Extract all positions of "ultrathink" keyword for UI highlighting.
///
/// Returns a vector of (start, end) byte positions for each match.
pub fn extract_keyword_positions(message: &str) -> Vec<(usize, usize)> {
    ULTRATHINK_REGEX
        .find_iter(message)
        .map(|m| (m.start(), m.end()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_ultrathink() {
        // Should match
        assert!(detect_ultrathink("ultrathink"));
        assert!(detect_ultrathink("ULTRATHINK"));
        assert!(detect_ultrathink("Ultrathink"));
        assert!(detect_ultrathink("please ultrathink about this"));
        assert!(detect_ultrathink("ultrathink: solve this problem"));

        // Should not match
        assert!(!detect_ultrathink("ultrathinking")); // Not word boundary
        assert!(!detect_ultrathink("myultrathink")); // Not word boundary
        assert!(!detect_ultrathink("ultra think")); // Space in middle
        assert!(!detect_ultrathink("think")); // Different word
    }

    #[test]
    fn test_extract_keyword_positions() {
        let positions = extract_keyword_positions("test ultrathink here");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], (5, 15));

        let positions = extract_keyword_positions("ultrathink and ULTRATHINK");
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], (0, 10));
        assert_eq!(positions[1], (15, 25));

        let positions = extract_keyword_positions("no keyword here");
        assert!(positions.is_empty());
    }
}
