use codex_core::config::Config;
use ratatui::text::Line;

use super::HeaderEmitter;
use super::StreamKind;
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
            .send(crate::app_event::AppEvent::InsertHistory(lines))
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
    states: [StreamState; 2],
    current_stream: Option<StreamKind>,
    finishing_after_drain: bool,
}

impl StreamController {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            header: HeaderEmitter::new(),
            states: [StreamState::new(), StreamState::new()],
            current_stream: None,
            finishing_after_drain: false,
        }
    }

    pub(crate) fn reset_headers_for_new_turn(&mut self) {
        self.header.reset_for_new_turn();
    }

    pub(crate) fn is_write_cycle_active(&self) -> bool {
        self.current_stream.is_some()
    }

    pub(crate) fn clear_all(&mut self) {
        self.states.iter_mut().for_each(|s| s.clear());
        self.current_stream = None;
        self.finishing_after_drain = false;
        // leave header state unchanged; caller decides when to reset
    }

    #[inline]
    fn idx(kind: StreamKind) -> usize {
        kind as usize
    }
    fn state(&self, kind: StreamKind) -> &StreamState {
        &self.states[Self::idx(kind)]
    }
    fn state_mut(&mut self, kind: StreamKind) -> &mut StreamState {
        &mut self.states[Self::idx(kind)]
    }

    fn emit_header_if_needed(&mut self, kind: StreamKind, out_lines: &mut Lines) -> bool {
        self.header.maybe_emit(kind, out_lines)
    }

    #[inline]
    fn ensure_single_trailing_blank(lines: &mut Lines) {
        if lines
            .last()
            .map(|l| !crate::render::line_utils::is_blank_line_trim(l))
            .unwrap_or(true)
        {
            lines.push(Line::from(""));
        }
    }

    /// Begin a stream, flushing previously completed lines from any other
    /// active stream to maintain ordering.
    pub(crate) fn begin(&mut self, kind: StreamKind, sink: &impl HistorySink) {
        if let Some(current) = self.current_stream {
            if current != kind {
                // Synchronously flush completed lines from previous stream.
                let cfg = self.config.clone();
                let prev_state = self.state_mut(current);
                let newly_completed = prev_state.collector.commit_complete_lines(&cfg);
                if !newly_completed.is_empty() {
                    prev_state.enqueue(newly_completed);
                }
                let step = prev_state.drain_all();
                if !step.history.is_empty() {
                    let mut lines: Lines = Vec::new();
                    self.emit_header_if_needed(current, &mut lines);
                    lines.extend(step.history);
                    // Ensure at most one trailing blank after the flushed block.
                    Self::ensure_single_trailing_blank(&mut lines);
                    sink.insert_history(lines);
                }
                self.current_stream = None;
            }
        }

        if self.current_stream != Some(kind) {
            let prev = self.current_stream;
            self.current_stream = Some(kind);
            // Starting a new stream cancels any pending finish-from-previous-stream animation.
            self.finishing_after_drain = false;
            if prev.is_some() {
                self.header.reset_for_stream(kind);
            }
            // Emit header immediately for reasoning; for answers, defer to first commit.
            if matches!(kind, StreamKind::Reasoning) {
                let mut header_lines = Vec::new();
                if self.emit_header_if_needed(kind, &mut header_lines) {
                    sink.insert_history(header_lines);
                }
            }
        }
    }

    /// Push a delta; if it contains a newline, commit completed lines and start animation.
    pub(crate) fn push_and_maybe_commit(&mut self, delta: &str, sink: &impl HistorySink) {
        let Some(kind) = self.current_stream else {
            return;
        };
        let cfg = self.config.clone();
        let state = self.state_mut(kind);
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

    /// Insert a reasoning section break and commit any newly completed lines.
    pub(crate) fn insert_reasoning_section_break(&mut self, sink: &impl HistorySink) {
        if self.current_stream != Some(StreamKind::Reasoning) {
            self.begin(StreamKind::Reasoning, sink);
        }
        let cfg = self.config.clone();
        let state = self.state_mut(StreamKind::Reasoning);
        state.collector.insert_section_break();
        let newly_completed = state.collector.commit_complete_lines(&cfg);
        if !newly_completed.is_empty() {
            state.enqueue(newly_completed);
            sink.start_commit_animation();
        }
    }

    /// Finalize the active stream. If `flush_immediately` is true, drain and emit now.
    pub(crate) fn finalize(
        &mut self,
        kind: StreamKind,
        flush_immediately: bool,
        sink: &impl HistorySink,
    ) -> bool {
        if self.current_stream != Some(kind) {
            return false;
        }
        let cfg = self.config.clone();
        // Finalize collector first.
        let remaining = {
            let state = self.state_mut(kind);
            state.collector.finalize_and_drain(&cfg)
        };
        if flush_immediately {
            // Collect all output first to avoid emitting headers when there is no content.
            let mut out_lines: Lines = Vec::new();
            {
                let state = self.state_mut(kind);
                if !remaining.is_empty() {
                    state.enqueue(remaining);
                }
                let step = state.drain_all();
                out_lines.extend(step.history);
            }
            if !out_lines.is_empty() {
                let mut lines_with_header: Lines = Vec::new();
                self.emit_header_if_needed(kind, &mut lines_with_header);
                lines_with_header.extend(out_lines);
                Self::ensure_single_trailing_blank(&mut lines_with_header);
                sink.insert_history(lines_with_header);
            }

            // Cleanup
            self.state_mut(kind).clear();
            // Allow a subsequent block of the same kind in this turn to emit its header.
            self.header.allow_reemit_for_same_kind_in_turn(kind);
            // Also clear the per-stream emitted flag so the header can render again.
            self.header.reset_for_stream(kind);
            self.current_stream = None;
            self.finishing_after_drain = false;
            true
        } else {
            if !remaining.is_empty() {
                let state = self.state_mut(kind);
                state.enqueue(remaining);
            }
            // Spacer animated out
            self.state_mut(kind).enqueue(vec![Line::from("")]);
            self.finishing_after_drain = true;
            sink.start_commit_animation();
            false
        }
    }

    /// Step animation: commit at most one queued line and handle end-of-drain cleanup.
    pub(crate) fn on_commit_tick(&mut self, sink: &impl HistorySink) -> bool {
        let Some(kind) = self.current_stream else {
            return false;
        };
        let step = {
            let state = self.state_mut(kind);
            state.step()
        };
        if !step.history.is_empty() {
            let mut lines: Lines = Vec::new();
            self.emit_header_if_needed(kind, &mut lines);
            let mut out = lines;
            out.extend(step.history);
            sink.insert_history(out);
        }

        let is_idle = self.state(kind).is_idle();
        if is_idle {
            sink.stop_commit_animation();
            if self.finishing_after_drain {
                // Reset and notify
                self.state_mut(kind).clear();
                // Allow a subsequent block of the same kind in this turn to emit its header.
                self.header.allow_reemit_for_same_kind_in_turn(kind);
                // Also clear the per-stream emitted flag so the header can render again.
                self.header.reset_for_stream(kind);
                self.current_stream = None;
                self.finishing_after_drain = false;
                return true;
            }
        }
        false
    }

    /// Apply a full final answer: replace queued content with only the remaining tail,
    /// then finalize immediately and notify completion.
    pub(crate) fn apply_final_answer(&mut self, message: &str, sink: &impl HistorySink) -> bool {
        self.apply_full_final(StreamKind::Answer, message, true, sink)
    }

    pub(crate) fn apply_final_reasoning(&mut self, message: &str, sink: &impl HistorySink) -> bool {
        self.apply_full_final(StreamKind::Reasoning, message, false, sink)
    }

    fn apply_full_final(
        &mut self,
        kind: StreamKind,
        message: &str,
        immediate: bool,
        sink: &impl HistorySink,
    ) -> bool {
        self.begin(kind, sink);

        {
            let state = self.state_mut(kind);
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

        self.finalize(kind, immediate, sink)
    }
}
