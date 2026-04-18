use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AppToolApproval;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const RMCP_SERVER: &str = "rmcp";
const RMCP_NAMESPACE: &str = "mcp__rmcp__";
const RMCP_ECHO_TOOL_NAME: &str = "mcp__rmcp__echo";
const RMCP_HOOK_MATCHER: &str = "mcp__rmcp__.*";
const RMCP_ECHO_MESSAGE: &str = "hook e2e ping";

fn write_pre_tool_use_hook(home: &Path, reason: &str) -> Result<()> {
    let script_path = home.join("pre_tool_use_hook.py");
    let log_path = home.join("pre_tool_use_hook_log.jsonl");
    let reason_json = serde_json::to_string(reason).context("serialize pre tool use reason")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": {reason_json}
    }}
}}))
"#,
        log_path = log_path.display(),
        reason_json = reason_json,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": RMCP_HOOK_MATCHER,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running MCP pre tool use hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write pre tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_post_tool_use_hook(home: &Path, additional_context: &str) -> Result<()> {
    let script_path = home.join("post_tool_use_hook.py");
    let log_path = home.join("post_tool_use_hook_log.jsonl");
    let additional_context_json =
        serde_json::to_string(additional_context).context("serialize post tool use context")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "PostToolUse",
        "additionalContext": {additional_context_json}
    }}
}}))
"#,
        log_path = log_path.display(),
        additional_context_json = additional_context_json,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PostToolUse": [{
                "matcher": RMCP_HOOK_MATCHER,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running MCP post tool use hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write post tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_permission_request_hook(home: &Path) -> Result<()> {
    let script_path = home.join("permission_request_hook.py");
    let log_path = home.join("permission_request_hook_log.jsonl");
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "PermissionRequest",
        "decision": {{"behavior": "allow"}}
    }}
}}))
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PermissionRequest": [{
                "matcher": RMCP_HOOK_MATCHER,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running MCP permission request hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write permission request hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn read_hook_inputs(home: &Path, log_name: &str) -> Result<Vec<Value>> {
    fs::read_to_string(home.join(log_name))
        .with_context(|| format!("read {log_name}"))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).with_context(|| format!("parse {log_name} line")))
        .collect()
}

fn insert_rmcp_test_server(config: &mut Config, command: String, approval_mode: AppToolApproval) {
    let mut servers = config.mcp_servers.get().clone();
    servers.insert(
        RMCP_SERVER.to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command,
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            experimental_environment: None,
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(10)),
            tool_timeout_sec: None,
            default_tools_approval_mode: Some(approval_mode),
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );
    if let Err(err) = config.mcp_servers.set(servers) {
        panic!("test mcp servers should accept any configuration: {err}");
    }
}

fn enable_hooks_and_rmcp_server(
    config: &mut Config,
    rmcp_test_server_bin: String,
    approval_mode: AppToolApproval,
) {
    if let Err(err) = config.features.enable(Feature::CodexHooks) {
        panic!("test config should allow feature update: {err}");
    }
    insert_rmcp_test_server(config, rmcp_test_server_bin, approval_mode);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_mcp_tool_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-rmcp-echo";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(call_id, RMCP_NAMESPACE, "echo", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "mcp hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let block_reason = "blocked mcp pre hook";
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            if let Err(error) = write_pre_tool_use_hook(home, block_reason) {
                panic!("failed to write MCP pre tool use hook fixture: {error}");
            }
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(config, rmcp_test_server_bin, AppToolApproval::Approve);
        })
        .build(&server)
        .await?;

    test.submit_turn("call the rmcp echo tool with the MCP pre hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("blocked MCP tool output string");
    assert!(
        output.contains(&format!(
            "Tool call blocked by PreToolUse hook: {block_reason}. Tool: {RMCP_ECHO_TOOL_NAME}"
        )),
        "blocked MCP tool output should surface the hook reason and tool name",
    );

    let hook_inputs = read_hook_inputs(test.codex_home_path(), "pre_tool_use_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_use_id": hook_inputs[0]["tool_use_id"],
            "tool_input": hook_inputs[0]["tool_input"],
        }),
        json!({
            "hook_event_name": "PreToolUse",
            "tool_name": RMCP_ECHO_TOOL_NAME,
            "tool_use_id": call_id,
            "tool_input": { "message": RMCP_ECHO_MESSAGE },
        })
    );
    let transcript_path = hook_inputs[0]["transcript_path"]
        .as_str()
        .expect("pre tool use hook transcript_path");
    assert!(
        Path::new(transcript_path).exists(),
        "pre tool use hook transcript_path should be materialized on disk",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_records_mcp_tool_payload_and_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-rmcp-echo";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let call_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(call_id, RMCP_NAMESPACE, "echo", &arguments),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "mcp post hook context observed"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let post_context = "Remember the MCP post-tool note.";
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            if let Err(error) = write_post_tool_use_hook(home, post_context) {
                panic!("failed to write MCP post tool use hook fixture: {error}");
            }
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(config, rmcp_test_server_bin, AppToolApproval::Approve);
        })
        .build(&server)
        .await?;

    test.submit_turn("call the rmcp echo tool with the MCP post hook")
        .await?;

    let final_request = final_mock.single_request();
    assert!(
        final_request
            .message_input_texts("developer")
            .contains(&post_context.to_string()),
        "follow-up request should include MCP post tool use additional context",
    );
    let output_item = final_request.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("MCP tool output string");
    assert!(
        output.contains(&format!("ECHOING: {RMCP_ECHO_MESSAGE}")),
        "MCP tool output should still reach the model",
    );

    let hook_inputs = read_hook_inputs(test.codex_home_path(), "post_tool_use_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_use_id": hook_inputs[0]["tool_use_id"],
            "tool_input": hook_inputs[0]["tool_input"],
            "tool_response": hook_inputs[0]["tool_response"],
        }),
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": RMCP_ECHO_TOOL_NAME,
            "tool_use_id": call_id,
            "tool_input": { "message": RMCP_ECHO_MESSAGE },
            "tool_response": {
                "content": [],
                "structuredContent": {
                    "echo": format!("ECHOING: {RMCP_ECHO_MESSAGE}"),
                    "env": null,
                },
                "isError": false,
            },
        })
    );
    let transcript_path = hook_inputs[0]["transcript_path"]
        .as_str()
        .expect("post tool use hook transcript_path");
    assert!(
        Path::new(transcript_path).exists(),
        "post tool use hook transcript_path should be materialized on disk",
    );

    call_mock.single_request();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_request_hook_allows_mcp_tool_call() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-rmcp-echo";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let call_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(call_id, RMCP_NAMESPACE, "echo", &arguments),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "mcp permission hook allowed it"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            if let Err(error) = write_permission_request_hook(home) {
                panic!("failed to write MCP permission request hook fixture: {error}");
            }
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(config, rmcp_test_server_bin, AppToolApproval::Prompt);
        })
        .build(&server)
        .await?;

    test.submit_turn_with_policies(
        "call the rmcp echo tool with the MCP permission hook",
        AskForApproval::OnRequest,
        SandboxPolicy::new_read_only_policy(),
    )
    .await?;

    let hook_inputs =
        read_hook_inputs(test.codex_home_path(), "permission_request_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_use_id": hook_inputs[0]["tool_use_id"],
            "tool_input": hook_inputs[0]["tool_input"],
        }),
        json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": RMCP_ECHO_TOOL_NAME,
            "tool_use_id": call_id,
            "tool_input": { "message": RMCP_ECHO_MESSAGE },
        })
    );

    let final_request = final_mock.single_request();
    let output_item = final_request.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("MCP tool output string");
    assert!(
        output.contains(&format!("ECHOING: {RMCP_ECHO_MESSAGE}")),
        "MCP tool output should reach the model after PermissionRequest allows it",
    );

    call_mock.single_request();

    Ok(())
}
