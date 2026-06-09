use super::LocalResponsesApiCallReducer;
use super::LocalResponsesApiCallStartedFact;
use super::LocalResponsesApiCallTerminalFact;
use super::LocalResponsesApiTransport;
use crate::LocalAnalyticsRecordType;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn reducer_emits_one_responses_record_after_terminal_fact() {
    let mut reducer = LocalResponsesApiCallReducer::default();
    reducer.ingest_started(LocalResponsesApiCallStartedFact {
        responses_call_id: "call-1".to_string(),
        session_id: "session-1".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        context_window_id: "thread-1:0".to_string(),
        transport: LocalResponsesApiTransport::Http,
        request_started_at_epoch_millis: 10,
        request_json: json!({"model": "gpt-test"}),
    });

    let record = reducer
        .ingest_terminal(LocalResponsesApiCallTerminalFact::Completed {
            responses_call_id: "call-1".to_string(),
            completed_at_epoch_millis: 20,
            response_id: "response-1".to_string(),
            upstream_request_id: Some("request-1".to_string()),
            token_usage_json: Some(json!({"total_tokens": 3})),
            output_items: vec![json!({"type": "message"})],
        })
        .expect("terminal fact should reduce");

    assert_eq!(
        record.record_type,
        LocalAnalyticsRecordType::ResponsesApiCall
    );
    assert_eq!(record.session_id.as_deref(), Some("session-1"));
    assert_eq!(
        record.payload,
        json!({
            "responses_call_id": "call-1",
            "context_window_id": "thread-1:0",
            "transport": "http",
            "status": "completed",
            "request_started_at_epoch_millis": 10,
            "completed_at_epoch_millis": 20,
            "response_id": "response-1",
            "upstream_request_id": "request-1",
            "request_json": {"model": "gpt-test"},
            "response_json": {
                "response_id": "response-1",
                "upstream_request_id": "request-1",
                "output_items": [{"type": "message"}]
            },
            "token_usage_json": {"total_tokens": 3},
            "error_json": null
        })
    );
}
