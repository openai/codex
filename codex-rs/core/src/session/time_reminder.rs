use chrono::DateTime;
use chrono::Utc;
use codex_features::CurrentTimeReminderDeliveryMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;

use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use crate::context_manager::is_user_turn_boundary;

#[derive(Default)]
pub(crate) struct CurrentTimeReminderState {
    last_delivery_time: Option<DateTime<Utc>>,
    last_window_id: Option<String>,
    pending_user_or_tool_output_boundary: bool,
}

impl CurrentTimeReminderState {
    pub(super) fn note_recorded_items(&mut self, items: &[ResponseItem]) {
        if items.iter().any(|item| {
            is_user_turn_boundary(item)
                || matches!(
                    item,
                    ResponseItem::FunctionCallOutput { .. }
                        | ResponseItem::CustomToolCallOutput { .. }
                        | ResponseItem::ToolSearchOutput { .. }
                )
        }) {
            self.pending_user_or_tool_output_boundary = true;
        }
    }

    fn take_reminder_due(
        &mut self,
        window_id: &str,
        current_time: DateTime<Utc>,
        interval_seconds: u64,
        delivery_mode: CurrentTimeReminderDeliveryMode,
    ) -> bool {
        let is_new_window = self.last_window_id.as_deref() != Some(window_id);
        // Consume the boundary for this inference even if the interval suppresses delivery.
        let follows_user_or_tool_output =
            std::mem::take(&mut self.pending_user_or_tool_output_boundary);
        if delivery_mode == CurrentTimeReminderDeliveryMode::AfterUserOrToolOutput
            && !is_new_window
            && !follows_user_or_tool_output
        {
            return false;
        }

        let reminder_is_due = is_new_window
            || interval_seconds == 0
            || self.last_delivery_time.is_none_or(|last_delivery_time| {
                current_time
                    .signed_duration_since(last_delivery_time)
                    .num_seconds()
                    >= i64::try_from(interval_seconds).unwrap_or(i64::MAX)
            });

        if reminder_is_due {
            self.last_delivery_time = Some(current_time);
            self.last_window_id = Some(window_id.to_string());
        }

        reminder_is_due
    }
}

pub(super) async fn maybe_record_current_time_reminder(
    sess: &Session,
    turn_context: &TurnContext,
    window_id: &str,
) -> CodexResult<()> {
    let Some(config) = turn_context.config.current_time_reminder else {
        return Ok(());
    };

    let current_time = sess
        .services
        .time_provider
        .current_time(sess.thread_id)
        .await
        .map_err(|err| CodexErr::Fatal(format!("failed to read current time: {err:#}")))?;

    let reminder_is_due = {
        let mut state = sess.state.lock().await;
        state.current_time_reminder.take_reminder_due(
            window_id,
            current_time,
            config.reminder_interval_seconds,
            config.delivery_mode,
        )
    };
    if !reminder_is_due {
        return Ok(());
    }

    let response_item =
        ContextualUserFragment::into(crate::context::CurrentTimeReminder::new(current_time));
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;

    Ok(())
}
