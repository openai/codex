use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::mpsc;

use super::*;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingNotification;

#[tokio::test]
async fn extension_event_sink_forwards_warning_with_request_and_thread_metadata() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel();
    let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
    let thread_id = ThreadId::new();
    let request_id = RequestId::String("request-1".into());
    let running_requests = Arc::new(Mutex::new(HashMap::from([(request_id.clone(), thread_id)])));
    let sink = McpServerExtensionEventSink {
        outgoing,
        running_requests,
    };

    sink.emit(Event {
        id: thread_id.to_string(),
        msg: EventMsg::Warning(WarningEvent {
            message: "skill warning".to_string(),
        }),
    });

    let OutgoingMessage::Notification(OutgoingNotification { method, params }) =
        outgoing_rx.recv().await.expect("notification")
    else {
        panic!("expected extension notification");
    };
    assert_eq!(method, "codex/event");
    assert_eq!(
        params,
        Some(json!({
            "_meta": {
                "requestId": request_id,
                "threadId": thread_id,
            },
            "id": thread_id.to_string(),
            "msg": {
                "type": "warning",
                "message": "skill warning",
            },
        }))
    );
}
