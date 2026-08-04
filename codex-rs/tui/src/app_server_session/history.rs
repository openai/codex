//! Bounded app-server transcript loading for resume, fork, and transcript views.

use super::AppServerSession;
use crate::history_cell::HistoryRenderMode;
use crate::legacy_core::config::Config;
use crate::resize_reflow_cap::resize_reflow_max_rows;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_to_transcript_cells;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;

pub(crate) const INITIAL_HISTORY_TURN_LIMIT: u32 = 5;
pub(crate) const HISTORY_ITEM_PAGE_LIMIT: u32 = 100;

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadHistoryPagination {
    pub(super) history_mode: ThreadHistoryMode,
    next_turn_cursor: Option<String>,
    next_item_cursor: Option<String>,
    loading_older: bool,
}

impl AppServerSession {
    pub(crate) fn has_older_history(&self, thread_id: ThreadId) -> bool {
        self.history_pagination
            .get(&thread_id)
            .is_some_and(|page| page.next_item_cursor.is_some())
    }

    pub(crate) fn begin_older_history_page(&mut self, thread_id: ThreadId) -> Option<String> {
        let page = self.history_pagination.get_mut(&thread_id)?;
        if page.loading_older {
            return None;
        }
        let cursor = page.next_item_cursor.clone()?;
        page.loading_older = true;
        Some(cursor)
    }

    pub(crate) fn cancel_older_history_page(&mut self, thread_id: ThreadId) {
        if let Some(page) = self.history_pagination.get_mut(&thread_id) {
            page.loading_older = false;
        }
    }

    pub(crate) async fn apply_older_history_page(
        &mut self,
        thread_id: ThreadId,
        cursor: &str,
        page: ThreadItemsListResponse,
        turns: &mut Vec<Turn>,
    ) -> Result<Vec<ThreadItem>> {
        let Some(mut state) = self.history_pagination.get(&thread_id).cloned() else {
            return Ok(Vec::new());
        };
        if !state.loading_older || state.next_item_cursor.as_deref() != Some(cursor) {
            return Ok(Vec::new());
        }
        let items = self
            .merge_thread_item_page(thread_id, page, &mut state, turns)
            .await?;
        state.loading_older = false;
        self.history_pagination.insert(thread_id, state);
        Ok(items)
    }

    async fn thread_turns_page(
        &mut self,
        thread_id: ThreadId,
        cursor: Option<String>,
    ) -> Result<ThreadTurnsListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadTurnsList {
                request_id,
                params: ThreadTurnsListParams {
                    thread_id: thread_id.to_string(),
                    cursor,
                    limit: Some(INITIAL_HISTORY_TURN_LIMIT),
                    sort_direction: Some(SortDirection::Desc),
                    items_view: Some(TurnItemsView::NotLoaded),
                },
            })
            .await
            .wrap_err("failed to load a bounded thread history page")
    }

    async fn merge_thread_item_page(
        &mut self,
        thread_id: ThreadId,
        page: ThreadItemsListResponse,
        state: &mut ThreadHistoryPagination,
        turns: &mut Vec<Turn>,
    ) -> Result<Vec<ThreadItem>> {
        state.next_item_cursor = page.next_cursor;
        let mut items = Vec::new();
        for entry in page.data {
            while !turns.iter().any(|turn| turn.id == entry.turn_id) {
                let Some(cursor) = state.next_turn_cursor.take() else {
                    break;
                };
                let page = self.thread_turns_page(thread_id, Some(cursor)).await?;
                state.next_turn_cursor = page.next_cursor;
                turns.splice(0..0, page.data.into_iter().rev());
            }
            if let Some(turn) = turns.iter_mut().find(|turn| turn.id == entry.turn_id)
                && !turn.items.iter().any(|item| item.id() == entry.item.id())
            {
                items.push(entry.item.clone());
                turn.items.insert(/*index*/ 0, entry.item);
                turn.items_view = TurnItemsView::Summary;
            }
        }
        items.reverse();
        Ok(items)
    }

    pub(crate) async fn hydrate_initial_thread_history(
        &mut self,
        thread: &mut Thread,
        turn_cursor: Option<String>,
        item_cursor: Option<String>,
        config: Option<&Config>,
    ) -> Result<()> {
        let thread_id = ThreadId::from_string(&thread.id)
            .wrap_err("invalid thread id in bounded history response")?;
        if thread.history_mode == ThreadHistoryMode::Legacy {
            if thread.turns.is_empty() {
                thread.turns = Box::pin(self.thread_read(thread_id, /*include_turns*/ true))
                    .await?
                    .turns;
            }
            self.history_pagination.entry(thread_id).or_default();
            return Ok(());
        }

        let page = self.thread_turns_page(thread_id, turn_cursor).await?;
        thread.turns = page.data.into_iter().rev().collect();
        let mut state = ThreadHistoryPagination {
            history_mode: ThreadHistoryMode::Paginated,
            next_turn_cursor: page.next_cursor,
            next_item_cursor: item_cursor,
            ..ThreadHistoryPagination::default()
        };
        let row_budget =
            config.and_then(|config| resize_reflow_max_rows(config.terminal_resize_reflow));
        let item_budget = if config.is_some() && row_budget.is_none() {
            None
        } else {
            Some(HISTORY_ITEM_PAGE_LIMIT as usize)
        };
        let width = crossterm::terminal::size()
            .map(|(width, _)| width.max(/*other*/ 1))
            .unwrap_or(/*default*/ 80);
        let mut scanned_items = 0;
        loop {
            let remaining_rows = row_budget.map(|budget| {
                budget.saturating_sub(config.map_or(/*default*/ 0, |config| {
                    rendered_history_rows(thread, config, width)
                }))
            });
            let remaining_items = item_budget.map(|budget| budget.saturating_sub(scanned_items));
            if remaining_rows == Some(0) || remaining_items == Some(0) {
                break;
            }
            let limit = remaining_rows
                .unwrap_or(HISTORY_ITEM_PAGE_LIMIT as usize)
                .min(remaining_items.unwrap_or(HISTORY_ITEM_PAGE_LIMIT as usize))
                .min(HISTORY_ITEM_PAGE_LIMIT as usize) as u32;
            let request_id = self.next_request_id();
            let page: ThreadItemsListResponse = self
                .client
                .request_typed(ClientRequest::ThreadItemsList {
                    request_id,
                    params: ThreadItemsListParams {
                        thread_id: thread_id.to_string(),
                        turn_id: None,
                        cursor: state.next_item_cursor.clone(),
                        limit: Some(limit),
                        sort_direction: Some(SortDirection::Desc),
                    },
                })
                .await
                .wrap_err("failed to load a bounded thread item page")?;
            if page.data.is_empty() {
                state.next_item_cursor = None;
                break;
            }
            scanned_items = scanned_items.saturating_add(page.data.len());
            self.merge_thread_item_page(thread_id, page, &mut state, &mut thread.turns)
                .await?;
            if state.next_item_cursor.is_none() {
                break;
            }
        }
        if config.is_some() {
            self.history_pagination.insert(thread_id, state);
        }
        Ok(())
    }
}

fn rendered_history_rows(thread: &Thread, config: &Config, width: u16) -> usize {
    if thread.turns.iter().all(|turn| turn.items.is_empty()) {
        return 0;
    }
    let visibility = if config.show_raw_agent_reasoning {
        RawReasoningVisibility::Visible
    } else {
        RawReasoningVisibility::Hidden
    };
    let mode = if config.tui_raw_output_mode {
        HistoryRenderMode::Raw
    } else {
        HistoryRenderMode::Rich
    };
    thread_to_transcript_cells(
        thread.clone(),
        visibility,
        Some(config.codex_home.as_path()),
    )
    .into_iter()
    .fold(/*init*/ 0, |rows, cell| {
        let height = usize::from(cell.desired_height_for_mode(width, mode));
        rows + height + usize::from(height != 0 && rows != 0 && !cell.is_stream_continuation())
    })
}
