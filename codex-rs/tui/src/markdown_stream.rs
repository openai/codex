use std::collections::VecDeque;

use codex_core::config::Config;
use ratatui::text::Line;

use crate::markdown;

/// Newline-gated accumulator that renders markdown and commits only fully
/// completed logical lines.
pub(crate) struct MarkdownNewlineCollector {
    buffer: String,
    committed_line_count: usize,
}

impl MarkdownNewlineCollector {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            committed_line_count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.committed_line_count = 0;
    }

    pub fn push_delta(&mut self, delta: &str) {
        self.buffer.push_str(delta);
    }

    /// Render the full buffer and return only the newly completed logical lines
    /// since the last commit. When the buffer does not end with a newline, the
    /// final rendered line is considered incomplete and is not emitted.
    pub fn commit_complete_lines(&mut self, config: &Config) -> Vec<Line<'static>> {
        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(&self.buffer, &mut rendered, config);

        let ends_with_nl = self.buffer.ends_with('\n');
        let complete_line_count = if ends_with_nl {
            rendered.len()
        } else {
            rendered.len().saturating_sub(1)
        };

        if self.committed_line_count >= complete_line_count {
            return Vec::new();
        }

        let out = rendered[self.committed_line_count..complete_line_count].to_vec();
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

    /// Returns the currently rendered, incomplete last line if any.
    pub fn current_partial_line(&self, config: &Config) -> Option<Line<'static>> {
        if self.buffer.ends_with('\n') {
            return None;
        }
        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(&self.buffer, &mut rendered, config);
        rendered.into_iter().last()
    }
}

#[cfg(test)]
fn unwrap_markdown_language_fence_if_enabled(s: String) -> String {
    // In tests, keep content exactly as provided to simplify assertions.
    s
}

#[cfg(not(test))]
fn unwrap_markdown_language_fence_if_enabled(s: String) -> String {
    // Best-effort unwrap of a single outer ```markdown fence.
    // This is intentionally simple; we can refine as needed later.
    const OPEN: &str = "```markdown\n";
    const CLOSE: &str = "\n```\n";
    if s.starts_with(OPEN) && s.ends_with(CLOSE) {
        let inner = s[OPEN.len()..s.len() - CLOSE.len()].to_string();
        return inner;
    }
    s
}

pub(crate) struct StepResult {
    pub history: Vec<Line<'static>>, // lines to insert into history this step
    pub live: Vec<Line<'static>>,     // newest K rows to show in the live ring
}

/// Streams already-rendered rows into history while computing the newest K
/// rows to show in a live overlay.
pub(crate) struct RenderedLineStreamer {
    queue: VecDeque<Line<'static>>,
    tail: Vec<Line<'static>>, // last K lines that reached history
}

impl RenderedLineStreamer {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            tail: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.tail.clear();
    }

    pub fn enqueue(&mut self, lines: Vec<Line<'static>>) {
        for l in lines {
            self.queue.push_back(l);
        }
    }

    pub fn step(&mut self, live_max_rows: usize) -> StepResult {
        let mut history = Vec::new();
        // Move at least 1, up to a small burst, to feel responsive without flooding.
        let burst = 3usize.min(self.queue.len().max(1));
        for _ in 0..burst {
            if let Some(l) = self.queue.pop_front() {
                history.push(l);
            }
        }

        // Update tail with newly committed history and cap to K.
        if !history.is_empty() {
            self.tail.extend(history.iter().cloned());
            if self.tail.len() > live_max_rows {
                let drop = self.tail.len() - live_max_rows;
                self.tail.drain(0..drop);
            }
        }

        // Live rows are tail + queue head, capped to K, newest at the end.
        let mut live = self.tail.clone();
        if live.len() < live_max_rows {
            let need = live_max_rows - live.len();
            for l in self.queue.iter().take(need) {
                live.push(l.clone());
            }
        }
        if live.len() > live_max_rows {
            let drop = live.len() - live_max_rows;
            live.drain(0..drop);
        }

        StepResult { history, live }
    }

    pub fn drain_all(&mut self, live_max_rows: usize) -> StepResult {
        let mut history = Vec::new();
        while let Some(l) = self.queue.pop_front() {
            history.push(l);
        }
        if !history.is_empty() {
            self.tail.extend(history.iter().cloned());
            if self.tail.len() > live_max_rows {
                let drop = self.tail.len() - live_max_rows;
                self.tail.drain(0..drop);
            }
        }
        // Live shows the last K history rows.
        let live = if self.tail.len() > live_max_rows {
            self.tail[self.tail.len() - live_max_rows..].to_vec()
        } else {
            self.tail.clone()
        };
        StepResult { history, live }
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
    let mut collector = MarkdownNewlineCollector::new();
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
            cwd: Some(std::env::current_dir().unwrap()),
            ..Default::default()
        };
        Config::load_with_cli_overrides(vec![], overrides).expect("load test config")
    }

    #[test]
    fn no_commit_until_newline() {
        let cfg = test_config();
        let mut c = MarkdownNewlineCollector::new();
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
        let mut c = MarkdownNewlineCollector::new();
        c.push_delta("Line without newline");
        let out = c.finalize_and_drain(&cfg);
        assert_eq!(out.len(), 1);
    }
}

