use crate::history_cell;
use crate::history_cell::HistoryCell;
use codex_core::config::Config;
use ratatui::text::Line;

use super::StreamState;

/// Sink for history insertions and animation control.
pub(crate) trait HistorySink {
    fn insert_history_cell(&self, cell: Box<dyn HistoryCell>);
    fn start_commit_animation(&self);
    fn stop_commit_animation(&self);
}

/// Concrete sink backed by `AppEventSender`.
pub(crate) struct AppEventHistorySink(pub(crate) crate::app_event_sender::AppEventSender);

impl HistorySink for AppEventHistorySink {
    fn insert_history_cell(&self, cell: Box<dyn crate::history_cell::HistoryCell>) {
        self.0
            .send(crate::app_event::AppEvent::InsertHistoryCell(cell))
    }
    fn start_commit_animation(&self) {
        self.0
            .send(crate::app_event::AppEvent::StartCommitAnimation)
    }
    fn stop_commit_animation(&self) {
        self.0.send(crate::app_event::AppEvent::StopCommitAnimation)
    }
}

type Lines = Vec<Line<'static>>;

/// Controller that manages newline-gated streaming, header emission, and
/// commit animation across streams.
pub(crate) struct StreamController {
    config: Config,
    state: StreamState,
    finishing_after_drain: bool,
    header_emitted: bool,
}

impl StreamController {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            state: StreamState::new(),
            finishing_after_drain: false,
            header_emitted: false,
        }
    }

    /// Push a delta; if it contains a newline, commit completed lines and start animation.
    pub(crate) fn push_and_maybe_commit(&mut self, delta: &str, sink: &impl HistorySink) {
        let cfg = self.config.clone();
        let state = &mut self.state;
        if !delta.is_empty() {
            state.has_seen_delta = true;
        }
        state.collector.push_delta(delta);
        if delta.contains('\n') {
            let newly_completed = state.collector.commit_complete_lines(&cfg);
            if !newly_completed.is_empty() {
                state.enqueue(newly_completed);
                sink.start_commit_animation();
            }
        }
    }

    /// Finalize the active stream. Drain and emit now.
    pub(crate) fn finalize(&mut self, sink: &impl HistorySink) {
        let cfg = self.config.clone();
        // Finalize collector first.
        let remaining = {
            let state = &mut self.state;
            state.collector.finalize_and_drain(&cfg)
        };
        // Collect all output first to avoid emitting headers when there is no content.
        let mut out_lines: Lines = Vec::new();
        {
            let state = &mut self.state;
            if !remaining.is_empty() {
                state.enqueue(remaining);
            }
            let step = state.drain_all();
            out_lines.extend(step.history);
        }
        if !out_lines.is_empty() {
            // Insert as a HistoryCell so display drops the header while transcript keeps it.
            self.emit(sink, out_lines);
        }

        // Cleanup
        self.state.clear();
        self.finishing_after_drain = false;
    }

    /// Step animation: commit at most one queued line and handle end-of-drain cleanup.
    pub(crate) fn on_commit_tick(&mut self, sink: &impl HistorySink) -> bool {
        let step = { self.state.step() };
        if !step.history.is_empty() {
            self.emit(sink, step.history);
        }

        let is_idle = self.state.is_idle();
        if is_idle {
            sink.stop_commit_animation();
        }
        false
    }

    fn emit(&mut self, sink: &impl HistorySink, lines: Vec<Line<'static>>) {
        sink.insert_history_cell(Box::new(history_cell::AgentMessageCell::new(
            lines,
            !self.header_emitted,
        )));
        self.header_emitted = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;
    use std::cell::RefCell;

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

    struct TestSink {
        pub lines: RefCell<Vec<Vec<Line<'static>>>>,
    }
    impl TestSink {
        fn new() -> Self {
            Self {
                lines: RefCell::new(Vec::new()),
            }
        }
    }
    impl HistorySink for TestSink {
        fn insert_history_cell(&self, cell: Box<dyn crate::history_cell::HistoryCell>) {
            // For tests, store the transcript representation of the cell.
            self.lines.borrow_mut().push(cell.transcript_lines());
        }
        fn start_commit_animation(&self) {}
        fn stop_commit_animation(&self) {}
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
    fn controller_loose_vs_tight_with_commit_ticks_matches_full() {
        let cfg = test_config();
        let mut ctrl = StreamController::new(cfg.clone());
        let sink = TestSink::new();

        // Exact deltas from the session log (section: Loose vs. tight list items)
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

        // Simulate streaming with a commit tick attempt after each delta.
        for d in &deltas {
            ctrl.push_and_maybe_commit(d, &sink);
            let _ = ctrl.on_commit_tick(&sink);
        }
        // Finalize and flush remaining lines now.
        ctrl.finalize(&sink);

        // Flatten sink output and strip the header that the controller inserts (blank + "codex").
        let mut flat: Vec<ratatui::text::Line<'static>> = Vec::new();
        for batch in sink.lines.borrow().iter() {
            for l in batch {
                flat.push(l.clone());
            }
        }
        // Drop leading blank and header line if present.
        if !flat.is_empty() && lines_to_plain_strings(&[flat[0].clone()])[0].is_empty() {
            flat.remove(0);
        }
        if !flat.is_empty() {
            let s0 = lines_to_plain_strings(&[flat[0].clone()])[0].clone();
            if s0 == "codex" {
                flat.remove(0);
            }
        }
        let streamed = lines_to_plain_strings(&flat);

        // Full render of the same source
        let source: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&source, &mut rendered, &cfg);
        let rendered_strs = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, rendered_strs);

        // Also assert exact expected plain strings for clarity.
        let expected = vec![
            "Loose vs. tight list items:".to_string(),
            "".to_string(),
            "1. Tight item".to_string(),
            "2. Another tight item".to_string(),
            "3. Loose item with its own paragraph.".to_string(),
            "".to_string(),
            "   This paragraph belongs to the same list item.".to_string(),
            "4. Second loose item with a nested list after a blank line.".to_string(),
            "    - Nested bullet under a loose item".to_string(),
            "    - Another nested bullet".to_string(),
        ];
        assert_eq!(
            streamed, expected,
            "expected exact rendered lines for loose/tight section"
        );
    }
}
