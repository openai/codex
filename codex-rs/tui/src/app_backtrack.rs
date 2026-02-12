//! Backtracking and transcript overlay event routing.
//!
//! This file owns backtrack mode (Esc/Enter navigation in the transcript overlay) and also
//! mediates a key rendering boundary for the transcript overlay.
//!
//! Overall goal: keep the main chat view and the transcript overlay in sync while allowing
//! users to "rewind" to an earlier user message. We stage a rollback request, wait for core to
//! confirm it, then trim the local transcript to the matching history boundary. This avoids UI
//! state diverging from the agent if a rollback fails or targets a different thread.
//!
//! Backtrack operates as a small state machine:
//! - The first `Esc` in the main view "primes" the feature and captures a base thread id.
//! - A subsequent `Esc` opens the transcript overlay (`Ctrl+T`) and highlights a user message.
//! - `Enter` requests a rollback from core and records a `pending_rollback` guard.
//! - On `EventMsg::ThreadRolledBack`, we either finish an in-flight backtrack request or queue a
//!   rollback trim so it runs in event order with transcript inserts.
//!
//! The transcript overlay (`Ctrl+T`) renders committed transcript cells plus a render-only live
//! tail derived from the current in-flight `ChatWidget.active_cell`.
//!
//! That live tail is kept in sync during `TuiEvent::Draw` handling for `Overlay::Transcript` by
//! asking `ChatWidget` for an active-cell cache key and transcript lines and by passing them into
//! `TranscriptOverlay::sync_live_tail`. This preserves the invariant that the overlay reflects
//! both committed history and in-flight activity without changing flush or coalescing behavior.

use std::any::TypeId;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app::App;
use crate::app_event::AppEvent;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::Overlay;
use crate::tui;
use crate::tui::TuiEvent;
use codex_core::protocol::CodexErrorInfo;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::ThreadId;
use codex_protocol::user_input::TextElement;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

/// Aggregates all backtrack-related state used by the App.
#[derive(Default)]
pub(crate) struct BacktrackState {
    /// True when Esc has primed backtrack mode in the main view.
    pub(crate) primed: bool,
    /// Session id of the base thread to rollback.
    ///
    /// If the current thread changes, backtrack selections become invalid and must be ignored.
    pub(crate) base_id: Option<ThreadId>,
    /// Index of the currently highlighted user message.
    ///
    /// This is an index into the filtered "user messages since the last session start" view,
    /// not an index into `transcript_cells`. `usize::MAX` indicates "no selection".
    pub(crate) nth_user_message: usize,
    /// True when the transcript overlay is showing a backtrack preview.
    pub(crate) overlay_preview_active: bool,
    /// Pending rollback request awaiting confirmation from core.
    ///
    /// This acts as a guardrail: once we request a rollback, we block additional backtrack
    /// submissions until core responds with either a success or failure event.
    pub(crate) pending_rollback: Option<PendingBacktrackRollback>,
}

/// A user-visible backtrack choice that can be confirmed into a rollback request.
#[derive(Debug, Clone)]
pub(crate) struct BacktrackSelection {
    /// The selected user message, counted from the most recent session start.
    ///
    /// This value is used both to compute the rollback depth and to trim the local transcript
    /// after core confirms the rollback.
    pub(crate) nth_user_message: usize,
    /// Composer prefill derived from the selected user message.
    ///
    /// This is applied immediately on selection confirmation; if the rollback fails, the prefill
    /// remains as a convenience so the user can retry or edit.
    pub(crate) prefill: String,
    /// Text elements associated with the selected user message.
    pub(crate) text_elements: Vec<TextElement>,
    /// Local image paths associated with the selected user message.
    pub(crate) local_image_paths: Vec<PathBuf>,
}

/// An in-flight rollback requested from core.
///
/// We keep enough information to apply the corresponding local trim only if the response targets
/// the same active thread we issued the request for.
#[derive(Debug, Clone)]
pub(crate) struct PendingBacktrackRollback {
    pub(crate) selection: BacktrackSelection,
    pub(crate) thread_id: Option<ThreadId>,
}

impl App {
    /// Route overlay events while the transcript overlay is active.
    ///
    /// If backtrack preview is active, Esc / Left steps selection, Right steps forward, Enter
    /// confirms. Otherwise, Esc begins preview mode and all other events are forwarded to the
    /// overlay.
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
                    code: KeyCode::Left,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => {
                    self.overlay_step_backtrack(tui, event)?;
                    Ok(true)
                }
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Right,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => {
                    self.overlay_step_backtrack_forward(tui, event)?;
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
        if !self.chat_widget.composer_is_empty() {
            return;
        }

        if !self.backtrack.primed {
            self.prime_backtrack();
        } else if self.overlay.is_none() {
            self.open_backtrack_preview(tui);
        } else if self.backtrack.overlay_preview_active {
            self.step_backtrack_and_highlight(tui);
        }
    }

    /// Stage a backtrack and request thread history from the agent.
    ///
    /// We send the rollback request immediately, but we only mutate the transcript after core
    /// confirms success so the UI cannot get ahead of the actual thread state.
    ///
    /// The composer prefill is applied immediately as a UX convenience; it does not imply that
    /// core has accepted the rollback.
    pub(crate) fn apply_backtrack_rollback(&mut self, selection: BacktrackSelection) {
        let user_total = user_count(&self.transcript_cells);
        if user_total == 0 {
            return;
        }

        if self.backtrack.pending_rollback.is_some() {
            self.chat_widget
                .add_error_message("Backtrack rollback already in progress.".to_string());
            return;
        }

        let num_turns = user_total.saturating_sub(selection.nth_user_message);
        let num_turns = u32::try_from(num_turns).unwrap_or(u32::MAX);
        if num_turns == 0 {
            return;
        }

        let prefill = selection.prefill.clone();
        let text_elements = selection.text_elements.clone();
        let local_image_paths = selection.local_image_paths.clone();
        self.backtrack.pending_rollback = Some(PendingBacktrackRollback {
            selection,
            thread_id: self.chat_widget.thread_id(),
        });
        self.chat_widget.submit_op(Op::ThreadRollback { num_turns });
        if !prefill.is_empty() || !text_elements.is_empty() || !local_image_paths.is_empty() {
            self.chat_widget
                .set_composer_text(prefill, text_elements, local_image_paths);
        }
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
            let width = tui.terminal.last_known_screen_size.width;
            for cell in &self.transcript_cells {
                tui.insert_history_lines(cell.display_lines(width));
            }
        }
    }

    /// Initialize backtrack state and show composer hint.
    fn prime_backtrack(&mut self) {
        self.backtrack.primed = true;
        self.backtrack.nth_user_message = usize::MAX;
        self.backtrack.base_id = self.chat_widget.thread_id();
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
        self.backtrack.base_id = self.chat_widget.thread_id();
        self.backtrack.overlay_preview_active = true;
        let count = user_count(&self.transcript_cells);
        if let Some(last) = count.checked_sub(1) {
            self.apply_backtrack_selection_internal(last);
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

        self.apply_backtrack_selection_internal(next_selection);
        tui.frame_requester().schedule_frame();
    }

    /// Step selection to the next newer user message and update overlay.
    fn step_forward_backtrack_and_highlight(&mut self, tui: &mut tui::Tui) {
        let count = user_count(&self.transcript_cells);
        if count == 0 {
            return;
        }

        let last_index = count.saturating_sub(1);
        let next_selection = if self.backtrack.nth_user_message == usize::MAX {
            last_index
        } else {
            self.backtrack
                .nth_user_message
                .saturating_add(1)
                .min(last_index)
        };

        self.apply_backtrack_selection_internal(next_selection);
        tui.frame_requester().schedule_frame();
    }

    /// Apply a computed backtrack selection to the overlay and internal counter.
    fn apply_backtrack_selection_internal(&mut self, nth_user_message: usize) {
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

    /// Forwards an event to the overlay and closes it if done.
    ///
    /// The transcript overlay draw path is special because the overlay should match the main
    /// viewport while the active cell is still streaming or mutating.
    ///
    /// `TranscriptOverlay` owns committed transcript cells, while `ChatWidget` owns the current
    /// in-flight active cell (often a coalesced exec/tool group). During draws we append that
    /// in-flight cell as a cached, render-only live tail so `Ctrl+T` does not appear to "lose" tool
    /// calls until a later flush boundary.
    ///
    /// This logic lives here (instead of inside the overlay widget) because `ChatWidget` is the
    /// source of truth for the active cell and its cache invalidation key, and because `App` owns
    /// overlay lifecycle and frame scheduling for animations.
    fn overlay_forward_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        if let TuiEvent::Draw = &event
            && let Some(Overlay::Transcript(t)) = &mut self.overlay
        {
            let active_key = self.chat_widget.active_cell_transcript_key();
            let chat_widget = &self.chat_widget;
            tui.draw(u16::MAX, |frame| {
                let width = frame.area().width.max(1);
                t.sync_live_tail(width, active_key, |w| {
                    chat_widget.active_cell_transcript_lines(w)
                });
                t.render(frame.area(), frame.buffer);
            })?;
            let close_overlay = t.is_done();
            if !close_overlay
                && active_key.is_some_and(|key| key.animation_tick.is_some())
                && t.is_scrolled_to_bottom()
            {
                tui.frame_requester()
                    .schedule_frame_in(std::time::Duration::from_millis(50));
            }
            if close_overlay {
                self.close_transcript_overlay(tui);
                tui.frame_requester().schedule_frame();
            }
            return Ok(());
        }

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
        let selection = self.backtrack_selection(nth_user_message);
        self.close_transcript_overlay(tui);
        if let Some(selection) = selection {
            self.apply_backtrack_rollback(selection);
            tui.frame_requester().schedule_frame();
        }
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

    /// Handle Right in overlay backtrack preview: step selection forward if armed, else forward.
    fn overlay_step_backtrack_forward(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<()> {
        if self.backtrack.base_id.is_some() {
            self.step_forward_backtrack_and_highlight(tui);
        } else {
            self.overlay_forward_event(tui, event)?;
        }
        Ok(())
    }

    /// Confirm a primed backtrack from the main view (no overlay visible).
    /// Computes the prefill from the selected user message for rollback.
    pub(crate) fn confirm_backtrack_from_main(&mut self) -> Option<BacktrackSelection> {
        let selection = self.backtrack_selection(self.backtrack.nth_user_message);
        self.reset_backtrack_state();
        selection
    }

    /// Clear all backtrack-related state and composer hints.
    pub(crate) fn reset_backtrack_state(&mut self) {
        self.backtrack.primed = false;
        self.backtrack.base_id = None;
        self.backtrack.nth_user_message = usize::MAX;
        // In case a hint is somehow still visible (e.g., race with overlay open/close).
        self.chat_widget.clear_esc_backtrack_hint();
    }

    pub(crate) fn apply_backtrack_selection(
        &mut self,
        tui: &mut tui::Tui,
        selection: BacktrackSelection,
    ) {
        self.apply_backtrack_rollback(selection);
        tui.frame_requester().schedule_frame();
    }

    pub(crate) fn handle_backtrack_event(&mut self, event: &EventMsg) {
        match event {
            EventMsg::ThreadRolledBack(rollback) => {
                // `pending_rollback` is set only after this UI sends `Op::ThreadRollback`
                // from the backtrack flow. In that case, finish immediately using the
                // stored selection (nth user message) so local trim matches the exact
                // backtrack target.
                //
                // When it is `None`, rollback came from replay or another source. We
                // queue an AppEvent so rollback trim runs in FIFO order with
                // `InsertHistoryCell` events, avoiding races with in-flight transcript
                // inserts.
                if self.backtrack.pending_rollback.is_some() {
                    self.finish_pending_backtrack();
                } else {
                    self.app_event_tx.send(AppEvent::ApplyThreadRollback {
                        num_turns: rollback.num_turns,
                    });
                }
            }
            EventMsg::Error(ErrorEvent {
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                ..
            }) => {
                // Core rejected the rollback; clear the guard so the user can retry.
                self.backtrack.pending_rollback = None;
            }
            _ => {}
        }
    }

    /// Apply rollback semantics for `ThreadRolledBack` events where this TUI does not have an
    /// in-flight backtrack request (`pending_rollback` is `None`).
    ///
    /// Returns `true` when local transcript state changed.
    pub(crate) fn apply_non_pending_thread_rollback(&mut self, num_turns: u32) -> bool {
        if !trim_transcript_cells_drop_last_n_user_turns(&mut self.transcript_cells, num_turns) {
            return false;
        }
        self.sync_overlay_after_transcript_trim();
        self.backtrack_render_pending = true;
        true
    }

    /// Finish a pending rollback by applying the local trim and scheduling a scrollback refresh.
    ///
    /// We ignore events that do not correspond to the currently active thread to avoid applying
    /// stale updates after a session switch.
    fn finish_pending_backtrack(&mut self) {
        let Some(pending) = self.backtrack.pending_rollback.take() else {
            return;
        };
        if pending.thread_id != self.chat_widget.thread_id() {
            // Ignore rollbacks targeting a prior thread.
            return;
        }
        if trim_transcript_cells_to_nth_user(
            &mut self.transcript_cells,
            pending.selection.nth_user_message,
        ) {
            self.sync_overlay_after_transcript_trim();
            self.backtrack_render_pending = true;
        }
    }

    fn backtrack_selection(&self, nth_user_message: usize) -> Option<BacktrackSelection> {
        let base_id = self.backtrack.base_id?;
        if self.chat_widget.thread_id() != Some(base_id) {
            return None;
        }

        let (prefill, text_elements, local_image_paths) =
            nth_user_position(&self.transcript_cells, nth_user_message)
                .and_then(|idx| self.transcript_cells.get(idx))
                .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
                .map(|cell| {
                    (
                        cell.message.clone(),
                        cell.text_elements.clone(),
                        cell.local_image_paths.clone(),
                    )
                })
                .unwrap_or_else(|| (String::new(), Vec::new(), Vec::new()));

        Some(BacktrackSelection {
            nth_user_message,
            prefill,
            text_elements,
            local_image_paths,
        })
    }

    /// Keep transcript-related UI state aligned after `transcript_cells` was trimmed.
    ///
    /// This does three things:
    /// 1. If transcript overlay is open, replace its committed cells so removed turns disappear.
    /// 2. If backtrack preview is active, clamp/recompute the highlighted user selection.
    /// 3. Drop deferred transcript lines buffered while overlay was open to avoid flushing lines
    ///    for cells that were just removed by the trim.
    fn sync_overlay_after_transcript_trim(&mut self) {
        if let Some(Overlay::Transcript(t)) = &mut self.overlay {
            t.replace_cells(self.transcript_cells.clone());
        }
        if self.backtrack.overlay_preview_active {
            let total_users = user_count(&self.transcript_cells);
            let next_selection = if total_users == 0 {
                usize::MAX
            } else {
                self.backtrack
                    .nth_user_message
                    .min(total_users.saturating_sub(1))
            };
            self.apply_backtrack_selection_internal(next_selection);
        }
        // While overlay is open, we buffer rendered history lines and flush them on close.
        // If rollback trimmed cells meanwhile, those buffered lines can reference removed turns.
        self.deferred_history_lines.clear();
    }
}

fn trim_transcript_cells_to_nth_user(
    transcript_cells: &mut Vec<Arc<dyn crate::history_cell::HistoryCell>>,
    nth_user_message: usize,
) -> bool {
    if nth_user_message == usize::MAX {
        return false;
    }

    if let Some(cut_idx) = nth_user_position(transcript_cells, nth_user_message) {
        let original_len = transcript_cells.len();
        transcript_cells.truncate(cut_idx);
        return transcript_cells.len() != original_len;
    }
    false
}

pub(crate) fn trim_transcript_cells_drop_last_n_user_turns(
    transcript_cells: &mut Vec<Arc<dyn crate::history_cell::HistoryCell>>,
    num_turns: u32,
) -> bool {
    if num_turns == 0 {
        return false;
    }

    let user_positions: Vec<usize> = user_positions_iter(transcript_cells).collect();
    let Some(&first_user_idx) = user_positions.first() else {
        return false;
    };

    let turns_from_end = usize::try_from(num_turns).unwrap_or(usize::MAX);
    let cut_idx = if turns_from_end >= user_positions.len() {
        first_user_idx
    } else {
        user_positions[user_positions.len() - turns_from_end]
    };
    let original_len = transcript_cells.len();
    transcript_cells.truncate(cut_idx);
    transcript_cells.len() != original_len
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
    let session_start_type = TypeId::of::<SessionInfoCell>();
    let user_type = TypeId::of::<UserHistoryCell>();
    let type_of = |cell: &Arc<dyn crate::history_cell::HistoryCell>| cell.as_any().type_id();

    let start = cells
        .iter()
        .rposition(|cell| type_of(cell) == session_start_type)
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
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
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
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
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
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(vec![Line::from("between")], false))
                as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "second".to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
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

    #[test]
    fn trim_drop_last_n_user_turns_applies_rollback_semantics() {
        let mut cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(UserHistoryCell {
                message: "first".to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(
                vec![Line::from("after first")],
                false,
            )) as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "second".to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(
                vec![Line::from("after second")],
                false,
            )) as Arc<dyn HistoryCell>,
        ];

        let changed = trim_transcript_cells_drop_last_n_user_turns(&mut cells, 1);

        assert!(changed);
        assert_eq!(cells.len(), 2);
        let first_user = cells[0]
            .as_any()
            .downcast_ref::<UserHistoryCell>()
            .expect("first user");
        assert_eq!(first_user.message, "first");
    }

    #[test]
    fn trim_drop_last_n_user_turns_allows_overflow() {
        let mut cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(AgentMessageCell::new(vec![Line::from("intro")], true))
                as Arc<dyn HistoryCell>,
            Arc::new(UserHistoryCell {
                message: "first".to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
            }) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(vec![Line::from("after")], false))
                as Arc<dyn HistoryCell>,
        ];

        let changed = trim_transcript_cells_drop_last_n_user_turns(&mut cells, u32::MAX);

        assert!(changed);
        assert_eq!(cells.len(), 1);
        let intro = cells[0]
            .as_any()
            .downcast_ref::<AgentMessageCell>()
            .expect("intro agent");
        let intro_lines = intro.display_lines(u16::MAX);
        let intro_text: String = intro_lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(intro_text, "• intro");
    }
}
