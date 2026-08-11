use codex_code_mode_protocol::grpc;
use pretty_assertions::assert_eq;
use prost::Message;

use super::MAX_TOOL_ERROR_BYTES;
use super::TRUNCATED_SUFFIX;
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
fn delegate_errors_are_truncated_at_a_utf8_boundary() {
    let error = "🦀".repeat(MAX_TOOL_ERROR_BYTES);
    let completion = request("session", "invocation", Err(error));
    let Some(grpc::complete_tool_call_request::Outcome::Failed(failure)) = completion.outcome
    else {
        panic!("expected a failed tool completion");
    };

    assert!(failure.message.len() <= MAX_TOOL_ERROR_BYTES);
    assert!(failure.message.ends_with(TRUNCATED_SUFFIX));
    assert!(failure.message.starts_with('🦀'));
}

#[test]
fn completion_at_exact_message_limit_is_accepted() {
    let value = serde_json::json!({ "ok": true });
    let expected = request("session", "invocation", Ok(value.clone()));
    let actual = request_with_maximum("session", "invocation", Ok(value), expected.encoded_len());

    assert_eq!(actual, expected);
}
