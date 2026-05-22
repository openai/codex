//! Owns asynchronous next-prompt suggestion state for the active TUI thread.
//!
//! `App` starts one fire-and-forget RPC after stable conversation boundaries,
//! tags it with a generation number, and forwards only the newest result for the
//! still-visible thread into `ChatWidget`. Typing, paste, turn transitions,
//! backtrack, and thread replacement invalidate pending work so a late response
//! cannot overwrite newer composer state.

use super::*;

impl App {
    /// Clears both pending generation work and the visible ghost-text candidate.
    pub(crate) fn clear_next_prompt_suggestion(&mut self) {
        self.cancel_pending_next_prompt_suggestion();
        self.chat_widget.clear_next_prompt_suggestion();
    }

    /// Starts a suggestion request for the currently displayed primary thread.
    ///
    /// This is the normal post-turn entry point. Callers that already know the
    /// attached thread id, such as resume/fork lifecycle code, should use
    /// `request_next_prompt_suggestion_for_thread` so transient display state does
    /// not suppress the initial request.
    pub(super) fn request_next_prompt_suggestion(&mut self, app_server: &AppServerSession) {
        let Some(thread_id) = self.current_displayed_thread_id() else {
            tracing::debug!("skipping next prompt suggestion without displayed thread");
            return;
        };
        self.request_next_prompt_suggestion_for_thread(app_server, thread_id);
    }

    /// Starts a suggestion request for `thread_id` and invalidates older requests.
    ///
    /// Side conversations deliberately stay silent because their composer has a
    /// different placeholder contract. Starting a new request clears the current
    /// visible candidate so the user never accepts text predicted from an older
    /// completed boundary.
    pub(super) fn request_next_prompt_suggestion_for_thread(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
    ) {
        if self.chat_widget.side_conversation_active() {
            tracing::debug!(%thread_id, "skipping next prompt suggestion for side conversation");
            return;
        }
        if !self.chat_widget.can_show_next_prompt_suggestion() {
            tracing::debug!(%thread_id, "skipping next prompt suggestion while composer is unavailable");
            return;
        }

        self.cancel_pending_next_prompt_suggestion();
        self.chat_widget.clear_next_prompt_suggestion();
        self.next_prompt_suggestion_generation = self
            .next_prompt_suggestion_generation
            .saturating_add(/*rhs*/ 1);
        let generation = self.next_prompt_suggestion_generation;
        let request_handle = app_server.request_handle();
        let cancellation_token = format!("next-prompt-suggestion-{}", Uuid::new_v4());
        let cancel_request = NextPromptSuggestionCancelRequest {
            request_handle: request_handle.clone(),
            thread_id,
            cancellation_token: cancellation_token.clone(),
        };
        let app_event_tx = self.app_event_tx.clone();
        let task = tokio::spawn(async move {
            let requested_at = Instant::now();
            let result = super::background_requests::fetch_next_prompt_suggestion(
                request_handle,
                thread_id,
                cancellation_token,
            )
            .await
            .map_err(|err| format!("{err:#}"));
            app_event_tx.send(AppEvent::NextPromptSuggestionReady {
                generation,
                thread_id,
                latency_ms: u64::try_from(requested_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                result,
            });
        });
        self.pending_next_prompt_suggestion = Some(PendingNextPromptSuggestion {
            task,
            cancel_request: Some(cancel_request),
        });
    }

    /// Applies a completed request only when it still matches current UI state.
    ///
    /// Both the generation token and displayed thread must match. Ignoring stale
    /// results here is the last guard against a slow background request replacing
    /// a newer suggestion after the user typed, switched threads, or resumed a
    /// different session.
    pub(super) fn handle_next_prompt_suggestion_ready(
        &mut self,
        generation: u64,
        thread_id: ThreadId,
        latency_ms: u64,
        result: Result<Option<String>, String>,
    ) {
        if generation != self.next_prompt_suggestion_generation
            || self.current_displayed_thread_id() != Some(thread_id)
        {
            return;
        }
        self.pending_next_prompt_suggestion = None;
        if !self.chat_widget.can_show_next_prompt_suggestion() {
            tracing::debug!(%thread_id, "discarding next prompt suggestion while composer is unavailable");
            return;
        }
        match result {
            Ok(suggestion) => {
                tracing::debug!(
                    latency_ms = latency_ms,
                    has_suggestion = suggestion.is_some(),
                    "next prompt suggestion request finished"
                );
                self.chat_widget.set_next_prompt_suggestion(suggestion);
            }
            Err(err) => tracing::debug!(
                latency_ms = latency_ms,
                error = %err,
                "next prompt suggestion request failed"
            ),
        }
    }

    /// Aborts pending generation without clearing the last visible suggestion.
    ///
    /// Input edits use this path so a user can type over the ghost text, clear the
    /// draft, and still get the same already-produced suggestion back from
    /// `ChatWidget`'s placeholder refresh. In-flight requests also get a
    /// best-effort app-server cancellation so hidden sampling does not continue
    /// after the UI has invalidated it.
    pub(super) fn cancel_pending_next_prompt_suggestion(&mut self) {
        if let Some(pending) = self.pending_next_prompt_suggestion.take() {
            pending.task.abort();
            if let Some(cancel_request) = pending.cancel_request {
                tokio::spawn(async move {
                    if let Err(err) = super::background_requests::cancel_next_prompt_suggestion(
                        cancel_request.request_handle,
                        cancel_request.thread_id,
                        cancel_request.cancellation_token,
                    )
                    .await
                    {
                        tracing::debug!(error = %err, "next prompt suggestion cancellation failed");
                    }
                });
            }
        }
        self.next_prompt_suggestion_generation = self
            .next_prompt_suggestion_generation
            .saturating_add(/*rhs*/ 1);
    }

    /// Moves the visible suggestion into editable composer text.
    ///
    /// Acceptance never submits the prompt. Returning `false` means there was no
    /// currently stored suggestion to take.
    pub(crate) fn accept_next_prompt_suggestion(&mut self) -> bool {
        self.cancel_pending_next_prompt_suggestion();
        let Some(suggestion) = self.chat_widget.take_next_prompt_suggestion() else {
            return false;
        };
        self.chat_widget
            .set_composer_text(suggestion, Vec::new(), Vec::new());
        true
    }

    /// Returns whether this key event should accept the visible ghost text.
    pub(crate) fn next_prompt_suggestion_key_should_accept(&self, key_event: KeyEvent) -> bool {
        self.chat_widget.can_show_next_prompt_suggestion()
            && self.chat_widget.has_next_prompt_suggestion()
            && matches!(
                key_event,
                KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }
            )
    }
}
