use ratatui::text::Line;
use std::path::Path;
use std::path::PathBuf;

use crate::markdown;

/// Newline-gated accumulator that renders markdown and commits only fully
/// completed logical lines.
pub(crate) struct MarkdownStreamCollector {
    buffer: String,
    width: Option<usize>,
    cwd: PathBuf,
}

impl MarkdownStreamCollector {
    /// Create a collector that renders markdown using `cwd` for local file-link display.
    ///
    /// The collector snapshots `cwd` into owned state because stream commits can happen long after
    /// construction. The same `cwd` should be reused for the entire stream lifecycle; mixing
    /// different working directories within one stream would make the same link render with
    /// different path prefixes across incremental commits.
    pub fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            buffer: String::new(),
            width,
            cwd: cwd.to_path_buf(),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push_delta(&mut self, delta: &str) {
        tracing::trace!("push_delta: {delta:?}");
        self.buffer.push_str(delta);
    }

    /// Render the full buffer and return only the newly completed logical lines
    /// since the last commit. When the buffer does not end with a newline, the
    /// final rendered line is considered incomplete and is not emitted.
    pub fn commit_complete_lines(&mut self) -> Vec<Line<'static>> {
        let source = self.buffer.clone();
        let last_newline_idx = source.rfind('\n');
        let source = if let Some(last_newline_idx) = last_newline_idx {
            source[..=last_newline_idx].to_string()
        } else {
            return Vec::new();
        };
        self.render_source(&source)
    }

    /// Finalize the stream: emit all remaining lines beyond the last commit.
    /// If the buffer does not end with a newline, a temporary one is appended
    /// for rendering. Optionally unwraps ```markdown language fences in
    /// non-test builds.
    pub fn finalize_and_drain(&mut self) -> Vec<Line<'static>> {
        let raw_buffer = self.buffer.clone();
        let mut source: String = raw_buffer.clone();
        if !source.ends_with('\n') {
            source.push('\n');
        }
        tracing::debug!(
            raw_len = raw_buffer.len(),
            source_len = source.len(),
            "markdown finalize (raw length: {}, rendered length: {})",
            raw_buffer.len(),
            source.len()
        );
        tracing::trace!("markdown finalize (raw source):\n---\n{source}\n---");

        let out = self.render_source(&source);

        // Reset collector state for next stream.
        self.clear();
        out
    }

    fn render_source(&self, source: &str) -> Vec<Line<'static>> {
        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(source, self.width, Some(self.cwd.as_path()), &mut rendered);
        if rendered
            .last()
            .is_some_and(crate::render::line_utils::is_blank_line_spaces_only)
        {
            rendered.pop();
        }
        rendered
    }
}

#[cfg(test)]
fn test_cwd() -> PathBuf {
    // These tests only need a stable absolute cwd; using temp_dir() avoids baking Unix- or
    // Windows-specific root semantics into the fixtures.
    std::env::temp_dir()
}

#[cfg(test)]
pub(crate) fn simulate_stream_markdown_for_tests(
    deltas: &[&str],
    finalize: bool,
) -> Vec<Line<'static>> {
    let mut collector = MarkdownStreamCollector::new(None, &test_cwd());
    let mut out = Vec::new();
    for d in deltas {
        collector.push_delta(d);
        if d.contains('\n') {
            let commit = collector.commit_complete_lines();
            if !commit.is_empty() {
                out = commit;
            }
        }
    }
    if finalize {
        let final_render = collector.finalize_and_drain();
        if !final_render.is_empty() {
            out = final_render;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use std::fmt;

    #[tokio::test]
    async fn no_commit_until_newline() {
        let mut c = super::MarkdownStreamCollector::new(None, &super::test_cwd());
        c.push_delta("Hello, world");
        let out = c.commit_complete_lines();
        assert!(out.is_empty(), "should not commit without newline");
        c.push_delta("!\n");
        let out2 = c.commit_complete_lines();
        assert_eq!(out2.len(), 1, "one completed line after newline");
    }

    #[tokio::test]
    async fn finalize_commits_partial_line() {
        let mut c = super::MarkdownStreamCollector::new(None, &super::test_cwd());
        c.push_delta("Line without newline");
        let out = c.finalize_and_drain();
        assert_eq!(out.len(), 1);
    }

    #[tokio::test]
    async fn e2e_stream_blockquote_simple_is_green() {
        let out = super::simulate_stream_markdown_for_tests(&["> Hello\n"], true);
        assert_eq!(out.len(), 1);
        let l = &out[0];
        assert_eq!(
            l.style.fg,
            Some(Color::Green),
            "expected blockquote line fg green, got {:?}",
            l.style.fg
        );
    }

    #[tokio::test]
    async fn e2e_stream_blockquote_nested_is_green() {
        let out = super::simulate_stream_markdown_for_tests(&["> Level 1\n>> Level 2\n"], true);
        // Filter out any blank lines that may be inserted at paragraph starts.
        let non_blank: Vec<_> = out
            .into_iter()
            .filter(|l| {
                let s = l
                    .spans
                    .iter()
                    .map(|sp| sp.content.clone())
                    .collect::<Vec<_>>()
                    .join("");
                let t = s.trim();
                // Ignore quote-only blank lines like ">" inserted at paragraph boundaries.
                !(t.is_empty() || t == ">")
            })
            .collect();
        assert_eq!(non_blank.len(), 2);
        assert_eq!(non_blank[0].style.fg, Some(Color::Green));
        assert_eq!(non_blank[1].style.fg, Some(Color::Green));
    }

    #[tokio::test]
    async fn e2e_stream_blockquote_with_list_items_is_green() {
        let out = super::simulate_stream_markdown_for_tests(&["> - item 1\n> - item 2\n"], true);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].style.fg, Some(Color::Green));
        assert_eq!(out[1].style.fg, Some(Color::Green));
    }

    #[tokio::test]
    async fn e2e_stream_nested_mixed_lists_ordered_marker_is_light_blue() {
        let md = [
            "1. First\n",
            "   - Second level\n",
            "     1. Third level (ordered)\n",
            "        - Fourth level (bullet)\n",
            "          - Fifth level to test indent consistency\n",
        ];
        let out = super::simulate_stream_markdown_for_tests(&md, true);
        // Find the line that contains the third-level ordered text
        let find_idx = out.iter().position(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
                .contains("Third level (ordered)")
        });
        let idx = find_idx.expect("expected third-level ordered line");
        let line = &out[idx];
        // Expect at least one span on this line to be styled light blue
        let has_light_blue = line
            .spans
            .iter()
            .any(|s| s.style.fg == Some(ratatui::style::Color::LightBlue));
        assert!(
            has_light_blue,
            "expected an ordered-list marker span with light blue fg on: {line:?}"
        );
    }

    #[tokio::test]
    async fn e2e_stream_blockquote_wrap_preserves_green_style() {
        let long = "> This is a very long quoted line that should wrap across multiple columns to verify style preservation.";
        let out = super::simulate_stream_markdown_for_tests(&[long, "\n"], true);
        // Wrap to a narrow width to force multiple output lines.
        let wrapped =
            crate::wrapping::word_wrap_lines(out.iter(), crate::wrapping::RtOptions::new(24));
        // Filter out purely blank lines
        let non_blank: Vec<_> = wrapped
            .into_iter()
            .filter(|l| {
                let s = l
                    .spans
                    .iter()
                    .map(|sp| sp.content.clone())
                    .collect::<Vec<_>>()
                    .join("");
                !s.trim().is_empty()
            })
            .collect();
        assert!(
            non_blank.len() >= 2,
            "expected wrapped blockquote to span multiple lines"
        );
        for (i, l) in non_blank.iter().enumerate() {
            assert_eq!(
                l.spans[0].style.fg,
                Some(Color::Green),
                "wrapped line {} should preserve green style, got {:?}",
                i,
                l.spans[0].style.fg
            );
        }
    }

    #[tokio::test]
    async fn heading_starts_on_new_line_when_following_paragraph() {
        // Stream a paragraph line, then a heading on the next line.
        // The collector emits the latest committed snapshot, so the second
        // commit includes both the paragraph and the heading block.
        let mut c = super::MarkdownStreamCollector::new(None, &super::test_cwd());
        c.push_delta("Hello.\n");
        let out1 = c.commit_complete_lines();
        let s1: Vec<String> = out1
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        assert_eq!(
            out1.len(),
            1,
            "first commit should contain only the paragraph line, got {}: {:?}",
            out1.len(),
            s1
        );

        c.push_delta("## Heading\n");
        let out2 = c.commit_complete_lines();
        let s2: Vec<String> = out2
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        assert_eq!(
            s2,
            vec!["Hello.", "", "## Heading"],
            "expected the full committed snapshot with paragraph and heading"
        );

        let line_to_string = |l: &ratatui::text::Line<'_>| -> String {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<Vec<_>>()
                .join("")
        };

        assert_eq!(line_to_string(&out1[0]), "Hello.");
        assert_eq!(line_to_string(&out2[2]), "## Heading");
    }

    #[tokio::test]
    async fn heading_not_inlined_when_split_across_chunks() {
        // Paragraph without trailing newline, then a chunk that starts with the newline
        // and the heading text, then a final newline. The collector should
        // first commit only the paragraph line, then later re-emit the full
        // committed snapshot once the heading line is complete.
        let mut c = super::MarkdownStreamCollector::new(None, &super::test_cwd());
        c.push_delta("Sounds good!");
        // No commit yet
        assert!(c.commit_complete_lines().is_empty());

        // Introduce the newline that completes the paragraph and the start of the heading.
        c.push_delta("\n## Adding Bird subcommand");
        let out1 = c.commit_complete_lines();
        let s1: Vec<String> = out1
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        assert_eq!(
            s1,
            vec!["Sounds good!"],
            "expected paragraph followed by blank separator before heading chunk"
        );

        // Now finish the heading line with the trailing newline.
        c.push_delta("\n");
        let out2 = c.commit_complete_lines();
        let s2: Vec<String> = out2
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        assert_eq!(
            s2,
            vec!["Sounds good!", "", "## Adding Bird subcommand"],
            "expected the full committed snapshot on the final commit"
        );

        // Sanity check raw markdown rendering for a simple line does not produce spurious extras.
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        let test_cwd = super::test_cwd();
        crate::markdown::append_markdown("Hello.\n", None, Some(test_cwd.as_path()), &mut rendered);
        let rendered_strings: Vec<String> = rendered
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        assert_eq!(
            rendered_strings,
            vec!["Hello."],
            "unexpected markdown lines: {rendered_strings:?}"
        );
    }

    fn lines_to_plain_strings(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect()
    }

    #[derive(Debug)]
    struct StreamTrace {
        width: Option<usize>,
        deltas: Vec<String>,
        commits: Vec<Vec<String>>,
        finalize: Vec<String>,
        combined: Vec<String>,
        full_render: Vec<String>,
    }

    impl fmt::Display for StreamTrace {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            writeln!(f, "width: {:?}", self.width)?;
            writeln!(f, "deltas:")?;
            for (idx, delta) in self.deltas.iter().enumerate() {
                writeln!(f, "  [{idx}] {delta:?}")?;
            }
            writeln!(f, "commits:")?;
            for (idx, commit) in self.commits.iter().enumerate() {
                writeln!(f, "  [{idx}] {commit:?}")?;
            }
            writeln!(f, "finalize: {:?}", self.finalize)?;
            writeln!(f, "combined: {:?}", self.combined)?;
            writeln!(f, "full_render: {:?}", self.full_render)
        }
    }

    fn collect_stream_trace(deltas: &[&str], width: Option<usize>) -> StreamTrace {
        let mut collector = super::MarkdownStreamCollector::new(width, &super::test_cwd());
        let mut commits = Vec::new();
        let mut combined = Vec::new();

        for delta in deltas {
            collector.push_delta(delta);
            if delta.contains('\n') {
                let commit = collector.commit_complete_lines();
                if !commit.is_empty() {
                    let plain = lines_to_plain_strings(&commit);
                    combined = plain.clone();
                    commits.push(plain);
                }
            }
        }

        let finalize_lines = lines_to_plain_strings(&collector.finalize_and_drain());
        if !finalize_lines.is_empty() {
            combined = finalize_lines.clone();
        }

        let full_source: String = deltas.iter().copied().collect();
        let mut full_render = Vec::new();
        markdown::append_markdown(
            &if full_source.ends_with('\n') {
                full_source
            } else {
                format!("{full_source}\n")
            },
            width,
            Some(super::test_cwd().as_path()),
            &mut full_render,
        );
        if full_render
            .last()
            .is_some_and(crate::render::line_utils::is_blank_line_spaces_only)
        {
            full_render.pop();
        }

        StreamTrace {
            width,
            deltas: deltas.iter().map(|delta| (*delta).to_string()).collect(),
            commits,
            finalize: finalize_lines,
            combined,
            full_render: lines_to_plain_strings(&full_render),
        }
    }

    fn assert_stream_matches_full(label: &str, deltas: &[&str], width: Option<usize>) {
        let trace = collect_stream_trace(deltas, width);
        assert_eq!(
            trace.combined, trace.full_render,
            "{label} stream diverged from full render\n{trace}"
        );
    }

    #[tokio::test]
    async fn inline_code_completion_rewrites_prior_line_matches_full() {
        assert_stream_matches_full(
            "inline code completion",
            &["prefix `unfinished", " code`\nnext line\n"],
            Some(48),
        );
    }

    #[tokio::test]
    async fn list_item_continuation_rewrites_prior_block_matches_full() {
        assert_stream_matches_full(
            "list item continuation",
            &["- first item", "\n  continuation\n- second item\n"],
            Some(48),
        );
    }
}
