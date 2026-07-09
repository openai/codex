//! Backtracking and transcript overlay event routing.
//!
//! This file owns backtrack mode (Esc/Enter navigation in the transcript overlay) and also
//! mediates a key rendering boundary for the transcript overlay.
//!
//! Overall goal: keep the main chat view and the transcript overlay in sync while allowing users
//! to edit a prompt from a completed turn. Confirming a selection forks through the immediately
//! preceding canonical turn, switches the TUI to the branch, and restores the selected prompt in
//! the composer without changing the source thread.
//!
//! Backtrack operates as a small state machine:
//! - The first `Esc` in the main view "primes" the feature and captures a base thread id.
//! - A subsequent `Esc` opens the transcript overlay (`Ctrl+T`) and highlights one user message
//!   for each completed canonical turn.
//! - `Enter` requests a fork before the highlighted turn and reopens its prompt for editing.
//!
//! The transcript overlay (`Ctrl+T`) renders committed transcript cells plus a render-only live
//! tail derived from the current in-flight `ChatWidget.active_cell`.
//!
//! That live tail is kept in sync during `TuiEvent::Draw` handling for `Overlay::Transcript` by
//! asking `ChatWidget` for an active-cell cache key and transcript lines and by passing them into
//! `TranscriptOverlay::sync_live_tail`. This preserves the invariant that the overlay reflects
//! both committed history and in-flight activity without changing flush or coalescing behavior.

use std::any::TypeId;
use std::sync::Arc;

use crate::app::App;
use crate::app_event::AppEvent;
use crate::bottom_pane::LocalImageAttachment;
use crate::chatwidget::UserMessage;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::Overlay;
use crate::tui;
use crate::tui::TuiEvent;
use codex_protocol::ThreadId;
use codex_protocol::models::local_image_label_text;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

const NO_PREVIOUS_MESSAGE_TO_EDIT: &str = "No previous message to edit.";
pub(crate) const SIDE_EDIT_PREVIOUS_UNAVAILABLE_MESSAGE: &str =
    "Editing previous prompts is unavailable in side conversations.";

/// Aggregates all backtrack-related state used by the App.
#[derive(Default)]
pub(crate) struct BacktrackState {
    /// True when Esc has primed backtrack mode in the main view.
    pub(crate) primed: bool,
    /// Session id of the source thread to fork.
    ///
    /// If the current thread changes, backtrack selections become invalid and must be ignored.
    pub(crate) base_id: Option<ThreadId>,
    /// Index of the currently highlighted completed turn.
    ///
    /// This is an index into the filtered "canonical turns since the last session start" view,
    /// not an index into `transcript_cells`. Multiple user messages in one turn (steers) share a
    /// single entry. `usize::MAX` indicates "no selection".
    pub(crate) nth_user_message: usize,
    /// True when the transcript overlay is showing a backtrack preview.
    pub(crate) overlay_preview_active: bool,
}

/// A completed canonical turn selected for editing on a source-preserving branch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BacktrackSelection {
    pub(crate) thread_id: ThreadId,
    /// Canonical turn immediately before the selected prompt, or `None` for the first prompt.
    pub(crate) last_turn_id: Option<String>,
    pub(crate) prompt: UserMessage,
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

    /// Open transcript overlay (enters alternate screen and shows full transcript).
    pub(crate) fn open_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.enter_alt_screen();
        self.overlay = Some(Overlay::new_transcript(
            self.transcript_cells.clone(),
            self.keymap.pager.clone(),
        ));
        tui.frame_requester().schedule_frame();
    }

    /// Close transcript overlay and restore normal UI.
    pub(crate) fn close_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.leave_alt_screen();
        let was_backtrack = self.backtrack.overlay_preview_active;
        if !self.deferred_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.deferred_history_lines);
            tui.insert_history_hyperlink_lines_with_wrap_policy(
                lines,
                self.history_line_wrap_policy(),
            );
        }
        self.overlay = None;
        self.backtrack.overlay_preview_active = false;
        tui.frame_requester().schedule_frame();
        if was_backtrack {
            // Ensure backtrack state is fully reset when overlay closes (e.g. via 'q').
            self.reset_backtrack_state();
        }
    }

    /// Initialize backtrack state and show composer hint.
    fn prime_backtrack(&mut self) {
        self.backtrack.primed = true;
        self.backtrack.nth_user_message = usize::MAX;
        self.backtrack.base_id = self.chat_widget.thread_id();
        if has_backtrack_target(&self.transcript_cells, self.chat_widget.active_turn_id()) {
            self.chat_widget.show_esc_backtrack_hint();
        }
    }

    /// Open overlay and begin backtrack preview flow (first step + highlight).
    fn open_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        if !has_backtrack_target(&self.transcript_cells, self.chat_widget.active_turn_id()) {
            self.reset_backtrack_state();
            self.chat_widget
                .add_info_message(NO_PREVIOUS_MESSAGE_TO_EDIT.to_string(), /*hint*/ None);
            tui.frame_requester().schedule_frame();
            return;
        }

        self.open_transcript_overlay(tui);
        self.backtrack.overlay_preview_active = true;
        // Composer is hidden by overlay; clear its hint.
        self.chat_widget.clear_esc_backtrack_hint();
        self.step_backtrack_and_highlight(tui);
    }

    /// When overlay is already open, begin preview mode and select latest user message.
    fn begin_overlay_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        if !has_backtrack_target(&self.transcript_cells, self.chat_widget.active_turn_id()) {
            self.close_transcript_overlay(tui);
            self.chat_widget
                .add_info_message(NO_PREVIOUS_MESSAGE_TO_EDIT.to_string(), /*hint*/ None);
            tui.frame_requester().schedule_frame();
            return;
        }

        self.backtrack.primed = true;
        self.backtrack.base_id = self.chat_widget.thread_id();
        self.backtrack.overlay_preview_active = true;
        let count = forkable_turn_count(&self.transcript_cells, self.chat_widget.active_turn_id());
        if let Some(last) = count.checked_sub(1) {
            self.apply_backtrack_selection_internal(last);
        }
        tui.frame_requester().schedule_frame();
    }

    /// Step selection to the next older completed turn and update overlay.
    fn step_backtrack_and_highlight(&mut self, tui: &mut tui::Tui) {
        let count = forkable_turn_count(&self.transcript_cells, self.chat_widget.active_turn_id());
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

    /// Step selection to the next newer completed turn and update overlay.
    fn step_forward_backtrack_and_highlight(&mut self, tui: &mut tui::Tui) {
        let count = forkable_turn_count(&self.transcript_cells, self.chat_widget.active_turn_id());
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
        let cell_idx = nth_forkable_turn_position(
            &self.transcript_cells,
            nth_user_message,
            self.chat_widget.active_turn_id(),
        );
        if let Some(cell_idx) = cell_idx {
            self.backtrack.nth_user_message = nth_user_message;
            if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                t.set_highlight_cell(Some(cell_idx));
            }
        } else {
            self.backtrack.nth_user_message = usize::MAX;
            if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                t.set_highlight_cell(/*cell*/ None);
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
        if matches!(&event, TuiEvent::Draw | TuiEvent::Resize)
            && let Some(Overlay::Transcript(t)) = &mut self.overlay
        {
            let active_key = self.chat_widget.active_cell_transcript_key();
            let chat_widget = &self.chat_widget;
            tui.draw(u16::MAX, |frame| {
                let width = frame.area().width.max(1);
                t.sync_live_tail(width, active_key, |w| {
                    chat_widget.active_cell_transcript_hyperlink_lines(w)
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

    /// Handle Enter in overlay backtrack preview: branch before the selection and reset state.
    fn overlay_confirm_backtrack(&mut self, tui: &mut tui::Tui) {
        let nth_user_message = self.backtrack.nth_user_message;
        let selection = self.backtrack_selection(nth_user_message);
        self.close_transcript_overlay(tui);
        if let Some(selection) = selection {
            self.request_backtrack_fork(selection);
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

    /// Attach a canonical turn id to the latest locally rendered prompt.
    pub(crate) fn anchor_latest_user_history_cell(&mut self, turn_id: String) {
        if turn_id.is_empty() {
            return;
        }
        let Some(index) = self
            .transcript_cells
            .iter()
            .rposition(|cell| cell.as_any().is::<UserHistoryCell>())
        else {
            return;
        };
        let Some(user_cell) = self.transcript_cells[index]
            .as_any()
            .downcast_ref::<UserHistoryCell>()
        else {
            return;
        };
        if user_cell.turn_id.is_some() {
            return;
        }
        let mut anchored_cell = user_cell.clone();
        anchored_cell.turn_id = Some(turn_id);
        self.transcript_cells[index] = Arc::new(anchored_cell);
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.replace_cells(self.transcript_cells.clone());
        }
    }

    pub(crate) fn apply_backtrack_selection(
        &mut self,
        tui: &mut tui::Tui,
        selection: BacktrackSelection,
    ) {
        self.request_backtrack_fork(selection);
        tui.frame_requester().schedule_frame();
    }

    fn request_backtrack_fork(&mut self, selection: BacktrackSelection) {
        if self.chat_widget.side_conversation_active() {
            self.reset_backtrack_state();
            self.chat_widget
                .add_error_message(SIDE_EDIT_PREVIOUS_UNAVAILABLE_MESSAGE.to_string());
            return;
        }

        if self.chat_widget.thread_id() != Some(selection.thread_id) {
            return;
        }

        self.app_event_tx.send(AppEvent::ForkSessionForPromptEdit {
            thread_id: selection.thread_id,
            last_turn_id: selection.last_turn_id,
            prompt: selection.prompt,
        });
    }

    pub(crate) fn restore_backtrack_prompt_after_branch_error(
        &mut self,
        prompt: UserMessage,
        err: impl std::fmt::Display,
    ) {
        self.chat_widget.restore_user_message_to_composer(prompt);
        self.chat_widget.add_error_message(format!(
            "Failed to branch before the selected prompt: {err}"
        ));
    }

    fn backtrack_selection(&self, nth_user_message: usize) -> Option<BacktrackSelection> {
        let base_id = self.backtrack.base_id?;
        if self.chat_widget.thread_id() != Some(base_id) {
            return None;
        }

        let mut positions =
            forkable_turn_positions_iter(&self.transcript_cells, self.chat_widget.active_turn_id());
        let (previous_position, selected_position) = if nth_user_message == 0 {
            (None, positions.next()?)
        } else {
            (
                positions.nth(nth_user_message.saturating_sub(1)),
                positions.next()?,
            )
        };
        let last_turn_id = previous_position
            .and_then(|idx| self.transcript_cells.get(idx))
            .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
            .and_then(|cell| cell.turn_id.clone());
        let selected = self.transcript_cells[selected_position]
            .as_any()
            .downcast_ref::<UserHistoryCell>()?;
        let local_images = selected
            .local_image_paths
            .iter()
            .enumerate()
            .map(|(idx, path)| LocalImageAttachment {
                placeholder: local_image_label_text(idx + 1),
                path: path.clone(),
            })
            .collect();

        Some(BacktrackSelection {
            thread_id: base_id,
            last_turn_id,
            prompt: UserMessage {
                text: selected.message.clone(),
                local_images,
                remote_image_urls: selected.remote_image_urls.clone(),
                text_elements: selected.text_elements.clone(),
                mention_bindings: Vec::new(),
            },
        })
    }
}

fn has_backtrack_target(
    cells: &[Arc<dyn crate::history_cell::HistoryCell>],
    active_turn_id: Option<&str>,
) -> bool {
    forkable_turn_positions_iter(cells, active_turn_id)
        .next()
        .is_some()
}

fn forkable_turn_count(
    cells: &[Arc<dyn crate::history_cell::HistoryCell>],
    active_turn_id: Option<&str>,
) -> usize {
    forkable_turn_positions_iter(cells, active_turn_id).count()
}

fn nth_forkable_turn_position(
    cells: &[Arc<dyn crate::history_cell::HistoryCell>],
    nth: usize,
    active_turn_id: Option<&str>,
) -> Option<usize> {
    forkable_turn_positions_iter(cells, active_turn_id).nth(nth)
}

fn forkable_turn_positions_iter<'a>(
    cells: &'a [Arc<dyn crate::history_cell::HistoryCell>],
    active_turn_id: Option<&'a str>,
) -> impl Iterator<Item = usize> + 'a {
    let session_start_type = TypeId::of::<SessionInfoCell>();
    let type_of = |cell: &Arc<dyn crate::history_cell::HistoryCell>| cell.as_any().type_id();
    let start = cells
        .iter()
        .rposition(|cell| type_of(cell) == session_start_type)
        .map_or(0, |idx| idx + 1);
    let mut seen_turn_ids = std::collections::HashSet::new();

    cells
        .iter()
        .enumerate()
        .skip(start)
        .filter_map(move |(idx, cell)| {
            let turn_id = cell
                .as_any()
                .downcast_ref::<UserHistoryCell>()?
                .turn_id
                .as_deref()?;
            (!turn_id.is_empty()
                && active_turn_id != Some(turn_id)
                && seen_turn_ids.insert(turn_id))
            .then_some(idx)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use pretty_assertions::assert_eq;
    use ratatui::prelude::Line;
    use std::sync::Arc;

    fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    fn user_cell(turn_id: &str, message: &str) -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            turn_id: Some(turn_id.to_string()),
            message: message.to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        })
    }

    fn synthetic_user_cell(message: &str) -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            turn_id: None,
            message: message.to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        })
    }

    #[test]
    fn forkable_turns_group_user_messages_and_steers_by_turn_id() {
        let cells = vec![
            user_cell("turn-1", "initial prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("working")],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>,
            user_cell("turn-1", "steer"),
            user_cell("turn-2", "next prompt"),
        ];

        let positions: Vec<usize> =
            forkable_turn_positions_iter(&cells, /*active_turn_id*/ None).collect();

        assert_eq!(positions, vec![0, 3]);
        assert_eq!(forkable_turn_count(&cells, /*active_turn_id*/ None), 2);
    }

    #[test]
    fn forkable_turns_exclude_active_turn_but_keep_terminal_prefix() {
        let cells = vec![
            user_cell("turn-1", "completed prompt"),
            user_cell("turn-2", "in-progress prompt"),
        ];

        assert_eq!(forkable_turn_count(&cells, Some("turn-2")), 1);
        assert_eq!(
            nth_forkable_turn_position(&cells, /*nth*/ 0, Some("turn-2")),
            Some(0)
        );
        assert_eq!(
            nth_forkable_turn_position(&cells, /*nth*/ 1, Some("turn-2")),
            None
        );
    }

    #[test]
    fn forkable_turns_exclude_legacy_synthetic_turns() {
        let cells = vec![
            synthetic_user_cell("legacy prompt"),
            user_cell("turn-1", "canonical prompt"),
        ];

        let positions: Vec<usize> =
            forkable_turn_positions_iter(&cells, /*active_turn_id*/ None).collect();

        assert_eq!(positions, vec![1]);
    }

    #[test]
    fn backtrack_target_requires_user_message() {
        let mut cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant")],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>,
            Arc::new(crate::history_cell::new_info_event(
                "Context compacted".to_string(),
                /*hint*/ None,
            )) as Arc<dyn HistoryCell>,
        ];

        assert!(!has_backtrack_target(&cells, /*active_turn_id*/ None));

        cells.push(Arc::new(UserHistoryCell {
            turn_id: Some("turn-1".to_string()),
            message: "hello".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>);

        assert!(has_backtrack_target(&cells, /*active_turn_id*/ None));
    }

    #[test]
    fn backtrack_unavailable_info_message_snapshot() {
        let cell = crate::history_cell::new_info_event(
            NO_PREVIOUS_MESSAGE_TO_EDIT.to_string(),
            /*hint*/ None,
        );
        let rendered = render_lines(&cell.display_lines(/*width*/ 80)).join("\n");

        insta::assert_snapshot!(rendered);
    }
}
