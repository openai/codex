//! Canonical pipe-table structure detection and fenced-code-block tracking for
//! raw markdown source.
//!
//! Both the streaming controller (`streaming/controller.rs`) and the
//! markdown-fence unwrapper (`markdown.rs`) need to identify pipe-table
//! structure and fenced code blocks in raw markdown source.  This module
//! provides the canonical implementations so fixes only need to happen in one
//! place.
//!
//! ## Concepts
//!
//! A GFM pipe table is a sequence of lines where:
//! - A **header line** contains pipe-separated segments with at least one
//!   non-empty cell.
//! - A **delimiter line** immediately follows the header and contains only
//!   alignment markers (`---`, `:---`, `---:`, `:---:`), each with at least
//!   three dashes.
//! - **Body rows** follow the delimiter.
//!
//! A **fenced code block** starts with 3+ backticks or tildes and ends with a
//! matching close marker.  [`FenceTracker`] classifies each line as
//! [`FenceKind::Outside`], [`FenceKind::Markdown`], or [`FenceKind::Other`]
//! so callers can skip pipe characters that appear inside non-markdown fences.
//!
//! The table functions operate on single lines and do not maintain cross-line
//! state.  Callers (the streaming controller and fence unwrapper) are
//! responsible for pairing consecutive lines to confirm a table.

/// Split a pipe-delimited line into trimmed segments.
///
/// Returns `None` if the line is empty or has no unescaped separator marker.
/// Leading/trailing pipes are stripped before splitting.
///
/// This is intentionally a structural parser, not a renderer. It preserves
/// escaped pipes inside the returned segments because callers only care about
/// whether the line can participate in a table, not how the cell text should
/// finally be displayed.
pub(crate) fn parse_table_segments(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let has_outer_pipe = trimmed.starts_with('|') || trimmed.ends_with('|');
    let content = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let content = content.strip_suffix('|').unwrap_or(content);
    let raw_segments = split_unescaped_pipe(content);
    if !has_outer_pipe && raw_segments.len() <= 1 {
        return None;
    }

    let segments: Vec<&str> = raw_segments.into_iter().map(str::trim).collect();
    (!segments.is_empty()).then_some(segments)
}

/// Split `content` on unescaped `|` characters.
///
/// A pipe preceded by `\` is treated as literal text, not a column separator.
/// The backslash remains in the segment (this is structure detection, not
/// rendering).
fn split_unescaped_pipe(content: &str) -> Vec<&str> {
    let mut segments = Vec::with_capacity(8);
    let mut start = 0;
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            // Skip the escaped character.
            i += 2;
        } else if bytes[i] == b'|' {
            segments.push(&content[start..i]);
            start = i + 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    segments.push(&content[start..]);
    segments
}

// Small table-detection helpers inlined for the streaming hot path — they are
// called on every source line during incremental holdback scanning.

/// Whether `line` looks like a table header row (has pipe-separated
/// segments with at least one non-empty cell).
#[inline]
pub(crate) fn is_table_header_line(line: &str) -> bool {
    parse_table_segments(line).is_some_and(|segments| segments.iter().any(|s| !s.is_empty()))
}

/// Whether a single segment matches the `---`, `:---`, `---:`, or `:---:`
/// alignment-colon syntax used in markdown table delimiter rows.
#[inline]
fn is_table_delimiter_segment(segment: &str) -> bool {
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
#[inline]
pub(crate) fn is_table_delimiter_line(line: &str) -> bool {
    parse_table_segments(line)
        .is_some_and(|segments| segments.into_iter().all(is_table_delimiter_segment))
}

// ---------------------------------------------------------------------------
// Fenced code block tracking
// ---------------------------------------------------------------------------

/// Where a source line sits relative to fenced code blocks.
///
/// Table holdback only applies to lines that are `Outside` or inside a
/// `Markdown` fence. Lines inside `Other` fences (e.g. `sh`, `rust`) are
/// ignored by the table scanner because their pipe characters are code, not
/// table syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FenceKind {
    /// Not inside any fenced code block.
    Outside,
    /// Inside a `` ```md `` or `` ```markdown `` fence.
    Markdown,
    /// Inside a fence with a non-markdown info string.
    Other,
}

/// An opening fence shared by the Markdown unwrapper and the streaming scanner.
#[derive(Clone, Copy)]
pub(crate) struct Fence {
    marker: char,
    len: usize,
    pub(crate) is_blockquoted: bool,
    pub(crate) kind: FenceKind,
}

impl Fence {
    pub(crate) fn open(line: &str) -> Option<Self> {
        let line = strip_line_indent(line)?;
        let text = strip_blockquote_prefix(line);
        let (marker, len) = parse_fence_marker(text)?;
        Some(Self {
            marker,
            len,
            is_blockquoted: line.trim_start().starts_with('>'),
            kind: if is_markdown_fence_info(text, len) {
                FenceKind::Markdown
            } else {
                FenceKind::Other
            },
        })
    }

    pub(crate) fn closes(self, line: &str) -> bool {
        let Some(line) = strip_line_indent(line) else {
            return false;
        };
        let text = if self.is_blockquoted {
            if !line.trim_start().starts_with('>') {
                return false;
            }
            strip_blockquote_prefix(line)
        } else {
            line
        };
        parse_fence_marker(text).is_some_and(|(marker, len)| {
            marker == self.marker && len >= self.len && text[len..].trim().is_empty()
        })
    }
}

/// Fences allow up to three leading spaces. A leading tab reaches at least column four.
fn strip_line_indent(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches(' ');
    (line.len() - trimmed.len() < 4 && !trimmed.starts_with('\t')).then_some(trimmed)
}

/// Query the fence context before advancing past each raw source line.
pub(crate) struct FenceTracker {
    state: Option<Fence>,
}

impl FenceTracker {
    #[inline]
    pub(crate) fn new() -> Self {
        Self { state: None }
    }

    pub(crate) fn advance(&mut self, raw_line: &str) {
        match self.state {
            None => self.state = Fence::open(raw_line),
            Some(fence) if fence.closes(raw_line) => self.state = None,
            Some(_) => {}
        }
    }

    #[inline]
    pub(crate) fn kind(&self) -> FenceKind {
        self.state.map_or(FenceKind::Outside, |fence| fence.kind)
    }
}

/// Return fence marker character and run length for a potential fence line.
///
/// Recognises backtick and tilde fences with a minimum run of 3.
/// The input should already have leading whitespace and blockquote prefixes
/// stripped.
#[inline]
fn parse_fence_marker(line: &str) -> Option<(char, usize)> {
    let first = line.as_bytes().first().copied()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let len = line.bytes().take_while(|&b| b == first).count();
    if len < 3 {
        return None;
    }
    Some((first as char, len))
}

/// Whether the info string after a fence marker indicates markdown content.
///
/// Matches `md` and `markdown` (case-insensitive).
#[inline]
pub(crate) fn is_markdown_fence_info(trimmed_line: &str, marker_len: usize) -> bool {
    let info = trimmed_line[marker_len..]
        .split_whitespace()
        .next()
        .unwrap_or_default();
    info.eq_ignore_ascii_case("md") || info.eq_ignore_ascii_case("markdown")
}

/// Peel all leading `>` blockquote markers from a line.
///
/// Tables can appear inside blockquotes (`> | A | B |`), so the holdback
/// scanner must strip these markers before checking for table syntax.
#[inline]
pub(crate) fn strip_blockquote_prefix(line: &str) -> &str {
    let mut rest = line.trim_start();
    loop {
        let Some(stripped) = rest.strip_prefix('>') else {
            return rest;
        };
        rest = stripped.strip_prefix(' ').unwrap_or(stripped).trim_start();
    }
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
    fn parse_table_segments_escaped_pipe() {
        // Escaped pipe should NOT split — stays inside the segment.
        assert_eq!(
            parse_table_segments(r"| A \| B | C |"),
            Some(vec![r"A \| B", "C"])
        );
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

    // -----------------------------------------------------------------------
    // FenceTracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn fence_tracker_outside_by_default() {
        let tracker = FenceTracker::new();
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_opens_and_closes_backtick_fence() {
        let mut tracker = FenceTracker::new();
        tracker.advance("```rust");
        assert_eq!(tracker.kind(), FenceKind::Other);

        tracker.advance("let x = 1;");
        assert_eq!(tracker.kind(), FenceKind::Other);

        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_opens_and_closes_tilde_fence() {
        let mut tracker = FenceTracker::new();
        tracker.advance("~~~python");
        assert_eq!(tracker.kind(), FenceKind::Other);
        tracker.advance("~~~");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_markdown_fence() {
        let mut tracker = FenceTracker::new();
        tracker.advance("```md");
        assert_eq!(tracker.kind(), FenceKind::Markdown);
        tracker.advance("| A | B |");
        assert_eq!(tracker.kind(), FenceKind::Markdown);
        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_markdown_case_insensitive() {
        let mut tracker = FenceTracker::new();
        tracker.advance("```Markdown");
        assert_eq!(tracker.kind(), FenceKind::Markdown);
        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_nested_shorter_marker_does_not_close() {
        let mut tracker = FenceTracker::new();
        tracker.advance("````sh");
        assert_eq!(tracker.kind(), FenceKind::Other);
        // Shorter marker inside should not close.
        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Other);
        // Matching length closes.
        tracker.advance("````");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_mismatched_char_does_not_close() {
        let mut tracker = FenceTracker::new();
        tracker.advance("```sh");
        assert_eq!(tracker.kind(), FenceKind::Other);
        // Tilde marker should not close a backtick fence.
        tracker.advance("~~~");
        assert_eq!(tracker.kind(), FenceKind::Other);
        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_indented_4_spaces_ignored() {
        let mut tracker = FenceTracker::new();
        tracker.advance("    ```sh");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_blockquote_prefix_stripped() {
        let mut tracker = FenceTracker::new();
        tracker.advance("> ```sh");
        assert_eq!(tracker.kind(), FenceKind::Other);
        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Other);
        tracker.advance("> ```");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    #[test]
    fn fence_tracker_close_with_trailing_content_does_not_close() {
        let mut tracker = FenceTracker::new();
        tracker.advance("```sh");
        assert_eq!(tracker.kind(), FenceKind::Other);
        // Trailing content prevents closing.
        tracker.advance("``` extra");
        assert_eq!(tracker.kind(), FenceKind::Other);
        tracker.advance("```");
        assert_eq!(tracker.kind(), FenceKind::Outside);
    }

    // -----------------------------------------------------------------------
    // Fence helper function tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_fence_marker_backtick() {
        assert_eq!(parse_fence_marker("```rust"), Some(('`', 3)));
        assert_eq!(parse_fence_marker("````"), Some(('`', 4)));
    }

    #[test]
    fn parse_fence_marker_tilde() {
        assert_eq!(parse_fence_marker("~~~python"), Some(('~', 3)));
    }

    #[test]
    fn parse_fence_marker_too_short() {
        assert_eq!(parse_fence_marker("``"), None);
        assert_eq!(parse_fence_marker("~~"), None);
    }

    #[test]
    fn parse_fence_marker_not_fence() {
        assert_eq!(parse_fence_marker("hello"), None);
        assert_eq!(parse_fence_marker(""), None);
    }

    #[test]
    fn is_markdown_fence_info_basic() {
        assert!(is_markdown_fence_info("```md", /*marker_len*/ 3));
        assert!(is_markdown_fence_info("```markdown", /*marker_len*/ 3));
        assert!(is_markdown_fence_info("```MD", /*marker_len*/ 3));
        assert!(!is_markdown_fence_info("```rust", /*marker_len*/ 3));
        assert!(!is_markdown_fence_info("```", /*marker_len*/ 3));
    }

    #[test]
    fn strip_blockquote_prefix_basic() {
        assert_eq!(strip_blockquote_prefix("> hello"), "hello");
        assert_eq!(strip_blockquote_prefix("> > nested"), "nested");
        assert_eq!(strip_blockquote_prefix("no prefix"), "no prefix");
    }
}
