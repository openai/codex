//! Markdown-to-ratatui rendering entry points.
//!
//! This module provides the public API surface that the rest of the TUI uses
//! to turn markdown source into `Vec<Line<'static>>`.  Two variants exist:
//!
//! - [`append_markdown`] -- general-purpose, used for plan blocks and history
//!   cells that already hold pre-processed markdown (no fence unwrapping).
//! - [`append_markdown_agent`] -- for agent responses.  Runs
//!   [`unwrap_markdown_fences`] first so that `` ```md ``/`` ```markdown ``
//!   fences containing tables are stripped and `pulldown-cmark` sees raw
//!   table syntax instead of fenced code.
//!
//! ## Why fence unwrapping exists
//!
//! LLM agents frequently wrap tables in `` ```markdown `` fences, treating
//! them as code.  Without unwrapping, `pulldown-cmark` parses those lines
//! as a fenced code block and renders them as monospace code rather than a
//! structured table.  The unwrapper is intentionally conservative: it
//! buffers the entire fence body before deciding, only unwraps fences whose
//! info string is `md` or `markdown` AND whose body contains a
//! header+delimiter pair, and degrades gracefully on unclosed fences.
use ratatui::text::Line;
use std::ops::Range;

use crate::table_detect;

/// Render markdown source to styled ratatui lines and append them to `lines`.
///
/// This is the general-purpose entry point used for plan blocks and history cells that already
/// hold pre-processed markdown (no fence unwrapping).
pub(crate) fn append_markdown(
    markdown_source: &str,
    width: Option<usize>,
    lines: &mut Vec<Line<'static>>,
) {
    let rendered = crate::markdown_render::render_markdown_text_with_width(markdown_source, width);
    crate::render::line_utils::push_owned_lines(&rendered.lines, lines);
}

/// Render an agent message to styled ratatui lines.
///
/// Before rendering, the source is passed through [`unwrap_markdown_fences`] so that tables
/// wrapped in `` ```md `` fences are rendered as native tables rather than code blocks.
/// Non-markdown fences (e.g. `rust`, `sh`) are left
/// intact.
pub(crate) fn append_markdown_agent(
    markdown_source: &str,
    width: Option<usize>,
    lines: &mut Vec<Line<'static>>,
) {
    let normalized = unwrap_markdown_fences(markdown_source);
    let rendered = crate::markdown_render::render_markdown_text_with_width(&normalized, width);
    crate::render::line_utils::push_owned_lines(&rendered.lines, lines);
}

/// Strip `` ```md ``/`` ```markdown `` fences that contain tables, emitting their content as bare
/// markdown so `pulldown-cmark` parses the tables natively.
///
/// Fences whose info string is not `md` or `markdown` are passed through unchanged.  Markdown
/// fences that do *not* contain a table (detected by checking for a header row + delimiter row)
/// are also passed through so that non-table markdown inside a fence still renders as a code
/// block.
///
/// The fence unwrapping is intentionally conservative: it buffers the entire fence body before
/// deciding, and an unclosed fence at end-of-input is re-emitted with its opening line so partial
/// streams degrade to code display.
fn unwrap_markdown_fences(markdown_source: &str) -> String {
    if !markdown_source.contains("```") && !markdown_source.contains("~~~") {
        return markdown_source.to_string();
    }

    #[derive(Clone, Copy)]
    struct Fence {
        marker: u8,
        len: usize,
        is_blockquoted: bool,
    }

    // Strip a trailing newline and up to 3 leading spaces, returning the
    // trimmed slice.  Returns `None` when the line has 4+ leading spaces
    // (which makes it an indented code line per CommonMark).
    fn strip_line_indent(line: &str) -> Option<&str> {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let leading_ws = without_newline
            .as_bytes()
            .iter()
            .take_while(|byte| **byte == b' ')
            .count();
        if leading_ws > 3 {
            return None;
        }
        Some(&without_newline[leading_ws..])
    }

    // Parse an opening fence line, returning the fence metadata and whether
    // the fence info string indicates markdown content.
    fn parse_open_fence(line: &str) -> Option<(Fence, bool)> {
        let trimmed = strip_line_indent(line)?;
        let is_blockquoted = trimmed.trim_start().starts_with('>');
        let fence_scan_text = table_detect::strip_blockquote_prefix(trimmed);
        let (marker, len) = table_detect::parse_fence_marker(fence_scan_text)?;
        let is_markdown = table_detect::is_markdown_fence_info(fence_scan_text, len);
        Some((
            Fence {
                marker: marker as u8,
                len,
                is_blockquoted,
            },
            is_markdown,
        ))
    }

    fn is_close_fence(line: &str, fence: Fence) -> bool {
        let Some(trimmed) = strip_line_indent(line) else {
            return false;
        };
        let fence_scan_text = if fence.is_blockquoted {
            if !trimmed.trim_start().starts_with('>') {
                return false;
            }
            table_detect::strip_blockquote_prefix(trimmed)
        } else {
            trimmed
        };
        if let Some((marker, len)) = table_detect::parse_fence_marker(fence_scan_text) {
            marker as u8 == fence.marker
                && len >= fence.len
                && fence_scan_text[len..].trim().is_empty()
        } else {
            false
        }
    }

    fn markdown_fence_contains_table(content: &str, is_blockquoted_fence: bool) -> bool {
        let mut previous_non_empty: Option<&str> = None;
        for line in content.lines() {
            let text = if is_blockquoted_fence {
                table_detect::strip_blockquote_prefix(line)
            } else {
                line
            };
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(previous) = previous_non_empty
                && table_detect::is_table_header_line(previous)
                && !table_detect::is_table_delimiter_line(previous)
                && table_detect::is_table_delimiter_line(trimmed)
            {
                return true;
            }

            previous_non_empty = Some(trimmed);
        }
        false
    }

    fn content_from_ranges(source: &str, ranges: &[Range<usize>]) -> String {
        let mut content = String::new();
        for range in ranges {
            content.push_str(&source[range.clone()]);
        }
        content
    }

    enum ActiveFence {
        Passthrough(Fence),
        MarkdownCandidate {
            fence: Fence,
            opening_range: Range<usize>,
            content_ranges: Vec<Range<usize>>,
        },
    }

    let mut out = String::with_capacity(markdown_source.len());
    let mut active_fence: Option<ActiveFence> = None;
    let mut source_offset = 0usize;

    let mut push_source_range = |range: Range<usize>| {
        if !range.is_empty() {
            out.push_str(&markdown_source[range]);
        }
    };

    for line in markdown_source.split_inclusive('\n') {
        let line_start = source_offset;
        source_offset += line.len();
        let line_range = line_start..source_offset;

        if let Some(active) = active_fence.take() {
            match active {
                ActiveFence::Passthrough(fence) => {
                    push_source_range(line_range);
                    if !is_close_fence(line, fence) {
                        active_fence = Some(ActiveFence::Passthrough(fence));
                    }
                }
                ActiveFence::MarkdownCandidate {
                    fence,
                    opening_range,
                    mut content_ranges,
                } => {
                    if is_close_fence(line, fence) {
                        if markdown_fence_contains_table(&content_from_ranges(
                            markdown_source,
                            &content_ranges,
                        ), fence.is_blockquoted)
                        {
                            for range in content_ranges {
                                push_source_range(range);
                            }
                        } else {
                            push_source_range(opening_range);
                            for range in content_ranges {
                                push_source_range(range);
                            }
                            push_source_range(line_range);
                        }
                    } else {
                        content_ranges.push(line_range);
                        active_fence = Some(ActiveFence::MarkdownCandidate {
                            fence,
                            opening_range,
                            content_ranges,
                        });
                    }
                }
            }
            continue;
        }

        if let Some((fence, is_markdown)) = parse_open_fence(line) {
            if is_markdown {
                active_fence = Some(ActiveFence::MarkdownCandidate {
                    fence,
                    opening_range: line_range,
                    content_ranges: Vec::new(),
                });
            } else {
                push_source_range(line_range);
                active_fence = Some(ActiveFence::Passthrough(fence));
            }
            continue;
        }

        push_source_range(line_range);
    }

    if let Some(active) = active_fence {
        match active {
            ActiveFence::Passthrough(_) => {}
            ActiveFence::MarkdownCandidate {
                opening_range,
                content_ranges,
                ..
            } => {
                push_source_range(opening_range);
                for range in content_ranges {
                    push_source_range(range);
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::text::Line;

    fn lines_to_strings(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn citations_render_as_plain_text() {
        let src = "Before 【F:/x.rs†L1】\nAfter 【F:/x.rs†L3】\n";
        let mut out = Vec::new();
        append_markdown(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert_eq!(
            rendered,
            vec![
                "Before 【F:/x.rs†L1】".to_string(),
                "After 【F:/x.rs†L3】".to_string()
            ]
        );
    }

    #[test]
    fn indented_code_blocks_preserve_leading_whitespace() {
        // Basic sanity: indented code with surrounding blank lines should produce the indented line.
        let src = "Before\n\n    code 1\n\nAfter\n";
        let mut out = Vec::new();
        append_markdown(src, None, &mut out);
        let lines = lines_to_strings(&out);
        assert_eq!(lines, vec!["Before", "", "    code 1", "", "After"]);
    }

    #[test]
    fn append_markdown_preserves_full_text_line() {
        let src = "Hi! How can I help with codex-rs today? Want me to explore the repo, run tests, or work on a specific change?\n";
        let mut out = Vec::new();
        append_markdown(src, None, &mut out);
        assert_eq!(
            out.len(),
            1,
            "expected a single rendered line for plain text"
        );
        let rendered: String = out
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.clone())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            rendered,
            "Hi! How can I help with codex-rs today? Want me to explore the repo, run tests, or work on a specific change?"
        );
    }

    #[test]
    fn append_markdown_matches_tui_markdown_for_ordered_item() {
        let mut out = Vec::new();
        append_markdown("1. Tight item\n", None, &mut out);
        let lines = lines_to_strings(&out);
        assert_eq!(lines, vec!["1. Tight item".to_string()]);
    }

    #[test]
    fn append_markdown_keeps_ordered_list_line_unsplit_in_context() {
        let src = "Loose vs. tight list items:\n1. Tight item\n";
        let mut out = Vec::new();
        append_markdown(src, None, &mut out);

        let lines = lines_to_strings(&out);

        // Expect to find the ordered list line rendered as a single line,
        // not split into a marker-only line followed by the text.
        assert!(
            lines.iter().any(|s| s == "1. Tight item"),
            "expected '1. Tight item' rendered as a single line; got: {lines:?}"
        );
        assert!(
            !lines
                .windows(2)
                .any(|w| w[0].trim_end() == "1." && w[1] == "Tight item"),
            "did not expect a split into ['1.', 'Tight item']; got: {lines:?}"
        );
    }

    #[test]
    fn append_markdown_agent_unwraps_markdown_fences_for_table_rendering() {
        let src = "```markdown\n| A | B |\n|---|---|\n| 1 | 2 |\n```\n";
        let mut out = Vec::new();
        append_markdown_agent(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert!(rendered.iter().any(|line| line.contains("┌")));
        assert!(rendered.iter().any(|line| line.contains("│ 1   │ 2   │")));
    }

    #[test]
    fn append_markdown_agent_unwraps_markdown_fences_for_no_outer_table_rendering() {
        let src = "```md\nCol A | Col B | Col C\n--- | --- | ---\nx | y | z\n10 | 20 | 30\n```\n";
        let mut out = Vec::new();
        append_markdown_agent(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert!(rendered.iter().any(|line| line.contains("┌")));
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("│ Col A │ Col B │ Col C │"))
        );
        assert!(
            !rendered
                .iter()
                .any(|line| line.trim() == "Col A | Col B | Col C")
        );
    }

    #[test]
    fn append_markdown_agent_unwraps_markdown_fences_for_two_column_no_outer_table() {
        let src = "```md\nA | B\n--- | ---\nleft | right\n```\n";
        let mut out = Vec::new();
        append_markdown_agent(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert!(rendered.iter().any(|line| line.contains("┌")));
        assert!(rendered.iter().any(|line| line.contains("│ A")));
        assert!(!rendered.iter().any(|line| line.trim() == "A | B"));
    }

    #[test]
    fn append_markdown_agent_unwraps_markdown_fences_for_single_column_table() {
        let src = "```md\n| Only |\n|---|\n| value |\n```\n";
        let mut out = Vec::new();
        append_markdown_agent(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert!(rendered.iter().any(|line| line.contains("┌")));
        assert!(!rendered.iter().any(|line| line.trim() == "| Only |"));
    }

    #[test]
    fn append_markdown_agent_keeps_non_markdown_fences_as_code() {
        let src = "```rust\n| A | B |\n|---|---|\n| 1 | 2 |\n```\n";
        let mut out = Vec::new();
        append_markdown_agent(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert_eq!(
            rendered,
            vec![
                "| A | B |".to_string(),
                "|---|---|".to_string(),
                "| 1 | 2 |".to_string(),
            ]
        );
    }

    #[test]
    fn append_markdown_agent_unwraps_blockquoted_markdown_fence_table() {
        let src = "> ```markdown\n> | A | B |\n> |---|---|\n> | 1 | 2 |\n> ```\n";
        let rendered = unwrap_markdown_fences(src);
        assert!(
            !rendered.contains("```"),
            "expected markdown fence markers to be removed: {rendered:?}"
        );
    }

    #[test]
    fn append_markdown_agent_keeps_non_blockquoted_markdown_fence_with_blockquote_table_example() {
        let src = "```markdown\n> | A | B |\n> |---|---|\n> | 1 | 2 |\n```\n";
        let normalized = unwrap_markdown_fences(src);
        assert_eq!(normalized, src);
    }

    #[test]
    fn append_markdown_agent_keeps_markdown_fence_when_content_is_not_table() {
        let src = "```markdown\n**bold**\n```\n";
        let mut out = Vec::new();
        append_markdown_agent(src, None, &mut out);
        let rendered = lines_to_strings(&out);
        assert_eq!(rendered, vec!["**bold**".to_string()]);
    }

    #[test]
    fn unwrap_markdown_fences_repro_keeps_fence_without_header_delimiter_pair() {
        let src = "```markdown\n| A | B |\nnot a delimiter row\n| --- | --- |\n# Heading\n```\n";
        let normalized = unwrap_markdown_fences(src);
        assert_eq!(normalized, src);
    }

    #[test]
    fn append_markdown_agent_unwraps_markdown_fence_with_blank_line_between_header_and_delimiter() {
        let src = "```markdown\n| A | B |\n\n|---|---|\n| 1 | 2 |\n```\n";
        let rendered = unwrap_markdown_fences(src);
        assert!(
            !rendered.contains("```"),
            "expected markdown fence markers to be removed with spacing: {rendered:?}"
        );
    }
}
