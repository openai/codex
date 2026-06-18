use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;

use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;

#[derive(Default)]
pub(crate) struct CurrentTimeReminderState {
    model_requests_since_delivery: u64,
    last_window_id: Option<String>,
}

impl CurrentTimeReminderState {
    fn begin_model_request(&mut self, window_id: &str, interval: u64) -> bool {
        self.model_requests_since_delivery = self.model_requests_since_delivery.saturating_add(1);
        self.last_window_id.as_deref() != Some(window_id)
            || self.model_requests_since_delivery >= interval
    }

    fn record_delivery(&mut self, window_id: &str) {
        self.model_requests_since_delivery = 0;
        self.last_window_id = Some(window_id.to_string());
    }
}

pub(super) async fn maybe_record_current_time_reminder(
    sess: &Session,
    turn_context: &TurnContext,
    window_id: &str,
) -> CodexResult<()> {
    let Some(config) = turn_context.config.varlatency else {
        return Ok(());
    };

    let reminder_is_due = {
        let mut state = sess.state.lock().await;
        state
            .current_time_reminder
            .begin_model_request(window_id, config.reminder_interval_model_requests)
    };
    if !reminder_is_due {
        return Ok(());
    }

    let provider = sess
        .services
        .current_time_provider
        .as_ref()
        .ok_or_else(|| CodexErr::Fatal("current-time provider is not configured".to_string()))?;
    let current_time = provider
        .current_time(sess.thread_id)
        .await
        .map_err(|err| CodexErr::Fatal(format!("failed to read current time: {err:#}")))?;

    let response_item =
        ContextualUserFragment::into(crate::context::CurrentTimeReminder::new(current_time));
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;

    let mut state = sess.state.lock().await;
    state.current_time_reminder.record_delivery(window_id);
    Ok(())
}
