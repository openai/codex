use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolCallState;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::user_messaging::capture_delivery;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;

#[test_case("mcp__codex_apps__slack__send_message", json!({"text": "Staging only?"}), true; "other_app")]
#[test_case("mcp__codex_apps__user_message__send_message_to_thread", json!({"text": "Staging only?"}), true; "other_user_message_action")]
#[test_case("mcp__codex_apps__user_messaging__send_message", json!({"text": "Staging only?"}), false; "failed_send")]
#[test_case("mcp__codex_apps__user_messaging__send_message", json!({"text": " "}), true; "empty_text")]
fn undelivered_or_unrelated_messages_have_no_evidence(name: &str, input: Value, success: bool) {
    let state = ToolCallState::default();
    capture_delivery(&messaging_result(name, input, success), Some(&state));
    assert_eq!(state.delivered_assistant_message.get(), None);
}

#[test]
fn capture_keeps_the_first_confirmed_rewritten_message() {
    let state = ToolCallState::default();
    let name = "mcp__codex_apps__user_message__send_message";
    for text in ["Staging only?", "Replace the question?"] {
        capture_delivery(
            &messaging_result(name, json!({"text": text}), /*success*/ true),
            Some(&state),
        );
    }
    assert_eq!(
        state.delivered_assistant_message.get().map(String::as_str),
        Some("Staging only?")
    );
}

fn messaging_result(name: &str, input: Value, success: bool) -> AnyToolResult {
    AnyToolResult {
        call_id: "message-call".to_owned(),
        payload: ToolPayload::Function {
            arguments: r#"{"text":"Original question before hooks"}"#.to_owned(),
        },
        result: Box::new(FunctionToolOutput::from_text(
            "Message sent.".to_owned(),
            Some(success),
        )),
        post_tool_use_payload: Some(PostToolUsePayload {
            tool_cwd: None,
            tool_name: HookToolName::new(name),
            tool_use_id: "message-call".to_owned(),
            tool_input: input,
            tool_response: json!({}),
        }),
    }
}
