use codex_protocol::protocol::W3cTraceContext;
use pretty_assertions::assert_eq;

use super::JSONRPCNotification;

#[test]
fn notification_trace_is_optional_and_omitted_when_absent() {
    let notification: JSONRPCNotification = serde_json::from_value(serde_json::json!({
        "method": "initialized",
        "params": {}
    }))
    .expect("notification without trace should deserialize");

    assert_eq!(notification.trace, None);
    assert_eq!(
        serde_json::to_value(notification).expect("notification should serialize"),
        serde_json::json!({
            "method": "initialized",
            "params": {}
        })
    );
}

#[test]
fn notification_trace_round_trips() {
    let trace = W3cTraceContext {
        traceparent: Some("00-00000000000000000000000000000001-0000000000000002-01".to_string()),
        tracestate: Some("vendor=value".to_string()),
    };
    let notification = JSONRPCNotification {
        method: "process/exited".to_string(),
        params: Some(serde_json::json!({"processId": "process-1"})),
        trace: Some(trace.clone()),
    };

    let encoded = serde_json::to_value(&notification).expect("notification should serialize");
    assert_eq!(
        encoded["trace"],
        serde_json::to_value(trace).expect("trace should serialize")
    );
    assert_eq!(
        serde_json::from_value::<JSONRPCNotification>(encoded)
            .expect("notification should deserialize"),
        notification
    );
}

#[test]
fn malformed_notification_trace_is_ignored() {
    for trace in [serde_json::json!(7), serde_json::json!({"traceparent": 7})] {
        let notification: JSONRPCNotification = serde_json::from_value(serde_json::json!({
            "method": "process/exited",
            "params": {},
            "trace": trace,
        }))
        .expect("notification with malformed trace should deserialize");

        assert_eq!(notification.trace, None);
    }
}
