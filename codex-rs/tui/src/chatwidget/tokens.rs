//! Coordinates asynchronous `/usage` cards in the chat widget.
//!
//! The slash command builds a composite history cell immediately, but the widget
//! keeps that cell transient while the account request runs. The transient card is
//! rendered above the composer through [`ChatWidget::pending_token_activity_output`]
//! so loading never requires clearing or rewriting transcript history. When the
//! matching response arrives, [`TokenActivityHandle`] updates the shared card state
//! and [`ChatWidget::finish_token_activity_refresh`] moves the cell into a completed
//! slot. Event dispatch commits that completed cell into history only after active
//! output and stream consolidation no longer block insertion.
//!
//! Pure chart rendering and date bucketing live in [`chart`]. This module owns
//! request correlation, transient/completed card state, and integration with
//! `ChatWidget` history insertion.

mod chart;

use std::sync::Arc;
use std::sync::RwLock;

use chrono::NaiveDate;
use chrono::Utc;
use codex_app_server_protocol::GetAccountTokenUsageResponse;
use ratatui::style::Stylize;
use ratatui::text::Line;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::plain_lines;

pub(super) use chart::TokenActivityView;

/// Tracks the renderable lifecycle of one token activity history cell.
#[derive(Debug)]
enum TokenActivityState {
    Loading,
    Loaded {
        response: GetAccountTokenUsageResponse,
        today: NaiveDate,
    },
    Error,
}

/// Completes an asynchronously rendered token activity history cell.
///
/// Clones share the same card state, allowing the background request path to
/// update a cell still owned by the widget's transient-output state. The widget
/// remains responsible for request-ID matching, redraws, and history insertion.
#[derive(Clone, Debug)]
pub(super) struct TokenActivityHandle {
    state: Arc<RwLock<TokenActivityState>>,
}

/// Holds the one transient token activity card waiting on its background response.
///
/// The request ID prevents late results from mutating a newer `/usage` card. The
/// cell stays out of transcript history until the matching response completes and
/// the widget confirms that active output no longer blocks insertion.
pub(super) struct PendingTokenActivityOutput {
    request_id: u64,
    cell: CompositeHistoryCell,
    handle: TokenActivityHandle,
}

impl TokenActivityHandle {
    /// Replaces the loading state with either fetched activity or an unavailable state.
    ///
    /// This method intentionally discards the error string because the TUI exposes
    /// one stable unavailable message. Calling it more than once replaces the prior
    /// terminal state, so request-ID matching should happen before completion.
    pub(super) fn finish(&self, result: Result<GetAccountTokenUsageResponse, String>) {
        self.finish_with_today(result, Utc::now().date_naive());
    }

    fn finish_with_today(
        &self,
        result: Result<GetAccountTokenUsageResponse, String>,
        today: NaiveDate,
    ) {
        let state = match result {
            Ok(response) => TokenActivityState::Loaded { response, today },
            Err(_) => TokenActivityState::Error,
        };
        #[expect(clippy::expect_used)]
        let mut current = self.state.write().expect("token activity state poisoned");
        *current = state;
    }
}

/// Renders one `/usage` card from shared asynchronous state.
#[derive(Debug)]
struct TokenActivityHistoryCell {
    view: TokenActivityView,
    state: Arc<RwLock<TokenActivityState>>,
}

/// Creates the card contents and completion handle for one `/usage` invocation.
///
/// The composite cell includes the echoed slash command and a loading card from
/// the start. Callers must retain the returned handle and complete it when the
/// matching background response arrives; otherwise the transient card stays loading.
pub(super) fn new_token_activity_output(
    view: TokenActivityView,
) -> (CompositeHistoryCell, TokenActivityHandle) {
    let command = PlainHistoryCell::new(vec![
        format!("/usage {}", view.label().to_lowercase())
            .magenta()
            .into(),
    ]);
    let state = Arc::new(RwLock::new(TokenActivityState::Loading));
    let handle = TokenActivityHandle {
        state: Arc::clone(&state),
    };
    let card = TokenActivityHistoryCell { view, state };
    (
        CompositeHistoryCell::new(vec![Box::new(command), Box::new(card)]),
        handle,
    )
}

impl HistoryCell for TokenActivityHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        #[expect(clippy::expect_used)]
        let state = self.state.read().expect("token activity state poisoned");
        match &*state {
            TokenActivityState::Loading => {
                vec![
                    " Token activity".bold().into(),
                    "   Loading...".dim().into(),
                ]
            }
            TokenActivityState::Error => vec![
                " Token activity".bold().into(),
                "   Token activity unavailable".dim().into(),
            ],
            TokenActivityState::Loaded { response, today } => {
                chart::loaded_lines(self.view, response, *today, width)
            }
        }
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }
}

impl ChatWidget {
    /// Starts a token activity refresh and replaces the current transient card.
    ///
    /// Each invocation receives a request ID so background responses update only
    /// their own card. The card remains outside transcript history until completion,
    /// which keeps loading visible without disturbing existing transcript content.
    pub(super) fn add_token_activity_output(&mut self, view: TokenActivityView) {
        let request_id = self.next_token_activity_request_id;
        self.next_token_activity_request_id =
            self.next_token_activity_request_id.wrapping_add(/*rhs*/ 1);
        let (cell, handle) = new_token_activity_output(view);
        self.completed_token_activity_output = None;
        self.refreshing_token_activity_output = Some(PendingTokenActivityOutput {
            request_id,
            cell,
            handle,
        });
        self.bump_active_cell_revision();
        self.request_redraw();
        self.app_event_tx
            .send(AppEvent::RefreshTokenActivity { request_id });
    }

    /// Returns the transient token activity card that should render above the composer.
    ///
    /// A loading card takes precedence over a completed card waiting for history
    /// insertion. Callers should render the returned cell but leave ownership with
    /// the widget so completion and insertion can update it safely.
    pub(super) fn pending_token_activity_output(&self) -> Option<&dyn HistoryCell> {
        self.refreshing_token_activity_output
            .as_ref()
            .map(|output| &output.cell as &dyn HistoryCell)
            .or_else(|| {
                self.completed_token_activity_output
                    .as_ref()
                    .map(|cell| cell as &dyn HistoryCell)
            })
    }

    /// Applies a background token activity result to its matching transient card.
    ///
    /// Returns `true` when the pending request matched and moved into the completed
    /// slot. Late responses return `false`, including responses for cards replaced
    /// by a newer `/usage` invocation or cleared during transcript changes.
    pub(crate) fn finish_token_activity_refresh(
        &mut self,
        request_id: u64,
        result: Result<GetAccountTokenUsageResponse, String>,
    ) -> bool {
        let Some(output) = self.refreshing_token_activity_output.take() else {
            return false;
        };
        if output.request_id != request_id {
            self.refreshing_token_activity_output = Some(output);
            return false;
        }
        output.handle.finish(result);
        self.completed_token_activity_output = Some(output.cell);
        self.bump_active_cell_revision();
        self.request_redraw();
        true
    }

    /// Reports whether a completed token activity card must wait before insertion.
    ///
    /// Inserting while a stream, queued consolidation, or active transcript cell is
    /// present can reorder the card relative to visible output, so callers retry once
    /// these barriers clear.
    pub(crate) fn token_activity_history_insertion_blocked(&self) -> bool {
        self.stream_controller.is_some()
            || self.plan_stream_controller.is_some()
            || self.pending_stream_consolidations > 0
            || self.transcript.active_cell.is_some()
    }

    /// Records a stream consolidation barrier that delays token card insertion.
    ///
    /// Each queued consolidation should eventually call
    /// [`ChatWidget::note_stream_consolidation_completed`].
    pub(crate) fn note_stream_consolidation_queued(&mut self) {
        self.pending_stream_consolidations =
            self.pending_stream_consolidations.saturating_add(/*rhs*/ 1);
    }

    /// Releases one queued stream consolidation barrier.
    ///
    /// The counter saturates at zero so an unmatched completion does not underflow,
    /// but paired queue/completion calls are still the intended contract.
    pub(crate) fn note_stream_consolidation_completed(&mut self) {
        self.pending_stream_consolidations =
            self.pending_stream_consolidations.saturating_sub(/*rhs*/ 1);
    }

    /// Transfers the completed token activity card into the history insertion path.
    ///
    /// Callers should use this only after
    /// [`ChatWidget::token_activity_history_insertion_blocked`] returns `false`;
    /// taking the card removes it from the transient render area.
    pub(crate) fn take_completed_token_activity_output(&mut self) -> Option<CompositeHistoryCell> {
        let output = self.completed_token_activity_output.take()?;
        self.bump_active_cell_revision();
        Some(output)
    }

    /// Requests another insertion attempt when a completed card is waiting.
    ///
    /// This is used after stream or history lifecycle events that may have cleared
    /// the insertion barriers without directly owning the completed card.
    pub(crate) fn request_completed_token_activity_output_insertion(&self) {
        if self.completed_token_activity_output.is_some() {
            self.app_event_tx
                .send(AppEvent::CommitCompletedTokenActivityOutput);
        }
    }

    /// Drops transient and completed token cards that must no longer update.
    ///
    /// Late background responses cannot mutate cards after a transcript reset,
    /// backtrack, or replacement flow clears this widget-owned state.
    pub(crate) fn clear_pending_token_activity_refreshes(&mut self) {
        let cleared_refresh = self.refreshing_token_activity_output.take().is_some();
        let cleared_completed = self.completed_token_activity_output.take().is_some();
        if cleared_refresh || cleared_completed {
            self.bump_active_cell_revision();
            self.request_redraw();
        }
    }
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
