use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

use super::expand_mcp_argument_template;
use super::run_mcp_tool;
use crate::engine::ConfiguredHandler;
use crate::engine::ConfiguredHandlerKind;
use crate::mcp::HookMcpCall;
use crate::mcp::HookMcpExecutor;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;

struct RecordingExecutor {
    calls: Arc<Mutex<Vec<HookMcpCall>>>,
    output: String,
}

impl HookMcpExecutor for RecordingExecutor {
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<String>> {
        async move {
            self.calls.lock().expect("lock calls").push(call);
            Ok(self.output.clone())
        }
        .boxed()
    }
}

#[test]
fn placeholders_preserve_json_types_and_expand_nested_inputs() {
    let event_input = json!({
        "tool_input": {
            "file_path": "/tmp/example.rs",
            "count": 3,
            "optional": null,
            "metadata": { "language": "rust" },
        },
    });
    let input = serde_json::from_value::<Map<String, Value>>(json!({
        "path": "${tool_input.file_path}",
        "message": "scan ${tool_input.file_path}",
        "count": "${tool_input.count}",
        "optional": "${tool_input.optional}",
        "nested": ["${tool_input.metadata}", { "literal": true }],
    }))
    .expect("object input");

    assert_eq!(
        Value::Object(
            expand_mcp_argument_template(&input, &event_input).expect("expand MCP arguments")
        ),
        json!({
            "path": "/tmp/example.rs",
            "message": "scan /tmp/example.rs",
            "count": 3,
            "optional": null,
            "nested": [{ "language": "rust" }, { "literal": true }],
        })
    );
}

#[test]
fn missing_placeholder_fails_without_passing_unresolved_input() {
    let input = serde_json::from_value::<Map<String, Value>>(json!({
        "path": "${tool_input.missing}",
    }))
    .expect("object input");

    let error = expand_mcp_argument_template(&input, &json!({ "tool_input": {} }))
        .expect_err("missing placeholder should fail");

    assert_eq!(
        error.to_string(),
        "hook input placeholder `${tool_input.missing}` was not found"
    );
}

#[tokio::test]
async fn mcp_tool_results_use_command_hook_output_contract() {
    let configured_input = serde_json::from_value::<Map<String, Value>>(json!({
        "file_path": "${tool_input.file_path}",
    }))
    .expect("object input");
    let handler = ConfiguredHandler {
        builtin: false,
        event_name: HookEventName::PostToolUse,
        matcher: None,
        timeout_sec: 30,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: test_path_buf("/tmp/hooks.json").abs().into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::McpTool {
            server: "security".to_string(),
            tool: "scan".to_string(),
            input: configured_input.clone(),
        },
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let executor = RecordingExecutor {
        calls: Arc::clone(&calls),
        output: r#"{"decision":"block","reason":"unsafe file"}"#.to_string(),
    };

    let result = run_mcp_tool(
        &executor,
        &handler,
        "security",
        "scan",
        &configured_input,
        r#"{"tool_input":{"file_path":"/tmp/example.rs"}}"#,
        /*metadata*/ None,
    )
    .await;

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout, executor.output);
    assert_eq!(result.error, None);
    assert_eq!(
        *calls.lock().expect("lock calls"),
        vec![HookMcpCall {
            server: "security".to_string(),
            tool: "scan".to_string(),
            environment_id: None,
            metadata: None,
            input: serde_json::from_value(json!({ "file_path": "/tmp/example.rs" }))
                .expect("object input"),
            timeout: Duration::from_secs(30),
        }]
    );
}
