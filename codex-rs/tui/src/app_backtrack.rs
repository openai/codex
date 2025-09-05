use crate::app::App;
use crate::backtrack_helpers;
use crate::pager_overlay::Overlay;
use crate::tui;
use crate::tui::TuiEvent;
use codex_core::protocol::ConversationHistoryResponseEvent;
use codex_protocol::mcp_protocol::ConversationId;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::text::Line;

/// Aggregates all backtrack-related state used by the App.
#[derive(Default)]
pub(crate) struct BacktrackState {
    /// True when Esc has primed backtrack mode in the main view.
    pub(crate) primed: bool,
    /// Session id of the base conversation to fork from.
    pub(crate) base_id: Option<ConversationId>,
    /// Current step count (Nth last user message).
    pub(crate) count: usize,
    /// True when the transcript overlay is showing a backtrack preview.
    pub(crate) overlay_preview_active: bool,
    /// Pending fork request: (base_id, drop_count, prefill).
    pub(crate) pending: Option<(ConversationId, usize, String)>,
}

impl App {
    /// Route overlay events when transcript overlay is active.
    /// - If backtrack preview is active: Esc steps selection; Enter confirms.
    /// - Otherwise: Esc begins preview; all other events forward to overlay.
    ///   interactions (Esc to step target, Enter to confirm) and overlay lifecycle.
    pub(crate) async fn handle_backtrack_overlay_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        if self.backtrack.overlay_preview_active {
            match event {
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => {
                    self.overlay_step_backtrack(tui, event)?;
                    Ok(true)
                }
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    self.overlay_confirm_backtrack(tui);
                    Ok(true)
                }
                // Catchall: forward any other events to the overlay widget.
                _ => {
                    self.overlay_forward_event(tui, event)?;
                    Ok(true)
                }
            }
        } else if let TuiEvent::Key(KeyEvent {
            code: KeyCode::Esc,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        }) = event
        {
            // First Esc in transcript overlay: begin backtrack preview at latest user message.
            self.begin_overlay_backtrack_preview(tui);
            Ok(true)
        } else {
            // Not in backtrack mode: forward events to the overlay widget.
            self.overlay_forward_event(tui, event)?;
            Ok(true)
        }
    }

    /// Handle global Esc presses for backtracking when no overlay is present.
    pub(crate) fn handle_backtrack_esc_key(&mut self, tui: &mut tui::Tui) {
        // Only handle backtracking when composer is empty to avoid clobbering edits.
        if self.chat_widget.composer_is_empty() {
            if !self.backtrack.primed {
                self.prime_backtrack();
            } else if self.overlay.is_none() {
                self.open_backtrack_preview(tui);
            } else if self.backtrack.overlay_preview_active {
                self.step_backtrack_and_highlight(tui);
            }
        }
    }

    /// Stage a backtrack and request conversation history from the agent.
    pub(crate) fn request_backtrack(
        &mut self,
        prefill: String,
        base_id: ConversationId,
        drop_last_messages: usize,
    ) {
        self.backtrack.pending = Some((base_id, drop_last_messages, prefill));
        self.app_event_tx.send(crate::app_event::AppEvent::CodexOp(
            codex_core::protocol::Op::GetHistory,
        ));
    }

    /// Open transcript overlay (enters alternate screen and shows full transcript).
    pub(crate) fn open_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.enter_alt_screen();
        let (lines, _spans) = self.build_transcript_flattened();
        self.overlay = Some(Overlay::new_transcript(lines));
        tui.frame_requester().schedule_frame();
    }

    /// Close transcript overlay and restore normal UI.
    pub(crate) fn close_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.leave_alt_screen();
        let was_backtrack = self.backtrack.overlay_preview_active;
        if !self.deferred_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.deferred_history_lines);
            tui.insert_history_lines(lines);
        }
        self.overlay = None;
        self.backtrack.overlay_preview_active = false;
        if was_backtrack {
            // Ensure backtrack state is fully reset when overlay closes (e.g. via 'q').
            self.reset_backtrack_state();
        }
    }

    /// Initialize backtrack state and show composer hint.
    fn prime_backtrack(&mut self) {
        self.backtrack.primed = true;
        self.backtrack.count = 0;
        self.backtrack.base_id = self.chat_widget.conversation_id();
        self.chat_widget.show_esc_backtrack_hint();
    }

    /// Open overlay and begin backtrack preview flow (first step + highlight).
    fn open_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        self.open_transcript_overlay(tui);
        self.backtrack.overlay_preview_active = true;
        // Composer is hidden by overlay; clear its hint.
        self.chat_widget.clear_esc_backtrack_hint();
        self.step_backtrack_and_highlight(tui);
    }

    /// When overlay is already open, begin preview mode and select latest user message.
    fn begin_overlay_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        self.backtrack.primed = true;
        self.backtrack.base_id = self.chat_widget.conversation_id();
        self.backtrack.overlay_preview_active = true;
        let sel = self.compute_backtrack_selection(tui, 1);
        self.apply_backtrack_selection(sel);
        tui.frame_requester().schedule_frame();
    }

    /// Step selection to the next older user message and update overlay.
    fn step_backtrack_and_highlight(&mut self, tui: &mut tui::Tui) {
        let next = self.backtrack.count.saturating_add(1);
        let sel = self.compute_backtrack_selection(tui, next);
        self.apply_backtrack_selection(sel);
        tui.frame_requester().schedule_frame();
    }

    /// Compute normalized target, scroll offset, and highlight for requested step.
    fn compute_backtrack_selection(
        &self,
        tui: &tui::Tui,
        requested_n: usize,
    ) -> (usize, Option<usize>, Option<(usize, usize)>) {
        let (lines, spans) = self.build_transcript_flattened();
        let nth = backtrack_helpers::normalize_backtrack_n(&spans, requested_n);
        let header_idx = backtrack_helpers::find_nth_last_user_header_index(&spans, nth);
        let offset = header_idx.map(|idx| {
            backtrack_helpers::wrapped_offset_before(&lines, idx, tui.terminal.viewport_area.width)
        });
        let hl = backtrack_helpers::highlight_range_for_nth_last_user(&spans, nth);
        (nth, offset, hl)
    }

    /// Apply a computed backtrack selection to the overlay and internal counter.
    fn apply_backtrack_selection(
        &mut self,
        selection: (usize, Option<usize>, Option<(usize, usize)>),
    ) {
        let (nth, offset, hl) = selection;
        self.backtrack.count = nth;
        if let Some(Overlay::Transcript(t)) = &mut self.overlay {
            if let Some(off) = offset {
                t.set_scroll_offset(off);
            }
            t.set_highlight_range(hl);
        }
    }

    /// Forward any event to the overlay and close it if done.
    fn overlay_forward_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        if let Some(overlay) = &mut self.overlay {
            overlay.handle_event(tui, event)?;
            if overlay.is_done() {
                self.close_transcript_overlay(tui);
                tui.frame_requester().schedule_frame();
            }
        }
        Ok(())
    }

    /// Handle Enter in overlay backtrack preview: confirm selection and reset state.
    fn overlay_confirm_backtrack(&mut self, tui: &mut tui::Tui) {
        if let Some(base_id) = self.backtrack.base_id {
            let drop_last_messages = self.backtrack.count;
            let (lines, spans) = self.build_transcript_flattened();
            let prefill = backtrack_helpers::nth_last_user_text(&lines, &spans, drop_last_messages)
                .unwrap_or_default();
            self.close_transcript_overlay(tui);
            self.request_backtrack(prefill, base_id, drop_last_messages);
        }
        self.reset_backtrack_state();
    }

    /// Handle Esc in overlay backtrack preview: step selection if armed, else forward.
    fn overlay_step_backtrack(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        if self.backtrack.base_id.is_some() {
            self.step_backtrack_and_highlight(tui);
        } else {
            self.overlay_forward_event(tui, event)?;
        }
        Ok(())
    }

    /// Confirm a primed backtrack from the main view (no overlay visible).
    /// Computes the prefill from the selected user message and requests history.
    pub(crate) fn confirm_backtrack_from_main(&mut self) {
        if let Some(base_id) = self.backtrack.base_id {
            let drop_last_messages = self.backtrack.count;
            let (lines, spans) = self.build_transcript_flattened();
            let prefill = backtrack_helpers::nth_last_user_text(&lines, &spans, drop_last_messages)
                .unwrap_or_default();
            self.request_backtrack(prefill, base_id, drop_last_messages);
        }
        self.reset_backtrack_state();
    }

    /// Clear all backtrack-related state and composer hints.
    pub(crate) fn reset_backtrack_state(&mut self) {
        self.backtrack.primed = false;
        self.backtrack.base_id = None;
        self.backtrack.count = 0;
        // In case a hint is somehow still visible (e.g., race with overlay open/close).
        self.chat_widget.clear_esc_backtrack_hint();
    }

    /// Handle a ConversationHistory response while a backtrack is pending.
    /// If it matches the primed base session, fork and switch to the new conversation.
    pub(crate) async fn on_conversation_history_for_backtrack(
        &mut self,
        tui: &mut tui::Tui,
        ev: ConversationHistoryResponseEvent,
    ) -> Result<()> {
        if let Some((base_id, _, _)) = self.backtrack.pending.as_ref()
            && ev.conversation_id == *base_id
            && let Some((_, drop_count, prefill)) = self.backtrack.pending.take()
        {
            self.fork_and_switch_to_new_conversation(tui, ev, drop_count, prefill)
                .await;
        }
        Ok(())
    }

    /// Fork the conversation using provided history and switch UI/state accordingly.
    async fn fork_and_switch_to_new_conversation(
        &mut self,
        tui: &mut tui::Tui,
        ev: ConversationHistoryResponseEvent,
        drop_count: usize,
        prefill: String,
    ) {
        let cfg = self.chat_widget.config_ref().clone();
        // Perform the fork via a thin wrapper for clarity/testability.
        let result = self
            .perform_fork(ev.entries.clone(), drop_count, cfg.clone())
            .await;
        match result {
            Ok(new_conv) => {
                self.install_forked_conversation(tui, cfg, new_conv, drop_count, &prefill)
            }
            Err(e) => tracing::error!("error forking conversation: {e:#}"),
        }
    }

    /// Thin wrapper around ConversationManager::fork_conversation.
    async fn perform_fork(
        &self,
        entries: Vec<codex_protocol::models::ResponseItem>,
        drop_count: usize,
        cfg: codex_core::config::Config,
    ) -> codex_core::error::Result<codex_core::NewConversation> {
        self.server
            .fork_conversation(entries, drop_count, cfg)
            .await
    }

    /// Install a forked conversation into the ChatWidget and update UI to reflect selection.
    fn install_forked_conversation(
        &mut self,
        tui: &mut tui::Tui,
        cfg: codex_core::config::Config,
        new_conv: codex_core::NewConversation,
        drop_count: usize,
        prefill: &str,
    ) {
        let conv = new_conv.conversation;
        let session_configured = new_conv.session_configured;
        let init = crate::chatwidget::ChatWidgetInit {
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: self.enhanced_keys_supported,
        };
        self.chat_widget =
            crate::chatwidget::ChatWidget::new_from_existing(init, conv, session_configured);
        // Render transcript only up to the selected user message.
        self.render_transcript_up_to_backtrack(tui, drop_count);
        if !prefill.is_empty() {
            self.chat_widget.insert_str(prefill);
        }
        tui.frame_requester().schedule_frame();
    }

    /// Render only the prefix of the transcript up to the selected user message.
    fn render_transcript_up_to_backtrack(&mut self, tui: &mut tui::Tui, drop_count: usize) {
        let (lines, spans) = self.build_transcript_flattened();
        if let Some(cut_idx) =
            backtrack_helpers::find_nth_last_user_header_index(&spans, drop_count)
        {
            let prefix = lines.into_iter().take(cut_idx).collect::<Vec<_>>();
            if !prefix.is_empty() {
                tui.insert_history_lines(prefix);
            }
        } else if !lines.is_empty() {
            // If no cut index found (e.g., drop_count == 0), render nothing.
        }
    }

    /// Build flattened transcript lines and absolute user spans on demand.
    /// This replaces the previous persistent `transcript_lines`/`user_spans` state.
    pub(crate) fn build_transcript_flattened(&self) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
        let mut out: Vec<Line<'static>> = Vec::new();
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for cell in &self.transcript_cells {
            let is_stream = cell.is_stream_continuation();
            let mut lines = cell.transcript_lines();
            if !is_stream && !out.is_empty() && !lines.is_empty() {
                out.push("".into());
            }
            let start = out.len();
            if let Some(span) = cell.message_span()
                && matches!(cell.kind(), crate::history_cell::MessageKind::User)
            {
                let header_abs = start.saturating_add(span.header_offset);
                let end_abs = header_abs.saturating_add(1).saturating_add(span.body_len);
                spans.push((header_abs, end_abs));
            }
            out.append(&mut lines);
        }
        (out, spans)
    }
}
