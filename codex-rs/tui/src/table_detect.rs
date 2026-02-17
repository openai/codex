//! Shared pipe-table detection helpers.
//!
//! Both the streaming controller (`streaming/controller.rs`) and the
//! markdown-fence unwrapper (`markdown.rs`) need to identify pipe-table
//! structure in raw markdown source. This module provides the canonical
//! implementations so fixes only need to happen in one place.

/// Split a pipe-delimited line into trimmed segments.
///
/// Returns `None` if the line is empty or has no `|` marker.
/// Leading/trailing pipes are stripped before splitting.
pub(crate) fn parse_table_segments(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.contains('|') {
        return None;
    }

    let mut content = trimmed;
    if let Some(without_leading) = content.strip_prefix('|') {
        content = without_leading;
    }
    if let Some(without_trailing) = content.strip_suffix('|') {
        content = without_trailing;
    }

    let segments: Vec<&str> = content.split('|').map(str::trim).collect();
    (!segments.is_empty()).then_some(segments)
}

/// Whether `line` looks like a table header row (has pipe-separated
/// segments with at least one non-empty cell).
pub(crate) fn is_table_header_line(line: &str) -> bool {
    parse_table_segments(line).is_some_and(|segments| segments.iter().any(|s| !s.is_empty()))
}

/// Whether a single segment matches the `---`, `:---`, `---:`, or `:---:`
/// alignment-colon syntax used in markdown table delimiter rows.
pub(crate) fn is_table_delimiter_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return false;
    }
    let without_leading = trimmed.strip_prefix(':').unwrap_or(trimmed);
    let without_ends = without_leading.strip_suffix(':').unwrap_or(without_leading);
    without_ends.len() >= 3 && without_ends.chars().all(|c| c == '-')
}

/// Whether `line` is a valid table delimiter row (every segment passes
/// [`is_table_delimiter_segment`]).
pub(crate) fn is_table_delimiter_line(line: &str) -> bool {
    parse_table_segments(line)
        .is_some_and(|segments| segments.into_iter().all(is_table_delimiter_segment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table_segments_basic() {
        assert_eq!(
            parse_table_segments("| A | B | C |"),
            Some(vec!["A", "B", "C"])
        );
    }

    #[test]
    fn parse_table_segments_no_outer_pipes() {
        assert_eq!(parse_table_segments("A | B | C"), Some(vec!["A", "B", "C"]));
    }

    #[test]
    fn parse_table_segments_no_leading_pipe() {
        assert_eq!(
            parse_table_segments("A | B | C |"),
            Some(vec!["A", "B", "C"])
        );
    }

    #[test]
    fn parse_table_segments_no_trailing_pipe() {
        assert_eq!(
            parse_table_segments("| A | B | C"),
            Some(vec!["A", "B", "C"])
        );
    }

    #[test]
    fn parse_table_segments_single_segment_is_allowed() {
        assert_eq!(parse_table_segments("| only |"), Some(vec!["only"]));
    }

    #[test]
    fn parse_table_segments_without_pipe_returns_none() {
        assert_eq!(parse_table_segments("just text"), None);
    }

    #[test]
    fn parse_table_segments_empty_returns_none() {
        assert_eq!(parse_table_segments(""), None);
        assert_eq!(parse_table_segments("   "), None);
    }

    #[test]
    fn is_table_delimiter_segment_valid() {
        assert!(is_table_delimiter_segment("---"));
        assert!(is_table_delimiter_segment(":---"));
        assert!(is_table_delimiter_segment("---:"));
        assert!(is_table_delimiter_segment(":---:"));
        assert!(is_table_delimiter_segment(":-------:"));
    }

    #[test]
    fn is_table_delimiter_segment_invalid() {
        assert!(!is_table_delimiter_segment(""));
        assert!(!is_table_delimiter_segment("--"));
        assert!(!is_table_delimiter_segment("abc"));
        assert!(!is_table_delimiter_segment(":--"));
    }

    #[test]
    fn is_table_delimiter_line_valid() {
        assert!(is_table_delimiter_line("| --- | --- |"));
        assert!(is_table_delimiter_line("|:---:|---:|"));
        assert!(is_table_delimiter_line("--- | --- | ---"));
    }

    #[test]
    fn is_table_delimiter_line_invalid() {
        assert!(!is_table_delimiter_line("| A | B |"));
        assert!(!is_table_delimiter_line("| -- | -- |"));
    }

    #[test]
    fn is_table_header_line_valid() {
        assert!(is_table_header_line("| A | B |"));
        assert!(is_table_header_line("Name | Value"));
    }

    #[test]
    fn is_table_header_line_all_empty_segments() {
        assert!(!is_table_header_line("| | |"));
    }
}
