use codex_code_mode_protocol::grpc;
use pretty_assertions::assert_eq;
use prost::Message;

use super::request;
use super::request_with_maximum;

#[test]
fn completion_size_includes_the_protobuf_envelope() {
    let output = serde_json::Value::String("a".repeat(100));
    let raw_json_bytes = serde_json::to_vec(&output).expect("valid JSON").len();
    let completion = request_with_maximum("session", "invocation", Ok(output), raw_json_bytes);

    assert!(matches!(
        completion.outcome,
        Some(grpc::complete_tool_call_request::Outcome::Failed(grpc::ToolCallFailed {
            message,
        })) if message.contains("encoded bytes exceeds the gRPC message limit")
    ));
}

#[test]
fn delegate_errors_larger_than_64_kib_are_preserved() {
    let error = "🦀".repeat(64 * 1024);
    let completion = request("session", "invocation", Err(error.clone()));
    let Some(grpc::complete_tool_call_request::Outcome::Failed(failure)) = completion.outcome
    else {
        panic!("expected a failed tool completion");
    };

    assert_eq!(failure.message, error);
}

#[test]
fn completion_at_exact_message_limit_is_accepted() {
    let value = serde_json::json!({ "ok": true });
    let expected = request("session", "invocation", Ok(value.clone()));
    let actual = request_with_maximum("session", "invocation", Ok(value), expected.encoded_len());

    assert_eq!(actual, expected);
}
