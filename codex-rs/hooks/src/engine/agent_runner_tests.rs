use std::sync::Arc;
use std::sync::Mutex;

use pretty_assertions::assert_eq;

use super::*;
use crate::HookEventName;

#[tokio::test]
async fn agent_hook_without_runner_returns_error() {
    let result = run_agent(
        /*runner*/ None,
        &agent_handler(/*model*/ None),
        r#"{"hook_event_name":"Stop"}"#,
        "gpt-thread".to_string(),
    )
    .await;

    assert_eq!(result.exit_code, None);
    assert_eq!(
        result.error,
        Some("agent hook cannot run because no agent runner is configured".to_string())
    );
}

#[tokio::test]
async fn agent_hook_uses_rendered_prompt_and_default_model() {
    let captured = Arc::new(Mutex::new(None));
    let captured_for_runner = Arc::clone(&captured);
    let runner = AgentHookRunner::new(move |request| {
        *captured_for_runner.lock().expect("capture request") = Some(request);
        async { Ok(r#"{"ok":true}"#.to_string()) }
    });

    let result = run_agent(
        Some(&runner),
        &agent_handler(/*model*/ None),
        r#"{"hook_event_name":"Stop"}"#,
        "gpt-thread".to_string(),
    )
    .await;

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(
        captured.lock().expect("captured request").clone(),
        Some(AgentHookRequest {
            prompt: "Check: {\"hook_event_name\":\"Stop\"}".to_string(),
            model: "gpt-thread".to_string(),
        })
    );
}

#[tokio::test]
async fn agent_hook_ok_false_becomes_block_decision() {
    let runner = AgentHookRunner::new(|_| async {
        Ok(r#"{"ok":false,"reason":"mention tests"}"#.to_string())
    });

    let result = run_agent(
        Some(&runner),
        &agent_handler(Some("gpt-hook".to_string())),
        r#"{"hook_event_name":"Stop"}"#,
        "gpt-thread".to_string(),
    )
    .await;

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&result.stdout).expect("hook output"),
        serde_json::json!({
            "decision": "block",
            "reason": "mention tests",
        })
    );
}

fn agent_handler(model: Option<String>) -> ConfiguredHandler {
    ConfiguredHandler {
        event_name: HookEventName::Stop,
        matcher: None,
        kind: ConfiguredHandlerKind::Agent {
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
