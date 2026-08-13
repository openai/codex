//! Load older transcript pages without rewriting terminal-native scrollback.

use std::collections::HashSet;

use super::*;
use crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT;
use crate::app_server_session::thread_items_page_params;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::TranscriptHistoryState;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadItemsListResponse;

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
        if self.chat_widget.thread_id() != Some(thread_id) {
            app_server.cancel_older_history_page(thread_id);
            return Ok(());
        }
        let page = result.map_err(|err| color_eyre::eyre::eyre!(err))?;
        let Some(store) = self
            .thread_event_channels
            .get(&thread_id)
            .map(|channel| Arc::clone(&channel.store))
        else {
            app_server.cancel_older_history_page(thread_id);
            return Ok(());
        };
        let (cwd, mut turns) = {
            let store = store.lock().await;
            (
                store
                    .session
                    .as_ref()
                    .map_or_else(|| self.config.cwd.clone(), |session| session.cwd.clone()),
                store.turns.clone(),
            )
        };
        let mut items = app_server
            .apply_older_history_page(thread_id, cursor, page, &mut turns)
            .await?;
        let mut hidden_item_ids = HashSet::new();
        let mut review_mode = false;
        for (index, turn) in turns.iter().enumerate() {
            let hidden_nested_review_turn = index
                .checked_sub(/*rhs*/ 1)
                .and_then(|previous| turns.get(previous))
                .is_some_and(|previous| {
                    crate::app_backtrack::is_hidden_nested_review_turn(previous, turn)
                });
            for item in &turn.items {
                match item {
                    ThreadItem::EnteredReviewMode { .. } => review_mode = true,
                    ThreadItem::ExitedReviewMode { .. } => review_mode = false,
                    ThreadItem::UserMessage { .. } if review_mode || hidden_nested_review_turn => {
                        hidden_item_ids.insert(item.id());
                    }
                    _ => {}
                }
            }
        }
        let visibility = if self.config.show_raw_agent_reasoning {
            RawReasoningVisibility::Visible
        } else {
            RawReasoningVisibility::Hidden
        };
        let width = tui.terminal.last_known_screen_size.width;
        if !hidden_item_ids.is_empty() {
            let user_items = turns
                .iter()
                .flat_map(|turn| turn.items.iter())
                .filter(|item| matches!(item, ThreadItem::UserMessage { .. }))
                .map(|item| (item.id().to_string(), item.clone()))
                .collect::<Vec<_>>();
            let projected_user_cells = thread_items_to_transcript_cells(
                Some(thread_id),
                &cwd,
                user_items.iter().map(|(_, item)| item.clone()),
                visibility,
                Some(&self.config),
            );
            let mut persisted_user_cells = user_items
                .into_iter()
                .zip(projected_user_cells)
                .map(|((item_id, _), cell)| (item_id, cell))
                .collect::<Vec<_>>();
            let mut hidden_transcript_indices = Vec::new();
            for (index, cell) in self.transcript_cells.iter().enumerate().rev() {
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
                if hidden_item_ids.contains(persisted_user_cells[position].0.as_str()) {
                    hidden_transcript_indices.push(index);
                }
                persisted_user_cells.truncate(position);
            }
            if !hidden_transcript_indices.is_empty() {
                if self.backtrack.overlay_preview_active {
                    let selected_index = crate::app_backtrack::nth_user_position(
                        &self.transcript_cells,
                        self.backtrack.nth_user_message,
                    );
                    let removed_visible_users = hidden_transcript_indices
                        .iter()
                        .filter(|&&index| {
                            selected_index.is_some_and(|selected| index < selected)
                                && self.transcript_cells[index].desired_height(width) != 0
                        })
                        .count();
                    self.backtrack.nth_user_message = self
                        .backtrack
                        .nth_user_message
                        .saturating_sub(removed_visible_users);
                }
                for index in hidden_transcript_indices {
                    self.transcript_cells.remove(index);
                }
                if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
                    overlay.replace_cells(self.transcript_cells.clone());
                }
            }
        }
        items.retain(|item| !hidden_item_ids.contains(item.id()));
        {
            let mut store = store.lock().await;
            turns.retain_mut(|turn| {
                let Some(current) = store.turns.iter_mut().find(|current| current.id == turn.id)
                else {
                    return true;
                };
                let items = std::mem::take(&mut turn.items)
                    .into_iter()
                    .filter(|item| !current.items.iter().any(|known| known.id() == item.id()))
                    .collect::<Vec<_>>();
                current.items.splice(0..0, items);
                false
            });
            store.turns.splice(0..0, turns);
        }
        let cells = thread_items_to_transcript_cells(
            Some(thread_id),
            &cwd,
            items,
            visibility,
            Some(&self.config),
        );
        if self.backtrack.overlay_preview_active {
            self.backtrack.nth_user_message = self.backtrack.nth_user_message.saturating_add(
                cells
                    .iter()
                    .filter(|cell| {
                        cell.as_any().is::<UserHistoryCell>() && cell.desired_height(width) != 0
                    })
                    .count(),
            );
        }
        self.scrollback_has_older_history = app_server.has_older_history(thread_id);
        let mut continue_to_start = false;
        if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
            let index = overlay.prepend(cells.clone(), width);
            self.transcript_cells.splice(index..index, cells);
            let previous_state = overlay.set_history_state(if self.scrollback_has_older_history {
                TranscriptHistoryState::Partial
            } else {
                TranscriptHistoryState::Complete
            });
            continue_to_start = previous_state == TranscriptHistoryState::LoadingBeginning
                && self.scrollback_has_older_history;
        } else {
            let index = self
                .transcript_cells
                .iter()
                .rposition(|cell| cell.as_any().is::<SessionInfoCell>())
                .map_or(/*default*/ 0, |index| index.saturating_add(/*rhs*/ 1));
            self.transcript_cells.splice(index..index, cells);
            let wrap_width = self.chat_widget.history_wrap_width(width);
            let rendered_rows = self
                .render_transcript_lines_for_reflow(wrap_width)
                .lines
                .len();
            self.schedule_immediate_resize_reflow(tui);
            if self.scrollback_history_needs_top_up(rendered_rows)
                && self.request_older_history_page(app_server, thread_id)
            {
                return Ok(());
            }
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
        Ok(())
    }
}
