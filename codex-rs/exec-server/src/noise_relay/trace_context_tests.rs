use codex_exec_server_protocol::EXEC_CLOSED_METHOD;
use codex_exec_server_protocol::EXEC_METHOD;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use codex_protocol::protocol::W3cTraceContext;
use pretty_assertions::assert_eq;

use super::NoiseTraceContext;

fn trace_context() -> W3cTraceContext {
    W3cTraceContext {
        traceparent: Some("00-00000000000000000000000000000001-0000000000000002-01".to_string()),
        tracestate: None,
    }
}

fn process_start_request(trace: W3cTraceContext) -> JSONRPCMessage {
    JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(7),
        method: EXEC_METHOD.to_string(),
        params: Some(serde_json::json!({"processId": "process-1"})),
        trace: Some(trace),
    })
}

#[test]
fn correlates_response_and_terminal_notification_with_request_trace() {
    let trace = trace_context();
    let mut context = NoiseTraceContext::default();
    context.observe_request(&process_start_request(trace.clone()));

    let response = JSONRPCMessage::Response(JSONRPCResponse {
        id: RequestId::Integer(7),
        result: serde_json::Value::Null,
    });
    assert_eq!(context.return_trace(&response), Some(trace.clone()));
    assert_eq!(context.return_trace(&response), None);

    let closed = JSONRPCMessage::Notification(JSONRPCNotification {
        method: EXEC_CLOSED_METHOD.to_string(),
        params: Some(serde_json::json!({"processId": "process-1", "seq": 1})),
    });
    assert_eq!(context.return_trace(&closed), Some(trace));
    assert_eq!(context.return_trace(&closed), None);
}
