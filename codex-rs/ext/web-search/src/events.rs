use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_core::web_search_action_detail;
use codex_extension_api::ExtensionEventSink;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;

#[derive(Clone)]
pub(crate) struct WebSearchEventEmitter {
    sink: Arc<dyn ExtensionEventSink>,
}

impl WebSearchEventEmitter {
    pub(crate) fn new(sink: Arc<dyn ExtensionEventSink>) -> Self {
        Self { sink }
    }

    pub(crate) fn start(&self, thread_id: ThreadId, turn_id: &str, call_id: &str) {
        self.sink.emit(Event {
            id: turn_id.to_string(),
            msg: EventMsg::ItemStarted(ItemStartedEvent {
                thread_id,
                turn_id: turn_id.to_string(),
                item: web_search_item(call_id, WebSearchAction::Other),
                started_at_ms: now_unix_timestamp_ms(),
            }),
        });
    }

    pub(crate) fn complete(
        &self,
        thread_id: ThreadId,
        turn_id: &str,
        call_id: &str,
        action: WebSearchAction,
    ) {
        self.sink.emit(Event {
            id: turn_id.to_string(),
            msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id,
                turn_id: turn_id.to_string(),
                item: web_search_item(call_id, action),
                completed_at_ms: now_unix_timestamp_ms(),
            }),
        });
    }
}

fn web_search_item(call_id: &str, action: WebSearchAction) -> TurnItem {
    TurnItem::WebSearch(WebSearchItem {
        id: call_id.to_string(),
        query: web_search_action_detail(&action),
        action,
    })
}

fn now_unix_timestamp_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}
