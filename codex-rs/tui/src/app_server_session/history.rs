//! Bounded app-server transcript loading for resume, fork, and transcript views.

use std::collections::HashSet;

use super::AppServerSession;
use crate::history_cell::HistoryRenderMode;
use crate::legacy_core::config::Config;
use crate::local_settings::LocalSettings;
use crate::resize_reflow_cap::resize_reflow_max_rows;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadItemsListCursor;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::ThreadRevertParams;
use codex_app_server_protocol::ThreadRevertResponse;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;

pub(crate) const INITIAL_HISTORY_TURN_LIMIT: u32 = 5;
pub(crate) const HISTORY_ITEM_PAGE_LIMIT: u32 = 100;
pub(crate) const HISTORY_ITEM_SCAN_LIMIT: usize = 4 * HISTORY_ITEM_PAGE_LIMIT as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistoryHydrationScope<'a> {
    Initial,
    Complete,
    ThroughTurn(&'a str),
}

/// Limits initial hydration independently from the number of items retained by later paging.
struct HistoryLoadBudget {
    rows: Option<usize>,
    items: Option<usize>,
}

impl HistoryLoadBudget {
    fn new(
        scope: HistoryHydrationScope<'_>,
        config: Option<&Config>,
        local_settings: Option<&crate::local_settings::LocalSettings>,
        terminal_height: u16,
    ) -> Self {
        let transcript_mode = local_settings
            .map(|settings| settings.transcript_mode)
            .or_else(|| config.map(|config| LocalSettings::from(config).transcript_mode));
        if scope == HistoryHydrationScope::Initial
            && transcript_mode.is_some_and(crate::transcript_mode::TranscriptMode::is_owned)
        {
            // A few screenfuls cover the first viewport and nearby reading without coupling
            // owned history to terminal scrollback settings, including unlimited scrollback.
            return Self {
                rows: Some(usize::from(terminal_height.max(/*other*/ 1)) * 3),
                items: Some(HISTORY_ITEM_SCAN_LIMIT),
            };
        }
        let rows = local_settings
            .and_then(|settings| resize_reflow_max_rows(settings.terminal_resize_reflow()));
        let items = match (scope, config, rows) {
            (HistoryHydrationScope::Complete, _, _)
            | (HistoryHydrationScope::ThroughTurn(_), _, _)
            | (HistoryHydrationScope::Initial, Some(_), None) => None,
            (HistoryHydrationScope::Initial, Some(_), Some(max_rows)) => {
                Some(max_rows.saturating_add(HISTORY_ITEM_SCAN_LIMIT))
            }
            (HistoryHydrationScope::Initial, None, _) => Some(HISTORY_ITEM_PAGE_LIMIT as usize),
        };
        Self { rows, items }
    }

    fn next_page_size(&self, rendered_rows: usize, scanned_items: usize) -> Option<u32> {
        let remaining_rows = self.rows.map(|budget| budget.saturating_sub(rendered_rows));
        let remaining_items = self
            .items
            .map(|budget| budget.saturating_sub(scanned_items));
        if remaining_rows == Some(0) || remaining_items == Some(0) {
            return None;
        }
        let limit = remaining_items
            .unwrap_or(HISTORY_ITEM_PAGE_LIMIT as usize)
            .min(HISTORY_ITEM_PAGE_LIMIT as usize);
        // After the first page, hidden items must not reduce requests to one item.
        let limit = if scanned_items != 0 {
            limit
        } else {
            limit.min(remaining_rows.unwrap_or(limit))
        };
        Some(limit as u32)
    }
}

pub(crate) fn thread_items_page_params(
    thread_id: ThreadId,
    turn_id: Option<&str>,
    cursor: Option<String>,
    limit: u32,
) -> ThreadItemsListParams {
    ThreadItemsListParams {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.map(str::to_string),
        cursor: cursor.map(ThreadItemsListCursor::Opaque),
        limit: Some(limit),
        sort_direction: Some(SortDirection::Desc),
    }
}

fn advancing_cursor(
    current: Option<&str>,
    next: Option<String>,
    seen_cursors: &mut HashSet<String>,
) -> Option<String> {
    if let Some(current) = current {
        seen_cursors.insert(current.to_string());
    }
    next.filter(|next| seen_cursors.insert(next.clone()))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadHistoryPagination {
    pub(super) history_mode: ThreadHistoryMode,
    next_turn_cursor: Option<String>,
    next_item_cursor: Option<String>,
    seen_turn_cursors: HashSet<String>,
    seen_item_cursors: HashSet<String>,
    loading_older: bool,
}

impl AppServerSession {
    pub(crate) async fn revert_thread(
        &mut self,
        thread_id: ThreadId,
        before_turn_id: String,
        retained_turns: &[Turn],
    ) -> std::result::Result<ThreadRevertResponse, codex_app_server_client::TypedRequestError> {
        let request_id = self.next_request_id();
        let response: ThreadRevertResponse = self
            .client
            .request_typed(ClientRequest::ThreadRevert {
                request_id,
                params: ThreadRevertParams {
                    thread_id: thread_id.to_string(),
                    before_turn_id,
                },
            })
            .await?;
        // Older cursors in the retained prefix remain valid. If the entire displayed window
        // disappeared, resume paging at the replacement history's end instead.
        if retained_turns.iter().all(|turn| turn.items.is_empty()) {
            self.history_pagination.insert(
                thread_id,
                ThreadHistoryPagination {
                    history_mode: ThreadHistoryMode::Paginated,
                    next_turn_cursor: response.turns_backwards_cursor.clone(),
                    next_item_cursor: response.items_backwards_cursor.clone(),
                    ..ThreadHistoryPagination::default()
                },
            );
        } else if let Some(page) = self.history_pagination.get_mut(&thread_id) {
            page.loading_older = false;
        }
        Ok(response)
    }

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

    /// Match a completion to the page still pending for this thread, before interpreting its result.
    pub(crate) fn is_older_history_page_pending(&self, thread_id: ThreadId, cursor: &str) -> bool {
        self.history_pagination.get(&thread_id).is_some_and(|page| {
            page.loading_older && page.next_item_cursor.as_deref() == Some(cursor)
        })
    }

    pub(crate) fn cancel_older_history_page(&mut self, thread_id: ThreadId, cursor: &str) {
        if let Some(page) = self.history_pagination.get_mut(&thread_id)
            && page.next_item_cursor.as_deref() == Some(cursor)
        {
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
        if !self.is_older_history_page_pending(thread_id, cursor) {
            return Ok(Vec::new());
        }
        let Some(mut state) = self.history_pagination.get(&thread_id).cloned() else {
            return Ok(Vec::new());
        };
        let items = self
            .merge_thread_item_page(thread_id, page, &mut state, turns)
            .await?;
        state.loading_older = false;
        self.history_pagination.insert(thread_id, state);
        Ok(items)
    }

    pub(crate) async fn thread_items_page(
        &mut self,
        thread_id: ThreadId,
        turn_id: Option<&str>,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ThreadItemsListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadItemsList {
                request_id,
                params: thread_items_page_params(thread_id, turn_id, cursor, limit),
            })
            .await
            .wrap_err("failed to load a bounded thread item page")
    }

    pub(crate) async fn thread_turns_page(
        &mut self,
        thread_id: ThreadId,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ThreadTurnsListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ThreadTurnsList {
                request_id,
                params: ThreadTurnsListParams {
                    thread_id: thread_id.to_string(),
                    cursor,
                    limit: Some(limit),
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
        state.next_item_cursor = advancing_cursor(
            state.next_item_cursor.as_deref(),
            page.next_cursor,
            &mut state.seen_item_cursors,
        );
        let mut missing_turn_ids = page
            .data
            .iter()
            .filter(|entry| !turns.iter().any(|turn| turn.id == entry.turn_id))
            .map(|entry| entry.turn_id.clone())
            .collect::<HashSet<_>>();
        let mut items = Vec::new();
        for entry in page.data {
            while !turns.iter().any(|turn| turn.id == entry.turn_id) {
                let Some(cursor) = state.next_turn_cursor.take() else {
                    break;
                };
                // Fetch only the remaining item-backed turns so metadata does not run ahead
                // of this page. Empty turns between them may require another bounded request.
                let limit = missing_turn_ids.len().min(HISTORY_ITEM_PAGE_LIMIT as usize) as u32;
                let page = self
                    .thread_turns_page(thread_id, Some(cursor.clone()), limit)
                    .await?;
                state.next_turn_cursor = advancing_cursor(
                    Some(&cursor),
                    page.next_cursor,
                    &mut state.seen_turn_cursors,
                );
                for turn in &page.data {
                    missing_turn_ids.remove(&turn.id);
                }
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

    /// Hydrates paginated threads through bounded turn and item pages.
    ///
    /// Legacy servers expose neither paging method, so only legacy threads may
    /// use `thread/read(includeTurns: true)` to preserve their existing history.
    pub(crate) async fn hydrate_initial_thread_history(
        &mut self,
        thread: &mut Thread,
        turn_cursor: Option<String>,
        item_cursor: Option<String>,
        config: Option<&Config>,
        local_settings: Option<&crate::local_settings::LocalSettings>,
        scope: HistoryHydrationScope<'_>,
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

        let page = self
            .thread_turns_page(thread_id, turn_cursor, INITIAL_HISTORY_TURN_LIMIT)
            .await?;
        thread.turns = page.data.into_iter().rev().collect();
        let mut state = ThreadHistoryPagination {
            history_mode: ThreadHistoryMode::Paginated,
            next_turn_cursor: page.next_cursor,
            next_item_cursor: item_cursor,
            ..ThreadHistoryPagination::default()
        };
        let (width, height) = crossterm::terminal::size().unwrap_or(/*default*/ (80, 24));
        let width = width.max(/*other*/ 1);
        let budget = HistoryLoadBudget::new(scope, config, local_settings, height);
        let mut scanned_items = 0;
        let mut rendered_rows = 0;
        while let Some(limit) = budget.next_page_size(rendered_rows, scanned_items) {
            let page = self
                .thread_items_page(
                    thread_id,
                    /*turn_id*/ None,
                    state.next_item_cursor.clone(),
                    limit,
                )
                .await?;
            if page.data.is_empty() {
                state.next_item_cursor = None;
                break;
            }
            scanned_items = scanned_items.saturating_add(page.data.len());
            let items = self
                .merge_thread_item_page(thread_id, page, &mut state, &mut thread.turns)
                .await?;
            // Finish the anchor turn so prompt editing can detect earlier steers,
            // but do not scan history older than the displayed transcript.
            if let HistoryHydrationScope::ThroughTurn(anchor) = scope
                && let Some(index) = thread.turns.iter().position(|turn| turn.id == anchor)
                && thread.turns[..index]
                    .iter()
                    .any(|turn| !turn.items.is_empty())
            {
                break;
            }
            if let Some((config, local_settings)) = config.zip(local_settings) {
                rendered_rows = rendered_history_rows(
                    thread_id,
                    thread,
                    items,
                    config,
                    local_settings,
                    width,
                    rendered_rows,
                );
            } else {
                rendered_rows = rendered_rows.saturating_add(items.len());
            }
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

fn rendered_history_rows(
    thread_id: ThreadId,
    thread: &Thread,
    items: Vec<ThreadItem>,
    config: &Config,
    local_settings: &crate::local_settings::LocalSettings,
    width: u16,
    rendered_rows: usize,
) -> usize {
    let visibility = if config.show_raw_agent_reasoning {
        RawReasoningVisibility::Visible
    } else {
        RawReasoningVisibility::Hidden
    };
    let mode = if local_settings.tui.raw_output_mode {
        HistoryRenderMode::Raw
    } else {
        HistoryRenderMode::Rich
    };
    thread_items_to_transcript_cells(
        Some(thread_id),
        &thread.cwd,
        items,
        visibility,
        Some(config),
    )
    .into_iter()
    .fold(rendered_rows, |rows, cell| {
        let height = usize::from(cell.desired_height_for_mode(width, mode));
        rows + height + usize::from(height != 0 && rows != 0 && !cell.is_stream_continuation())
    })
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
