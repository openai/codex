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

pub(crate) struct AgentRenderedWithSourceMap {
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) line_end_source_bytes: Vec<usize>,
}

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
    let rendered = render_markdown_agent_with_source_map(markdown_source, width);
    lines.extend(rendered.lines);
}

pub(crate) fn render_markdown_agent_with_source_map(
    markdown_source: &str,
    width: Option<usize>,
) -> AgentRenderedWithSourceMap {
    let normalized = unwrap_markdown_fences_with_mapping(markdown_source);
    let rendered = crate::markdown_render::render_markdown_text_with_width_and_source_map(
        &normalized.text,
        width,
    );
    let lines = rendered.text.lines;
    let line_end_source_bytes = rendered
        .line_end_input_bytes
        .into_iter()
        .map(|offset| normalized.raw_source_offset(offset))
        .collect();
    AgentRenderedWithSourceMap {
        lines,
        line_end_source_bytes,
    }
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
#[cfg(test)]
fn unwrap_markdown_fences(markdown_source: &str) -> String {
    unwrap_markdown_fences_with_mapping(markdown_source).text
}

#[derive(Clone, Debug)]
struct NormalizedToRawSegment {
    normalized_start: usize,
    raw_start: usize,
}

struct NormalizedMarkdown {
    text: String,
    raw_source_len: usize,
    segments: Vec<NormalizedToRawSegment>,
}

impl NormalizedMarkdown {
    fn new(raw_source_len: usize, capacity: usize) -> Self {
        Self {
            text: String::with_capacity(capacity),
            raw_source_len,
            segments: Vec::new(),
        }
    }

    fn push_source_range(&mut self, source: &str, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let out_start = self.text.len();
        let out_text = &source[range.clone()];
        self.text.push_str(out_text);
        self.segments.push(NormalizedToRawSegment {
            normalized_start: out_start,
            raw_start: range.start,
        });
    }

    fn raw_source_offset(&self, normalized_offset: usize) -> usize {
        if normalized_offset == 0 {
            return 0;
        }
        if normalized_offset >= self.text.len() {
            return self.raw_source_len;
        }
        let idx = self
            .segments
            .partition_point(|segment| segment.normalized_start < normalized_offset);
        if idx == 0 {
            return 0;
        }
        let segment = &self.segments[idx - 1];
        segment.raw_start + normalized_offset.saturating_sub(segment.normalized_start)
    }
}

fn unwrap_markdown_fences_with_mapping(markdown_source: &str) -> NormalizedMarkdown {
    if !markdown_source.contains("```") && !markdown_source.contains("~~~") {
        let mut out = NormalizedMarkdown::new(markdown_source.len(), markdown_source.len());
        out.push_source_range(markdown_source, 0..markdown_source.len());
        return out;
    }

    #[derive(Clone, Copy)]
    struct Fence {
        marker: char,
        len: usize,
        is_markdown: bool,
    }

    fn parse_open_fence(line: &str) -> Option<Fence> {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let leading_ws = without_newline
            .as_bytes()
            .iter()
            .take_while(|byte| **byte == b' ')
            .count();
        if leading_ws > 3 {
            return None;
        }
        let trimmed = &without_newline[leading_ws..];
        let marker = trimmed.chars().next()?;
        if marker != '`' && marker != '~' {
            return None;
        }
        let len = trimmed.chars().take_while(|ch| *ch == marker).count();
        if len < 3 {
            return None;
        }
        let rest = trimmed[len..].trim();
        let info = rest.split_whitespace().next().unwrap_or_default();
        let is_markdown = info.eq_ignore_ascii_case("md") || info.eq_ignore_ascii_case("markdown");
        Some(Fence {
            marker,
            len,
            is_markdown,
        })
    }

    fn is_close_fence(line: &str, fence: Fence) -> bool {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let leading_ws = without_newline
            .as_bytes()
            .iter()
            .take_while(|byte| **byte == b' ')
            .count();
        if leading_ws > 3 {
            return false;
        }
        let trimmed = &without_newline[leading_ws..];
        if !trimmed.starts_with(fence.marker) {
            return false;
        }
        let len = trimmed.chars().take_while(|ch| *ch == fence.marker).count();
        if len < fence.len {
            return false;
        }
        trimmed[len..].trim().is_empty()
    }

    fn markdown_fence_contains_table(content: &str) -> bool {
        let mut previous_non_empty: Option<&str> = None;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                previous_non_empty = None;
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

    let mut out = NormalizedMarkdown::new(markdown_source.len(), markdown_source.len());
    let mut active_fence: Option<ActiveFence> = None;
    let mut source_offset = 0usize;

    for line in markdown_source.split_inclusive('\n') {
        let line_start = source_offset;
        source_offset += line.len();
        let line_range = line_start..source_offset;

        if let Some(active) = active_fence.take() {
            match active {
                ActiveFence::Passthrough(fence) => {
                    out.push_source_range(markdown_source, line_range);
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
                        )) {
                            for range in content_ranges {
                                out.push_source_range(markdown_source, range);
                            }
                        } else {
                            out.push_source_range(markdown_source, opening_range);
                            for range in content_ranges {
                                out.push_source_range(markdown_source, range);
                            }
                            out.push_source_range(markdown_source, line_range);
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

        if let Some(fence) = parse_open_fence(line) {
            if fence.is_markdown {
                active_fence = Some(ActiveFence::MarkdownCandidate {
                    fence,
                    opening_range: line_range,
                    content_ranges: Vec::new(),
                });
            } else {
                out.push_source_range(markdown_source, line_range);
                active_fence = Some(ActiveFence::Passthrough(fence));
            }
            continue;
        }

        out.push_source_range(markdown_source, line_range);
    }

    if let Some(active) = active_fence {
        match active {
            ActiveFence::Passthrough(_) => {}
            ActiveFence::MarkdownCandidate {
                opening_range,
                content_ranges,
                ..
            } => {
                out.push_source_range(markdown_source, opening_range);
                for range in content_ranges {
                    out.push_source_range(markdown_source, range);
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
    fn render_markdown_agent_with_source_map_offsets_align_with_line_count() {
        let src = "alpha\n\n```md\n| A | B |\n|---|---|\n| 1 | 2 |\n```\nomega\n";
        let rendered = render_markdown_agent_with_source_map(src, Some(80));
        assert_eq!(rendered.lines.len(), rendered.line_end_source_bytes.len());

        let mut prev = 0usize;
        for offset in rendered.line_end_source_bytes {
            assert!(
                offset <= src.len(),
                "line-end offset must stay within source bounds: {offset} > {}",
                src.len()
            );
            assert!(
                offset >= prev,
                "line-end offsets must be non-decreasing: {offset} < {prev}"
            );
            prev = offset;
        }
    }

    #[test]
    fn unwrap_markdown_fences_mapping_preserves_raw_offsets_across_removed_fence_lines() {
        let src = "before\n```md\n| A | B |\n|---|---|\n| 1 | 2 |\n```\nafter\n";
        let normalized = unwrap_markdown_fences_with_mapping(src);
        assert!(
            !normalized.text.contains("```"),
            "markdown table fence should be removed in normalized text"
        );
        let after_in_normalized = normalized
            .text
            .find("after")
            .expect("expected 'after' in normalized text");
        let after_in_raw = src.find("after").expect("expected 'after' in raw source");
        assert_eq!(
            normalized.raw_source_offset(after_in_normalized + 1),
            after_in_raw + 1
        );
        assert_eq!(
            normalized.raw_source_offset(normalized.text.len()),
            src.len()
        );
    }
}
