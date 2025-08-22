use codex_core::config::Config;
use ratatui::text::Line;

use super::HeaderEmitter;
use super::StreamState;

/// Sink for history insertions and animation control.
pub(crate) trait HistorySink {
    fn insert_history(&self, lines: Vec<Line<'static>>);
    fn start_commit_animation(&self);
    fn stop_commit_animation(&self);
}

/// Concrete sink backed by `AppEventSender`.
pub(crate) struct AppEventHistorySink(pub(crate) crate::app_event_sender::AppEventSender);

impl HistorySink for AppEventHistorySink {
    fn insert_history(&self, lines: Vec<Line<'static>>) {
        self.0
            .send(crate::app_event::AppEvent::InsertHistoryLines(lines))
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
    header: HeaderEmitter,
    state: StreamState,
    active: bool,
    finishing_after_drain: bool,
}

impl StreamController {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            header: HeaderEmitter::new(),
            state: StreamState::new(),
            active: false,
            finishing_after_drain: false,
        }
    }

    pub(crate) fn reset_headers_for_new_turn(&mut self) {
        self.header.reset_for_new_turn();
    }

    pub(crate) fn is_write_cycle_active(&self) -> bool {
        self.active
    }

    pub(crate) fn clear_all(&mut self) {
        self.state.clear();
        self.active = false;
        self.finishing_after_drain = false;
        // leave header state unchanged; caller decides when to reset
    }

    fn emit_header_if_needed(&mut self, out_lines: &mut Lines) -> bool {
        self.header.maybe_emit(out_lines)
    }

    /// Begin an answer stream. Does not emit header yet; it is emitted on first commit.
    pub(crate) fn begin(&mut self, _sink: &impl HistorySink) {
        // Starting a new stream cancels any pending finish-from-previous-stream animation.
        if !self.active {
            self.header.reset_for_stream();
        }
        self.finishing_after_drain = false;
        self.active = true;
    }

    /// Push a delta; if it contains a newline, commit completed lines and start animation.
    pub(crate) fn push_and_maybe_commit(&mut self, delta: &str, sink: &impl HistorySink) {
        if !self.active {
            return;
        }
        let cfg = self.config.clone();
        let state = &mut self.state;
        // Record that at least one delta was received for this stream
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

    /// Finalize the active stream. If `flush_immediately` is true, drain and emit now.
    pub(crate) fn finalize(&mut self, flush_immediately: bool, sink: &impl HistorySink) -> bool {
        if !self.active {
            return false;
        }
        let cfg = self.config.clone();
        // Finalize collector first.
        let remaining = {
            let state = &mut self.state;
            state.collector.finalize_and_drain(&cfg)
        };
        if flush_immediately {
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
                let mut lines_with_header: Lines = Vec::new();
                self.emit_header_if_needed(&mut lines_with_header);
                lines_with_header.extend(out_lines);
                sink.insert_history(lines_with_header);
            }

            // Cleanup
            self.state.clear();
            // Allow a subsequent block in this turn to emit its header.
            self.header.allow_reemit_in_turn();
            // Also clear the per-stream emitted flag so the header can render again.
            self.header.reset_for_stream();
            self.active = false;
            self.finishing_after_drain = false;
            true
        } else {
            if !remaining.is_empty() {
                let state = &mut self.state;
                state.enqueue(remaining);
            }
            // Spacer animated out
            self.state.enqueue(vec![Line::from("")]);
            self.finishing_after_drain = true;
            sink.start_commit_animation();
            false
        }
    }

    /// Step animation: commit at most one queued line and handle end-of-drain cleanup.
    pub(crate) fn on_commit_tick(&mut self, sink: &impl HistorySink) -> bool {
        if !self.active {
            return false;
        }
        let step = { self.state.step() };
        if !step.history.is_empty() {
            let mut lines: Lines = Vec::new();
            self.emit_header_if_needed(&mut lines);
            let mut out = lines;
            out.extend(step.history);
            sink.insert_history(out);
        }

        let is_idle = self.state.is_idle();
        if is_idle {
            sink.stop_commit_animation();
            if self.finishing_after_drain {
                // Reset and notify
                self.state.clear();
                // Allow a subsequent block in this turn to emit its header.
                self.header.allow_reemit_in_turn();
                // Also clear the per-stream emitted flag so the header can render again.
                self.header.reset_for_stream();
                self.active = false;
                self.finishing_after_drain = false;
                return true;
            }
        }
        false
    }

    /// Apply a full final answer: replace queued content with only the remaining tail,
    /// then finalize immediately and notify completion.
    pub(crate) fn apply_final_answer(&mut self, message: &str, sink: &impl HistorySink) -> bool {
        self.apply_full_final(message, sink)
    }

    fn apply_full_final(&mut self, message: &str, sink: &impl HistorySink) -> bool {
        self.begin(sink);

        {
            let state = &mut self.state;
            // Only inject the final full message if we have not seen any deltas for this stream.
            // If deltas were received, rely on the collector's existing buffer to avoid duplication.
            if !state.has_seen_delta && !message.is_empty() {
                // normalize to end with newline
                let mut msg = message.to_owned();
                if !msg.ends_with('\n') {
                    msg.push('\n');
                }

                // replace while preserving already committed count
                let committed = state.collector.committed_count();
                state
                    .collector
                    .replace_with_and_mark_committed(&msg, committed);
            }
        }
        self.finalize(true, sink)
    }
}
