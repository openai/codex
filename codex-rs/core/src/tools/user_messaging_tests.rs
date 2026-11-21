use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolCallState;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::user_messaging::capture_delivery;
use codex_guardian_context::ContextBudgeting;
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
    capture_delivery(
        &messaging_result(name, input, success),
        Some(&state),
        ContextBudgeting::Fixed,
    );
    assert_eq!(state.delivered_assistant_message.get(), None);
}

const LONG_MESSAGE_BYTES: usize = 4_000;

#[test_case(ContextBudgeting::Fixed; "fixed")]
#[test_case(ContextBudgeting::Elastic; "elastic")]
fn capture_keeps_the_first_confirmed_rewritten_message(budgeting: ContextBudgeting) {
    let state = ToolCallState::default();
    let name = "mcp__codex_apps__user_message__send_message";
    let rewritten = "x".repeat(LONG_MESSAGE_BYTES);
    for text in [rewritten.as_str(), "Replace the question?"] {
        capture_delivery(
            &messaging_result(name, json!({"text": text}), /*success*/ true),
            Some(&state),
            budgeting,
        );
    }
    let expected = match budgeting {
        ContextBudgeting::Fixed => {
            crate::guardian::guardian_truncate_text(
                &rewritten,
                crate::guardian::GUARDIAN_MAX_ROOT_MESSAGE_TOKENS,
            )
            .0
        }
        ContextBudgeting::Elastic => rewritten,
    };
    assert_eq!(state.delivered_assistant_message.get(), Some(&expected));
}

fn messaging_result(name: &str, input: Value, success: bool) -> AnyToolResult {
    AnyToolResult {
        call_id: "message-call".into(),
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
