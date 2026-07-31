use std::sync::Arc;

use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionWarning;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;
use pretty_assertions::assert_eq;
use rmcp::model::RequestId;
use serde_json::json;
use tokio::sync::mpsc;

use super::extension_event_sink;
use crate::active_turn_registry::ActiveTurnRegistry;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotification;

fn test_sink() -> (
    Arc<dyn ExtensionEventSink>,
    Arc<OutgoingMessageSender>,
    Arc<ActiveTurnRegistry>,
    mpsc::UnboundedReceiver<OutgoingMessage>,
) {
    let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();
    let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
    let active_turns = Arc::new(ActiveTurnRegistry::default());
    (
        extension_event_sink(Arc::clone(&outgoing), Arc::clone(&active_turns)),
        outgoing,
        active_turns,
        outgoing_rx,
    )
}

#[tokio::test(flavor = "current_thread")]
async fn warning_is_enqueued_before_following_response() {
    let (sink, outgoing, active_turns, mut outgoing_rx) = test_sink();
    let request_id = RequestId::Number(7);
    let thread_id = ThreadId::new();
    let turn_id = "turn-7".to_string();
    active_turns.register(request_id.clone(), thread_id, turn_id.clone());

    sink.emit_warning(ExtensionWarning {
        thread_id: thread_id.to_string(),
        turn_id: Some(turn_id),
        message: "warning".to_string(),
    });
    active_turns.finish(&request_id, || {
        outgoing.send_response(request_id.clone(), json!({}));
    });

    let Some(OutgoingMessage::Notification(notification)) = outgoing_rx.recv().await else {
        panic!("warning notification should be enqueued before the response");
    };
    assert_eq!(notification.method, "codex/event");
    assert_eq!(
        notification.params.as_ref().map(|params| &params["msg"]),
        Some(&json!({
            "message": "warning",
            "type": "warning",
        }))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn colliding_request_id_strings_route_warnings_independently() {
    let (sink, _outgoing, active_turns, mut outgoing_rx) = test_sink();
    let thread_id = ThreadId::new();
    let numeric_turn_id = "019c0000-0000-7000-8000-000000000001".to_string();
    let string_turn_id = "019c0000-0000-7000-8000-000000000002".to_string();
    active_turns.register(RequestId::Number(7), thread_id, numeric_turn_id.clone());
    active_turns.register(
        RequestId::String("7".into()),
        thread_id,
        string_turn_id.clone(),
    );

    sink.emit_warning(ExtensionWarning {
        thread_id: thread_id.to_string(),
        turn_id: Some(numeric_turn_id.clone()),
        message: "numeric warning".to_string(),
    });
    sink.emit_warning(ExtensionWarning {
        thread_id: thread_id.to_string(),
        turn_id: Some(string_turn_id.clone()),
        message: "string warning".to_string(),
    });

    let Some(OutgoingMessage::Notification(numeric_warning)) = outgoing_rx.recv().await else {
        panic!("expected warning for numeric request id");
    };
    assert_eq!(
        numeric_warning,
        OutgoingNotification {
            method: "codex/event".to_string(),
            params: Some(json!({
                "_meta": {
                    "requestId": 7,
                    "threadId": thread_id,
                },
                "id": numeric_turn_id,
                "msg": {
                    "message": "numeric warning",
                    "type": "warning",
                },
            })),
        }
    );

    let Some(OutgoingMessage::Notification(string_warning)) = outgoing_rx.recv().await else {
        panic!("expected warning for string request id");
    };
    assert_eq!(
        string_warning,
        OutgoingNotification {
            method: "codex/event".to_string(),
            params: Some(json!({
                "_meta": {
                    "requestId": "7",
                    "threadId": thread_id,
                },
                "id": string_turn_id,
                "msg": {
                    "message": "string warning",
                    "type": "warning",
                },
            })),
        }
    );
}

#[tokio::test(flavor = "current_thread")]
async fn warning_requires_a_matching_active_turn() {
    let (sink, _outgoing, active_turns, mut outgoing_rx) = test_sink();
    let request_id = RequestId::Number(7);
    let thread_id = ThreadId::new();
    active_turns.register(request_id, thread_id, "turn-7".to_string());

    sink.emit_warning(ExtensionWarning {
        thread_id: thread_id.to_string(),
        turn_id: Some("different-turn".to_string()),
        message: "warning".to_string(),
    });
    tokio::task::yield_now().await;

    assert!(outgoing_rx.try_recv().is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn warning_is_truncated_to_256_utf8_bytes() {
    let (sink, _outgoing, active_turns, mut outgoing_rx) = test_sink();
    let request_id = RequestId::Number(7);
    let thread_id = ThreadId::new();
    let turn_id = "turn-7".to_string();
    active_turns.register(request_id.clone(), thread_id, turn_id.clone());

    sink.emit_warning(ExtensionWarning {
        thread_id: thread_id.to_string(),
        turn_id: Some(turn_id),
        message: format!("{}é", "a".repeat(255)),
    });

    let Some(OutgoingMessage::Notification(notification)) = outgoing_rx.recv().await else {
        panic!("expected warning notification");
    };
    assert_eq!(
        notification.params.as_ref().map(|params| &params["msg"]),
        Some(&json!({
            "message": "a".repeat(255),
            "type": "warning",
        }))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn generic_extension_events_are_dropped() {
    let (sink, _outgoing, _running_requests, mut outgoing_rx) = test_sink();

    sink.emit(Event {
        id: "turn".to_string(),
        msg: EventMsg::Warning(WarningEvent {
            message: "warning".to_string(),
        }),
    });
    tokio::task::yield_now().await;

    assert!(outgoing_rx.try_recv().is_err());
}
