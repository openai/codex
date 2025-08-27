use std::collections::VecDeque;

use codex_core::config::Config;
use ratatui::text::Line;

use crate::markdown;
use crate::render::markdown_utils::is_inside_unclosed_fence;
use crate::render::markdown_utils::strip_empty_fenced_code_blocks;

/// Newline-gated accumulator that renders markdown and commits only fully
/// completed logical lines.
pub(crate) struct MarkdownStreamCollector {
    buffer: String,
    committed_line_count: usize,
}

impl MarkdownStreamCollector {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            committed_line_count: 0,
        }
    }

    /// Returns the number of logical lines that have already been committed
    /// (i.e., previously returned from `commit_complete_lines`).
    pub fn committed_count(&self) -> usize {
        self.committed_line_count
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.committed_line_count = 0;
    }

    /// Replace the buffered content and mark that the first `committed_count`
    /// logical lines are already committed.
    pub fn replace_with_and_mark_committed(&mut self, s: &str, committed_count: usize) {
        self.buffer.clear();
        self.buffer.push_str(s);
        self.committed_line_count = committed_count;
    }

    pub fn push_delta(&mut self, delta: &str) {
        self.buffer.push_str(delta);
    }

    /// Render the full buffer and return only the newly completed logical lines
    /// since the last commit. When the buffer does not end with a newline, the
    /// final rendered line is considered incomplete and is not emitted.
    pub fn commit_complete_lines(&mut self, config: &Config) -> Vec<Line<'static>> {
        // In non-test builds, unwrap an outer ```markdown fence during commit as well,
        // so fence markers never appear in streamed history.
        let source = unwrap_markdown_language_fence_if_enabled(self.buffer.clone());
        let source = strip_empty_fenced_code_blocks(&source);

        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(&source, &mut rendered, config);

        let mut complete_line_count = rendered.len();
        if complete_line_count > 0
            && crate::render::line_utils::is_blank_line_spaces_only(
                &rendered[complete_line_count - 1],
            )
        {
            complete_line_count -= 1;
        }
        // Heuristic: if the buffer ends with a double newline and the last non-blank
        // rendered line looks like a list bullet with inline content (e.g., "- item"),
        // defer committing that line. Subsequent context (e.g., another list item)
        // can cause the renderer to split the bullet marker and text into separate
        // logical lines ("- " then "item"), which would otherwise duplicate content.
        if self.buffer.ends_with("\n\n") && complete_line_count > 0 {
            let last = &rendered[complete_line_count - 1];
            let mut text = String::new();
            for s in &last.spans {
                text.push_str(&s.content);
            }
            if text.starts_with("- ") && text.trim() != "-" {
                complete_line_count = complete_line_count.saturating_sub(1);
            }
        }
        if !self.buffer.ends_with('\n') {
            complete_line_count = complete_line_count.saturating_sub(1);
            // If we're inside an unclosed fenced code block, also drop the
            // last rendered line to avoid committing a partial code line.
            if is_inside_unclosed_fence(&source) {
                complete_line_count = complete_line_count.saturating_sub(1);
            }
            // If the next (incomplete) line appears to begin a list item,
            // also defer the previous completed line because the renderer may
            // retroactively treat it as part of the list (e.g., ordered list item 1).
            if let Some(last_nl) = source.rfind('\n') {
                let tail = &source[last_nl + 1..];
                if starts_with_list_marker(tail) {
                    complete_line_count = complete_line_count.saturating_sub(1);
                }
            }
        }

        // Conservatively withhold trailing list-like lines (unordered or ordered)
        // because streaming mid-item can cause the renderer to later split or
        // restructure them (e.g., duplicating content or separating the marker).
        // Only defers lines at the end of the out slice so previously committed
        // lines remain stable.
        if complete_line_count > self.committed_line_count {
            let mut safe_count = complete_line_count;
            while safe_count > self.committed_line_count {
                let l = &rendered[safe_count - 1];
                let mut text = String::new();
                for s in &l.spans {
                    text.push_str(&s.content);
                }
                let listish = is_potentially_volatile_list_line(&text);
                if listish {
                    safe_count -= 1;
                    continue;
                }
                break;
            }
            complete_line_count = safe_count;
        }

        if self.committed_line_count >= complete_line_count {
            return Vec::new();
        }

        let out_slice = &rendered[self.committed_line_count..complete_line_count];
        // Strong correctness: while a fenced code block is open (no closing fence yet),
        // do not emit any new lines from inside it. Wait until the fence closes to emit
        // the entire block together. This avoids stray backticks and misformatted content.
        if is_inside_unclosed_fence(&source) {
            return Vec::new();
        }

        // Additional conservative hold-back: if exactly one short, plain word
        // line would be emitted, defer it. This avoids committing a lone word
        // that might become the first ordered-list item once the next delta
        // arrives (e.g., next line starts with "2 " or "2. ").
        if out_slice.len() == 1 {
            let mut s = String::new();
            for sp in &out_slice[0].spans {
                s.push_str(&sp.content);
            }
            if is_short_plain_word(&s) {
                return Vec::new();
            }
        }

        let out = out_slice.to_vec();
        self.committed_line_count = complete_line_count;
        out
    }

    /// Finalize the stream: emit all remaining lines beyond the last commit.
    /// If the buffer does not end with a newline, a temporary one is appended
    /// for rendering. Optionally unwraps ```markdown language fences in
    /// non-test builds.
    pub fn finalize_and_drain(&mut self, config: &Config) -> Vec<Line<'static>> {
        let mut source: String = self.buffer.clone();
        if !source.ends_with('\n') {
            source.push('\n');
        }
        let source = unwrap_markdown_language_fence_if_enabled(source);
        let source = strip_empty_fenced_code_blocks(&source);

        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(&source, &mut rendered, config);

        let out = if self.committed_line_count >= rendered.len() {
            Vec::new()
        } else {
            rendered[self.committed_line_count..].to_vec()
        };

        // Reset collector state for next stream.
        self.clear();
        out
    }
}

#[inline]
fn is_potentially_volatile_list_line(text: &str) -> bool {
    let t = text.trim_end();
    if t == "-" || t == "*" || t == "- " || t == "* " {
        return true;
    }
    if t.starts_with("- ") || t.starts_with("* ") {
        return true;
    }
    // ordered list like "1. " or "23. "
    let mut it = t.chars().peekable();
    let mut saw_digit = false;
    while let Some(&ch) = it.peek() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            it.next();
            continue;
        }
        break;
    }
    if saw_digit && it.peek() == Some(&'.') {
        // consume '.'
        it.next();
        if it.peek() == Some(&' ') {
            return true;
        }
    }
    false
}

#[inline]
fn starts_with_list_marker(text: &str) -> bool {
    let t = text.trim_start();
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("-\t") || t.starts_with("*\t") {
        return true;
    }
    // ordered list marker like "1 ", "1. ", "23 ", "23. "
    let mut it = t.chars().peekable();
    let mut saw_digit = false;
    while let Some(&ch) = it.peek() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            it.next();
        } else {
            break;
        }
    }
    if !saw_digit {
        return false;
    }
    match it.peek() {
        Some('.') => {
            it.next();
            matches!(it.peek(), Some(' '))
        }
        Some(' ') => true,
        _ => false,
    }
}

#[inline]
fn is_short_plain_word(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.len() > 5 {
        return false;
    }
    t.chars().all(|c| c.is_alphanumeric())
}

/// fence helpers are provided by `crate::render::markdown_utils`
#[cfg(test)]
fn unwrap_markdown_language_fence_if_enabled(s: String) -> String {
    // In tests, keep content exactly as provided to simplify assertions.
    s
}

#[cfg(not(test))]
fn unwrap_markdown_language_fence_if_enabled(s: String) -> String {
    // Best-effort unwrap of a single outer fenced markdown block.
    // Recognizes common forms like ```markdown, ```md (any case), optional
    // surrounding whitespace, and flexible trailing newlines/CRLF.
    // If the block is not recognized, return the input unchanged.
    let lines = s.lines().collect::<Vec<_>>();
    if lines.len() < 2 {
        return s;
    }

    // Identify opening fence and language.
    let open = lines.first().map(|l| l.trim_start()).unwrap_or("");
    if !open.starts_with("```") {
        return s;
    }
    let lang = open.trim_start_matches("```").trim();
    let is_markdown_lang = lang.eq_ignore_ascii_case("markdown") || lang.eq_ignore_ascii_case("md");
    if !is_markdown_lang {
        return s;
    }

    // Find the last non-empty line and ensure it is a closing fence.
    let mut last_idx = lines.len() - 1;
    while last_idx > 0 && lines[last_idx].trim().is_empty() {
        last_idx -= 1;
    }
    if lines[last_idx].trim() != "```" {
        return s;
    }

    // Reconstruct the inner content between the fences.
    let mut out = String::new();
    for l in lines.iter().take(last_idx).skip(1) {
        out.push_str(l);
        out.push('\n');
    }
    out
}

pub(crate) struct StepResult {
    pub history: Vec<Line<'static>>, // lines to insert into history this step
}

/// Streams already-rendered rows into history while computing the newest K
/// rows to show in a live overlay.
pub(crate) struct AnimatedLineStreamer {
    queue: VecDeque<Line<'static>>,
}

impl AnimatedLineStreamer {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn enqueue(&mut self, lines: Vec<Line<'static>>) {
        for l in lines {
            self.queue.push_back(l);
        }
    }

    pub fn step(&mut self) -> StepResult {
        let mut history = Vec::new();
        // Move exactly one per tick to animate gradual insertion.
        let burst = if self.queue.is_empty() { 0 } else { 1 };
        for _ in 0..burst {
            if let Some(l) = self.queue.pop_front() {
                history.push(l);
            }
        }

        StepResult { history }
    }

    pub fn drain_all(&mut self) -> StepResult {
        let mut history = Vec::new();
        while let Some(l) = self.queue.pop_front() {
            history.push(l);
        }
        StepResult { history }
    }

    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
pub(crate) fn simulate_stream_markdown_for_tests(
    deltas: &[&str],
    finalize: bool,
    config: &Config,
) -> Vec<Line<'static>> {
    let mut collector = MarkdownStreamCollector::new();
    let mut out = Vec::new();
    for d in deltas {
        collector.push_delta(d);
        if d.contains('\n') {
            out.extend(collector.commit_complete_lines(config));
        }
    }
    if finalize {
        out.extend(collector.finalize_and_drain(config));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;

    fn test_config() -> Config {
        let overrides = ConfigOverrides {
            cwd: std::env::current_dir().ok(),
            ..Default::default()
        };
        match Config::load_with_cli_overrides(vec![], overrides) {
            Ok(c) => c,
            Err(e) => panic!("load test config: {e}"),
        }
    }

    #[test]
    fn no_commit_until_newline() {
        let cfg = test_config();
        let mut c = super::MarkdownStreamCollector::new();
        c.push_delta("Hello, world");
        let out = c.commit_complete_lines(&cfg);
        assert!(out.is_empty(), "should not commit without newline");
        c.push_delta("!\n");
        let out2 = c.commit_complete_lines(&cfg);
        assert_eq!(out2.len(), 1, "one completed line after newline");
    }

    #[test]
    fn finalize_commits_partial_line() {
        let cfg = test_config();
        let mut c = super::MarkdownStreamCollector::new();
        c.push_delta("Line without newline");
        let out = c.finalize_and_drain(&cfg);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn heading_starts_on_new_line_when_following_paragraph() {
        let cfg = test_config();

        // Stream a paragraph line, then a heading on the next line.
        // Expect two distinct rendered lines: "Hello." and "Heading".
        let mut c = super::MarkdownStreamCollector::new();
        c.push_delta("Hello.\n");
        let out1 = c.commit_complete_lines(&cfg);
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
        let out2 = c.commit_complete_lines(&cfg);
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
            vec!["", "## Heading"],
            "expected a blank separator then the heading line"
        );

        let line_to_string = |l: &ratatui::text::Line<'_>| -> String {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<Vec<_>>()
                .join("")
        };

        assert_eq!(line_to_string(&out1[0]), "Hello.");
        assert_eq!(line_to_string(&out2[1]), "## Heading");
    }

    #[test]
    fn heading_not_inlined_when_split_across_chunks() {
        let cfg = test_config();

        // Paragraph without trailing newline, then a chunk that starts with the newline
        // and the heading text, then a final newline. The collector should first commit
        // only the paragraph line, and later commit the heading as its own line.
        let mut c = super::MarkdownStreamCollector::new();
        c.push_delta("Sounds good!");
        // No commit yet
        assert!(c.commit_complete_lines(&cfg).is_empty());

        // Introduce the newline that completes the paragraph and the start of the heading.
        c.push_delta("\n## Adding Bird subcommand");
        let out1 = c.commit_complete_lines(&cfg);
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
            vec!["Sounds good!", ""],
            "expected paragraph followed by blank separator before heading chunk"
        );

        // Now finish the heading line with the trailing newline.
        c.push_delta("\n");
        let out2 = c.commit_complete_lines(&cfg);
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
            vec!["## Adding Bird subcommand"],
            "expected the heading line only on the final commit"
        );

        // Sanity check raw markdown rendering for a simple line does not produce spurious extras.
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown("Hello.\n", &mut rendered, &cfg);
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

        let line_to_string = |l: &ratatui::text::Line<'_>| -> String {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<Vec<_>>()
                .join("")
        };

        assert_eq!(line_to_string(&out1[0]), "Sounds good!");
        assert_eq!(line_to_string(&out1[1]), "");
        assert_eq!(line_to_string(&out2[0]), "## Adding Bird subcommand");
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

    #[test]
    fn lists_and_fences_commit_without_duplication() {
        let cfg = test_config();

        // List case
        let deltas = vec!["- a\n- ", "b\n- c\n"];
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_str = lines_to_plain_strings(&streamed);

        let mut rendered_all: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown("- a\n- b\n- c\n", &mut rendered_all, &cfg);
        let rendered_all_str = lines_to_plain_strings(&rendered_all);

        assert_eq!(
            streamed_str, rendered_all_str,
            "list streaming should equal full render without duplication"
        );

        // Fenced code case: stream in small chunks
        let deltas2 = vec!["```", "\nco", "de 1\ncode 2\n", "```\n"];
        let streamed2 = simulate_stream_markdown_for_tests(&deltas2, true, &cfg);
        let streamed2_str = lines_to_plain_strings(&streamed2);

        let mut rendered_all2: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown("```\ncode 1\ncode 2\n```\n", &mut rendered_all2, &cfg);
        let rendered_all2_str = lines_to_plain_strings(&rendered_all2);

        assert_eq!(
            streamed2_str, rendered_all2_str,
            "fence streaming should equal full render without duplication"
        );
    }

    #[test]
    fn utf8_boundary_safety_and_wide_chars() {
        let cfg = test_config();

        // Emoji (wide), CJK, control char, digit + combining macron sequences
        let input = "🙂🙂🙂\n汉字漢字\nA\u{0003}0\u{0304}\n";
        let deltas = vec![
            "🙂",
            "🙂",
            "🙂\n汉",
            "字漢",
            "字\nA",
            "\u{0003}",
            "0",
            "\u{0304}",
            "\n",
        ];

        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_str = lines_to_plain_strings(&streamed);

        let mut rendered_all: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(input, &mut rendered_all, &cfg);
        let rendered_all_str = lines_to_plain_strings(&rendered_all);

        assert_eq!(
            streamed_str, rendered_all_str,
            "utf8/wide-char streaming should equal full render without duplication or truncation"
        );
    }

    #[test]
    fn empty_fenced_block_is_dropped_and_separator_preserved_before_heading() {
        let cfg = test_config();
        // An empty fenced code block followed by a heading should not render the fence,
        // but should preserve a blank separator line so the heading starts on a new line.
        let deltas = vec!["```bash\n```\n", "## Heading\n"]; // empty block and close in same commit
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let texts = lines_to_plain_strings(&streamed);
        assert!(
            texts.iter().all(|s| !s.contains("```")),
            "no fence markers expected: {texts:?}"
        );
        // Expect the heading and no fence markers. A blank separator may or may not be rendered at start.
        assert!(
            texts.iter().any(|s| s == "## Heading"),
            "expected heading line: {texts:?}"
        );
    }

    #[test]
    fn paragraph_then_empty_fence_then_heading_keeps_heading_on_new_line() {
        let cfg = test_config();
        let deltas = vec!["Para.\n", "```\n```\n", "## Title\n"]; // empty fence block in one commit
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let texts = lines_to_plain_strings(&streamed);
        let para_idx = match texts.iter().position(|s| s == "Para.") {
            Some(i) => i,
            None => panic!("para present"),
        };
        let head_idx = match texts.iter().position(|s| s == "## Title") {
            Some(i) => i,
            None => panic!("heading present"),
        };
        assert!(
            head_idx > para_idx,
            "heading should not merge with paragraph: {texts:?}"
        );
    }

    #[test]
    fn loose_list_with_split_dashes_matches_full_render() {
        let cfg = test_config();
        // Minimized failing sequence discovered by the helper: two chunks
        // that still reproduce the mismatch.
        let deltas = vec!["- item.\n\n", "-"];

        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_strs = lines_to_plain_strings(&streamed);

        let full: String = deltas.iter().copied().collect();
        let mut rendered_all: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&full, &mut rendered_all, &cfg);
        let rendered_all_strs = lines_to_plain_strings(&rendered_all);

        assert_eq!(
            streamed_strs, rendered_all_strs,
            "streamed output should match full render without dangling '-' lines"
        );
    }

    #[test]
    fn loose_vs_tight_list_items_streaming_matches_full() {
        let cfg = test_config();
        // Deltas extracted from the session log around 2025-08-27T00:33:18.216Z
        let deltas = vec![
            "\n\n",
            "Loose",
            " vs",
            ".",
            " tight",
            " list",
            " items",
            ":\n",
            "1",
            ".",
            " Tight",
            " item",
            "\n",
            "2",
            ".",
            " Another",
            " tight",
            " item",
            "\n\n",
            "1",
            ".",
            " Loose",
            " item",
            " with",
            " its",
            " own",
            " paragraph",
            ".\n\n",
            "  ",
            " This",
            " paragraph",
            " belongs",
            " to",
            " the",
            " same",
            " list",
            " item",
            ".\n\n",
            "2",
            ".",
            " Second",
            " loose",
            " item",
            " with",
            " a",
            " nested",
            " list",
            " after",
            " a",
            " blank",
            " line",
            ".\n\n",
            "  ",
            " -",
            " Nested",
            " bullet",
            " under",
            " a",
            " loose",
            " item",
            "\n",
            "  ",
            " -",
            " Another",
            " nested",
            " bullet",
            "\n\n",
        ];

        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_strs = lines_to_plain_strings(&streamed);

        // Compute a full render for diagnostics only.
        let full: String = deltas.iter().copied().collect();
        let mut rendered_all: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&full, &mut rendered_all, &cfg);

        // Also assert exact expected plain strings for clarity.
        let expected = vec![
            "Loose vs. tight list items:".to_string(),
            "".to_string(),
            "1. ".to_string(),
            "Tight item".to_string(),
            "2. ".to_string(),
            "Another tight item".to_string(),
            "3. ".to_string(),
            "Loose item with its own paragraph.".to_string(),
            "".to_string(),
            "This paragraph belongs to the same list item.".to_string(),
            "4. ".to_string(),
            "Second loose item with a nested list after a blank line.".to_string(),
            "    - Nested bullet under a loose item".to_string(),
            "    - Another nested bullet".to_string(),
        ];
        assert_eq!(
            streamed_strs, expected,
            "expected exact rendered lines for loose/tight section"
        );
    }

    // Targeted tests derived from fuzz findings. Each asserts streamed == full render.

    #[test]
    fn fuzz_class_bare_dash_then_task_item() {
        let cfg = test_config();
        // Case similar to: ["two\n", "- \n* [x] done "]
        let deltas = vec!["two\n", "- \n* [x] done \n"];
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_strs = lines_to_plain_strings(&streamed);
        let full: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&full, &mut rendered, &cfg);
        let rendered_strs = lines_to_plain_strings(&rendered);
        assert_eq!(streamed_strs, rendered_strs);
    }

    #[test]
    fn fuzz_class_bullet_duplication_variant_1() {
        let cfg = test_config();
        // Case similar to: ["aph.\n- let one\n- bull", "et two\n\n  second paragraph "]
        let deltas = vec!["aph.\n- let one\n- bull", "et two\n\n  second paragraph \n"];
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_strs = lines_to_plain_strings(&streamed);
        let full: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&full, &mut rendered, &cfg);
        let rendered_strs = lines_to_plain_strings(&rendered);
        assert_eq!(streamed_strs, rendered_strs);
    }

    #[test]
    fn fuzz_class_bullet_duplication_variant_2() {
        let cfg = test_config();
        // Case similar to: ["- e\n  c", "e\n- bullet two\n\n  second paragraph in bullet two\n"]
        let deltas = vec![
            "- e\n  c",
            "e\n- bullet two\n\n  second paragraph in bullet two\n",
        ];
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_strs = lines_to_plain_strings(&streamed);
        let full: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&full, &mut rendered, &cfg);
        let rendered_strs = lines_to_plain_strings(&rendered);
        assert_eq!(streamed_strs, rendered_strs);
    }

    #[test]
    fn fuzz_class_ordered_list_split_weirdness() {
        let cfg = test_config();
        // Case similar to: ["one\n2", " two\n- \n* [x] d"]
        let deltas = vec!["one\n2", " two\n- \n* [x] d\n"];
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_strs = lines_to_plain_strings(&streamed);
        let full: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&full, &mut rendered, &cfg);
        let rendered_strs = lines_to_plain_strings(&rendered);
        assert_eq!(streamed_strs, rendered_strs);
    }
}
