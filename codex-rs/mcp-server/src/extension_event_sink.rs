use std::sync::Arc;

use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionWarning;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;

use crate::active_turn_registry::ActiveTurnRegistry;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotificationMeta;

const MAX_EXTENSION_WARNING_BYTES: usize = 256;

pub(crate) fn extension_event_sink(
    outgoing: Arc<OutgoingMessageSender>,
    active_turns: Arc<ActiveTurnRegistry>,
) -> Arc<dyn ExtensionEventSink> {
    Arc::new(McpExtensionEventSink {
        outgoing,
        active_turns,
    })
}

struct McpExtensionEventSink {
    outgoing: Arc<OutgoingMessageSender>,
    active_turns: Arc<ActiveTurnRegistry>,
}

impl ExtensionEventSink for McpExtensionEventSink {
    fn emit(&self, event: Event) {
        tracing::debug!(
            event_id = %event.id,
            msg = ?event.msg,
            "dropping unsupported extension event"
        );
    }

    fn emit_warning(&self, warning: ExtensionWarning) {
        let ExtensionWarning {
            thread_id,
            turn_id,
            message,
        } = warning;
        let Ok(thread_id) = ThreadId::from_string(&thread_id) else {
            tracing::warn!(%thread_id, "dropping extension warning with invalid thread id");
            return;
        };
        let Some(turn_id) = turn_id else {
            tracing::debug!(%thread_id, "dropping extension warning without a turn id");
            return;
        };
        let mut message = message;
        if message.len() > MAX_EXTENSION_WARNING_BYTES {
            let mut truncate_at = MAX_EXTENSION_WARNING_BYTES;
            while !message.is_char_boundary(truncate_at) {
                truncate_at -= 1;
            }
            message.truncate(truncate_at);
        }
        let found_active_turn =
            self.active_turns
                .with_request_id(thread_id, &turn_id, |request_id| {
                    let event = Event {
                        id: turn_id.clone(),
                        msg: EventMsg::Warning(WarningEvent { message }),
                    };
                    self.outgoing.send_event_as_notification(
                        &event,
                        Some(OutgoingNotificationMeta {
                            request_id: Some(request_id.clone()),
                            thread_id: Some(thread_id),
                        }),
                    );
                });
        if !found_active_turn {
            tracing::debug!(
                %thread_id,
                %turn_id,
                "dropping extension warning without a matching active turn"
            );
        }
    }
}

#[cfg(test)]
#[path = "extension_event_sink_tests.rs"]
mod tests;
