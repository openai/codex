use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn prompt_hook_without_runner_returns_error() {
    let result = run_prompt(
        /*runner*/ None,
        &prompt_handler(/*model*/ None),
        r#"{"hook_event_name":"Stop"}"#,
        "gpt-thread".to_string(),
    )
    .await;

    assert_eq!(result.exit_code, None);
    assert_eq!(
        result.error,
        Some("prompt hook cannot run because no prompt runner is configured".to_string())
    );
}

#[test]
fn render_prompt_replaces_arguments_placeholder() {
    assert_eq!(
        render_model_hook_prompt("Check: $ARGUMENTS", r#"{"event":"Stop"}"#),
        r#"Check: {"event":"Stop"}"#
    );
}

#[test]
fn render_prompt_appends_arguments_without_placeholder() {
    assert_eq!(
        render_model_hook_prompt("Check the turn.", r#"{"event":"Stop"}"#),
        "Check the turn.\n\n{\"event\":\"Stop\"}"
    );
}

#[test]
fn render_prompt_caps_model_input() {
    let rendered = render_model_hook_prompt("$ARGUMENTS", &"word ".repeat(20_000));

    assert!(codex_utils_output_truncation::approx_token_count(&rendered) <= 10_000);
    assert!(rendered.contains("tokens truncated"));
}

#[test]
fn stop_ok_false_becomes_block_decision() {
    assert_json_eq(
        model_hook_output_to_command_stdout(
            "prompt",
            HookEventName::Stop,
            /*continue_on_block*/ false,
            r#"{"ok":false,"reason":"mention tests"}"#,
        )
        .expect("prompt output"),
        json!({
            "decision": "block",
            "reason": "mention tests",
        }),
    );
}

#[test]
fn permission_request_ok_false_records_reason_without_decision() {
    assert_json_eq(
        model_hook_output_to_command_stdout(
            "prompt",
            HookEventName::PermissionRequest,
            /*continue_on_block*/ false,
            r#"{"ok":false,"reason":"looks suspicious"}"#,
        )
        .expect("prompt output"),
        json!({
            "systemMessage": "looks suspicious",
        }),
    );
}

#[test]
fn post_tool_use_ok_false_honors_continue_on_block() {
    assert_json_eq(
        model_hook_output_to_command_stdout(
            "prompt",
            HookEventName::PostToolUse,
            /*continue_on_block*/ true,
            r#"{"ok":false,"reason":"summarize the command output"}"#,
        )
        .expect("prompt output"),
        json!({
            "decision": "block",
            "reason": "summarize the command output",
        }),
    );
    assert_json_eq(
        model_hook_output_to_command_stdout(
            "prompt",
            HookEventName::PostToolUse,
            /*continue_on_block*/ false,
            r#"{"ok":false,"reason":"stop here"}"#,
        )
        .expect("prompt output"),
        json!({
            "continue": false,
            "decision": "block",
            "reason": "stop here",
            "stopReason": "stop here",
        }),
    );
}

fn assert_json_eq(actual: String, expected: serde_json::Value) {
    let actual: serde_json::Value = serde_json::from_str(&actual).expect("json output");
    assert_eq!(actual, expected);
}

fn prompt_handler(model: Option<String>) -> ConfiguredHandler {
    ConfiguredHandler {
        event_name: HookEventName::Stop,
        matcher: None,
        kind: ConfiguredHandlerKind::Prompt {
            prompt: "Check: $ARGUMENTS".to_string(),
            model,
            timeout_sec: 30,
            continue_on_block: true,
        },
        status_message: None,
        source_path: codex_utils_absolute_path::AbsolutePathBuf::current_dir().expect("cwd"),
        source: codex_protocol::protocol::HookSource::User,
        display_order: 0,
        env: std::collections::HashMap::new(),
    }
}
