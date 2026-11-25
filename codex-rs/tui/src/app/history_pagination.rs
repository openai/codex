//! Load bounded history pages into the transcript before updating its owned or inline viewport.

use std::collections::HashSet;
use std::ops::Range;

use super::*;
use crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT;
use crate::app_server_session::thread_items_page_params;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::is_session_history_prefix_cell;
use crate::pager_overlay::TranscriptHistoryState;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::TranscriptCells;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadItemsListResponse;

#[path = "history_completion.rs"]
mod completion;

const SCROLLBACK_REFILL_MAX_WAIT: Duration = Duration::from_secs(5);

impl App {
    /// Start one bounded page request shared by scrollback refill and the transcript overlay.
    pub(crate) fn request_older_history_page(
        &self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> bool {
        let Some(cursor) = app_server.begin_older_history_page(thread_id) else {
            return false;
        };
        tracing::debug!(
            %thread_id,
            %cursor,
            overlay = self.overlay.is_some(),
            "loading older transcript history page"
        );
        let request_id = app_server.next_request_id();
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = request_handle
                .request_typed::<ThreadItemsListResponse>(ClientRequest::ThreadItemsList {
                    request_id,
                    params: thread_items_page_params(
                        thread_id,
                        /*turn_id*/ None,
                        Some(cursor.clone()),
                        HISTORY_ITEM_PAGE_LIMIT,
                    ),
                })
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::OlderThreadHistoryLoaded {
                thread_id,
                cursor,
                result,
            });
        });
        true
    }

    pub(super) async fn handle_older_history_page(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        cursor: &str,
        result: Result<ThreadItemsListResponse, String>,
    ) -> Result<()> {
        if !app_server.is_older_history_page_pending(thread_id, cursor) {
            return Ok(());
        }
        // The widget can already belong to the destination while the transcript still replays.
        let parked = self.chat_widget.thread_id() != Some(thread_id);
        if self.awaiting_side_replay(tui)
            || (parked && !self.side_companions.contains_key(&thread_id))
            || (!parked && tui.is_owned_screen() && !self.scrollback_has_older_history)
        {
            app_server.cancel_older_history_page(thread_id, cursor);
            return Ok(());
        }
        let page = result.map_err(|err| color_eyre::eyre::eyre!(err))?;
        let Some(store) = self
            .thread_event_channels
            .get(&thread_id)
            .map(|channel| Arc::clone(&channel.store))
        else {
            app_server.cancel_older_history_page(thread_id, cursor);
            return Ok(());
        };
        let (cwd, mut turns) = {
            let store = store.lock().await;
            (
                store
                    .session
                    .as_ref()
                    .map(|session| session.cwd.to_path_buf())
                    .unwrap_or_else(|| self.config.cwd.to_path_buf()),
                store.turns.clone(),
            )
        };
        let items = app_server
            .apply_older_history_page(thread_id, cursor, page, &mut turns)
            .await?;
        let hidden_item_ids = hidden_review_item_ids(&turns);
        let visibility = if self.config.show_raw_agent_reasoning {
            RawReasoningVisibility::Visible
        } else {
            RawReasoningVisibility::Hidden
        };
        let width = tui.terminal.last_known_screen_size.width;
        self.remove_hidden_review_cells(tui, &turns, &hidden_item_ids, thread_id, &cwd, visibility);
        let split_turn = items
            .last()
            .and_then(|(turn_id, last)| {
                turns.iter().find(|turn| {
                    turn.id == *turn_id
                        && matches!(turn.status, TurnStatus::Completed | TurnStatus::Interrupted)
                        && turn.items.last().is_some_and(|item| item.id() != last.id())
                })
            })
            .filter(|_| self.local_settings.transcript_mode.is_owned());
        let existing = self
            .side_companions
            .get(&thread_id)
            .filter(|_| parked)
            .map_or(self.transcript_cells.as_slice(), |pane| {
                pane.cells.as_slice()
            });
        let split_insertion = split_image_insertion(existing, split_turn);
        let (mut cells, split_images) = self.project_older_history_cells(
            items,
            &turns,
            &hidden_item_ids,
            thread_id,
            &cwd,
            visibility,
            split_insertion.is_some(),
        );
        for cell in &mut cells {
            if let Some(cell) = Arc::get_mut(cell) {
                self.prepare_collab_history(app_server, cell);
            }
        }
        if let Some(pane) = self.side_companions.get_mut(&thread_id).filter(|_| parked) {
            let images = split_insertion.and_then(|index| {
                if split_images.is_empty() {
                    return None;
                }
                let inserted = index..index + split_images.len();
                pane.splice_older_cells(index..index, split_images);
                Some(inserted)
            });
            let index = history_prefix_len(&pane.cells);
            let inserted = index..index + cells.len();
            pane.splice_older_cells(index..index, cells);
            pane.view.history_page_loaded(&pane.cells, inserted, images);
            pane.view.history = if app_server.has_older_history(thread_id) {
                TranscriptHistoryState::Partial
            } else {
                TranscriptHistoryState::Complete
            };
            merge_older_turns(&mut store.lock().await.turns, turns);
            tui.frame_requester().schedule_frame();
            return Ok(());
        }
        let images = split_insertion.and_then(|index| {
            if split_images.is_empty() {
                return None;
            }
            let inserted = index..index + split_images.len();
            if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
                overlay.insert_history_page(index, split_images.clone());
            }
            self.transcript_cells.splice(index..index, split_images);
            Some(inserted)
        });
        let inserted = self.prepend_older_transcript_cells(cells);
        self.transcript_view
            .history_page_loaded(&self.transcript_cells, inserted.clone(), images);
        if !inserted.is_empty() {
            self.join_older_activity_group(inserted.end, &turns);
        }
        merge_older_turns(&mut store.lock().await.turns, turns);
        self.scrollback_has_older_history = app_server.has_older_history(thread_id);
        if self.backtrack.overlay_preview_active
            && self.backtrack.nth_user_message == usize::MAX
            && !self.scrollback_has_older_history
        {
            self.cancel_transcript_browsing(tui);
            self.chat_widget.add_info_message(
                "No previous message to edit.".to_string(),
                /*hint*/ None,
            );
        }

        if tui.is_owned_screen() {
            self.finish_owned_history_page(tui, app_server, thread_id);
            return Ok(());
        }
        self.finish_inline_history_page(tui, app_server, thread_id, width);
        Ok(())
    }

    /// Remove prompts revealed as internal review input by an older page's review marker.
    fn remove_hidden_review_cells(
        &mut self,
        tui: &mut tui::Tui,
        turns: &[Turn],
        hidden_item_ids: &HashSet<(&str, &str)>,
        thread_id: ThreadId,
        cwd: &std::path::Path,
        visibility: RawReasoningVisibility,
    ) {
        if hidden_item_ids.is_empty() {
            return;
        }
        let user_items = turns
            .iter()
            .flat_map(|turn| {
                turn.item_payloads()
                    .map(move |item| (turn.id.as_str(), item))
            })
            .filter(|(_, item)| matches!(item, ThreadItemPayload::UserMessage { .. }))
            .map(|(turn_id, item)| (hidden_item_ids.contains(&(turn_id, item.id())), item))
            .collect::<Vec<_>>();
        let projected_user_cells = thread_items_to_transcript_cells(
            Some(thread_id),
            cwd,
            user_items.iter().map(|(_, item)| (*item).clone()),
            visibility,
            Some(&self.config),
        );
        let persisted_user_cells = user_items
            .into_iter()
            .zip(projected_user_cells)
            .map(|((hidden, _), cell)| (hidden, cell))
            .collect();
        if self.chat_widget.thread_id() != Some(thread_id)
            && let Some(pane) = self.side_companions.get_mut(&thread_id)
        {
            for index in hidden_transcript_indices(&pane.cells, persisted_user_cells) {
                pane.splice_older_cells(index..index + 1, Vec::new());
                pane.view.restart_search();
            }
            return;
        }
        let indices = hidden_transcript_indices(&self.transcript_cells, persisted_user_cells);
        if indices.is_empty() {
            return;
        }
        if self.backtrack.overlay_preview_active {
            let selected_index = crate::app_backtrack::nth_user_position(
                &self.transcript_cells,
                self.backtrack.nth_user_message,
            );
            if selected_index.is_some_and(|selected| indices.contains(&selected)) {
                self.cancel_transcript_browsing(tui);
            } else {
                let removed_visible_users = indices
                    .iter()
                    .filter(|&&index| {
                        selected_index.is_some_and(|selected| index < selected)
                            && crate::app_backtrack::user_count(std::slice::from_ref(
                                &self.transcript_cells[index],
                            )) != 0
                    })
                    .count();
                self.backtrack.nth_user_message = self
                    .backtrack
                    .nth_user_message
                    .saturating_sub(removed_visible_users);
            }
        }
        for index in indices {
            self.transcript_cells.remove(index);
        }
        self.native_history.retain(&self.transcript_cells);
        self.transcript_view.restart_search();
        self.transcript_view
            .history_loaded(&self.transcript_cells, 0..0);
        if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
            overlay.replace_cells(self.transcript_cells.clone());
        }
    }

    /// Project each terminal row on the page containing the end of its turn.
    #[allow(
        clippy::too_many_arguments,
        reason = "Projection needs the thread context and whether its split turn has a rendered end."
    )]
    fn project_older_history_cells(
        &mut self,
        items: Vec<(String, ThreadItemPayload)>,
        turns: &[Turn],
        hidden_item_ids: &HashSet<(&str, &str)>,
        thread_id: ThreadId,
        cwd: &std::path::Path,
        visibility: RawReasoningVisibility,
        relocate_split_images: bool,
    ) -> (TranscriptCells, TranscriptCells) {
        let cloud = matches!(self.app_server_target, AppServerTarget::Cloud)
            && self.chat_widget.thread_id() == Some(thread_id);
        let mut unfinished = HashSet::new();
        if cloud {
            for (turn_id, item) in items.iter().rev().filter(|(turn, item)| {
                matches!(item, ThreadItemPayload::AgentMessage { .. })
                    && !hidden_item_ids.contains(&(turn.as_str(), item.id()))
            }) {
                if let Some(turn) = turns.iter().find(|turn| turn.id == *turn_id)
                    && let Some(entry) = turn.items.iter().find(|entry| entry.id() == item.id())
                    && let Some(key) =
                        self.chat_widget
                            .remember_older_cloud_reply(turn_id, entry, &turn.status)
                {
                    unfinished.insert(key);
                }
            }
        }
        let mut cells = Vec::new();
        let mut split_images = Vec::new();
        let project = |items: &[(String, ThreadItemPayload)]| {
            thread_items_to_transcript_cells(
                Some(thread_id),
                cwd,
                items.iter().map(|(_, item)| item.clone()),
                visibility,
                Some(&self.config),
            )
        };
        for (range, turn_end) in completion::group_turn_items(&items, turns) {
            let start = cells.len();
            // Internal prompts stay invisible, but still separate adjacent tool groups.
            for visible in items[range]
                .split(|(turn_id, item)| hidden_item_ids.contains(&(turn_id.as_str(), item.id())))
            {
                for chunk in visible.split_inclusive(|(turn, item)| {
                    unfinished.contains(&(turn.clone(), item.id().to_owned()))
                }) {
                    let anchor = chunk.last().and_then(|(turn, item)| {
                        let key = (turn.clone(), item.id().to_owned());
                        unfinished.get(&key).cloned()
                    });
                    let split = chunk.len() - usize::from(anchor.is_some());
                    cells.extend(project(&chunk[..split]));
                    if let Some(key) = anchor {
                        let mut cell =
                            project(&chunk[split..])
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| {
                                    Arc::new(history_cell::PendingAgentMessageCell(key.clone()))
                                });
                        if let Some(markdown) = Arc::get_mut(&mut cell).and_then(|cell| {
                            cell.as_any_mut()
                                .downcast_mut::<history_cell::AgentMarkdownCell>()
                        }) {
                            markdown.interrupted_item = Some(key);
                        }
                        cells.push(cell);
                    }
                }
            }
            let Some(turn) = turn_end else {
                if relocate_split_images {
                    split_images.extend(
                        cells.extract_if(start.., |cell| generated_image(cell.as_ref()).is_some()),
                    );
                }
                continue;
            };
            if self.local_settings.transcript_mode.is_owned()
                && turn.status != TurnStatus::InProgress
            {
                cells[start..].sort_by_key(|cell| generated_image(cell.as_ref()).is_some());
            }
            if turn.status == TurnStatus::Interrupted && !self.side_threads.contains_key(&thread_id)
            {
                cells.push(Arc::new(history_cell::InterruptionNotice(Some(
                    turn.id.clone(),
                ))));
            }
            if turn.status == TurnStatus::Completed
                && let Some(completion) = if let Some(pane) =
                    self.side_companions.get_mut(&thread_id)
                    && self.chat_widget.thread_id() != Some(thread_id)
                {
                    history_cell::FinalMessageSeparator::from_turn_once(
                        turn,
                        &mut pane.display.rendered_completion_turn_ids,
                        /*fallback_elapsed*/ None,
                        /*fallback_completed_at*/ None,
                    )
                } else {
                    self.chat_widget
                        .completion_cell(turn, Some(ReplayKind::ResumeInitialMessages))
                }
            {
                cells.push(Arc::new(completion));
            }
        }
        for cell in cells
            .iter_mut()
            .chain(&mut split_images)
            .filter_map(Arc::get_mut)
        {
            self.link_viewed_image(cell);
        }
        (cells, split_images)
    }

    /// Insert each page once, keeping session headers and any backtrack selection in place.
    fn prepend_older_transcript_cells(&mut self, cells: Vec<Arc<dyn HistoryCell>>) -> Range<usize> {
        if self.backtrack.overlay_preview_active {
            let added_prompts = crate::app_backtrack::user_count(&cells);
            self.backtrack.nth_user_message = if self.backtrack.nth_user_message == usize::MAX {
                added_prompts.checked_sub(1).unwrap_or(usize::MAX)
            } else {
                self.backtrack
                    .nth_user_message
                    .saturating_add(added_prompts)
            };
        }
        let index = if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
            overlay.prepend(cells.clone())
        } else {
            history_prefix_len(&self.transcript_cells)
        };
        let inserted = index..index + cells.len();
        self.transcript_cells.splice(index..index, cells);
        inserted
    }

    /// Continue explicit history traversal without rewriting native scrollback.
    fn finish_owned_history_page(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let previous = self.transcript_view.history;
        self.transcript_view.history = if self.scrollback_has_older_history {
            TranscriptHistoryState::Partial
        } else {
            TranscriptHistoryState::Complete
        };
        let continue_to_start = previous == TranscriptHistoryState::LoadingBeginning;
        if continue_to_start && !self.scrollback_has_older_history {
            if self.transcript_view.is_select_mode() {
                self.transcript_view
                    .complete_select_beginning(&self.transcript_cells);
            } else {
                self.transcript_view
                    .jump_to_beginning(&self.transcript_cells);
            }
        }
        if self.scrollback_has_older_history
            && (continue_to_start
                || self.browsing_needs_history()
                || self.transcript_view.needs_history(&self.transcript_cells))
            && self.request_older_history_page(app_server, thread_id)
        {
            self.transcript_view.history = if continue_to_start {
                TranscriptHistoryState::LoadingBeginning
            } else {
                TranscriptHistoryState::LoadingOlder
            };
        }
        if self.backtrack.overlay_preview_active {
            self.apply_backtrack_selection_internal(self.backtrack.nth_user_message);
        }
        tui.frame_requester().schedule_frame();
    }

    /// Update the overlay per page and batch native scrollback replays.
    fn finish_inline_history_page(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        width: u16,
    ) {
        let mut continue_to_start = false;
        if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
            let previous_state = overlay.set_history_state(if self.scrollback_has_older_history {
                TranscriptHistoryState::Partial
            } else {
                TranscriptHistoryState::Complete
            });
            continue_to_start = previous_state == TranscriptHistoryState::LoadingBeginning
                && self.scrollback_has_older_history;
            // Finish a deferred refill if the overlay opened between pages.
            if self.transcript_reflow.refill_deadline.take().is_some() {
                self.schedule_immediate_resize_reflow(tui);
            }
        } else {
            let wrap_width = self.chat_widget.history_wrap_width(width);
            let rendered_rows = self
                .render_transcript_lines_for_reflow(wrap_width)
                .lines
                .len();
            if self.scrollback_history_needs_top_up(rendered_rows)
                && self.request_older_history_page(app_server, thread_id)
            {
                // Bound the wait from the first buffered page, even if later requests stall.
                if self.transcript_reflow.refill_deadline.is_none() {
                    self.transcript_reflow.refill_deadline =
                        Some(Instant::now() + SCROLLBACK_REFILL_MAX_WAIT);
                    tui.frame_requester()
                        .schedule_frame_in(SCROLLBACK_REFILL_MAX_WAIT);
                }
                return;
            }
            self.transcript_reflow.refill_deadline = None;
            self.schedule_immediate_resize_reflow(tui);
        }
        if continue_to_start
            && self.request_older_history_page(app_server, thread_id)
            && let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut()
        {
            overlay.set_history_state(TranscriptHistoryState::LoadingBeginning);
        }
        if self.backtrack.overlay_preview_active {
            self.apply_backtrack_selection_internal(self.backtrack.nth_user_message);
        }
        tui.frame_requester().schedule_frame();
    }
}

fn generated_image(cell: &dyn HistoryCell) -> Option<&history_cell::ImageGenerationHistoryCell> {
    cell.as_any()
        .downcast_ref::<history_cell::ImageGenerationHistoryCell>()
        .filter(|image| image.is_turn_output())
}

/// The first page can contain the answer while an older page contains its image.
fn split_image_insertion(
    existing: &[Arc<dyn HistoryCell>],
    split_turn: Option<&Turn>,
) -> Option<usize> {
    let turn = split_turn?;
    let end = existing.iter().position(|cell| {
        let cell = cell.as_any();
        cell.downcast_ref::<history_cell::FinalMessageSeparator>()
            .is_some_and(|cell| cell.turn_id.as_deref() == Some(turn.id.as_str()))
            || cell
                .downcast_ref::<history_cell::InterruptionNotice>()
                .is_some_and(|cell| cell.0.as_deref() == Some(turn.id.as_str()))
    })?;
    Some(
        existing[..end]
            .iter()
            .rposition(|cell| generated_image(cell.as_ref()).is_none())
            .map_or(0, |index| index + 1),
    )
}

/// Classify internal review prompts across page boundaries before rendering any older items.
pub(super) fn hidden_review_item_ids(turns: &[Turn]) -> HashSet<(&str, &str)> {
    let mut hidden = HashSet::new();
    let mut review_mode = false;
    for (index, turn) in turns.iter().enumerate() {
        let hidden_nested_review_turn = index
            .checked_sub(/*rhs*/ 1)
            .and_then(|previous| turns.get(previous))
            .is_some_and(|previous| {
                crate::app_backtrack::is_hidden_nested_review_turn(previous, turn)
            });
        for item in &turn.items {
            match &item.payload {
                ThreadItemPayload::EnteredReviewMode { .. } => review_mode = true,
                ThreadItemPayload::ExitedReviewMode { .. } => review_mode = false,
                ThreadItemPayload::UserMessage { .. }
                    if review_mode || hidden_nested_review_turn =>
                {
                    hidden.insert((turn.id.as_str(), item.id()));
                }
                _ => {}
            }
        }
    }
    hidden
}

fn history_prefix_len(cells: &[Arc<dyn HistoryCell>]) -> usize {
    cells
        .iter()
        .rposition(|cell| is_session_history_prefix_cell(cell.as_ref()))
        .map_or(/*default*/ 0, |index| index + 1)
}

/// Match repeated user messages from newest to oldest so identical visible prompts stay distinct.
fn hidden_transcript_indices(
    cells: &[Arc<dyn HistoryCell>],
    mut persisted_user_cells: Vec<(bool, Arc<dyn HistoryCell>)>,
) -> Vec<usize> {
    let mut indices = Vec::new();
    for (index, cell) in cells.iter().enumerate().rev() {
        let Some(user_cell) = cell.as_any().downcast_ref::<UserHistoryCell>() else {
            continue;
        };
        let Some(position) = persisted_user_cells.iter().rposition(|(_, projected)| {
            projected
                .as_any()
                .downcast_ref::<UserHistoryCell>()
                .is_some_and(|projected| {
                    projected.message == user_cell.message
                        && projected.text_elements == user_cell.text_elements
                        && projected.local_image_paths == user_cell.local_image_paths
                        && projected.remote_image_urls == user_cell.remote_image_urls
                })
        }) else {
            continue;
        };
        if persisted_user_cells[position].0 {
            indices.push(index);
        }
        persisted_user_cells.truncate(position);
    }
    indices
}

/// Preserve events received while the request was in flight and deduplicate overlapping pages.
fn merge_older_turns(current_turns: &mut Vec<Turn>, mut older_turns: Vec<Turn>) {
    older_turns.retain_mut(|turn| {
        let Some(current) = current_turns
            .iter_mut()
            .find(|current| current.id == turn.id)
        else {
            return true;
        };
        let items = std::mem::take(&mut turn.items)
            .into_iter()
            .filter(|item| !current.items.iter().any(|known| known.id() == item.id()))
            .collect::<Vec<_>>();
        if current.items_view == codex_app_server_protocol::TurnItemsView::NotLoaded {
            current.items_view = turn.items_view;
        }
        current.items.splice(0..0, items);
        false
    });
    current_turns.splice(0..0, older_turns);
}

#[cfg(test)]
#[path = "history_pagination_tests.rs"]
mod tests;
