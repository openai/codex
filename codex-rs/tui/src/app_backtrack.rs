use std::any::TypeId;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app::App;
use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::Overlay;
use crate::tui;
use crate::tui::TuiEvent;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_protocol::ConversationId;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

/// Aggregates all backtrack-related state used by the App.
#[derive(Default)]
pub(crate) struct BacktrackState {
    /// True when Esc has primed backtrack mode in the main view.
    pub(crate) primed: bool,
    /// Session id of the base conversation to fork from.
    pub(crate) base_id: Option<ConversationId>,
    /// Index in the transcript of the last user message.
    pub(crate) nth_user_message: usize,
    /// True when the transcript overlay is showing a backtrack preview.
    pub(crate) overlay_preview_active: bool,
    /// Pending fork request: (base_id, nth_user_message, prefill).
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
        nth_user_message: usize,
    ) {
        self.backtrack.pending = Some((base_id, nth_user_message, prefill));
        self.app_event_tx.send(crate::app_event::AppEvent::CodexOp(
            codex_core::protocol::Op::GetPath,
        ));
    }

    /// Open transcript overlay (enters alternate screen and shows full transcript).
    pub(crate) fn open_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.enter_alt_screen();
        self.overlay = Some(Overlay::new_transcript(self.transcript_cells.clone()));
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

    /// Re-render the full transcript into the terminal scrollback in one call.
    /// Useful when switching sessions to ensure prior history remains visible.
    pub(crate) fn render_transcript_once(&mut self, tui: &mut tui::Tui) {
        if !self.transcript_cells.is_empty() {
            for cell in &self.transcript_cells {
                tui.insert_history_lines(cell.transcript_lines());
            }
        }
    }

    /// Initialize backtrack state and show composer hint.
    fn prime_backtrack(&mut self) {
        self.backtrack.primed = true;
        self.backtrack.nth_user_message = usize::MAX;
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
        let count = user_count(&self.transcript_cells);
        if let Some(last) = count.checked_sub(1) {
            self.apply_backtrack_selection(last);
        }
        tui.frame_requester().schedule_frame();
    }

    /// Step selection to the next older user message and update overlay.
    fn step_backtrack_and_highlight(&mut self, tui: &mut tui::Tui) {
        let count = user_count(&self.transcript_cells);
        if count == 0 {
            return;
        }

        let last_index = count.saturating_sub(1);
        let next_selection = if self.backtrack.nth_user_message == usize::MAX {
            last_index
        } else if self.backtrack.nth_user_message == 0 {
            0
        } else {
            self.backtrack
                .nth_user_message
                .saturating_sub(1)
                .min(last_index)
        };

        self.apply_backtrack_selection(next_selection);
        tui.frame_requester().schedule_frame();
    }

    /// Apply a computed backtrack selection to the overlay and internal counter.
    fn apply_backtrack_selection(&mut self, nth_user_message: usize) {
        if let Some(cell_idx) = nth_user_position(&self.transcript_cells, nth_user_message) {
            self.backtrack.nth_user_message = nth_user_message;
            if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                t.set_highlight_cell(Some(cell_idx));
            }
        } else {
            self.backtrack.nth_user_message = usize::MAX;
            if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                t.set_highlight_cell(None);
            }
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
        let nth_user_message = self.backtrack.nth_user_message;
        if let Some(base_id) = self.backtrack.base_id {
            let prefill = nth_user_position(&self.transcript_cells, nth_user_message)
                .and_then(|idx| self.transcript_cells.get(idx))
                .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
                .map(|c| c.message.clone())
                .unwrap_or_default();
            self.close_transcript_overlay(tui);
            self.request_backtrack(prefill, base_id, nth_user_message);
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
            let prefill =
                nth_user_position(&self.transcript_cells, self.backtrack.nth_user_message)
                    .and_then(|idx| self.transcript_cells.get(idx))
                    .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
                    .map(|c| c.message.clone())
                    .unwrap_or_default();
            self.request_backtrack(prefill, base_id, self.backtrack.nth_user_message);
        }
        self.reset_backtrack_state();
    }

    /// Clear all backtrack-related state and composer hints.
    pub(crate) fn reset_backtrack_state(&mut self) {
        self.backtrack.primed = false;
        self.backtrack.base_id = None;
        self.backtrack.nth_user_message = usize::MAX;
        // In case a hint is somehow still visible (e.g., race with overlay open/close).
        self.chat_widget.clear_esc_backtrack_hint();
    }

    /// Handle a ConversationHistory response while a backtrack is pending.
    /// If it matches the primed base session, fork and switch to the new conversation.
    pub(crate) async fn on_conversation_history_for_backtrack(
        &mut self,
        tui: &mut tui::Tui,
        ev: ConversationPathResponseEvent,
    ) -> Result<()> {
        if let Some((base_id, _, _)) = self.backtrack.pending.as_ref()
            && ev.conversation_id == *base_id
            && let Some((_, nth_user_message, prefill)) = self.backtrack.pending.take()
        {
            self.fork_and_switch_to_new_conversation(tui, ev, nth_user_message, prefill)
                .await;
        }
        Ok(())
    }

    /// Fork the conversation using provided history and switch UI/state accordingly.
    async fn fork_and_switch_to_new_conversation(
        &mut self,
        tui: &mut tui::Tui,
        ev: ConversationPathResponseEvent,
        nth_user_message: usize,
        prefill: String,
    ) {
        let cfg = self.chat_widget.config_ref().clone();
        // Perform the fork via a thin wrapper for clarity/testability.
        let result = self
            .perform_fork(ev.path.clone(), nth_user_message, cfg.clone())
            .await;
        match result {
            Ok(new_conv) => {
                self.install_forked_conversation(tui, cfg, new_conv, nth_user_message, &prefill)
            }
            Err(e) => tracing::error!("error forking conversation: {e:#}"),
        }
    }

    /// Thin wrapper around ConversationManager::fork_conversation.
    async fn perform_fork(
        &self,
        path: PathBuf,
        nth_user_message: usize,
        cfg: codex_core::config::Config,
    ) -> codex_core::error::Result<codex_core::NewConversation> {
        self.server
            .fork_conversation(nth_user_message, cfg, path)
            .await
    }

    /// Install a forked conversation into the ChatWidget and update UI to reflect selection.
    fn install_forked_conversation(
        &mut self,
        tui: &mut tui::Tui,
        cfg: codex_core::config::Config,
        new_conv: codex_core::NewConversation,
        nth_user_message: usize,
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
            auth_manager: self.auth_manager.clone(),
        };
        self.chat_widget =
            crate::chatwidget::ChatWidget::new_from_existing(init, conv, session_configured);
        // Trim transcript up to the selected user message and re-render it.
        self.trim_transcript_for_backtrack(nth_user_message);
        self.render_transcript_once(tui);
        if !prefill.is_empty() {
            self.chat_widget.set_composer_text(prefill.to_string());
        }
        tui.frame_requester().schedule_frame();
    }

    /// Trim transcript_cells to preserve only content up to the selected user message.
    fn trim_transcript_for_backtrack(&mut self, nth_user_message: usize) {
        trim_transcript_cells_to_nth_user(&mut self.transcript_cells, nth_user_message);
    }
}

fn trim_transcript_cells_to_nth_user(
    transcript_cells: &mut Vec<Arc<dyn crate::history_cell::HistoryCell>>,
    nth_user_message: usize,
) {
    if nth_user_message == usize::MAX {
        return;
    }

    if let Some(cut_idx) = nth_user_position(transcript_cells, nth_user_message) {
        transcript_cells.truncate(cut_idx);
    }
}

pub(crate) fn user_count(cells: &[Arc<dyn crate::history_cell::HistoryCell>]) -> usize {
    user_positions_iter(cells).count()
}

fn nth_user_position(
    cells: &[Arc<dyn crate::history_cell::HistoryCell>],
    nth: usize,
) -> Option<usize> {
    user_positions_iter(cells)
        .enumerate()
        .find_map(|(i, idx)| (i == nth).then_some(idx))
}

fn user_positions_iter(
    cells: &[Arc<dyn crate::history_cell::HistoryCell>],
) -> impl Iterator<Item = usize> + '_ {
    let header_type = TypeId::of::<CompositeHistoryCell>();
    let user_type = TypeId::of::<UserHistoryCell>();
    let type_of = |cell: &Arc<dyn crate::history_cell::HistoryCell>| cell.as_any().type_id();

    let start = cells
        .iter()
        .rposition(|cell| type_of(cell) == header_type)
        .map_or(0, |idx| idx + 1);

    cells
        .iter()
        .enumerate()
        .skip(start)
        .filter_map(move |(idx, cell)| (type_of(cell) == user_type).then_some(idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use ratatui::prelude::Line;
    use std::sync::Arc;

    #[test]
    fn trim_transcript_for_first_user_drops_user_and_newer_cells() {
        let mut cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(UserHistoryCell {
                message: "first user".to_string(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(vec![Line::from("assistant")], true))
                as Arc<dyn HistoryCell>,
        ];
        trim_transcript_cells_to_nth_user(&mut cells, 0);

        assert!(cells.is_empty());
    }

    #[test]
    fn trim_transcript_preserves_cells_before_selected_user() {
        let mut cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(AgentMessageCell::new(vec![Line::from("intro")], true))
                as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "first".to_string(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(vec![Line::from("after")], false))
                as Arc<dyn HistoryCell>,
        ];
        trim_transcript_cells_to_nth_user(&mut cells, 0);

        assert_eq!(cells.len(), 1);
        let agent = cells[0]
            .as_any()
            .downcast_ref::<AgentMessageCell>()
            .expect("agent cell");
        let agent_lines = agent.display_lines(u16::MAX);
        assert_eq!(agent_lines.len(), 1);
        let intro_text: String = agent_lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(intro_text, "• intro");
    }

    #[test]
    fn trim_transcript_for_later_user_keeps_prior_history() {
        let mut cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(AgentMessageCell::new(vec![Line::from("intro")], true))
                as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "first".to_string(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(vec![Line::from("between")], false))
                as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "second".to_string(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(vec![Line::from("tail")], false))
                as Arc<dyn HistoryCell>,
        ];
        trim_transcript_cells_to_nth_user(&mut cells, 1);

        assert_eq!(cells.len(), 3);
        let agent_intro = cells[0]
            .as_any()
            .downcast_ref::<AgentMessageCell>()
            .expect("intro agent");
        let intro_lines = agent_intro.display_lines(u16::MAX);
        let intro_text: String = intro_lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(intro_text, "• intro");

        let user_first = cells[1]
            .as_any()
            .downcast_ref::<UserHistoryCell>()
            .expect("first user");
        assert_eq!(user_first.message, "first");

        let agent_between = cells[2]
            .as_any()
            .downcast_ref::<AgentMessageCell>()
            .expect("between agent");
        let between_lines = agent_between.display_lines(u16::MAX);
        let between_text: String = between_lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(between_text, "  between");
    }
}
