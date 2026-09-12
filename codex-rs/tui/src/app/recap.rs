//! Schedules and generates bounded recaps through the existing temporary request path.
//! The TUI opt-out suppresses automatic scheduling and requests, but not manual `/recap`.

use std::time::Duration;
use std::time::Instant;

use super::App;
use crate::app_event::AppEvent;
use crate::app_event::RecapTrigger;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerSession;
use crate::history_cell::ThreadRecapHistoryCell;
use crate::history_cell::ThreadRecapLoadingCell;
use crate::pager_overlay::Overlay;
use crate::temporary_structured_request::TemporaryStructuredThreadOptions;
use crate::temporary_structured_request::run_temporary_structured_turn;
use crate::temporary_structured_request::start_temporary_thread;
use crate::temporary_structured_request::unsubscribe_temporary_thread;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_context_fragments::ContextualUserFragment;
use codex_context_fragments::RecapPrompt;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

const MIN_COMPLETED_TURNS: usize = 3;
const MIN_TURNS_BETWEEN_RECAPS: usize = 2;
pub(super) const RECAP_DELAY: Duration = Duration::from_secs(/*secs*/ 30 * 60);
const RECAP_MAX_CHARS: usize = 700;
const RECAP_NEXT_MAX_CHARS: usize = 200;
const RECAP_RETRY_DELAY: Duration = Duration::from_secs(/*secs*/ 30);
const MANUAL_RECAP_FAILURE_MESSAGE: &str = "Could not generate a recap. Please try again.";
const MANUAL_RECAP_IN_PROGRESS_MESSAGE: &str = "A recap is already being generated.";
const MANUAL_RECAP_EMPTY_HISTORY_MESSAGE: &str = "There is no conversation history to recap.";
#[cfg(test)]
pub(super) const RECAP_PROMPT_MAX_BYTES: usize = RecapPrompt::MAX_BYTES;

#[path = "recap_history.rs"]
mod history;
use history::recap_history;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct GeneratedRecap {
    summary: String,
    #[serde(deserialize_with = "Option::deserialize")]
    next_action: Option<String>,
}

fn recap_prompt(history: &str) -> String {
    RecapPrompt::new(history).render()
}

fn recap_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": {
                "type": "string",
                "minLength": 1,
                "maxLength": RECAP_MAX_CHARS,
            },
            "next_action": {
                "type": ["string", "null"],
                "maxLength": RECAP_NEXT_MAX_CHARS,
            },
        },
        "required": ["summary", "next_action"],
        "additionalProperties": false,
    })
}

fn parse_recap(response: &str) -> Option<GeneratedRecap> {
    let mut recap = serde_json::from_str::<GeneratedRecap>(response).ok()?;
    recap.summary = recap.summary.trim().to_string();
    if recap.summary.is_empty() || recap.summary.chars().count() > RECAP_MAX_CHARS {
        return None;
    }
    recap.next_action = recap
        .next_action
        .map(|action| action.trim().to_string())
        .filter(|action| !action.is_empty());
    if recap
        .next_action
        .as_ref()
        .is_some_and(|action| action.chars().count() > RECAP_NEXT_MAX_CHARS)
    {
        return None;
    }
    Some(recap)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecapRequest {
    pub(super) thread_id: ThreadId,
    pub(super) request_id: Uuid,
    pub(super) trigger: RecapTrigger,
    pub(super) completed_turn_count: usize,
    pub(super) turn_revision: usize,
}

impl App {
    fn recap_trigger_enabled(&self, trigger: RecapTrigger) -> bool {
        match trigger {
            RecapTrigger::Automatic => self.local_settings.tui.auto_recap,
            RecapTrigger::Manual => true,
        }
    }

    pub(super) fn schedule_recap_check(&mut self, thread_id: ThreadId, now: Instant) {
        if let Some(task) = self.recap.scheduled_check.take() {
            task.abort();
        }

        if !self.local_settings.tui.auto_recap {
            return;
        }
        let Some(deadline) = self.recap.next_check_deadline() else {
            return;
        };
        let delay = deadline.saturating_duration_since(now);
        let sleep = tokio::time::sleep(delay);
        let app_event_tx = self.app_event_tx.clone();

        self.recap.scheduled_check = Some(tokio::spawn(async move {
            sleep.await;
            app_event_tx.send(AppEvent::CheckRecap { thread_id });
        }));
    }

    pub(super) fn clear_recap_request(&mut self, trigger: RecapTrigger) {
        self.recap.clear_in_flight_request();

        if !matches!(trigger, RecapTrigger::Manual) {
            return;
        }

        self.chat_widget.clear_recap_loading();

        let Some(index) = self
            .transcript_cells
            .iter()
            .rposition(|cell| cell.as_any().is::<ThreadRecapLoadingCell>())
        else {
            return;
        };

        self.transcript_cells.remove(index);
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.replace_cells(self.transcript_cells.clone());
        }
    }

    fn retry_or_report_recap_failure(&mut self, request: RecapRequest) {
        if !self.recap_trigger_enabled(request.trigger) {
            return;
        }
        match request.trigger {
            RecapTrigger::Automatic => self.recap.schedule_retry(
                request.thread_id,
                self.app_event_tx.clone(),
                request.turn_revision,
            ),
            RecapTrigger::Manual => self
                .chat_widget
                .add_error_message(MANUAL_RECAP_FAILURE_MESSAGE.to_string()),
        }
    }

    pub(super) fn request_recap(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        trigger: RecapTrigger,
    ) {
        if !self.recap_trigger_enabled(trigger) {
            return;
        }
        if self.recap.in_flight_request.is_some() {
            if matches!(trigger, RecapTrigger::Manual) {
                self.chat_widget
                    .add_error_message(MANUAL_RECAP_IN_PROGRESS_MESSAGE.to_string());
            }
            return;
        }

        let history = recap_history(&self.transcript_cells);
        if history.is_empty() {
            if matches!(trigger, RecapTrigger::Manual) {
                self.chat_widget
                    .add_error_message(MANUAL_RECAP_EMPTY_HISTORY_MESSAGE.to_string());
            }
            return;
        }

        if matches!(trigger, RecapTrigger::Manual) {
            self.chat_widget.show_recap_loading();
        }

        let request = RecapRequest {
            thread_id,
            request_id: Uuid::new_v4(),
            trigger,
            completed_turn_count: self.recap.completed_turns,
            turn_revision: self.recap.turn_revision,
        };
        let request_handle = app_server.request_handle();
        let model = self.chat_widget.current_model().to_string();
        let config = self.chat_widget.config_ref();
        let cwd = if app_server.uses_remote_workspace() {
            app_server
                .remote_cwd_override()
                .map(|cwd| cwd.to_string_lossy().into_owned())
                .unwrap_or_else(|| config.cwd.display().to_string())
        } else {
            config.cwd.display().to_string()
        };
        let options = TemporaryStructuredThreadOptions {
            model,
            model_provider: config.model_provider_id.clone(),
            cwd,
            active_permission_profile: config
                .permissions
                .active_permission_profile()
                .map(|profile| profile.id),
            mcp_server_names: config.mcp_servers.get().keys().cloned().collect(),
        };
        let event_sender = self.app_event_tx.clone();
        let task = tokio::spawn(async move {
            let result = start_temporary_thread(&request_handle, options)
                .await
                .map(|thread| thread.thread.id)
                .map_err(|error| error.to_string());

            event_sender.send(AppEvent::RecapStarted {
                thread_id: request.thread_id,
                request_id: request.request_id,
                trigger: request.trigger,
                completed_turn_count: request.completed_turn_count,
                turn_revision: request.turn_revision,
                history,
                result,
            });
        });

        self.recap.in_flight_request_id = Some(request.request_id);
        self.recap.in_flight_trigger = Some(request.trigger);
        self.recap.in_flight_request = Some(task);
    }

    pub(super) fn handle_recap_started(
        &mut self,
        app_server: &AppServerSession,
        request: RecapRequest,
        history: String,
        result: Result<String, String>,
    ) {
        let RecapRequest {
            thread_id,
            request_id,
            trigger,
            completed_turn_count,
            turn_revision,
        } = request;
        let is_current_request = self.recap.in_flight_request_id == Some(request_id)
            && self.recap.in_flight_trigger == Some(trigger);
        if is_current_request {
            self.recap.in_flight_request.take();
        }

        let temporary_thread_id_text = match result {
            Ok(temporary_thread_id) => temporary_thread_id,
            Err(error) => {
                if is_current_request {
                    self.clear_recap_request(trigger);
                    tracing::warn!(%thread_id, %error, "failed to start thread recap request");
                    self.retry_or_report_recap_failure(request);
                }
                return;
            }
        };

        let trigger_is_eligible = match trigger {
            RecapTrigger::Automatic => self
                .recap
                .should_generate(tokio::time::Instant::now().into_std()),
            RecapTrigger::Manual => true,
        };

        let request_is_fresh = is_current_request
            && self.current_displayed_thread_id() == Some(thread_id)
            && !self.chat_widget.is_user_turn_pending_or_running()
            && self.recap.completed_turns == completed_turn_count
            && self.recap.turn_revision == turn_revision
            && self.recap_trigger_enabled(trigger)
            && trigger_is_eligible;
        let Ok(temporary_thread_id) = ThreadId::from_string(&temporary_thread_id_text) else {
            if is_current_request {
                self.clear_recap_request(trigger);
                tracing::warn!(%thread_id, "thread recap request returned an invalid thread ID");
                self.retry_or_report_recap_failure(request);
            }
            return;
        };

        if !request_is_fresh {
            if is_current_request {
                self.clear_recap_request(trigger);
            }
            let request_handle = app_server.request_handle();
            tokio::spawn(async move {
                unsubscribe_temporary_thread(&request_handle, temporary_thread_id_text).await;
            });
            return;
        }

        let (sender, receiver) = mpsc::unbounded_channel();
        self.temporary_structured_requests
            .insert(temporary_thread_id, sender);

        let request_handle = app_server.request_handle();
        let event_sender = self.app_event_tx.clone();
        let task = tokio::spawn(async move {
            let result = run_temporary_structured_turn(
                request_handle,
                temporary_thread_id_text,
                recap_prompt(&history),
                recap_output_schema(),
                /*effort*/ None,
                receiver,
            )
            .await
            .map_err(|error| error.to_string());

            event_sender.send(AppEvent::RecapGenerated {
                thread_id,
                request_id,
                trigger,
                temporary_thread_id,
                completed_turn_count,
                turn_revision,
                result,
            });
        });

        self.recap.in_flight_thread_id = Some(temporary_thread_id);
        self.recap.in_flight_request = Some(task);
    }

    pub(super) fn handle_generated_recap(
        &mut self,
        request: RecapRequest,
        temporary_thread_id: ThreadId,
        result: Result<String, String>,
    ) -> Option<ThreadRecapHistoryCell> {
        let RecapRequest {
            thread_id,
            request_id,
            trigger,
            completed_turn_count,
            turn_revision,
        } = request;
        if self.recap.in_flight_request_id != Some(request_id)
            || self.recap.in_flight_trigger != Some(trigger)
            || self.recap.in_flight_thread_id != Some(temporary_thread_id)
        {
            return None;
        }

        self.clear_recap_request(trigger);

        if !self.recap_trigger_enabled(trigger)
            || self.current_displayed_thread_id() != Some(thread_id)
            || self.chat_widget.is_user_turn_pending_or_running()
            || self.recap.completed_turns != completed_turn_count
            || self.recap.turn_revision != turn_revision
        {
            return None;
        }

        match result {
            Ok(response) => {
                let Some(recap) = parse_recap(&response) else {
                    tracing::warn!(%thread_id, "generated thread recap was invalid");
                    self.retry_or_report_recap_failure(request);
                    return None;
                };

                self.recap.mark_recapped(completed_turn_count);
                Some(ThreadRecapHistoryCell::new(recap.summary).with_next_action(recap.next_action))
            }
            Err(error) => {
                tracing::warn!(%thread_id, %error, "failed to generate thread recap");
                self.retry_or_report_recap_failure(request);
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RecapProgress {
    pub(super) completed_turns: usize,
    pub(super) last_recapped_turn_count: Option<usize>,
}

impl RecapProgress {
    pub(super) fn from_turns(turns: &[Turn]) -> Self {
        Self {
            completed_turns: turns
                .iter()
                .filter(|turn| matches!(turn.status, TurnStatus::Completed))
                .count(),
            last_recapped_turn_count: None,
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.completed_turns = self.completed_turns.max(other.completed_turns);
        self.last_recapped_turn_count = self
            .last_recapped_turn_count
            .max(other.last_recapped_turn_count);
    }
}

#[derive(Debug, Default)]
pub(super) struct RecapState {
    unfocused_since: Option<Instant>,
    last_turn_finished_at: Option<Instant>,
    completed_turns: usize,
    last_recapped_turn_count: Option<usize>,
    turn_revision: usize,
    scheduled_check: Option<JoinHandle<()>>,
    retry_revision: Option<usize>,
    in_flight_request_id: Option<Uuid>,
    in_flight_trigger: Option<RecapTrigger>,
    in_flight_thread_id: Option<ThreadId>,
    in_flight_request: Option<JoinHandle<()>>,
}

impl RecapState {
    fn clear_in_flight_request(&mut self) {
        self.in_flight_request_id = None;
        self.in_flight_trigger = None;
        self.in_flight_thread_id = None;
        self.in_flight_request.take();
    }

    pub(super) fn seed_from_turns(&mut self, turns: &[Turn], now: Instant) {
        self.seed_from_progress(RecapProgress::from_turns(turns), now);
    }

    pub(super) fn seed_from_progress(&mut self, progress: RecapProgress, now: Instant) {
        self.completed_turns = self.completed_turns.max(progress.completed_turns);
        self.last_recapped_turn_count = self
            .last_recapped_turn_count
            .max(progress.last_recapped_turn_count);

        if progress.completed_turns > 0 {
            self.last_turn_finished_at.get_or_insert(now);
        }
    }

    pub(super) fn progress(&self) -> RecapProgress {
        RecapProgress {
            completed_turns: self.completed_turns,
            last_recapped_turn_count: self.last_recapped_turn_count,
        }
    }

    pub(super) fn reset_for_new_thread(&mut self, now: Instant) {
        let unfocused_since = self.unfocused_since.map(|_| now);
        let mut replacement = Self::default();
        replacement.unfocused_since = unfocused_since;
        std::mem::swap(self, &mut replacement);
    }

    pub(super) fn note_focus_lost(&mut self, now: Instant) {
        if self.unfocused_since.is_none() {
            self.retry_revision = None;
        }
        self.unfocused_since.get_or_insert(now);
    }

    pub(super) fn note_focus_gained(&mut self) {
        self.unfocused_since = None;

        if let Some(task) = self.scheduled_check.take() {
            task.abort();
        }

        self.retry_revision = None;
        if self.in_flight_trigger == Some(RecapTrigger::Automatic) {
            self.clear_in_flight_request();
        }
    }

    pub(super) fn note_turn_finished(&mut self, status: &TurnStatus, now: Instant) {
        if matches!(status, TurnStatus::Completed) {
            self.completed_turns += 1;
        }
        self.turn_revision += 1;
        self.last_turn_finished_at = Some(now);
    }

    pub(super) fn schedule_retry(
        &mut self,
        thread_id: ThreadId,
        app_event_tx: AppEventSender,
        turn_revision: usize,
    ) {
        if self.turn_revision != turn_revision || self.retry_revision == Some(turn_revision) {
            return;
        }
        self.retry_revision = Some(turn_revision);

        if let Some(task) = self.scheduled_check.take() {
            task.abort();
        }
        self.scheduled_check = Some(tokio::spawn(async move {
            tokio::time::sleep(RECAP_RETRY_DELAY).await;
            app_event_tx.send(AppEvent::CheckRecap { thread_id });
        }));
    }

    pub(super) fn should_generate(&self, now: Instant) -> bool {
        self.next_check_deadline()
            .is_some_and(|deadline| now >= deadline)
    }

    pub(super) fn mark_recapped(&mut self, completed_turn_count: usize) {
        self.last_recapped_turn_count = Some(completed_turn_count);
    }

    fn next_check_deadline(&self) -> Option<Instant> {
        let unfocused_since = self.unfocused_since?;

        if self.completed_turns < MIN_COMPLETED_TURNS {
            return None;
        }

        if self.last_recapped_turn_count.is_some_and(|previous| {
            self.completed_turns.saturating_sub(previous) < MIN_TURNS_BETWEEN_RECAPS
        }) {
            return None;
        }

        let last_turn_finished_at = self.last_turn_finished_at?;

        unfocused_since
            .max(last_turn_finished_at)
            .checked_add(RECAP_DELAY)
    }
}

impl Drop for RecapState {
    fn drop(&mut self) {
        if let Some(task) = self.scheduled_check.take() {
            task.abort();
        }

        // Let an in-flight request finish so it can unsubscribe its temporary thread.
    }
}

#[cfg(test)]
#[path = "recap_tests.rs"]
mod tests;
