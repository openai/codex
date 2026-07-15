//! Backtracking and transcript overlay event routing.
//!
//! This file owns backtrack mode (Esc/Enter navigation in the transcript overlay) and also
//! mediates a key rendering boundary for the transcript overlay.
//!
//! Overall goal: keep the main chat view and the transcript overlay in sync while allowing users
//! to edit an earlier prompt on a source-preserving branch. Confirming a selection forks before
//! the selected turn and restores its prompt in the new composer.
//!
//! Backtrack operates as a small state machine:
//! - The first `Esc` in the main view "primes" the feature and captures a base thread id.
//! - A subsequent `Esc` opens the transcript overlay (`Ctrl+T`) and highlights a user message when
//!   there is a prompt to reuse.
//! - `Enter` requests a fork before the selected prompt and reopens it for editing.
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
use crate::chatwidget::ChatWidget;
use crate::chatwidget::UserMessage;
use crate::chatwidget::mention_bindings_from_user_inputs;
#[cfg(test)]
use crate::history_cell::AgentMessageCell;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::Overlay;
use crate::tui;
use crate::tui::TuiEvent;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use codex_protocol::models::local_image_label_text;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;
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
    /// Session id of the base thread whose transcript is being inspected.
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
}

/// A user-visible backtrack choice that can be reopened on a source-preserving branch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BacktrackSelection {
    pub(crate) thread_id: ThreadId,
    /// The selected user message, counted from the most recent session start.
    pub(crate) nth_user_message: usize,
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

    /// Request a source-preserving branch before the selected prompt.
    pub(crate) fn apply_backtrack_selection(&mut self, selection: BacktrackSelection) {
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
            nth_user_message: selection.nth_user_message,
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
        if has_backtrack_target(&self.transcript_cells) {
            self.chat_widget.show_esc_backtrack_hint();
        }
    }

    /// Open overlay and begin backtrack preview flow (first step + highlight).
    fn open_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        if !has_backtrack_target(&self.transcript_cells) {
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
        if !has_backtrack_target(&self.transcript_cells) {
            self.close_transcript_overlay(tui);
            self.chat_widget
                .add_info_message(NO_PREVIOUS_MESSAGE_TO_EDIT.to_string(), /*hint*/ None);
            tui.frame_requester().schedule_frame();
            return;
        }

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

    /// Handle Enter in overlay backtrack preview: confirm selection and reset state.
    fn overlay_confirm_backtrack(&mut self, tui: &mut tui::Tui) {
        let nth_user_message = self.backtrack.nth_user_message;
        let selection = self.backtrack_selection(nth_user_message);
        self.close_transcript_overlay(tui);
        if let Some(selection) = selection {
            self.apply_backtrack_selection(selection);
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
    /// Computes the prompt state from the selected user message.
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

    fn backtrack_selection(&self, nth_user_message: usize) -> Option<BacktrackSelection> {
        let base_id = self.backtrack.base_id?;
        if self.chat_widget.thread_id() != Some(base_id) {
            return None;
        }

        let selected = nth_user_position(&self.transcript_cells, nth_user_message)
            .and_then(|idx| self.transcript_cells.get(idx))
            .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())?;
        let local_images = selected
            .local_image_paths
            .iter()
            .enumerate()
            .map(|(index, path)| LocalImageAttachment {
                placeholder: local_image_label_text(index + 1),
                path: path.clone(),
            })
            .collect();

        Some(BacktrackSelection {
            thread_id: base_id,
            nth_user_message,
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

/// Find the persisted turn that contains a selected transcript prompt.
///
/// Replay hides review prompts and other display-empty inputs, so the selected ordinal must be
/// resolved against the same visible projection before restoring its canonical mention bindings.
///
/// A turn can contain multiple user messages when it was steered. Only its initial prompt can be
/// reopened independently because app-server cannot fork in the middle of a turn.
pub(crate) fn backtrack_fork_before_turn_id(
    turns: &[Turn],
    nth_user_message: usize,
    prompt: &mut UserMessage,
) -> Result<Option<String>> {
    let mut visible_user_messages_seen = 0_usize;
    let mut review_mode = false;
    for (turn_index, turn) in turns.iter().enumerate() {
        let hidden_nested_review_turn = turn_index
            .checked_sub(/*rhs*/ 1)
            .and_then(|index| turns.get(index))
            .is_some_and(|previous| is_hidden_nested_review_turn(previous, turn));
        let mut user_messages_in_turn = 0_usize;
        for item in &turn.items {
            let content = match item {
                ThreadItem::EnteredReviewMode { .. } => {
                    review_mode = true;
                    continue;
                }
                ThreadItem::ExitedReviewMode { .. } => {
                    review_mode = false;
                    continue;
                }
                ThreadItem::UserMessage { content, .. } => content,
                _ => continue,
            };
            let is_steer = user_messages_in_turn > 0;
            user_messages_in_turn = user_messages_in_turn.saturating_add(/*rhs*/ 1);
            if review_mode {
                continue;
            }

            let display = ChatWidget::user_message_display_from_inputs(content);
            if hidden_nested_review_turn {
                continue;
            }
            if display.message.trim().is_empty()
                && display.text_elements.is_empty()
                && display.local_images.is_empty()
                && display.remote_image_urls.is_empty()
            {
                continue;
            }
            if visible_user_messages_seen != nth_user_message {
                visible_user_messages_seen =
                    visible_user_messages_seen.saturating_add(/*rhs*/ 1);
                continue;
            }

            if is_steer {
                bail!("the selected prompt is a steer and cannot be branched independently");
            }
            if matches!(turn.status, TurnStatus::InProgress) {
                bail!("the selected prompt belongs to a turn that is still in progress");
            }

            let selected_local_images = prompt.local_images.iter().map(|image| &image.path);
            if prompt.text != display.message
                || prompt.text_elements != display.text_elements
                || prompt.remote_image_urls != display.remote_image_urls
                || !selected_local_images.eq(display.local_images.iter())
            {
                bail!("the selected transcript prompt no longer matches the persisted thread");
            }
            prompt.mention_bindings = mention_bindings_from_user_inputs(content, &display.message);

            return Ok((turn_index > 0).then(|| turn.id.clone()));
        }
    }

    bail!("the selected prompt was not found in the persisted thread")
}

/// Returns whether a turn is the reconstructed inline-review child with duplicated prompt inputs.
pub(crate) fn is_hidden_nested_review_turn(previous: &Turn, turn: &Turn) -> bool {
    if previous.status != TurnStatus::Completed
        || turn.status != TurnStatus::Interrupted
        || turn.completed_at.is_some()
        || !previous
            .items
            .iter()
            .any(|item| matches!(item, ThreadItem::EnteredReviewMode { .. }))
        || !previous
            .items
            .iter()
            .any(|item| matches!(item, ThreadItem::ExitedReviewMode { .. }))
    {
        return false;
    }

    let mut user_messages = turn.items.iter().filter_map(|item| match item {
        ThreadItem::UserMessage { content, .. } => Some(content),
        _ => None,
    });
    matches!(
        (
            user_messages.next(),
            user_messages.next(),
            user_messages.next(),
        ),
        (Some(first), Some(second), None) if first == second
    )
}

pub(crate) fn user_count(cells: &[Arc<dyn crate::history_cell::HistoryCell>]) -> usize {
    user_positions_iter(cells).count()
}

fn has_backtrack_target(cells: &[Arc<dyn crate::history_cell::HistoryCell>]) -> bool {
    user_count(cells) > 0
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
fn agent_group_count(cells: &[Arc<dyn crate::history_cell::HistoryCell>]) -> usize {
    agent_group_positions_iter(cells).count()
}

#[cfg(test)]
fn agent_group_positions_iter(
    cells: &[Arc<dyn crate::history_cell::HistoryCell>],
) -> impl Iterator<Item = usize> + '_ {
    let session_start_type = TypeId::of::<SessionInfoCell>();
    let type_of = |cell: &Arc<dyn crate::history_cell::HistoryCell>| cell.as_any().type_id();

    let start = cells
        .iter()
        .rposition(|cell| type_of(cell) == session_start_type)
        .map_or(0, |idx| idx + 1);

    cells
        .iter()
        .enumerate()
        .skip(start)
        .filter_map(move |(idx, cell)| {
            let is_agent = cell.as_any().downcast_ref::<AgentMessageCell>().is_some();
            let is_copy_source_group = is_agent && !cell.is_stream_continuation();
            is_copy_source_group.then_some(idx)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bottom_pane::MentionBinding;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use codex_app_server_protocol::UserInput;
    use pretty_assertions::assert_eq;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
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

    fn turn(turn_id: &str, status: TurnStatus, user_messages: usize) -> Turn {
        Turn {
            id: turn_id.to_string(),
            items: (0..user_messages)
                .map(|index| ThreadItem::UserMessage {
                    id: format!("user-{index}"),
                    client_id: None,
                    content: vec![UserInput::Text {
                        text: format!("{turn_id}-prompt-{index}"),
                        text_elements: Vec::new(),
                    }],
                })
                .collect(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }

    fn prompt(text: &str) -> UserMessage {
        UserMessage {
            text: text.to_string(),
            local_images: Vec::new(),
            remote_image_urls: Vec::new(),
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        }
    }

    #[test]
    fn backtrack_fork_before_turn_id_resolves_first_and_later_prompts() {
        let turns = vec![
            turn("turn-1", TurnStatus::Completed, /*user_messages*/ 1),
            turn(
                "turn-compaction",
                TurnStatus::Completed,
                /*user_messages*/ 0,
            ),
            turn("turn-2", TurnStatus::Completed, /*user_messages*/ 1),
        ];

        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 0,
                &mut prompt("turn-1-prompt-0"),
            )
            .expect("first prompt should resolve"),
            None
        );
        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 1,
                &mut prompt("turn-2-prompt-0"),
            )
            .expect("later prompt should resolve"),
            Some("turn-2".to_string())
        );
    }

    #[test]
    fn backtrack_fork_before_turn_id_rejects_mid_turn_steers() {
        let turns = vec![turn(
            "turn-1",
            TurnStatus::Completed,
            /*user_messages*/ 2,
        )];

        let error = backtrack_fork_before_turn_id(
            &turns,
            /*nth_user_message*/ 1,
            &mut prompt("turn-1-prompt-1"),
        )
        .expect_err("a steer cannot be branched independently");

        assert_eq!(
            error.to_string(),
            "the selected prompt is a steer and cannot be branched independently"
        );
    }

    #[test]
    fn backtrack_fork_before_turn_id_rejects_in_progress_and_missing_prompts() {
        let turns = vec![turn(
            "turn-1",
            TurnStatus::InProgress,
            /*user_messages*/ 1,
        )];

        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 0,
                &mut prompt("turn-1-prompt-0"),
            )
            .expect_err("in-progress prompt cannot be branched")
            .to_string(),
            "the selected prompt belongs to a turn that is still in progress"
        );
        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 1,
                &mut prompt("missing prompt"),
            )
            .expect_err("missing prompt cannot be branched")
            .to_string(),
            "the selected prompt was not found in the persisted thread"
        );

        let completed_turns = vec![turn(
            "turn-1",
            TurnStatus::Completed,
            /*user_messages*/ 1,
        )];
        assert_eq!(
            backtrack_fork_before_turn_id(
                &completed_turns,
                /*nth_user_message*/ 0,
                &mut prompt("different prompt"),
            )
            .expect_err("a stale transcript prompt cannot be branched")
            .to_string(),
            "the selected transcript prompt no longer matches the persisted thread"
        );
    }

    #[test]
    fn backtrack_fork_before_turn_id_skips_hidden_review_prompts() {
        let mut review_turn = turn(
            "turn-review",
            TurnStatus::Completed,
            /*user_messages*/ 1,
        );
        review_turn.items.insert(
            /*index*/ 0,
            ThreadItem::EnteredReviewMode {
                id: "review-start".to_string(),
                review: "changes against main".to_string(),
            },
        );
        review_turn.items.push(ThreadItem::ExitedReviewMode {
            id: "review-end".to_string(),
            review: "review complete".to_string(),
        });
        let turns = vec![
            turn("turn-1", TurnStatus::Completed, /*user_messages*/ 1),
            review_turn,
            turn("turn-2", TurnStatus::Completed, /*user_messages*/ 1),
        ];

        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 1,
                &mut prompt("turn-2-prompt-0"),
            )
            .expect("the visible prompt after review should resolve"),
            Some("turn-2".to_string())
        );
    }

    #[test]
    fn backtrack_fork_before_turn_id_skips_hidden_nested_review_prompts() {
        let review_hint = "current changes";
        let review_prompt =
            "Review the current code changes (staged, unstaged, and untracked files).";
        let review_turn = Turn {
            items: vec![
                ThreadItem::EnteredReviewMode {
                    id: "review-start".to_string(),
                    review: review_hint.to_string(),
                },
                ThreadItem::ExitedReviewMode {
                    id: "review-end".to_string(),
                    review: "review complete".to_string(),
                },
            ],
            ..turn(
                "turn-review",
                TurnStatus::Completed,
                /*user_messages*/ 0,
            )
        };
        let review_child_turn = Turn {
            items: (0..2)
                .map(|index| ThreadItem::UserMessage {
                    id: format!("review-prompt-{index}"),
                    client_id: None,
                    content: vec![UserInput::Text {
                        text: review_prompt.to_string(),
                        text_elements: Vec::new(),
                    }],
                })
                .collect(),
            ..turn(
                "turn-review-child",
                TurnStatus::Interrupted,
                /*user_messages*/ 0,
            )
        };
        let interrupted_steered_turn = Turn {
            items: review_child_turn.items.clone(),
            completed_at: Some(1),
            ..turn(
                "turn-interrupted-steer",
                TurnStatus::Interrupted,
                /*user_messages*/ 0,
            )
        };
        assert!(!is_hidden_nested_review_turn(
            &review_turn,
            &interrupted_steered_turn,
        ));
        let turns = vec![
            review_turn,
            review_child_turn,
            turn("turn-2", TurnStatus::Completed, /*user_messages*/ 1),
        ];

        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 0,
                &mut prompt("turn-2-prompt-0"),
            )
            .expect("the visible prompt after a nested review should resolve"),
            Some("turn-2".to_string())
        );
    }

    #[test]
    fn backtrack_fork_before_turn_id_restores_canonical_mention_bindings() {
        let mut selected_turn = turn("turn-2", TurnStatus::Completed, /*user_messages*/ 1);
        selected_turn.items = vec![ThreadItem::UserMessage {
            id: "selected-prompt".to_string(),
            client_id: None,
            content: vec![
                UserInput::Text {
                    text: "use $skill @sample $google-calendar".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "skill".to_string(),
                    path: PathBuf::from("/tmp/skills/skill/SKILL.md"),
                },
                UserInput::Mention {
                    name: "Sample Plugin".to_string(),
                    path: "plugin://sample@test".to_string(),
                },
                UserInput::Mention {
                    name: "Google Calendar".to_string(),
                    path: "app://google_calendar".to_string(),
                },
            ],
        }];
        let turns = vec![
            turn("turn-1", TurnStatus::Completed, /*user_messages*/ 1),
            selected_turn,
        ];
        let mut selected_prompt = prompt("use $skill @sample $google-calendar");

        assert_eq!(
            backtrack_fork_before_turn_id(
                &turns,
                /*nth_user_message*/ 1,
                &mut selected_prompt,
            )
            .expect("the selected prompt should resolve"),
            Some("turn-2".to_string())
        );
        assert_eq!(
            selected_prompt.mention_bindings,
            vec![
                MentionBinding {
                    sigil: '$',
                    mention: "skill".to_string(),
                    path: "/tmp/skills/skill/SKILL.md".to_string(),
                },
                MentionBinding {
                    sigil: '@',
                    mention: "sample".to_string(),
                    path: "plugin://sample@test".to_string(),
                },
                MentionBinding {
                    sigil: '$',
                    mention: "google-calendar".to_string(),
                    path: "app://google_calendar".to_string(),
                },
            ]
        );
    }

    #[test]
    fn agent_group_count_ignores_context_compacted_marker() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(AgentMessageCell::new(
                vec![Line::from("first")],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>,
            Arc::new(crate::history_cell::new_info_event(
                "Context compacted".to_string(),
                /*hint*/ None,
            )) as Arc<dyn HistoryCell>,
            Arc::new(AgentMessageCell::new(
                vec![Line::from("second")],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>,
        ];

        assert_eq!(agent_group_count(&cells), 2);
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

        assert!(!has_backtrack_target(&cells));

        cells.push(Arc::new(UserHistoryCell {
            message: "hello".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>);

        assert!(has_backtrack_target(&cells));
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
