use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AppToolApproval;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

const RMCP_SERVER: &str = "rmcp";
const RMCP_PREFIXED_NAMESPACE: &str = "mcp__rmcp";
const RMCP_UNPREFIXED_NAMESPACE: &str = "rmcp";
const RMCP_ECHO_TOOL_NAME: &str = "mcp__rmcp__echo";
const RMCP_HOOK_MATCHER: &str = RMCP_ECHO_TOOL_NAME;
const RMCP_ECHO_MESSAGE: &str = "hook e2e ping";

#[derive(Clone, Copy)]
enum PermissionRequestHookOutcome {
    Allow,
    Deny(&'static str),
}

fn enable_mcp_tool_name_features(config: &mut Config, prefix_mcp_tool_names: bool) {
    if !prefix_mcp_tool_names {
        let _ = config.features.enable(Feature::NonPrefixedMcpToolNames);
    }
}

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

fn write_updating_pre_tool_use_hook(home: &Path, updated_message: &str) -> Result<()> {
    let script_path = home.join("pre_tool_use_hook.py");
    let log_path = home.join("pre_tool_use_hook_log.jsonl");
    let updated_message_json =
        serde_json::to_string(updated_message).context("serialize updated MCP message")?;
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
        "permissionDecision": "allow",
        "updatedInput": {{ "message": {updated_message_json} }}
    }}
}}))
"#,
        log_path = log_path.display(),
        updated_message_json = updated_message_json,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": RMCP_HOOK_MATCHER,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "rewriting MCP pre tool input",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write updating pre tool use hook script")?;
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

fn write_permission_request_hook(home: &Path, outcome: PermissionRequestHookOutcome) -> Result<()> {
    let script_path = home.join("permission_request_hook.py");
    let log_path = home.join("permission_request_hook_log.jsonl");
    let decision = match outcome {
        PermissionRequestHookOutcome::Allow => json!({ "behavior": "allow" }),
        PermissionRequestHookOutcome::Deny(message) => {
            json!({ "behavior": "deny", "message": message })
        }
    };
    let hook_output = json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": decision,
        }
    });
    let python_output_literal = serde_json::to_string(&hook_output.to_string())
        .context("serialize MCP permission request hook output")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print({python_output_literal})
"#,
        log_path = log_path.display(),
    );
    let hooks = json!({
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

    fs::write(&script_path, script).context("write MCP permission request hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string())
        .context("write MCP permission request hooks")?;
    Ok(())
}

fn write_mcp_tool_hook(
    home: &Path,
    event_name: &str,
    matcher: Option<&str>,
    server: &str,
    output: &str,
) -> Result<()> {
    let hooks = json!({
        "hooks": {
            event_name: [{
                "matcher": matcher,
                "hooks": [{
                    "type": "mcp_tool",
                    "server": server,
                    "tool": "image_scenario",
                    "input": {
                        "scenario": "text_only",
                        "caption": output,
                    },
                    "timeout": 5,
                }],
            }],
        },
    });
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write MCP tool hook config")
}

fn read_hook_inputs(home: &Path, log_name: &str) -> Result<Vec<Value>> {
    fs::read_to_string(home.join(log_name))
        .with_context(|| format!("read {log_name}"))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).with_context(|| format!("parse {log_name} line")))
        .collect()
}

fn insert_rmcp_test_server(
    config: &mut Config,
    command: String,
    approval_mode: AppToolApproval,
    environment_id: String,
) {
    let mut servers = config.mcp_servers.get().clone();
    servers.insert(
        RMCP_SERVER.to_string(),
        McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::Stdio {
                command,
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            environment_id,
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            omit_tools_from: None,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(10)),
            tool_timeout_sec: None,
            default_tools_approval_mode: Some(approval_mode),
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );
    config
        .mcp_servers
        .set(servers)
        .expect("test mcp servers should accept any configuration");
}

fn enable_hooks_and_rmcp_server(
    config: &mut Config,
    rmcp_test_server_bin: String,
    approval_mode: AppToolApproval,
    prefix_mcp_tool_names: bool,
) {
    trust_discovered_hooks(config);
    enable_mcp_tool_name_features(config, prefix_mcp_tool_names);
    insert_rmcp_test_server(
        config,
        rmcp_test_server_bin,
        approval_mode,
        codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_request_hook_allows_mcp_tool_without_user_or_guardian_review() -> Result<()> {
    run_mcp_permission_request_hook_test(PermissionRequestHookOutcome::Allow).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_request_hook_denies_mcp_tool_without_user_or_guardian_review() -> Result<()> {
    run_mcp_permission_request_hook_test(PermissionRequestHookOutcome::Deny(
        "MCP tool access denied by the integration-test hook",
    ))
    .await
}

async fn run_mcp_permission_request_hook_test(outcome: PermissionRequestHookOutcome) -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = match outcome {
        PermissionRequestHookOutcome::Allow => "permissionrequest-rmcp-allow",
        PermissionRequestHookOutcome::Deny(_) => "permissionrequest-rmcp-deny",
    };
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            write_permission_request_hook(home, outcome)
                .expect("failed to write MCP permission request hook fixture");
        })
        .with_config(move |config| {
            trust_discovered_hooks(config);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            insert_rmcp_test_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Prompt,
                remote_aware_environment_id(),
            );
        });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-mcp-permission-hook-1"),
                ev_function_call_with_namespace(
                    call_id,
                    RMCP_PREFIXED_NAMESPACE,
                    "echo",
                    &arguments,
                ),
                ev_completed("resp-mcp-permission-hook-1"),
            ]),
            sse(vec![
                ev_response_created("resp-mcp-permission-hook-2"),
                ev_assistant_message("msg-mcp-permission-hook", "done"),
                ev_completed("resp-mcp-permission-hook-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_approval_and_permission_profile(
        "call the rmcp echo tool with the MCP permission request hook",
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests.iter().all(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() != Some("guardian")
        }),
        "a permission request hook should resolve MCP approval before Guardian review",
    );

    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("MCP tool output should be a string");
    match outcome {
        PermissionRequestHookOutcome::Allow => assert!(
            output.contains(&format!("ECHOING: {RMCP_ECHO_MESSAGE}")),
            "an allowed MCP tool should execute",
        ),
        PermissionRequestHookOutcome::Deny(message) => assert!(
            output.contains(message),
            "a denied MCP tool should surface the hook's rejection message",
        ),
    }

    let hook_inputs =
        read_hook_inputs(test.codex_home_path(), "permission_request_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_input": hook_inputs[0]["tool_input"],
        }),
        json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": RMCP_ECHO_TOOL_NAME,
            "tool_input": { "message": RMCP_ECHO_MESSAGE },
        }),
    );
    assert!(
        hook_inputs[0].get("tool_use_id").is_none(),
        "PermissionRequest input should not include a tool_use_id",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_hook_interpolates_prompt_and_runs_without_tool_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hook context received"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_mcp_tool_hook(
                home,
                "UserPromptSubmit",
                /*matcher*/ None,
                RMCP_SERVER,
                r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"MCP scanner checked ${prompt}"}}"#,
            )
            .expect("write MCP prompt hook fixture");
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Prompt,
                /*prefix_mcp_tool_names*/ true,
            );
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    tokio::time::timeout(
        Duration::from_secs(15),
        test.submit_turn("review checkout.rs"),
    )
    .await
    .context("MCP hook must not wait for model-tool approval")??;

    assert!(
        response
            .single_request()
            .message_input_texts("developer")
            .contains(&"MCP scanner checked review checkout.rs".to_string())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_hook_passes_thread_metadata_to_model_hidden_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "thread context received"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            let hooks = json!({
                "hooks": {
                    "UserPromptSubmit": [{
                        "hooks": [{
                            "type": "mcp_tool",
                            "server": RMCP_SERVER,
                            "tool": "thread_hint",
                            "input": {},
                            "timeout": 5,
                        }],
                    }],
                },
            });
            fs::write(home.join("hooks.json"), hooks.to_string())
                .expect("write model-hidden MCP tool hook config");
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Prompt,
                /*prefix_mcp_tool_names*/ true,
            );
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    test.submit_turn("load thread context").await?;

    let request = response.single_request();
    let expected_context = format!(
        "manual history hint for thread {}\nunstructured notes/thread_hint fixture result",
        test.session_configured.thread_id,
    );
    assert!(
        request
            .message_input_texts("developer")
            .contains(&expected_context)
    );
    assert!(
        request
            .tool_by_name(RMCP_PREFIXED_NAMESPACE, "thread_hint")
            .is_none(),
        "the hook tool must remain hidden from the model"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_hook_marks_thread_memory_mode_polluted_when_configured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hook context received"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_mcp_tool_hook(
                home,
                "UserPromptSubmit",
                /*matcher*/ None,
                RMCP_SERVER,
                r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"external MCP context"}}"#,
            )
            .expect("write MCP prompt hook fixture");
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Prompt,
                /*prefix_mcp_tool_names*/ true,
            );
            config
                .features
                .enable(Feature::Sqlite)
                .expect("test config should allow feature update");
            config.memories.disable_on_external_context = true;
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.thread_id;
    test.submit_turn("review checkout.rs").await?;

    assert_eq!(
        db.get_thread_memory_mode(thread_id).await?,
        Some("polluted".to_string())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_hook_blocks_model_tool_without_recursive_hooks_or_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "mcp-hook-denied-echo";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    call_id,
                    RMCP_PREFIXED_NAMESPACE,
                    "echo",
                    &arguments,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "the MCP hook blocked that tool"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_mcp_tool_hook(
                home,
                "PreToolUse",
                Some("mcp__rmcp__.*"),
                RMCP_SERVER,
                r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"MCP blocked ${tool_input.message}"}}"#,
            )
            .expect("write MCP denial hook fixture");
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Prompt,
                /*prefix_mcp_tool_names*/ true,
            );
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    tokio::time::timeout(
        Duration::from_secs(15),
        test.submit_turn("call the blocked MCP tool"),
    )
    .await
    .context("MCP hook must not recurse or request model-tool approval")??;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].function_call_output(call_id);
    let output = output["output"]
        .as_str()
        .context("blocked model tool output")?;
    assert!(output.contains("MCP blocked hook e2e ping"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_hook_fails_open_when_server_is_unavailable() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "continued without the missing server"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_mcp_tool_hook(
                home,
                "UserPromptSubmit",
                /*matcher*/ None,
                "missing-server",
                r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"this must not appear"}}"#,
            )
            .expect("write unavailable MCP server fixture");
        })
        .with_config(trust_discovered_hooks)
        .build(&server)
        .await?;

    tokio::time::timeout(Duration::from_secs(5), test.submit_turn("continue"))
        .await
        .context("unavailable MCP hook server must fail immediately")??;

    assert!(
        !response
            .single_request()
            .message_input_texts("developer")
            .iter()
            .any(|message| message.contains("this must not appear"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_mcp_tool_before_execution_with_legacy_prefixed_names() -> Result<()> {
    pre_tool_use_blocks_mcp_tool_before_execution(
        /*prefix_mcp_tool_names*/ true,
        RMCP_PREFIXED_NAMESPACE,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_mcp_tool_before_execution_with_non_prefixed_names() -> Result<()> {
    pre_tool_use_blocks_mcp_tool_before_execution(
        /*prefix_mcp_tool_names*/ false,
        RMCP_UNPREFIXED_NAMESPACE,
    )
    .await
}

async fn pre_tool_use_blocks_mcp_tool_before_execution(
    prefix_mcp_tool_names: bool,
    mcp_namespace: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-rmcp-echo";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(call_id, mcp_namespace, "echo", &arguments),
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
            write_pre_tool_use_hook(home, block_reason)
                .expect("failed to write MCP pre tool use hook fixture");
        })
        .with_config(move |config| {
            let _ = config.features.enable(Feature::ExecutedToolCallMetadata);
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Approve,
                prefix_mcp_tool_names,
            );
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    test.submit_turn("call the rmcp echo tool with the MCP pre hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    assert_eq!(
        output_item["internal_chat_message_metadata_passthrough"]["executed_tool_calls"],
        json!([{
            "name": format!("{mcp_namespace}__echo"),
            "arguments": { "message": RMCP_ECHO_MESSAGE },
        }]),
        "a blocked MCP request must still retain the original model-attempted call",
    );
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("blocked MCP tool output should be a string");
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
        .expect("pre tool use hook transcript_path should be a string");
    assert!(
        Path::new(transcript_path).exists(),
        "pre tool use hook transcript_path should be materialized on disk",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_rewrites_mcp_tool_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-rmcp-echo-rewrite";
    let rewritten_message = "rewritten mcp hook input";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let call_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(call_id, RMCP_PREFIXED_NAMESPACE, "echo", &arguments),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "mcp pre hook rewrote it"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            write_updating_pre_tool_use_hook(home, rewritten_message)
                .expect("failed to write MCP updating pre tool use hook fixture");
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Approve,
                /*prefix_mcp_tool_names*/ true,
            );
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

    test.submit_turn("call the rmcp echo tool with the MCP pre hook rewrite")
        .await?;

    let final_request = final_mock.single_request();
    let output_item = final_request.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("MCP tool output should be a string");
    assert!(
        output.contains(&format!("ECHOING: {rewritten_message}")),
        "MCP tool should execute the rewritten input",
    );
    assert!(
        !output.contains(RMCP_ECHO_MESSAGE),
        "MCP tool should not execute the original input",
    );

    let hook_inputs = read_hook_inputs(test.codex_home_path(), "pre_tool_use_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_input"],
        json!({ "message": RMCP_ECHO_MESSAGE }),
    );

    call_mock.single_request();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_records_mcp_tool_payload_and_context_with_legacy_prefixed_names()
-> Result<()> {
    post_tool_use_records_mcp_tool_payload_and_context(
        /*prefix_mcp_tool_names*/ true,
        RMCP_PREFIXED_NAMESPACE,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_records_mcp_tool_payload_and_context_with_non_prefixed_names() -> Result<()>
{
    post_tool_use_records_mcp_tool_payload_and_context(
        /*prefix_mcp_tool_names*/ false,
        RMCP_UNPREFIXED_NAMESPACE,
    )
    .await
}

async fn post_tool_use_records_mcp_tool_payload_and_context(
    prefix_mcp_tool_names: bool,
    mcp_namespace: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-rmcp-echo";
    let arguments = json!({ "message": RMCP_ECHO_MESSAGE }).to_string();
    let call_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(call_id, mcp_namespace, "echo", &arguments),
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
            write_post_tool_use_hook(home, post_context)
                .expect("failed to write MCP post tool use hook fixture");
        })
        .with_config(move |config| {
            enable_hooks_and_rmcp_server(
                config,
                rmcp_test_server_bin,
                AppToolApproval::Approve,
                prefix_mcp_tool_names,
            );
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, RMCP_SERVER).await?;

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
        .expect("MCP tool output should be a string");
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
        .expect("post tool use hook transcript_path should be a string");
    assert!(
        Path::new(transcript_path).exists(),
        "post tool use hook transcript_path should be materialized on disk",
    );

    call_mock.single_request();

    Ok(())
}
