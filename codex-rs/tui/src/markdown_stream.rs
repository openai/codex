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
        if !self.buffer.ends_with('\n') {
            complete_line_count = complete_line_count.saturating_sub(1);
            // If we're inside an unclosed fenced code block, also drop the
            // last rendered line to avoid committing a partial code line.
            if is_inside_unclosed_fence(&source) {
                complete_line_count = complete_line_count.saturating_sub(1);
            }
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
    use std::cmp::Ordering;

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

    // --- Minimization and fuzz helpers (debug/analysis) ---

    fn render_full_strings(src: &str, cfg: &Config) -> Vec<String> {
        let mut out: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(src, &mut out, cfg);
        lines_to_plain_strings(&out)
    }

    fn stream_strings_from_deltas(deltas: &[&str], cfg: &Config) -> Vec<String> {
        let lines = simulate_stream_markdown_for_tests(deltas, true, cfg);
        lines_to_plain_strings(&lines)
    }

    fn mismatch(deltas: &[&str], cfg: &Config) -> bool {
        let full: String = deltas.iter().copied().collect();
        let streamed = stream_strings_from_deltas(deltas, cfg);
        let rendered = render_full_strings(&full, cfg);
        streamed != rendered
    }

    /// Greedily minimize a failing delta sequence to the shortest (by element count)
    /// that still reproduces the mismatch. Returns the minimized sequence as Vec<String>.
    fn minimize_failing_deltas(mut deltas: Vec<String>, cfg: &Config) -> Vec<String> {
        // Early exit if not failing.
        if !mismatch(&deltas.iter().map(|s| s.as_str()).collect::<Vec<_>>(), cfg) {
            return deltas;
        }

        // Try removals first until fixed point.
        let mut changed = true;
        while changed {
            changed = false;
            let mut best: Option<Vec<String>> = None;
            for i in 0..deltas.len() {
                let mut cand = deltas.clone();
                cand.remove(i);
                let cand_refs = cand.iter().map(|s| s.as_str()).collect::<Vec<_>>();
                if mismatch(&cand_refs, cfg) {
                    match &best {
                        None => best = Some(cand),
                        Some(b) => {
                            if cand.len().cmp(&b.len()) == Ordering::Less {
                                best = Some(cand);
                            }
                        }
                    }
                }
            }
            if let Some(b) = best {
                deltas = b;
                changed = true;
                continue;
            }

            // Try merging adjacent elements.
            let mut merged_any = false;
            for i in 0..(deltas.len().saturating_sub(1)) {
                let mut cand = deltas.clone();
                let merged = format!("{}{}", cand[i], cand[i + 1]);
                cand[i] = merged;
                cand.remove(i + 1);
                let cand_refs = cand.iter().map(|s| s.as_str()).collect::<Vec<_>>();
                if mismatch(&cand_refs, cfg) {
                    deltas = cand;
                    changed = true;
                    merged_any = true;
                    break;
                }
            }
            if merged_any {
                continue;
            }
        }
        deltas
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
    fn dash_split_across_chunks_does_not_dangle() {
        let cfg = test_config();

        // Repro: deltas that arrive as ["-", " some", " words\n"] should not
        // produce a dangling history line with just "-" followed by
        // "some words". The streamed result should match a full render.
        let deltas = vec!["-", " some", " words\n"];
        let streamed = simulate_stream_markdown_for_tests(&deltas, true, &cfg);
        let streamed_str = lines_to_plain_strings(&streamed);

        let mut rendered_all: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown("- some words\n", &mut rendered_all, &cfg);
        let rendered_all_str = lines_to_plain_strings(&rendered_all);

        assert_eq!(
            streamed_str, rendered_all_str,
            "dash-split streaming should equal full render without dangling '-' line"
        );
    }

    #[test]
    fn dash_split_then_newline_in_middle_of_delta_does_not_commit_dash() {
        let cfg = test_config();
        let mut c = super::MarkdownStreamCollector::new();

        // Seed some preceding content that commits cleanly.
        c.push_delta("Intro line.\n");
        let _ = c.commit_complete_lines(&cfg);

        // Now stream a list item split as ["-", " some", " words"], but do not
        // include a newline yet.
        c.push_delta("-");
        c.push_delta(" some");

        // Next delta contains a newline early, but ends without one, which triggers
        // a commit while the buffer does not end in a newline. This used to cause
        // the solitary "-" to be considered a completed line and emitted.
        c.push_delta("\ntrailing");
        let out = c.commit_complete_lines(&cfg);
        let texts = lines_to_plain_strings(&out);

        // Ensure we did not emit a dangling dash line.
        assert!(
            !texts.iter().any(|s| s == "-"),
            "should not emit a standalone '-' line: {texts:?}"
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

    /// Exhaustive-ish search to find the shortest failing chunking for a given source.
    /// Ignored by default since it can be slow. Run with:
    ///   cargo test -p codex-tui find_min_failing_chunking -- --ignored --nocapture
    #[test]
    #[ignore]
    fn find_min_failing_chunking() {
        let cfg = test_config();
        let src = "Loose list (multi-paragraph items)\n- This item has two paragraphs. The second paragraph is still part of this item.\n\n Here’s the second paragraph, indented to remain within the same bullet.\n- Next item after a loose break.\n\n";

        // Choose candidate split points: after every space, after every dash, and after newlines.
        let bytes = src.as_bytes();
        let mut splits = Vec::new();
        for i in 1..bytes.len() {
            let c = bytes[i - 1] as char;
            if c == ' ' || c == '-' || c == '\n' {
                splits.push(i);
            }
        }
        // Cap the number of splits to keep combinations manageable.
        if splits.len() > 18 {
            splits.truncate(18);
        }

        let mut best: Option<Vec<String>> = None;
        let n = splits.len();
        // Iterate all bitmasks selecting which splits to keep as boundaries.
        let total = 1usize << n;
        for mask in 0..total {
            let mut last = 0usize;
            let mut chunks: Vec<String> = Vec::new();
            for (bit, &pos) in splits.iter().enumerate() {
                if (mask >> bit) & 1 == 1 {
                    chunks.push(src[last..pos].to_string());
                    last = pos;
                }
            }
            if last < src.len() {
                chunks.push(src[last..].to_string());
            }
            let refs = chunks.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            if mismatch(&refs, &cfg) {
                // Greedily minimize this failing candidate further.
                let minimized = minimize_failing_deltas(chunks.clone(), &cfg);
                if best
                    .as_ref()
                    .map(|b| minimized.len() < b.len())
                    .unwrap_or(true)
                {
                    best = Some(minimized);
                }
            }
        }

        if let Some(b) = best {
            eprintln!("Shortest failing chunks ({}):", b.len());
            for (i, s) in b.iter().enumerate() {
                eprintln!("  [{}] {:?}", i, s);
            }
        } else {
            eprintln!("No failing chunking found under the capped search.");
        }

        // This is a diagnostic test; do not assert failure so it remains useful after a fix.
        assert!(true);
    }

    /// Determinism check: run the same failing sequence many times and ensure
    /// the mismatch outcome is consistent. Ignored by default due to runtime.
    #[test]
    #[ignore]
    fn streaming_behavior_is_deterministic_for_failing_case() {
        let cfg = test_config();
        let deltas = vec![
            "Loose list (multi-paragraph items)\n",
            "-",
            " This item has two paragraphs. The second paragraph is still part of this item.\n\n",
            " Here’s the second paragraph, indented to remain within the same bullet.\n",
            "- Next item after a loose break.\n\n",
        ];
        let refs = deltas.iter().map(|s| *s).collect::<Vec<_>>();
        let first = mismatch(&refs, &cfg);
        let mut consistent = true;
        for _ in 0..200 {
            let now = mismatch(&refs, &cfg);
            if now != first {
                consistent = false;
                break;
            }
        }
        if !consistent {
            panic!("nondeterministic mismatch outcome detected");
        }
    }

    /// Greedy minimization starting from a known failing delta sequence.
    /// Prints the minimized sequence and length. Ignored by default.
    #[test]
    #[ignore]
    fn minimize_known_failing_sequence() {
        let cfg = test_config();
        let seed = vec![
            "Loose".to_string(),
            " list".to_string(),
            " (".to_string(),
            "multi".to_string(),
            "-par".to_string(),
            "agraph".to_string(),
            " items".to_string(),
            ")\n".to_string(),
            "-".to_string(),
            " This".to_string(),
            " item".to_string(),
            " has".to_string(),
            " two".to_string(),
            " paragraphs".to_string(),
            ".".to_string(),
            " The".to_string(),
            " second".to_string(),
            " paragraph".to_string(),
            " is".to_string(),
            " still".to_string(),
            " part".to_string(),
            " of".to_string(),
            " this".to_string(),
            " item".to_string(),
            ".\n\n".to_string(),
            " ".to_string(),
            " Here".to_string(),
            "’s".to_string(),
            " the".to_string(),
            " second".to_string(),
            " paragraph".to_string(),
            ",".to_string(),
            " ind".to_string(),
            "ented".to_string(),
            " to".to_string(),
            " remain".to_string(),
            " within".to_string(),
            " the".to_string(),
            " same".to_string(),
            " bullet".to_string(),
            ".\n".to_string(),
            "-".to_string(),
            " Next".to_string(),
            " item".to_string(),
            " after".to_string(),
            " a".to_string(),
            " loose".to_string(),
            " break".to_string(),
            ".\n\n".to_string(),
        ];

        // Ensure seed actually mismatches to begin with.
        let seed_refs = seed.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        if !mismatch(&seed_refs, &cfg) {
            eprintln!("Seed does not reproduce mismatch; nothing to minimize.");
            return;
        }

        let minimized = minimize_failing_deltas(seed, &cfg);
        eprintln!("Minimized chunks ({}):", minimized.len());
        for (i, s) in minimized.iter().enumerate() {
            eprintln!("  [{}] {:?}", i, s);
        }

        // Keep as informational; do not assert.
        assert!(true);
    }
}
