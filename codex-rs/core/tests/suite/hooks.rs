use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::config::ThreadStoreConfig;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::built_in_model_providers;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginId;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use codex_thread_store::InMemoryThreadStore;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::TestTargetOs;
use core_test_support::fs_wait;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::hooks::trust_hooks;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_host_windows;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::test_target_os;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::time::timeout;

const FIRST_CONTINUATION_PROMPT: &str = "Retry with exactly the phrase meow meow meow.";
const SECOND_CONTINUATION_PROMPT: &str = "Now tighten it to just: meow.";
const BLOCKED_PROMPT_CONTEXT: &str = "Remember the blocked lighthouse note.";
const PERMISSION_REQUEST_HOOK_MATCHER: &str = "^Bash$";
const PERMISSION_REQUEST_ALLOW_REASON: &str = "should not be used for allow";
// copybara:strip-for-public begin
async fn with_tool_analytics(
    builder: core_test_support::test_codex::TestCodexBuilder,
    server: &wiremock::MockServer,
) -> Result<core_test_support::test_codex::TestCodexBuilder> {
    if !cfg!(debug_assertions) {
        return Ok(builder);
    }

    wiremock::Mock::given(wiremock::matchers::path("/codex/analytics-events/events"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(server)
        .await;
    let auth = codex_login::CodexAuth::from_external_chatgpt_tokens(
        "e30.eyJlbWFpbCI6ImFsaWNlQG9wZW5haS5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoiZW1wbG95ZWUtYWxpY2UifX0.sig",
        "account-test",
        /*chatgpt_plan_type*/ None,
    )?;
    let chatgpt_base_url = server.uri();
    Ok(builder.with_auth(auth).with_config(move |config| {
        config.chatgpt_base_url = chatgpt_base_url;
        config.analytics_enabled = Some(true);
    }))
}
async fn assert_internal_tool_input(
    server: &wiremock::MockServer,
    tool_call_id: &str,
    expected: Value,
) -> Result<()> {
    if !cfg!(debug_assertions) {
        return Ok(());
    }

    timeout(Duration::from_secs(10), async {
        loop {
            for request in server.received_requests().await.unwrap_or_default() {
                if request.url.path() != "/codex/analytics-events/events" {
                    continue;
                }
                let envelope: Value = serde_json::from_slice(&request.body)?;
                let event = &envelope["events"][0];
                if event["event_type"] != "codex_internal_structured_log_event" {
                    continue;
                }
                let encoded = event["event_params"]["fields_json"]
                    .as_str()
                    .context("encoded tool input")?;
                let fields: Value = serde_json::from_str(encoded)?;
                if fields["tool_call_id"] != tool_call_id {
                    continue;
                }
                assert_eq!(envelope["events"], serde_json::json!([event]));
                assert_eq!(fields["item_id"], tool_call_id);
                for (key, value) in expected.as_object().context("expected analytics fields")? {
                    assert_eq!(fields[key], *value, "internal tool field {key}");
                }
                let nested = fields["source_kind"] == "code_mode";
                for key in ["code_mode_cell_id", "code_mode_runtime_tool_call_id"] {
                    assert_eq!(fields[key].is_string(), nested);
                }
                assert_eq!(event["event_params"]["thread_id"], fields["thread_id"]);
                assert_eq!(event["event_params"]["turn_id"], fields["turn_id"]);
                return Ok::<_, anyhow::Error>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await?
}
#[tokio::test]
async fn exec_command_without_hooks_logs_command_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let call_id = "exec-command-without-hooks";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&serde_json::json!({ "cmd": "git version" }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = with_tool_analytics(test_codex(), &server).await?;
    let test = builder.build(&server).await?;
    test.submit_turn_with_permission_profile(
        "run the shell command without hooks",
        PermissionProfile::Disabled,
    )
    .await?;
    let metadata =
        serde_json::json!({"commands": [{"command": "git", "subcommand": "version", "flags": []}]});
    assert_internal_tool_input(
        &server,
        call_id,
        serde_json::json!({
            "tool_name": "exec_command",
            "source_kind": "direct",
            "arguments_before_hooks": metadata.clone(),
            "hook_normalized_input": metadata.clone(),
            "arguments_after_hooks": metadata,
            "hook_outcome": "unchanged",
        }),
    )
    .await
}

#[tokio::test]
async fn user_shell_logs_command_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "user-shell logging requires a host-native shell");

    let server = start_mock_server().await;
    let mut builder = with_tool_analytics(test_codex(), &server).await?;
    let test = builder.build(&server).await?;
    test.codex
        .submit(Op::RunUserShellCommand {
            command: "git version".to_string(),
        })
        .await?;
    let EventMsg::ExecCommandEnd(end) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecCommandEnd(_))
    })
    .await
    else {
        unreachable!("waited for the user-shell command to finish")
    };
    assert_eq!(end.exit_code, 0);
    let metadata = serde_json::json!({
        "commands": [{"command": "git", "subcommand": "version", "flags": []}],
    });
    #[cfg(windows)]
    let arguments_after_hooks = serde_json::json!({});
    #[cfg(not(windows))]
    let arguments_after_hooks = metadata.clone();
    assert_internal_tool_input(
        &server,
        &end.call_id,
        serde_json::json!({
            "tool_name": "user_shell",
            "source_kind": "user_shell",
            "arguments_before_hooks": metadata.clone(),
            "arguments_after_hooks": arguments_after_hooks,
            "hook_normalized_input": null,
            "hook_outcome": "not_applicable",
        }),
    )
    .await
}
// copybara:strip-for-public end

#[tokio::test]
async fn managed_hook_discovery_failure_rejects_session_startup_when_hooks_enabled() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_cloud_config_bundle(
        CloudConfigBundleFixture::loader_with_enterprise_requirement(
            r#"
[hooks]

[[hooks.PreToolUse]]
matcher = "["

[[hooks.PreToolUse.hooks]]
type = "command"
command = "echo managed"
"#,
        ),
    );

    let result = builder.build_with_auto_env(&server).await;
    let Err(error) = result else {
        panic!("session startup should reject an invalid managed hook matcher");
    };
    let error = format!("{error:#}");
    assert!(
        error.contains("managed") && error.contains("invalid matcher"),
        "unexpected managed hook startup error: {error}"
    );

    Ok(())
}

fn restrictive_workspace_write_profile() -> PermissionProfile {
    PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    )
}

fn network_workspace_write_profile() -> PermissionProfile {
    PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    )
}

fn code_mode_custom_tool_output_text(output_item: &Value) -> String {
    match output_item.get("output") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::Object(output)) => output
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        output => panic!("unexpected code mode custom tool output: {output:?}"),
    }
}

fn non_openai_model_provider(server: &wiremock::MockServer) -> ModelProviderInfo {
    let mut provider =
        built_in_model_providers(/* openai_base_url */ /*openai_base_url*/ None)["openai"].clone();
    provider.name = "OpenAI (test)".into();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.supports_websockets = false;
    provider
}

fn trust_plugin_hooks(config: &mut Config, plugin_hook_sources: Vec<PluginHookSource>) {
    config
        .features
        .enable(Feature::CodexHooks)
        .expect("test config should allow feature update");
    let listed = codex_hooks::list_hooks(codex_hooks::HooksConfig {
        feature_enabled: true,
        config_layer_stack: Some(config.config_layer_stack.clone()),
        plugin_hook_sources,
        ..codex_hooks::HooksConfig::default()
    });
    assert!(
        !listed.hooks.is_empty(),
        "trusted plugin hook fixture should discover at least one hook"
    );
    trust_hooks(config, listed.hooks);
}

fn write_stop_hook(home: &Path, block_prompts: &[&str]) -> Result<()> {
    let script_path = home.join("stop_hook.py");
    let log_path = home.join("stop_hook_log.jsonl");
    let prompts_json =
        serde_json::to_string(block_prompts).context("serialize stop hook prompts for test")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
block_prompts = {prompts_json}

payload = json.load(sys.stdin)
existing = []
if log_path.exists():
    existing = [line for line in log_path.read_text(encoding="utf-8").splitlines() if line.strip()]

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

invocation_index = len(existing)
if invocation_index < len(block_prompts):
    print(json.dumps({{"decision": "block", "reason": block_prompts[invocation_index]}}))
else:
    print(json.dumps({{"systemMessage": f"stop hook pass {{invocation_index + 1}} complete"}}))
"#,
        log_path = log_path.display(),
        prompts_json = prompts_json,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running stop hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write stop hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_session_end_hook(home: &Path) -> Result<()> {
    let script_path = home.join("session_end_hook.py");
    let log_path = home.join("session_end_hook_log.jsonl");
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
transcript = Path(payload["transcript_path"])
payload["transcript_exists"] = transcript.exists()
payload["transcript_text"] = transcript.read_text(encoding="utf-8") if transcript.exists() else ""
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
print(json.dumps({{"continue": False, "decision": "block", "reason": "ignored"}}))
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionEnd": [{
                "matcher": "other",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write session end hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_parallel_stop_hooks(home: &Path, prompts: &[&str]) -> Result<()> {
    let hook_entries = prompts
        .iter()
        .enumerate()
        .map(|(index, prompt)| {
            let script_path = home.join(format!("stop_hook_{index}.py"));
            let script = format!(
                r#"import json
import sys

payload = json.load(sys.stdin)
if payload["stop_hook_active"]:
    print(json.dumps({{"systemMessage": "done"}}))
else:
    print(json.dumps({{"decision": "block", "reason": {prompt:?}}}))
"#
            );
            fs::write(&script_path, script).with_context(|| {
                format!(
                    "write stop hook script fixture at {}",
                    script_path.display()
                )
            })?;
            Ok(serde_json::json!({
                "type": "command",
                "command": format!("python3 {}", script_path.display()),
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    let hooks = serde_json::json!({
        "hooks": {
            "Stop": [{
                "hooks": hook_entries,
            }]
        }
    });

    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_user_prompt_submit_hook(
    home: &Path,
    blocked_prompt: &str,
    additional_context: &str,
) -> Result<()> {
    let script_path = home.join("user_prompt_submit_hook.py");
    let log_path = home.join("user_prompt_submit_hook_log.jsonl");
    let log_path = log_path.display();
    let blocked_prompt_json =
        serde_json::to_string(blocked_prompt).context("serialize blocked prompt for test")?;
    let additional_context_json = serde_json::to_string(additional_context)
        .context("serialize user prompt submit additional context for test")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if payload.get("prompt") == {blocked_prompt_json}:
    print(json.dumps({{
        "decision": "block",
        "reason": "blocked by hook",
        "hookSpecificOutput": {{
            "hookEventName": "UserPromptSubmit",
            "additionalContext": {additional_context_json}
        }}
    }}))
"#,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running user prompt submit hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write user prompt submit hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_async_user_prompt_submit_hook(home: &Path, gated: bool) -> Result<()> {
    let script_path = home.join("async_user_prompt_submit_hook.py");
    let started_path = home.join("async_user_prompt_submit_started");
    let finished_path = home.join("async_user_prompt_submit_finished");
    let release_path = home.join("async_user_prompt_submit_release");
    let script = format!(
        r#"import json
from pathlib import Path
import sys
import time

prompt = json.load(sys.stdin).get("prompt")
Path(r"{started_path}").write_text(prompt, encoding="utf-8")
while {gated} and not Path(r"{release_path}").exists():
    time.sleep(0.01)
print(json.dumps({{
    "continue": False,
    "decision": "block",
    "reason": "an async hook cannot block its launching operation",
    "systemMessage": f"async warning for {{prompt}}",
    "hookSpecificOutput": {{
        "hookEventName": "UserPromptSubmit",
        "additionalContext": f"async context for {{prompt}}"
    }}
}}), flush=True)
Path(r"{finished_path}").write_text(prompt, encoding="utf-8")
"#,
        started_path = started_path.display(),
        finished_path = finished_path.display(),
        release_path = release_path.display(),
        gated = if gated { "True" } else { "False" },
    );
    let hooks = serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "async": true,
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write async user prompt submit hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write async hooks.json")?;
    Ok(())
}

fn write_session_start_and_user_prompt_submit_order_hooks(home: &Path) -> Result<()> {
    let session_start_script_path = home.join("session_start_order_hook.py");
    let user_prompt_submit_script_path = home.join("user_prompt_submit_order_hook.py");
    let log_path = home.join("hook_order_log.jsonl");

    let session_start_script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps({{
        "hook_event_name": payload.get("hook_event_name"),
        "source": payload.get("source"),
    }}) + "\n")
"#,
        log_path = log_path.display(),
    );
    let user_prompt_submit_script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps({{
        "hook_event_name": payload.get("hook_event_name"),
        "prompt": payload.get("prompt"),
    }}) + "\n")
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", session_start_script_path.display()),
                    "statusMessage": "running session start order hook",
                }]
            }],
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", user_prompt_submit_script_path.display()),
                    "statusMessage": "running user prompt submit order hook",
                }]
            }]
        }
    });

    fs::write(&session_start_script_path, session_start_script)
        .context("write session start order hook script")?;
    fs::write(&user_prompt_submit_script_path, user_prompt_submit_script)
        .context("write user prompt submit order hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_pre_tool_use_hook(
    home: &Path,
    matcher: Option<&str>,
    mode: &str,
    reason: &str,
) -> Result<()> {
    let script_path = home.join("pre_tool_use_hook.py");
    let log_path = home.join("pre_tool_use_hook_log.jsonl");
    let mode_json = serde_json::to_string(mode).context("serialize pre tool use mode")?;
    let reason_json = serde_json::to_string(reason).context("serialize pre tool use reason")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
mode = {mode_json}
reason = {reason_json}

payload = json.load(sys.stdin)

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if mode == "json_deny":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason
        }}
    }}))
elif mode == "context":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PreToolUse",
            "additionalContext": reason
        }}
    }}))
elif mode == "json_deny_with_context":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
            "additionalContext": reason
        }}
    }}))
elif mode == "exit_2":
    sys.stderr.write(reason + "\n")
    raise SystemExit(2)
"#,
        log_path = log_path.display(),
        mode_json = mode_json,
        reason_json = reason_json,
    );

    let mut group = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": format!("python3 {}", script_path.display()),
            "statusMessage": "running pre tool use hook",
        }]
    });
    if let Some(matcher) = matcher {
        group["matcher"] = Value::String(matcher.to_string());
    }

    let hooks = serde_json::json!({
        "hooks": {
            "PreToolUse": [group]
        }
    });

    fs::write(&script_path, script).context("write pre tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_updating_pre_tool_use_hook(
    home: &Path,
    matcher: &str,
    updated_input: &Value,
) -> Result<()> {
    let script_path = home.join("pre_tool_use_hook.py");
    let log_path = home.join("pre_tool_use_hook_log.jsonl");
    let updated_input_json =
        serde_json::to_string(updated_input).context("serialize updated pre tool input")?;
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
        "updatedInput": {updated_input_json}
    }}
}}))
"#,
        log_path = log_path.display(),
        updated_input_json = updated_input_json,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "rewriting pre tool input",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write updating pre tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_pre_tool_use_hook_toml(
    home: &Path,
    script_name: &str,
    log_name: &str,
    matcher: Option<&str>,
    mode: &str,
    reason: &str,
) -> Result<()> {
    let script_path = home.join(script_name);
    let log_path = home.join(log_name);
    let mode_json = serde_json::to_string(mode).context("serialize pre tool use mode")?;
    let reason_json = serde_json::to_string(reason).context("serialize pre tool use reason")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
mode = {mode_json}
reason = {reason_json}

payload = json.load(sys.stdin)

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if mode == "json_deny":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason
        }}
    }}))
elif mode == "exit_2":
    sys.stderr.write(reason + "\n")
    raise SystemExit(2)
"#,
        log_path = log_path.display(),
        mode_json = mode_json,
        reason_json = reason_json,
    );
    let matcher_line = matcher
        .map(|matcher| format!("matcher = '{matcher}'\n"))
        .unwrap_or_default();
    let config_toml = format!(
        r#"[features]
hooks = true

[hooks]

[[hooks.PreToolUse]]
{matcher_line}

[[hooks.PreToolUse.hooks]]
type = "command"
command = 'python3 {script_path}'
statusMessage = "running pre tool use hook"
"#,
        matcher_line = matcher_line,
        script_path = script_path.display(),
    );

    fs::write(&script_path, script).context("write TOML pre tool use hook script")?;
    fs::write(home.join("config.toml"), config_toml).context("write config.toml hooks")?;
    Ok(())
}

fn write_permission_request_hook(
    home: &Path,
    matcher: Option<&str>,
    mode: &str,
    reason: &str,
) -> Result<()> {
    let script_path = home.join("permission_request_hook.py");
    let log_path = home.join("permission_request_hook_log.jsonl");
    let mode_json = serde_json::to_string(mode).context("serialize permission request mode")?;
    let reason_json =
        serde_json::to_string(reason).context("serialize permission request reason")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
mode = {mode_json}
reason = {reason_json}

payload = json.load(sys.stdin)

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if mode == "allow":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PermissionRequest",
            "decision": {{"behavior": "allow"}}
        }}
    }}))
elif mode == "deny":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PermissionRequest",
            "decision": {{
                "behavior": "deny",
                "message": reason
            }}
        }}
    }}))
elif mode == "exit_2":
    sys.stderr.write(reason + "\n")
    raise SystemExit(2)
"#,
        log_path = log_path.display(),
        mode_json = mode_json,
        reason_json = reason_json,
    );

    let mut group = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": format!("python3 {}", script_path.display()),
            "statusMessage": "running permission request hook",
        }]
    });
    if let Some(matcher) = matcher {
        group["matcher"] = Value::String(matcher.to_string());
    }

    let hooks = serde_json::json!({
        "hooks": {
            "PermissionRequest": [group]
        }
    });

    fs::write(&script_path, script).context("write permission request hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn install_allow_permission_request_hook(home: &Path) -> Result<()> {
    write_permission_request_hook(
        home,
        Some(PERMISSION_REQUEST_HOOK_MATCHER),
        "allow",
        PERMISSION_REQUEST_ALLOW_REASON,
    )
}

fn write_post_tool_use_hook(
    home: &Path,
    matcher: Option<&str>,
    mode: &str,
    reason: &str,
) -> Result<()> {
    let script_path = home.join("post_tool_use_hook.py");
    let log_path = home.join("post_tool_use_hook_log.jsonl");
    let mode_json = serde_json::to_string(mode).context("serialize post tool use mode")?;
    let reason_json = serde_json::to_string(reason).context("serialize post tool use reason")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
mode = {mode_json}
reason = {reason_json}

payload = json.load(sys.stdin)

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if mode == "context":
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PostToolUse",
            "additionalContext": reason
        }}
    }}))
elif mode == "decision_block":
    print(json.dumps({{
        "decision": "block",
        "reason": reason
    }}))
elif mode == "continue_false":
    print(json.dumps({{
        "continue": False,
        "stopReason": reason
    }}))
elif mode == "exit_2":
    sys.stderr.write(reason + "\n")
    raise SystemExit(2)
"#,
        log_path = log_path.display(),
        mode_json = mode_json,
        reason_json = reason_json,
    );

    let mut group = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": format!("python3 {}", script_path.display()),
            "statusMessage": "running post tool use hook",
        }]
    });
    if let Some(matcher) = matcher {
        group["matcher"] = Value::String(matcher.to_string());
    }

    let hooks = serde_json::json!({
        "hooks": {
            "PostToolUse": [group]
        }
    });

    fs::write(&script_path, script).context("write post tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_logging_pre_and_blocking_post_tool_use_hooks(home: &Path, feedback: &str) -> Result<()> {
    let pre_script_path = home.join("pre_tool_use_hook.py");
    let pre_log_path = home.join("pre_tool_use_hook_log.jsonl");
    let post_script_path = home.join("post_tool_use_hook.py");
    let post_log_path = home.join("post_tool_use_hook_log.jsonl");
    let feedback_json =
        serde_json::to_string(feedback).context("serialize post tool use feedback")?;
    let pre_script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{pre_log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
"#,
        pre_log_path = pre_log_path.display(),
    );
    let post_script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{post_log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
sys.stderr.write({feedback_json} + "\n")
raise SystemExit(2)
"#,
        post_log_path = post_log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", pre_script_path.display()),
                    "statusMessage": "running pre tool use hook",
                }]
            }],
            "PostToolUse": [{
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", post_script_path.display()),
                    "statusMessage": "running post tool use hook",
                }]
            }]
        }
    });

    fs::write(&pre_script_path, pre_script).context("write pre tool use hook script")?;
    fs::write(&post_script_path, post_script).context("write post tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_session_start_hook_recording_transcript(home: &Path, stop: bool) -> Result<()> {
    let script_path = home.join("session_start_hook.py");
    let log_path = home.join("session_start_hook_log.jsonl");
    let stop_output = if stop {
        r#"print(json.dumps({"continue": False, "stopReason": "integration assertion complete"}))"#
    } else {
        ""
    };
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
transcript_path = payload.get("transcript_path")
if transcript_path is not None and not Path(transcript_path).is_file():
    raise RuntimeError(
        f"transcript did not exist when the hook ran: {{transcript_path}}"
    )

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

{stop_output}
"#,
        log_path = log_path.display(),
        stop_output = stop_output,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running session start hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write session start hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_session_start_hook_with_context(home: &Path, additional_context: &str) -> Result<()> {
    let script_path = home.join("session_start_hook.py");
    let additional_context_json = serde_json::to_string(additional_context)
        .context("serialize session start additional context for test")?;
    let script = format!(
        r#"import json

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "SessionStart",
        "additionalContext": {additional_context_json}
    }}
}}))
"#,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running session start hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write session start hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_session_start_hooks_with_individual_context_limits(
    home: &Path,
    limited_additional_context: &str,
    expanded_additional_context: &str,
) -> Result<()> {
    let mut hook_handlers = Vec::new();
    for (additional_context, additional_context_limit) in [
        (limited_additional_context, 1),
        (expanded_additional_context, 100),
    ] {
        hook_handlers.push(serde_json::json!({
            "type": "command",
            "command": format!("echo {additional_context}"),
            "additionalContextLimit": additional_context_limit,
        }));
    }
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "hooks": hook_handlers,
            }]
        }
    });

    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_compact_session_start_hook_with_context(
    home: &Path,
    additional_context: &str,
) -> Result<()> {
    let script_path = home.join("compact_session_start_hook.py");
    let log_path = home.join("session_start_hook_log.jsonl");
    let additional_context_json = serde_json::to_string(additional_context)
        .context("serialize compact session start additional context for test")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "SessionStart",
        "additionalContext": {additional_context_json}
    }}
}}))
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "matcher": "compact",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running compact session start hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write compact session start hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

enum DynamicCompactSessionStartHook {
    IndexedContexts,
    Stop,
}

fn write_dynamic_compact_session_start_hook(
    home: &Path,
    behavior: DynamicCompactSessionStartHook,
) -> Result<()> {
    let script_path = home.join("dynamic_compact_session_start_hook.py");
    let log_path = home.join("session_start_hook_log.jsonl");
    let output = match behavior {
        DynamicCompactSessionStartHook::IndexedContexts => {
            r#"invocation_index = len(existing) + 1
print(json.dumps({
    "hookSpecificOutput": {
        "hookEventName": "SessionStart",
        "additionalContext": f"compact hook context {invocation_index}",
    }
}))"#
        }
        DynamicCompactSessionStartHook::Stop => {
            r#"print(json.dumps({
    "continue": False,
    "stopReason": "compact hook stopped continuation",
}))"#
        }
    };
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
log_path = Path(r"{log_path}")
existing = []
if log_path.exists():
    existing = [line for line in log_path.read_text(encoding="utf-8").splitlines() if line.strip()]

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

{output}
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "matcher": "compact",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running compact session start hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write dynamic compact session start hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_resume_and_compact_session_start_hook_with_context(
    home: &Path,
    resume_context: &str,
    compact_context: &str,
) -> Result<()> {
    let script_path = home.join("resume_and_compact_session_start_hook.py");
    let log_path = home.join("session_start_hook_log.jsonl");
    let resume_context_json = serde_json::to_string(resume_context)
        .context("serialize resume session start additional context for test")?;
    let compact_context_json = serde_json::to_string(compact_context)
        .context("serialize compact session start additional context for test")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

contexts = {{
    "resume": {resume_context_json},
    "compact": {compact_context_json},
}}
print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "SessionStart",
        "additionalContext": contexts[payload["source"]]
    }}
}}))
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "matcher": "resume",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running resume session start hook",
                }]
            }, {
                "matcher": "compact",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running compact session start hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script)
        .context("write resume and compact session start hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn rollout_hook_prompt_texts(text: &str) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = serde_json::from_str(trimmed).context("parse rollout line")?;
        if let RolloutItem::ResponseItem(envelope) = rollout.item
            && let ResponseItem::Message { role, content, .. } = envelope.item
            && role == "user"
        {
            for item in content {
                if let ContentItem::InputText { text } = item
                    && let Some(fragment) = parse_hook_prompt_fragment(&text)
                {
                    texts.push(fragment.text);
                }
            }
        }
    }
    Ok(texts)
}

fn request_hook_prompt_texts(
    request: &core_test_support::responses::ResponsesRequest,
) -> Vec<String> {
    request
        .message_input_texts("user")
        .into_iter()
        .filter_map(|text| parse_hook_prompt_fragment(&text).map(|fragment| fragment.text))
        .collect()
}

fn spilled_hook_output_path(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("Full hook output saved to: "))
}

fn read_stop_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    fs::read_to_string(home.join("stop_hook_log.jsonl"))
        .context("read stop hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse stop hook log line"))
        .collect()
}

fn read_pre_tool_use_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    read_hook_inputs_from_log(home.join("pre_tool_use_hook_log.jsonl").as_path())
}

fn read_permission_request_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    fs::read_to_string(home.join("permission_request_hook_log.jsonl"))
        .context("read permission request hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse permission request hook log line"))
        .collect()
}

fn assert_permission_request_hook_input(
    hook_input: &Value,
    tool_name: &str,
    command: &str,
    description: Option<&str>,
) {
    assert_eq!(hook_input["hook_event_name"], "PermissionRequest");
    assert_eq!(hook_input["tool_name"], tool_name);
    assert_eq!(hook_input["tool_input"]["command"], command);
    assert_eq!(
        hook_input["tool_input"]["description"],
        description.map_or(Value::Null, Value::from)
    );
    assert!(hook_input.get("approval_attempt").is_none());
    assert!(hook_input.get("sandbox_permissions").is_none());
    assert!(hook_input.get("additional_permissions").is_none());
    assert!(hook_input.get("justification").is_none());
    assert!(hook_input.get("host").is_none());
    assert!(hook_input.get("protocol").is_none());
}

fn assert_single_permission_request_hook_input(
    home: &Path,
    command: &str,
    description: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    assert_single_permission_request_hook_input_for_tool(home, "Bash", command, description)
}

fn assert_single_permission_request_hook_input_for_tool(
    home: &Path,
    tool_name: &str,
    command: &str,
    description: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    let hook_inputs = read_permission_request_hook_inputs(home)?;
    assert_eq!(hook_inputs.len(), 1);
    assert_permission_request_hook_input(&hook_inputs[0], tool_name, command, description);
    Ok(hook_inputs)
}

fn read_post_tool_use_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    read_hook_inputs_from_log(home.join("post_tool_use_hook_log.jsonl").as_path())
}

fn read_hook_inputs_from_log(log_path: &Path) -> Result<Vec<serde_json::Value>> {
    fs::read_to_string(log_path)
        .with_context(|| format!("read hook log {}", log_path.display()))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse hook log line"))
        .collect()
}

fn read_session_start_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    fs::read_to_string(home.join("session_start_hook_log.jsonl"))
        .context("read session start hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse session start hook log line"))
        .collect()
}

fn read_user_prompt_submit_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    fs::read_to_string(home.join("user_prompt_submit_hook_log.jsonl"))
        .context("read user prompt submit hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse user prompt submit hook log line"))
        .collect()
}

fn read_hook_order_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    read_hook_inputs_from_log(home.join("hook_order_log.jsonl").as_path())
}

fn ev_message_item_done(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": id,
            "content": [{"type": "output_text", "text": text}]
        }
    })
}

fn sse_event(event: Value) -> String {
    sse(vec![event])
}

fn request_message_input_texts(body: &[u8], role: &str) -> Vec<String> {
    let body: Value = serde_json::from_slice(body).expect("parse request body");
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter(|item| item.get("role").and_then(Value::as_str) == Some(role))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn stop_hook_can_block_multiple_times_in_same_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "draft one"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "draft two"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "final draft"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_stop_hook(
                home,
                &[FIRST_CONTINUATION_PROMPT, SECOND_CONTINUATION_PROMPT],
            )
            .expect("failed to write stop hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("hello from the sea").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        request_hook_prompt_texts(&requests[1]),
        vec![FIRST_CONTINUATION_PROMPT.to_string()],
        "second request should include the first continuation prompt as user hook context",
    );
    assert_eq!(
        request_hook_prompt_texts(&requests[2]),
        vec![
            FIRST_CONTINUATION_PROMPT.to_string(),
            SECOND_CONTINUATION_PROMPT.to_string(),
        ],
        "third request should retain hook prompts in user history",
    );

    let hook_inputs = read_stop_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 3);
    let stop_turn_ids = hook_inputs
        .iter()
        .map(|input| {
            input["turn_id"]
                .as_str()
                .expect("stop hook input turn_id")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(
        stop_turn_ids.iter().all(|turn_id| !turn_id.is_empty()),
        "stop hook turn ids should be non-empty",
    );
    let first_stop_turn_id = stop_turn_ids
        .first()
        .expect("stop hook inputs should include a first turn id")
        .clone();
    assert_eq!(
        stop_turn_ids,
        vec![
            first_stop_turn_id.clone(),
            first_stop_turn_id.clone(),
            first_stop_turn_id,
        ],
    );
    assert_eq!(
        hook_inputs
            .iter()
            .map(|input| input["stop_hook_active"]
                .as_bool()
                .expect("stop_hook_active bool"))
            .collect::<Vec<_>>(),
        vec![false, true, true],
    );

    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let rollout_text = fs::read_to_string(&rollout_path)?;
    let hook_prompt_texts = rollout_hook_prompt_texts(&rollout_text)?;
    assert!(
        hook_prompt_texts.contains(&FIRST_CONTINUATION_PROMPT.to_string()),
        "rollout should persist the first continuation prompt",
    );
    assert!(
        hook_prompt_texts.contains(&SECOND_CONTINUATION_PROMPT.to_string()),
        "rollout should persist the second continuation prompt",
    );

    Ok(())
}

#[tokio::test]
async fn session_start_hook_sees_materialized_transcript_path() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hello from the reef"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_session_start_hook_recording_transcript(home, /*stop*/ false)
                .expect("failed to write session start hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;

    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    let transcript_path = hook_inputs[0]["transcript_path"]
        .as_str()
        .expect("local session start hook transcript_path should be a string");
    assert!(
        Path::new(transcript_path).exists(),
        "local session start hook transcript_path should be materialized",
    );

    Ok(())
}

#[tokio::test]
async fn session_start_hook_skips_persistence_for_non_local_thread_store() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let store_id = uuid::Uuid::new_v4().to_string();
    let store = InMemoryThreadStore::for_id(store_id.clone());
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_session_start_hook_recording_transcript(home, /*stop*/ true)
                .expect("failed to write session start hook test fixture");
        })
        .with_config(move |config| {
            config.experimental_thread_store = ThreadStoreConfig::InMemory { id: store_id };
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("run the non-local session start hook")
        .await?;

    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["transcript_path"], Value::Null);
    assert_eq!(store.calls().await.persist_thread, 0);

    Ok(())
}

#[tokio::test]
async fn session_end_flushes_transcript_and_ignores_control_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "persisted answer"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_session_end_hook(home).expect("write session end hook fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("persist this before shutdown").await?;
    test.codex.shutdown_and_wait().await?;

    let inputs = read_hook_inputs_from_log(
        test.codex_home_path()
            .join("session_end_hook_log.jsonl")
            .as_path(),
    )?;
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0]["hook_event_name"], "SessionEnd");
    assert_eq!(inputs[0]["reason"], "other");
    assert_eq!(inputs[0]["transcript_exists"], true);
    let transcript = inputs[0]["transcript_text"]
        .as_str()
        .expect("session end transcript text");
    assert!(transcript.contains("persist this before shutdown"));
    assert!(transcript.contains("persisted answer"));
    Ok(())
}

#[tokio::test]
async fn session_end_skips_subagents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_session_end_hook(home).expect("write session end hook fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;
    for source in [
        SubAgentSource::Review,
        SubAgentSource::ThreadSpawn {
            parent_thread_id: test.session_configured.thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        },
    ] {
        let subagent = test
            .thread_manager
            .start_thread(StartThreadOptions {
                session_source: Some(SessionSource::SubAgent(source)),
                environments: Some(Vec::new()),
                ..StartThreadOptions::new(test.config.clone())
            })
            .await?;

        subagent.thread.shutdown_and_wait().await?;
    }

    assert!(
        !test
            .codex_home_path()
            .join("session_end_hook_log.jsonl")
            .exists(),
        "subagents must not run SessionEnd hooks"
    );
    Ok(())
}

#[tokio::test]
async fn session_start_runs_before_user_prompt_submit_on_first_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hello after hooks"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_session_start_and_user_prompt_submit_order_hooks(home)
                .expect("failed to write hook ordering fixtures");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;

    let hook_inputs = read_hook_order_inputs(test.codex_home_path())?;
    assert_eq!(
        hook_inputs
            .iter()
            .map(|input| input["hook_event_name"]
                .as_str()
                .expect("hook input event name"))
            .collect::<Vec<_>>(),
        vec!["SessionStart", "UserPromptSubmit"],
    );
    assert_eq!(
        hook_inputs[0].get("source").and_then(Value::as_str),
        Some("startup")
    );
    assert_eq!(
        hook_inputs[1].get("prompt").and_then(Value::as_str),
        Some("hello")
    );

    Ok(())
}

#[tokio::test]
async fn async_hook_context_is_injected_into_the_active_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let gate = TempDir::new()?;
    let release_path = gate.path().join("release");
    let call_id = "async-hook-context-gated-exec-command";
    let args = serde_json::json!({
        "cmd": format!(
            r#"python3 -c 'import time; from pathlib import Path; gate = Path(r"{}"); exec("while not gate.exists(): time.sleep(0.01)")'"#,
            release_path.display()
        )
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "async context observed in this turn"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_async_user_prompt_submit_hook(home, /*gated*/ false)
                .expect("write immediate async user prompt submit hook");
        })
        .with_config(trust_discovered_hooks)
        .build(&server)
        .await?;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "observe async context immediately".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(
                codex_protocol::protocol::ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::Never),
                    sandbox_policy: Some(sandbox_policy),
                    permission_profile,
                    ..Default::default()
                },
            ),
        )
        .await?;

    let finished_path = test
        .codex_home_path()
        .join("async_user_prompt_submit_finished");
    fs_wait::wait_for_path_exists(finished_path, Duration::from_secs(5))
        .await
        .context("timed out waiting for the async hook to finish")?;
    assert!(
        timeout(
            Duration::from_millis(150),
            wait_for_event(&test.codex, |event| {
                matches!(
                    event,
                    EventMsg::Warning(warning)
                        if warning.message == "async warning for observe async context immediately"
                )
            }),
        )
        .await
        .is_err(),
        "async hook output must not interrupt the active sampling request"
    );
    fs::write(release_path, "ready").context("release gated shell command")?;
    timeout(
        Duration::from_secs(5),
        wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::Warning(warning)
                    if warning.message == "async warning for observe async context immediately"
            )
        }),
    )
    .await
    .context("timed out waiting for the async hook warning after sampling")?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&"async context for observe async context immediately".to_string()),
        "an async hook should steer its context into the same active turn"
    );

    Ok(())
}

#[tokio::test]
async fn async_hook_finishing_while_idle_waits_for_the_next_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "first turn completed"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "idle async context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_async_user_prompt_submit_hook(home, /*gated*/ true)
                .expect("write gated async user prompt submit hook");
        })
        .with_config(trust_discovered_hooks)
        .build(&server)
        .await?;

    test.submit_turn("finish before the async hook").await?;
    let first_turn_id = responses.requests()[0].body_json()["client_metadata"]["turn_id"]
        .as_str()
        .context("first model request should include its turn ID")?
        .to_string();
    let started_path = test
        .codex_home_path()
        .join("async_user_prompt_submit_started");
    fs_wait::wait_for_path_exists(started_path, Duration::from_secs(5))
        .await
        .context("timed out waiting for the async hook to start")?;

    fs::write(
        test.codex_home_path()
            .join("async_user_prompt_submit_release"),
        "ready",
    )
    .context("release gated async hook")?;

    let finished_path = test
        .codex_home_path()
        .join("async_user_prompt_submit_finished");
    fs_wait::wait_for_path_exists(finished_path, Duration::from_secs(5))
        .await
        .context("timed out waiting for the async hook to finish")?;

    assert!(
        timeout(Duration::from_millis(150), test.codex.next_event())
            .await
            .is_err(),
        "an async hook result from the previous turn must not emit warnings or raw response items"
    );

    assert_eq!(
        responses.requests().len(),
        1,
        "an async hook result from the previous turn must not start a model turn"
    );

    let next_prompt = "observe the buffered async context";
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: next_prompt.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let mut warning_event = None;
    timeout(Duration::from_secs(5), async {
        loop {
            let event = test.codex.next_event().await?;
            if matches!(
                &event.msg,
                EventMsg::Warning(warning)
                    if warning.message == "async warning for finish before the async hook"
            ) {
                warning_event = Some(event);
                continue;
            }
            if matches!(event.msg, EventMsg::TurnComplete(_)) {
                return Ok::<_, anyhow::Error>(());
            }
        }
    })
    .await
    .context("timed out waiting for the next turn to complete")??;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let second_turn_id = requests[1].body_json()["client_metadata"]["turn_id"]
        .as_str()
        .context("second model request should include its turn ID")?
        .to_string();
    assert_ne!(first_turn_id, second_turn_id);
    assert_eq!(
        warning_event
            .context("buffered async hook warning should be delivered during the next turn")?
            .id,
        second_turn_id
    );

    let input = requests[1].input();
    let context_index = input
        .iter()
        .position(|item| {
            item.to_string()
                .contains("async context for finish before the async hook")
        })
        .context("buffered async hook context should be present in the next model request")?;
    let prompt_index = input
        .iter()
        .position(|item| item.to_string().contains(next_prompt))
        .context("next user prompt should be present in the next model request")?;
    assert!(
        context_index < prompt_index,
        "buffered async hook context should precede the next user prompt"
    );

    Ok(())
}

#[tokio::test]
async fn session_start_hook_spills_large_additional_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hello from the reef"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let additional_context = "remember the reef ".repeat(800);

    let mut builder = test_codex()
        .with_pre_build_hook({
            let additional_context = additional_context.clone();
            move |home| {
                write_session_start_hook_with_context(home, &additional_context)
                    .expect("failed to write session start hook test fixture");
            }
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;

    let request = response.single_request();
    let developer_messages = request.message_input_texts("developer");
    let developer_message = developer_messages
        .iter()
        .find(|message| spilled_hook_output_path(message).is_some())
        .context("spilled developer hook message")?;
    assert!(developer_message.contains("tokens truncated"));
    let path = spilled_hook_output_path(developer_message).context("spill path")?;
    assert_eq!(fs::read_to_string(path)?, additional_context);

    Ok(())
}

#[tokio::test]
async fn session_start_hooks_apply_additional_context_limits_individually() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hello from the reef"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let limited_additional_context = "spill this limited reef context".to_string();
    let expanded_additional_context = "keep this expanded reef context inline".to_string();

    let test = test_codex()
        .with_pre_build_hook({
            let limited_additional_context = limited_additional_context.clone();
            let expanded_additional_context = expanded_additional_context.clone();
            move |home| {
                write_session_start_hooks_with_individual_context_limits(
                    home,
                    &limited_additional_context,
                    &expanded_additional_context,
                )
                .expect("failed to write session start hook test fixtures");
            }
        })
        .with_config(trust_discovered_hooks)
        .build(&server)
        .await?;

    test.submit_turn("hello").await?;

    let request = response.single_request();
    let developer_messages = request.message_input_texts("developer");
    let spilled_message = developer_messages
        .iter()
        .find(|message| spilled_hook_output_path(message).is_some())
        .context("spilled limited session start context")?;
    let path = spilled_hook_output_path(spilled_message).context("spill path")?;
    assert_eq!(fs::read_to_string(path)?, limited_additional_context);
    assert!(
        developer_messages
            .iter()
            .any(|message| message == &expanded_additional_context),
        "expected the expanded-limit hook context inline, got {developer_messages:?}"
    );
    Ok(())
}

#[tokio::test]
async fn pre_tool_use_hook_spills_large_additional_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-exec-command-large-context";
    let command = "printf pre-tool-output".to_string();
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "pre hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let additional_context = "remember the pre tool reef ".repeat(800);

    let mut builder = test_codex()
        .with_pre_build_hook({
            let additional_context = additional_context.clone();
            move |home| {
                write_pre_tool_use_hook(home, Some("^Bash$"), "context", &additional_context)
                    .expect("failed to write pre tool use hook test fixture");
            }
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("run the shell command with large pre hook context")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let developer_messages = requests[1].message_input_texts("developer");
    let developer_message = developer_messages
        .iter()
        .find(|message| spilled_hook_output_path(message).is_some())
        .context("spilled developer hook message")?;
    assert!(developer_message.contains("tokens truncated"));
    let path = spilled_hook_output_path(developer_message).context("spill path")?;
    assert_eq!(fs::read_to_string(path)?, additional_context);

    Ok(())
}

#[tokio::test]
async fn compact_session_start_hook_records_additional_context_for_next_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "hello before compact"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "summary after compact"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "hello after compact"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let additional_context = "remember the compacted reef";
    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            write_compact_session_start_hook_with_context(home, additional_context)
                .expect("failed to write compact session start hook fixture");
        })
        .with_config(move |config| {
            config.model_provider = model_provider;
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("hello before compact").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("hello after compact").await?;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3);
    assert!(
        !requests[0]
            .message_input_texts("developer")
            .iter()
            .any(|message| message == additional_context),
        "compact matcher should not run for initial startup",
    );
    assert!(
        requests[2]
            .message_input_texts("developer")
            .iter()
            .any(|message| message == additional_context),
        "compact matcher should inject additional context before the next model turn",
    );

    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0].get("source").and_then(Value::as_str),
        Some("compact")
    );

    Ok(())
}

#[tokio::test]
async fn mid_turn_auto_compact_session_start_hooks_run_before_each_continuation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let limit = 200_000;
    let over_limit_tokens = 250_000;
    let compacted_tokens = 50;
    let _first_turn = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call("call-1", "test_tool", "{}"),
            ev_completed_with_tokens("resp-1", over_limit_tokens),
        ]),
    )
    .await;
    let _first_compact = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("summary-1", "first compact summary"),
            ev_completed_with_tokens("resp-2", compacted_tokens),
        ]),
    )
    .await;
    let first_continuation = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_function_call("call-2", "test_tool", "{}"),
            ev_completed_with_tokens("resp-3", over_limit_tokens),
        ]),
    )
    .await;
    let _second_compact = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-4"),
            ev_assistant_message("summary-2", "second compact summary"),
            ev_completed_with_tokens("resp-4", compacted_tokens),
        ]),
    )
    .await;
    let second_continuation = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            ev_assistant_message("final", "finished after compaction"),
            ev_completed_with_tokens("resp-5", compacted_tokens),
        ]),
    )
    .await;
    let next_turn = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-6"),
            ev_assistant_message("next", "finished next turn"),
            ev_completed_with_tokens("resp-6", compacted_tokens),
        ]),
    )
    .await;
    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_dynamic_compact_session_start_hook(
                home,
                DynamicCompactSessionStartHook::IndexedContexts,
            )
            .expect("failed to write indexed compact session start hook fixture");
        })
        .with_config(move |config| {
            config.model_provider = model_provider;
            config.model_auto_compact_token_limit = Some(limit);
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("start auto compact turn").await?;

    let first_continuation_context = first_continuation
        .single_request()
        .message_input_texts("developer");
    assert!(
        first_continuation_context
            .iter()
            .any(|message| message == "compact hook context 1"),
        "the first compact hook context should reach the immediate continuation request",
    );
    assert!(
        !first_continuation_context
            .iter()
            .any(|message| message == "compact hook context 2"),
        "the second compact hook should not run before its compaction",
    );
    assert!(
        second_continuation
            .single_request()
            .message_input_texts("developer")
            .iter()
            .any(|message| message == "compact hook context 2"),
        "the second compact hook context should reach the immediate continuation request",
    );

    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(
        hook_inputs
            .iter()
            .filter_map(|input| input.get("source").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec!["compact", "compact"],
        "each successful mid-turn compaction should drain exactly one compact hook",
    );

    test.submit_turn("next user turn").await?;

    assert!(
        !next_turn
            .single_request()
            .message_input_texts("developer")
            .iter()
            .any(|message| message == "compact hook context 3"),
        "the next user turn should not receive a stale compact hook",
    );
    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(
        hook_inputs
            .iter()
            .filter_map(|input| input.get("source").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec!["compact", "compact"],
        "the next user turn should not invoke stale compact hooks",
    );

    Ok(())
}

#[tokio::test]
async fn mid_turn_auto_compact_session_start_hook_stop_blocks_continuation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let limit = 200_000;
    let over_limit_tokens = 250_000;
    let compacted_tokens = 50;
    let first_turn = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call("call-1", "test_tool", "{}"),
            ev_completed_with_tokens("resp-1", over_limit_tokens),
        ]),
    )
    .await;
    let compact = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("summary-1", "compact summary"),
            ev_completed_with_tokens("resp-2", compacted_tokens),
        ]),
    )
    .await;
    let unexpected_continuation = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("unexpected", "continued after hook stop"),
            ev_completed_with_tokens("resp-3", compacted_tokens),
        ]),
    )
    .await;
    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_dynamic_compact_session_start_hook(home, DynamicCompactSessionStartHook::Stop)
                .expect("failed to write stopping compact session start hook fixture");
        })
        .with_config(move |config| {
            config.model_provider = model_provider;
            config.model_auto_compact_token_limit = Some(limit);
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("stop after auto compact").await?;

    first_turn.single_request();
    compact.single_request();
    assert!(
        unexpected_continuation.requests().is_empty(),
        "a compact SessionStart stop should prevent the next sampling request",
    );
    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(
        hook_inputs
            .iter()
            .filter_map(|input| input.get("source").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec!["compact"],
    );

    Ok(())
}

#[tokio::test]
async fn resumed_thread_runs_resume_then_compact_session_start_hooks() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let limit = 200_000;
    let over_limit_tokens = 250_000;
    let remote_summary = "remote compact summary";
    let resume_context = "remember the resumed reef";
    let compact_context = "remember the compacted reef";
    let responses_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "hello before resume"),
                ev_completed_with_tokens("resp-1", over_limit_tokens),
            ]),
            sse(vec![
                serde_json::json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": remote_summary,
                    }
                }),
                ev_completed("resp-compact"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "hello after resume"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            write_resume_and_compact_session_start_hook_with_context(
                home,
                resume_context,
                compact_context,
            )
            .expect("failed to write resume/compact session start hook fixture");
        })
        .with_config(move |config| {
            config.model_auto_compact_token_limit = Some(limit);
            trust_discovered_hooks(config);
        });
    let initial = builder.build(&server).await?;

    initial.submit_turn("hello before resume").await?;
    assert_eq!(responses_mock.requests().len(), 1);

    let mut resume_builder = test_codex().with_config(move |config| {
        config.model_auto_compact_token_limit = Some(limit);
        trust_discovered_hooks(config);
    });
    let resumed = resume_builder.restart(&server, &initial).await?;
    resumed.submit_turn("hello after resume").await?;

    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 3);
    let developer_messages = requests[2].message_input_texts("developer");
    assert!(
        developer_messages
            .iter()
            .any(|message| message == resume_context),
        "resume matcher should inject additional context before the next model turn",
    );
    assert!(
        developer_messages
            .iter()
            .any(|message| message == compact_context),
        "compact matcher should inject additional context before the next model turn",
    );

    let hook_inputs = read_session_start_hook_inputs(resumed.codex_home_path())?;
    assert_eq!(
        hook_inputs
            .iter()
            .filter_map(|input| input.get("source").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec!["resume", "compact"],
    );

    Ok(())
}

#[tokio::test]
async fn stop_hook_spills_large_continuation_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "draft one"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "draft two"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let continuation_prompt = std::iter::repeat_n("retry with the reef note", 800)
        .collect::<Vec<_>>()
        .join(" ");

    let mut builder = test_codex()
        .with_pre_build_hook({
            let continuation_prompt = continuation_prompt.clone();
            move |home| {
                write_stop_hook(home, &[&continuation_prompt])
                    .expect("failed to write stop hook test fixture");
            }
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("hello from the sea").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let hook_prompt_texts = request_hook_prompt_texts(&requests[1]);
    assert_eq!(hook_prompt_texts.len(), 1);
    let hook_prompt_text = &hook_prompt_texts[0];
    assert!(hook_prompt_text.contains("tokens truncated"));
    let path = spilled_hook_output_path(hook_prompt_text).context("spill path")?;
    assert_eq!(fs::read_to_string(path)?, continuation_prompt);

    Ok(())
}

#[tokio::test]
async fn resumed_thread_keeps_stop_continuation_prompt_in_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let initial_responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "initial draft"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "revised draft"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut initial_builder = test_codex()
        .with_pre_build_hook(|home| {
            write_stop_hook(home, &[FIRST_CONTINUATION_PROMPT])
                .expect("failed to write stop hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let initial = initial_builder.build(&server).await?;

    initial.submit_turn("tell me something").await?;

    assert_eq!(initial_responses.requests().len(), 2);

    let resumed_response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-3", "fresh turn after resume"),
            ev_completed("resp-3"),
        ]),
    )
    .await;

    let mut resume_builder = test_codex().with_config(trust_discovered_hooks);
    let resumed = resume_builder.restart(&server, &initial).await?;

    resumed.submit_turn("and now continue").await?;

    let resumed_request = resumed_response.single_request();
    assert_eq!(
        request_hook_prompt_texts(&resumed_request),
        vec![FIRST_CONTINUATION_PROMPT.to_string()],
        "resumed request should keep the persisted continuation prompt in user history",
    );

    Ok(())
}

#[tokio::test]
async fn multiple_blocking_stop_hooks_persist_multiple_hook_prompt_fragments() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "draft one"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "final draft"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_parallel_stop_hooks(
                home,
                &[FIRST_CONTINUATION_PROMPT, SECOND_CONTINUATION_PROMPT],
            )
            .expect("failed to write parallel stop hook fixtures");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("hello again").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        request_hook_prompt_texts(&requests[1]),
        vec![
            FIRST_CONTINUATION_PROMPT.to_string(),
            SECOND_CONTINUATION_PROMPT.to_string(),
        ],
        "second request should receive one user hook prompt message with both fragments",
    );

    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let rollout_text = fs::read_to_string(&rollout_path)?;
    assert_eq!(
        rollout_hook_prompt_texts(&rollout_text)?,
        vec![
            FIRST_CONTINUATION_PROMPT.to_string(),
            SECOND_CONTINUATION_PROMPT.to_string(),
        ],
        "rollout should preserve both hook prompt fragments in order",
    );

    Ok(())
}

#[tokio::test]
async fn blocked_user_prompt_submit_persists_additional_context_for_next_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "second prompt handled"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_user_prompt_submit_hook(home, "blocked first prompt", BLOCKED_PROMPT_CONTEXT)
                .expect("failed to write user prompt submit hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("blocked first prompt").await?;
    test.submit_turn("second prompt").await?;

    let request = response.single_request();
    assert!(
        request
            .message_input_texts("developer")
            .contains(&BLOCKED_PROMPT_CONTEXT.to_string()),
        "second request should include developer context persisted from the blocked prompt",
    );
    assert!(
        request
            .message_input_texts("user")
            .iter()
            .all(|text| !text.contains("blocked first prompt")),
        "blocked prompt should not be sent to the model",
    );
    assert!(
        request
            .message_input_texts("user")
            .iter()
            .any(|text| text.contains("second prompt")),
        "second request should include the accepted prompt",
    );

    let hook_inputs = read_user_prompt_submit_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 2);
    assert_eq!(
        hook_inputs
            .iter()
            .map(|input| {
                input["prompt"]
                    .as_str()
                    .expect("user prompt submit hook prompt")
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec![
            "blocked first prompt".to_string(),
            "second prompt".to_string()
        ],
    );
    assert!(
        hook_inputs.iter().all(|input| input["turn_id"]
            .as_str()
            .is_some_and(|turn_id| !turn_id.is_empty())),
        "blocked and accepted prompt hooks should both receive a non-empty turn_id",
    );

    Ok(())
}

#[tokio::test]
async fn blocked_queued_prompt_does_not_strand_earlier_accepted_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();
    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_added("msg-1", "")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_output_text_delta("first ")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_done("msg-1", "first response")),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(ev_completed("resp-1")),
        },
    ];
    let second_chunks = vec![StreamingSseChunk {
        gate: None,
        body: sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-2", "accepted queued prompt handled"),
            ev_completed("resp-2"),
        ]),
    }];
    let (server, _completions) =
        start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_pre_build_hook(|home| {
            write_user_prompt_submit_hook(home, "blocked queued prompt", BLOCKED_PROMPT_CONTEXT)
                .expect("failed to write user prompt submit hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build_with_streaming_server(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "initial prompt".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::AgentMessageContentDelta(_))
    })
    .await;

    for text in ["accepted queued prompt", "blocked queued prompt"] {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
    }

    sleep(Duration::from_millis(100)).await;
    let _ = gate_completed_tx.send(());

    let requests = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let requests = server.requests().await;
            if requests.len() >= 2 {
                break requests;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("second request should arrive")
    .into_iter()
    .collect::<Vec<_>>();

    sleep(Duration::from_millis(100)).await;

    assert_eq!(requests.len(), 2);

    let second_user_texts = request_message_input_texts(&requests[1], "user");
    assert!(
        second_user_texts.contains(&"accepted queued prompt".to_string()),
        "second request should include the accepted queued prompt",
    );
    assert!(
        !second_user_texts.contains(&"blocked queued prompt".to_string()),
        "second request should not include the blocked queued prompt",
    );

    let hook_inputs = read_user_prompt_submit_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 3);
    assert_eq!(
        hook_inputs
            .iter()
            .map(|input| {
                input["prompt"]
                    .as_str()
                    .expect("queued prompt hook prompt")
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec![
            "initial prompt".to_string(),
            "accepted queued prompt".to_string(),
            "blocked queued prompt".to_string(),
        ],
    );
    let queued_turn_ids = hook_inputs
        .iter()
        .map(|input| {
            input["turn_id"]
                .as_str()
                .expect("queued prompt hook turn_id")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(
        queued_turn_ids.iter().all(|turn_id| !turn_id.is_empty()),
        "queued prompt hook turn ids should be non-empty",
    );
    let first_queued_turn_id = queued_turn_ids
        .first()
        .expect("queued prompt hook inputs should include a first turn id")
        .clone();
    assert_eq!(
        queued_turn_ids,
        vec![
            first_queued_turn_id.clone(),
            first_queued_turn_id.clone(),
            first_queued_turn_id,
        ],
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn permission_request_hook_allows_exec_command_without_user_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-exec-command";
    let marker = std::env::temp_dir().join("permissionrequest-exec-command-marker");
    let command = format!("rm -f {}", marker.display());
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "permission request hook allowed it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            install_allow_permission_request_hook(home)
                .expect("failed to write permission request hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    fs::write(&marker, "seed").context("create permission request marker")?;

    test.submit_turn_with_approval_and_permission_profile(
        "run the shell command after hook approval",
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].function_call_output(call_id);
    assert!(
        !marker.exists(),
        "approved command should remove marker file"
    );

    let hook_inputs = assert_single_permission_request_hook_input(
        test.codex_home_path(),
        &command,
        /*description*/ None,
    )?;
    assert!(
        hook_inputs[0].get("tool_use_id").is_none(),
        "PermissionRequest input should not include a tool_use_id",
    );
    assert!(
        hook_inputs[0]["turn_id"]
            .as_str()
            .is_some_and(|turn_id| !turn_id.is_empty())
    );

    Ok(())
}

#[tokio::test]
async fn permission_request_hook_allow_bypasses_strict_auto_review() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "request_permissions currently requires a host-native cwd"
    );

    let server = start_mock_server().await;
    let permission_call_id = "strict-hook-permissions";
    let command_call_id = "strict-hook-exec-command";
    let marker_name = "strict-hook-exec-command-marker";
    let command = match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => format!("rm -f {marker_name}"),
        TestTargetOs::Windows => {
            format!("Remove-Item -Force -ErrorAction SilentlyContinue {marker_name}")
        }
    };
    let requested_permissions = RequestPermissionProfile {
        network: Some(NetworkPermissions {
            enabled: Some(true),
        }),
        ..Default::default()
    };
    let request_permissions_args = serde_json::json!({
        "reason": "Enable strict auto review",
        "permissions": requested_permissions,
    });
    let command_args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-strict-hook-1"),
                ev_function_call(
                    permission_call_id,
                    "request_permissions",
                    &serde_json::to_string(&request_permissions_args)?,
                ),
                ev_completed("resp-strict-hook-1"),
            ]),
            sse(vec![
                ev_response_created("resp-strict-hook-2"),
                ev_function_call(
                    command_call_id,
                    "exec_command",
                    &serde_json::to_string(&command_args)?,
                ),
                ev_completed("resp-strict-hook-2"),
            ]),
            sse(vec![
                ev_response_created("resp-strict-hook-3"),
                ev_assistant_message("msg-strict-hook", "permission hook allowed it"),
                ev_completed("resp-strict-hook-3"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            install_allow_permission_request_hook(home)
                .expect("failed to write permission request hook test fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("test config should allow feature update");
        });
    // Command hooks currently need a local executor even when the tool executor is remote.
    let test = builder.build_with_remote_and_local_env(&server).await?;

    let marker = test
        .executor_environment()
        .selection()
        .cwd
        .join(marker_name)?;
    test.fs()
        .write_file(
            &marker,
            b"seed".to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await
        .context("create strict auto-review marker")?;
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "request strict review, then run the shell command".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;

    let request = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RequestPermissions(_))
    })
    .await;
    let EventMsg::RequestPermissions(request) = request else {
        panic!("expected request permissions event");
    };
    assert_eq!(request.call_id, permission_call_id);
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: permission_call_id.to_string(),
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: true,
            },
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    requests[2].function_call_output(command_call_id);
    assert!(
        test.fs()
            .read_file(&marker, Default::default(), /*sandbox*/ None)
            .await
            .is_err(),
        "hook-approved command should remove marker without Guardian review"
    );
    assert_single_permission_request_hook_input(
        test.codex_home_path(),
        &command,
        /*description*/ None,
    )?;

    Ok(())
}

#[tokio::test]
async fn permission_request_hook_allows_apply_patch_with_write_alias() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-apply-patch";
    let file_name = "permission_request_apply_patch.txt";
    let patch_path = format!("../{file_name}");
    let patch = format!(
        r#"*** Begin Patch
*** Add File: {patch_path}
+approved
*** End Patch"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "permission request hook allowed apply_patch"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_permission_request_hook(
                home,
                Some("^Write$"),
                "allow",
                PERMISSION_REQUEST_ALLOW_REASON,
            )
            .expect("failed to write permission request hook test fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;
    let target_path = test.workspace_path(&patch_path);

    test.submit_turn_with_approval_and_permission_profile(
        "apply the patch after hook approval",
        AskForApproval::OnRequest,
        restrictive_workspace_write_profile(),
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].custom_tool_call_output(call_id);
    assert!(
        target_path.exists(),
        "approved apply_patch should create the out-of-workspace file"
    );

    assert_single_permission_request_hook_input_for_tool(
        test.codex_home_path(),
        "apply_patch",
        &patch,
        /*description*/ None,
    )?;

    Ok(())
}

#[tokio::test]
async fn permission_request_hook_sees_raw_exec_command_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-exec-command";
    let marker = std::env::temp_dir().join("permissionrequest-exec-command-marker");
    let command = format!("rm -f {}", marker.display());
    let justification = "remove the temporary marker";
    let args = serde_json::json!({
        "cmd": command,
        "login": true,
        "sandbox_permissions": "require_escalated",
        "justification": justification,
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "permission request hook allowed exec_command"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            install_allow_permission_request_hook(home)
                .expect("failed to write permission request hook test fixture");
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    fs::write(&marker, "seed").context("create exec command permission request marker")?;

    test.submit_turn_with_approval_and_permission_profile(
        "run the exec command after hook approval",
        AskForApproval::OnRequest,
        PermissionProfile::read_only(),
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].function_call_output(call_id);
    assert!(
        !marker.exists(),
        "approved exec command should remove marker file"
    );

    assert_single_permission_request_hook_input(
        test.codex_home_path(),
        &command,
        Some(justification),
    )?;

    Ok(())
}

#[tokio::test]
async fn permission_request_hook_allows_network_approval_without_prompt() -> Result<()> {
    let command = r#"python3 -c "import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); print('OK:' + opener.open('http://codex-network-test.invalid', timeout=2).read().decode(errors='replace'))""#;
    run_network_permission_hook_test(
        "allow",
        PERMISSION_REQUEST_ALLOW_REASON,
        "permissionrequest-network-approval",
        command,
        /*expected_denial*/ None,
    )
    .await
}

#[tokio::test]
async fn permission_request_hook_denies_network_approval_with_custom_message() -> Result<()> {
    let denial = "network access denied by the integration-test hook";
    let command = r#"python3 -c "import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); opener.open('http://codex-network-test.invalid', timeout=2).read()""#;
    run_network_permission_hook_test(
        "deny",
        denial,
        "permissionrequest-network-denied",
        command,
        Some(denial),
    )
    .await
}

async fn run_network_permission_hook_test(
    hook_mode: &'static str,
    hook_reason: &'static str,
    call_id: &'static str,
    command: &'static str,
    expected_denial: Option<&'static str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    fs::write(
        home.path().join("config.toml"),
        r#"default_permissions = "workspace"

[permissions.workspace.filesystem]
":minimal" = "read"

[permissions.workspace.network]
enabled = true
mode = "limited"
allow_local_binding = true
"#,
    )?;
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-network-hook-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
                ev_completed("resp-network-hook-1"),
            ]),
            sse(vec![
                ev_response_created("resp-network-hook-2"),
                ev_assistant_message("msg-network-hook", "done"),
                ev_completed("resp-network-hook-2"),
            ]),
        ],
    )
    .await;

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = network_workspace_write_profile();
    let permission_profile_for_config = permission_profile.clone();
    let test = test_codex()
        .with_home(Arc::clone(&home))
        .with_pre_build_hook(move |home| {
            write_permission_request_hook(
                home,
                Some(PERMISSION_REQUEST_HOOK_MATCHER),
                hook_mode,
                hook_reason,
            )
            .expect("failed to write permission request hook test fixture");
        })
        .with_cloud_config_bundle(managed_network_requirements_loader())
        .with_config(move |config| {
            trust_discovered_hooks(config);
            config.approvals_reviewer = codex_config::types::ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(approval_policy);
            config
                .permissions
                .set_permission_profile(permission_profile_for_config)
                .expect("set permission profile");
        })
        .build(&server)
        .await?;

    test.submit_turn_with_approval_and_permission_profile(
        "run the shell command after the network permission hook",
        approval_policy,
        permission_profile,
    )
    .await?;
    if expected_denial.is_none() {
        timeout(Duration::from_secs(10), async {
            loop {
                if test
                    .codex_home_path()
                    .join("permission_request_hook_log.jsonl")
                    .exists()
                {
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("expected network approval hook to run");
        assert!(
            timeout(
                Duration::from_secs(2),
                wait_for_event(&test.codex, |event| matches!(
                    event,
                    EventMsg::ExecApprovalRequest(_)
                ))
            )
            .await
            .is_err(),
            "expected the network approval hook to bypass the approval prompt"
        );
    }

    assert_single_permission_request_hook_input(
        test.codex_home_path(),
        command,
        Some("network-access http://codex-network-test.invalid:80"),
    )?;
    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .filter(|request| {
                request.body_json()["client_metadata"]["x-openai-subagent"].as_str()
                    == Some("guardian")
            })
            .count(),
        0
    );
    if let Some(expected_denial) = expected_denial {
        let tool_output = requests
            .iter()
            .find_map(|request| request.function_call_output_text(call_id))
            .expect("expected denied tool output");
        assert!(tool_output.contains(expected_denial));
    } else {
        test.codex.submit(Op::Shutdown {}).await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::ShutdownComplete)
        })
        .await;
    }

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_json_deny_blocks_exec_command_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-exec-command";
    let marker_dir = TempDir::new()?;
    let marker = marker_dir.path().join("pretooluse-exec-command-marker");
    let command = format!("git init --quiet {}", marker.display());
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^Bash$"), "json_deny", "blocked by pre hook")
                .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    builder = with_tool_analytics(builder, &server).await?; // copybara:strip-for-public
    let test = builder.build(&server).await?;

    test.submit_turn_with_permission_profile(
        "run the blocked shell command",
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked by pre hook"),
        "blocked tool output should surface the hook reason",
    );
    assert!(
        output.contains(&format!("Command: {command}")),
        "blocked tool output should surface the blocked command",
    );
    assert!(
        !marker.join(".git").is_dir(),
        "blocked command should not initialize the marker repository"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["hook_event_name"], "PreToolUse");
    assert_eq!(hook_inputs[0]["tool_name"], "Bash");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);
    let transcript_path = hook_inputs[0]["transcript_path"]
        .as_str()
        .expect("pre tool use hook transcript_path");
    assert!(
        !transcript_path.is_empty(),
        "pre tool use hook should receive a non-empty transcript_path",
    );
    assert!(
        Path::new(transcript_path).exists(),
        "pre tool use hook transcript_path should be materialized on disk",
    );
    assert!(
        hook_inputs[0]["turn_id"]
            .as_str()
            .is_some_and(|turn_id| !turn_id.is_empty())
    );

    // copybara:strip-for-public begin
    let metadata = serde_json::json!({
        "commands": [{"command": "git", "subcommand": "init", "flags": ["--quiet"]}],
    });
    assert_internal_tool_input(
        &server,
        call_id,
        serde_json::json!({
            "tool_name": "exec_command",
            "source_kind": "direct",
            "arguments_before_hooks": metadata.clone(),
            "hook_normalized_input": metadata,
            "arguments_after_hooks": null,
            "hook_outcome": "blocked",
        }),
    )
    .await?;
    // copybara:strip-for-public end
    Ok(())
}

#[tokio::test]
async fn pre_tool_use_records_additional_context_for_exec_command() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-exec-command-context";
    let command = "printf pre-tool-output".to_string();
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "pre hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let pre_context = "Remember the bash pre-tool note.";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^Bash$"), "context", pre_context)
                .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("run the shell command with pre hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&pre_context.to_string()),
        "follow-up request should include pre tool use additional context",
    );
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("pre-tool-output"),
        "shell command output should still reach the model",
    );

    Ok(())
}

#[tokio::test]
async fn blocked_pre_tool_use_records_additional_context_for_exec_command() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-exec-command-blocked-context";
    let marker = std::env::temp_dir().join("pretooluse-exec-command-blocked-context-marker");
    let command = format!("printf blocked > {}", marker.display());
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "blocked pre hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let pre_context = "blocked by pre hook with context";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^Bash$"), "json_deny_with_context", pre_context)
                .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover pre tool use marker")?;
    }

    test.submit_turn_with_permission_profile(
        "run the blocked shell command with pre hook context",
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&pre_context.to_string()),
        "follow-up request should include blocked pre tool use additional context",
    );
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked by pre hook with context"),
        "blocked tool output should still surface the hook reason",
    );
    assert!(
        !marker.exists(),
        "blocked command should not create marker file"
    );
    Ok(())
}

#[tokio::test]
async fn async_pre_tool_use_cannot_block_or_rewrite_and_still_records_additional_context()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let gate = TempDir::new()?;
    let release_path = gate.path().join("release");
    let hook_finished_path = gate.path().join("hook-finished");
    let hook_finished_path_for_fixture = hook_finished_path.clone();
    let original_marker = gate.path().join("original");
    let rewritten_marker = gate.path().join("rewritten");
    let call_id = "async-pretooluse-exec-command";
    let original_command = format!(
        r#"python3 -c 'import time; from pathlib import Path; gate = Path(r"{}"); exec("while not gate.exists(): time.sleep(0.01)"); Path(r"{}").write_text("original"); print("original-output")'"#,
        release_path.display(),
        original_marker.display()
    );
    let args = serde_json::json!({ "cmd": original_command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "async pre-tool hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let pre_context = "async pre-tool hook cannot block its launching command";
    let updated_input = serde_json::json!({
        "command": format!("printf rewritten > {}", rewritten_marker.display())
    });
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            let denying_script = home.join("async_deny_pre_tool_use_hook.py");
            fs::write(
                &denying_script,
                format!(
                    r#"import json, sys
from pathlib import Path
json.load(sys.stdin)
print(json.dumps({{
    "systemMessage": "{pre_context}",
    "hookSpecificOutput": {{
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": "{pre_context}",
        "additionalContext": "{pre_context}"
    }}
}}), flush=True)
Path(r"{hook_finished_path}").write_text("finished", encoding="utf-8")
"#,
                    hook_finished_path = hook_finished_path_for_fixture.display(),
                ),
            )
            .expect("write async denying pre-tool hook fixture");
            write_updating_pre_tool_use_hook(home, "^Bash$", &updated_input)
                .expect("write async rewriting pre-tool hook fixture");

            let hooks_path = home.join("hooks.json");
            let mut hooks: Value = serde_json::from_slice(
                &fs::read(&hooks_path).expect("read async pre-tool hook config"),
            )
            .expect("parse async pre-tool hook config");
            let handlers = hooks["hooks"]["PreToolUse"][0]["hooks"]
                .as_array_mut()
                .expect("pre-tool hook handlers");
            handlers[0]["async"] = Value::Bool(true);
            handlers.push(serde_json::json!({
                "type": "command",
                "command": format!("python3 {}", denying_script.display()),
                "async": true,
            }));
            fs::write(hooks_path, hooks.to_string()).expect("write async pre-tool hook config");
        })
        .with_config(trust_discovered_hooks)
        .build(&server)
        .await?;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run the original command with async pre-tool hooks".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(
                codex_protocol::protocol::ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::Never),
                    sandbox_policy: Some(sandbox_policy),
                    permission_profile,
                    ..Default::default()
                },
            ),
        )
        .await?;

    fs_wait::wait_for_path_exists(hook_finished_path, Duration::from_secs(5))
        .await
        .context("timed out waiting for the async pre-tool hook to finish")?;
    assert!(
        timeout(
            Duration::from_millis(150),
            wait_for_event(
                &test.codex,
                |event| matches!(event, EventMsg::Warning(warning) if warning.message == pre_context),
            ),
        )
        .await
        .is_err(),
        "async pre-tool output must not interrupt the launching command"
    );
    fs::write(release_path, "ready").context("release original shell command")?;
    timeout(
        Duration::from_secs(5),
        wait_for_event(
            &test.codex,
            |event| matches!(event, EventMsg::Warning(warning) if warning.message == pre_context),
        ),
    )
    .await
    .context("timed out waiting for the async pre-tool warning after sampling")?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&pre_context.to_string()),
        "async pre-tool hook context should reach the follow-up model request",
    );
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("original-output"),
        "async pre-tool hooks should not block the original command",
    );
    assert!(original_marker.exists(), "original command should execute");
    assert!(
        !rewritten_marker.exists(),
        "async pre-tool hooks should not rewrite the original command",
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum BashRewriteSurface {
    ExecCommand,
}

impl BashRewriteSurface {
    fn slug(self) -> &'static str {
        match self {
            BashRewriteSurface::ExecCommand => "exec-command",
        }
    }

    fn tool_call(self, call_id: &str, command_text: &str) -> Result<Value> {
        match self {
            BashRewriteSurface::ExecCommand => Ok(ev_function_call(
                call_id,
                "exec_command",
                &serde_json::to_string(&serde_json::json!({ "cmd": command_text }))?,
            )),
        }
    }

    fn original_command(self, marker: &Path) -> String {
        match self {
            BashRewriteSurface::ExecCommand => {
                format!("git init --quiet {}", marker.display())
            }
        }
    }

    fn rewritten_command(self, marker: &Path) -> String {
        match self {
            BashRewriteSurface::ExecCommand => {
                format!("git init {}", marker.display())
            }
        }
    }

    fn configure(self, config: &mut Config) {
        trust_discovered_hooks(config);
        if matches!(self, BashRewriteSurface::ExecCommand) {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        }
    }
}

async fn assert_pre_tool_use_rewrites_bash_surface(surface: BashRewriteSurface) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let slug = surface.slug();
    let call_id = format!("pretooluse-{slug}-rewrite");
    let marker_dir = TempDir::new()?;
    let original_marker = marker_dir.path().join("original");
    let rewritten_marker = marker_dir.path().join("rewritten");
    let original_command = surface.original_command(&original_marker);
    let rewritten_command = surface.rewritten_command(&rewritten_marker);
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                surface.tool_call(&call_id, &original_command)?,
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "hook rewrote it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let updated_input = serde_json::json!({ "command": rewritten_command });
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            write_updating_pre_tool_use_hook(home, "^Bash$", &updated_input)
                .expect("failed to write updating pre tool use hook fixture");
        })
        .with_config(move |config| surface.configure(config));
    builder = with_tool_analytics(builder, &server).await?; // copybara:strip-for-public
    let test = builder.build(&server).await?;

    test.submit_turn_with_permission_profile(
        &format!("run the rewritten {slug} command"),
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].function_call_output(&call_id);
    assert!(
        !original_marker.join(".git").is_dir(),
        "original {slug} command should not execute after rewrite"
    );
    assert!(
        rewritten_marker.join(".git").is_dir(),
        "rewritten {slug} command should initialize the marker repository"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], original_command);

    // copybara:strip-for-public begin
    let original_metadata = serde_json::json!({
        "commands": [{"command": "git", "subcommand": "init", "flags": ["--quiet"]}],
    });
    let rewritten_metadata = serde_json::json!({
        "commands": [{"command": "git", "subcommand": "init", "flags": []}],
    });
    assert_internal_tool_input(
        &server,
        &call_id,
        serde_json::json!({
            "tool_name": surface.slug().replace('-', "_"),
            "source_kind": "direct",
            "arguments_before_hooks": original_metadata.clone(),
            "hook_normalized_input": original_metadata,
            "arguments_after_hooks": rewritten_metadata,
            "hook_outcome": "rewritten",
        }),
    )
    .await?;
    // copybara:strip-for-public end
    Ok(())
}

#[tokio::test]
async fn pre_tool_use_rewrites_exec_command_before_execution() -> Result<()> {
    assert_pre_tool_use_rewrites_bash_surface(BashRewriteSurface::ExecCommand).await
}

#[tokio::test]
async fn pre_tool_use_rewrites_code_mode_nested_exec_command_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-code-mode-rewrite";
    let marker_dir = TempDir::new().context("create pre tool rewrite marker directory")?;
    let original_marker = marker_dir.path().join("original");
    let rewritten_marker = marker_dir.path().join("rewritten");
    let original_command = format!(
        "git init --quiet {}; git version",
        original_marker.display()
    );
    let rewritten_command = format!("git init {}; git version", rewritten_marker.display());
    let original_command_json =
        serde_json::to_string(&original_command).context("serialize original command")?;
    let code = format!(
        r#"
const output = await tools.exec_command({{ cmd: {original_command_json} }});
text(output.output);
"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call(call_id, "exec", &code),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "hook rewrote the nested command"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let updated_input = serde_json::json!({ "command": rewritten_command });
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_pre_build_hook(move |home| {
            write_updating_pre_tool_use_hook(home, "^Bash$", &updated_input)
                .expect("failed to write updating pre tool use hook fixture");
        })
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
            trust_discovered_hooks(config);
        });
    builder = with_tool_analytics(builder, &server).await?; // copybara:strip-for-public
    let test = builder.build(&server).await?;

    test.submit_turn_with_permission_profile(
        "run the rewritten shell command from code mode",
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].custom_tool_call_output(call_id);
    let output = code_mode_custom_tool_output_text(&output_item);
    assert!(
        output.contains("git version"),
        "code mode should receive the rewritten command result"
    );
    assert!(
        !original_marker.join(".git").is_dir(),
        "original nested shell command should not execute after rewrite"
    );
    assert!(
        rewritten_marker.join(".git").is_dir(),
        "rewritten nested command should initialize the marker repository"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], original_command);

    // copybara:strip-for-public begin
    let nested_call_id = hook_inputs[0]["tool_use_id"]
        .as_str()
        .context("code-mode hook should retain the nested tool call id")?;
    let original_metadata = serde_json::json!({
        "commands": [
            {"command": "git", "subcommand": "init", "flags": ["--quiet"]},
            {"command": "git", "subcommand": "version", "flags": []},
        ],
    });
    let rewritten_metadata = serde_json::json!({
        "commands": [
            {"command": "git", "subcommand": "init", "flags": []},
            {"command": "git", "subcommand": "version", "flags": []},
        ],
    });
    assert_internal_tool_input(
        &server,
        nested_call_id,
        serde_json::json!({
            "tool_name": "exec_command",
            "source_kind": "code_mode",
            "arguments_before_hooks": original_metadata.clone(),
            "hook_normalized_input": original_metadata,
            "arguments_after_hooks": rewritten_metadata,
            "hook_outcome": "rewritten",
        }),
    )
    .await?;
    // copybara:strip-for-public end
    Ok(())
}

#[tokio::test]
async fn pre_tool_use_block_rejects_code_mode_tool_promise_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-code-mode-block";
    let marker_dir = TempDir::new().context("create pre tool block marker directory")?;
    let marker = marker_dir.path().join("blocked");
    let command = format!("printf blocked > {}", marker.display());
    let command_json = serde_json::to_string(&command).context("serialize blocked command")?;
    let code = format!(
        r#"
try {{
  const result = await tools.exec_command({{ cmd: {command_json} }});
  text(JSON.stringify({{ kind: "unexpected-success", result }}));
}} catch (error) {{
  text(JSON.stringify({{ kind: "caught", error: String(error) }}));
}}
"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call(call_id, "exec", &code),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "pre hook block observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let reason = "blocked nested command";
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_pre_build_hook(move |home| {
            write_pre_tool_use_hook(home, Some("^Bash$"), "json_deny", reason)
                .expect("failed to write blocking pre tool use hook fixture");
        })
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_permission_profile(
        "run the blocked shell command from code mode",
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].custom_tool_call_output(call_id);
    let output = code_mode_custom_tool_output_text(&output_item);
    assert!(output.contains(r#""kind":"caught""#));
    assert!(output.contains(reason));
    assert!(!output.contains("unexpected-success"));
    assert!(
        !marker.exists(),
        "PreToolUse-blocked nested command should not execute"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);

    Ok(())
}

async fn assert_post_tool_use_blocks_code_mode_tool_result(
    hook_mode: &'static str,
    reason: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = format!("posttooluse-code-mode-{hook_mode}");
    let marker_dir = TempDir::new().context("create post tool block marker directory")?;
    let marker = marker_dir.path().join(hook_mode);
    let command = format!(
        "printf executed > {}; printf original-post-tool-result",
        marker.display()
    );
    let command_json = serde_json::to_string(&command).context("serialize post hook command")?;
    let code = format!(
        r#"
try {{
  const result = await tools.exec_command({{ cmd: {command_json} }});
  text(JSON.stringify({{ kind: "unexpected-success", result }}));
}} catch (error) {{
  text(JSON.stringify({{ kind: "caught", error: String(error) }}));
}}
"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call(&call_id, "exec", &code),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "post hook block observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_pre_build_hook(move |home| {
            write_post_tool_use_hook(home, Some("^Bash$"), hook_mode, reason)
                .expect("failed to write blocking post tool use hook fixture");
        })
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_permission_profile(
        "run the shell command blocked after execution from code mode",
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].custom_tool_call_output(&call_id);
    let output = code_mode_custom_tool_output_text(&output_item);
    assert!(output.contains(r#""kind":"caught""#));
    assert!(output.contains(reason));
    assert!(!output.contains("unexpected-success"));
    assert!(
        !output.contains("original-post-tool-result"),
        "blocked post tool result should not reach code mode"
    );
    assert_eq!(
        fs::read_to_string(&marker).context("read blocking post tool marker")?,
        "executed",
        "PostToolUse should run after the nested command executes"
    );

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);
    assert_eq!(
        hook_inputs[0]["tool_response"],
        Value::String("original-post-tool-result".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn post_tool_use_block_decision_rejects_code_mode_tool_promise() -> Result<()> {
    assert_post_tool_use_blocks_code_mode_tool_result(
        "decision_block",
        "blocked nested result by decision",
    )
    .await
}

#[tokio::test]
async fn post_tool_use_exit_two_rejects_code_mode_tool_promise() -> Result<()> {
    assert_post_tool_use_blocks_code_mode_tool_result("exit_2", "blocked nested result by exit two")
        .await
}

#[tokio::test]
async fn plugin_pre_tool_use_blocks_exec_command_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "plugin-pretooluse-exec-command";
    let marker = std::env::temp_dir().join("plugin-pretooluse-exec-command-marker");
    let command = format!("printf blocked > {}", marker.display());
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "plugin hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let home = Arc::new(TempDir::new()?);
    let plugin_root = home.path().join("plugins/cache/test/sample/local");
    let hooks_dir = plugin_root.join("hooks");
    fs::create_dir_all(plugin_root.join(".codex-plugin"))
        .context("create plugin manifest directory")?;
    fs::create_dir_all(&hooks_dir).context("create plugin hooks directory")?;
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .context("write plugin manifest")?;
    fs::write(
        home.path().join("config.toml"),
        r#"[plugins."sample@test"]
enabled = true
"#,
    )
    .context("write plugin config")?;

    let script_path = hooks_dir.join("pre_tool_use_hook.py");
    let log_path = hooks_dir.join("pre_tool_use_hook_log.jsonl");
    fs::write(
        &script_path,
        format!(
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
        "permissionDecisionReason": "blocked by plugin hook"
    }}
}}))
"#,
            log_path = log_path.display(),
        ),
    )
    .context("write plugin pre tool use hook script")?;
    let plugin_hooks_json = r#"{
  "hooks": {
    "PreToolUse": [{
      "matcher": "^Bash$",
      "hooks": [{
        "type": "command",
        "command": "python3 ${PLUGIN_ROOT}/hooks/pre_tool_use_hook.py"
      }]
    }]
  }
}"#;
    let plugin_hooks_path = hooks_dir.join("hooks.json");
    fs::write(&plugin_hooks_path, plugin_hooks_json).context("write plugin hooks config")?;
    let plugin_root_abs =
        AbsolutePathBuf::try_from(plugin_root.clone()).context("absolute plugin root")?;
    let plugin_hooks_path_abs =
        AbsolutePathBuf::try_from(plugin_hooks_path).context("absolute plugin hooks path")?;
    let plugin_data_root =
        AbsolutePathBuf::try_from(plugin_root.join("data")).context("absolute plugin data root")?;
    let plugin_hook_sources = vec![PluginHookSource {
        plugin_id: PluginId::parse("sample@test").context("plugin id")?,
        plugin_root: plugin_root_abs,
        plugin_data_root,
        source_path: plugin_hooks_path_abs,
        source_relative_path: "hooks/hooks.json".to_string(),
        hooks: serde_json::from_str::<codex_config::HooksFile>(plugin_hooks_json)
            .context("parse plugin hooks")?
            .hooks,
    }];

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Plugins)
                .expect("test config should allow feature update");
            trust_plugin_hooks(config, plugin_hook_sources);
        });
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover plugin pre tool use marker")?;
    }

    test.submit_turn_with_policy(
        "run the shell command blocked by a plugin hook",
        codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked by plugin hook"),
        "blocked tool output should surface the plugin hook reason",
    );
    assert!(
        !marker.exists(),
        "plugin hook should block shell command execution"
    );

    let hook_inputs = read_hook_inputs_from_log(&log_path)?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["hook_event_name"], "PreToolUse");
    assert_eq!(hook_inputs[0]["tool_name"], "Bash");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_blocks_shell_when_defined_in_config_toml() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-config-toml";
    let marker = std::env::temp_dir().join("pretooluse-config-toml-marker");
    let command = format!("printf blocked > {}", marker.display());
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "config.toml hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook_toml(
                home,
                "pre_tool_use_config_hook.py",
                "pre_tool_use_config_hook_log.jsonl",
                Some("^Bash$"),
                "json_deny",
                "blocked by config toml hook",
            )
            .expect("failed to write config.toml hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover config.toml marker")?;
    }

    test.submit_turn_with_permission_profile(
        "run the blocked shell command from config toml",
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked by config toml hook"),
        "blocked tool output should surface the config.toml hook reason",
    );
    assert!(
        !marker.exists(),
        "config.toml hook should block command execution"
    );

    let hook_inputs = read_hook_inputs_from_log(
        test.codex_home_path()
            .join("pre_tool_use_config_hook_log.jsonl")
            .as_path(),
    )?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["hook_event_name"], "PreToolUse");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_merges_hooks_json_and_config_toml() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-merged-sources";
    let command = "printf merged-hooks".to_string();
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "merged hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^Bash$"), "allow", "unused")
                .expect("failed to write hooks.json hook fixture");
            write_pre_tool_use_hook_toml(
                home,
                "pre_tool_use_toml_hook.py",
                "pre_tool_use_toml_hook_log.jsonl",
                Some("^Bash$"),
                "allow",
                "unused",
            )
            .expect("failed to write config.toml hook fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("run the shell command with merged hook sources")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("merged-hooks"),
        "shell command output should still reach the model",
    );

    let json_hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?
        .into_iter()
        .map(|hook_input| {
            serde_json::json!({
                "hook_event_name": hook_input["hook_event_name"],
                "tool_name": hook_input["tool_name"],
                "tool_use_id": hook_input["tool_use_id"],
                "tool_input": hook_input["tool_input"],
            })
        })
        .collect::<Vec<_>>();
    let toml_hook_inputs = read_hook_inputs_from_log(
        test.codex_home_path()
            .join("pre_tool_use_toml_hook_log.jsonl")
            .as_path(),
    )?
    .into_iter()
    .map(|hook_input| {
        serde_json::json!({
            "hook_event_name": hook_input["hook_event_name"],
            "tool_name": hook_input["tool_name"],
            "tool_use_id": hook_input["tool_use_id"],
            "tool_input": hook_input["tool_input"],
        })
    })
    .collect::<Vec<_>>();
    let expected_hook_inputs = vec![serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_use_id": call_id,
        "tool_input": {
            "command": command,
        },
    })];
    assert_eq!(expected_hook_inputs, json_hook_inputs);
    assert_eq!(expected_hook_inputs, toml_hook_inputs);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_blocks_exec_command_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-exec-command";
    let marker = std::env::temp_dir().join("pretooluse-exec-command-marker");
    let command = format!("printf blocked > {}", marker.display());
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "exec command blocked"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^Bash$"), "exit_2", "blocked exec command")
                .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover exec marker")?;
    }

    test.submit_turn("run the blocked exec command").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("exec command output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked exec command"),
        "blocked exec command output should surface the hook reason",
    );
    assert!(
        output.contains(&format!("Command: {command}")),
        "blocked exec command output should surface the blocked command",
    );
    assert!(!marker.exists(), "blocked exec command should not execute");

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);
    assert!(
        hook_inputs[0]["turn_id"]
            .as_str()
            .is_some_and(|turn_id| !turn_id.is_empty())
    );

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_blocks_apply_patch_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-apply-patch";
    let file_name = "pre_tool_use_apply_patch.txt";
    let patch = format!(
        r#"*** Begin Patch
*** Add File: {file_name}
+blocked
*** End Patch"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "apply_patch blocked"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(
                home,
                Some("^apply_patch$"),
                "json_deny",
                "blocked apply_patch",
            )
            .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the blocked patch").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].custom_tool_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked apply_patch"),
        "blocked apply_patch output should surface the hook reason",
    );
    assert!(
        !test.workspace_path(file_name).exists(),
        "blocked apply_patch should not create the file"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_name"], "apply_patch");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], patch);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_rewrites_apply_patch_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-apply-patch-rewrite";
    let original_file = "pre_tool_use_apply_patch_original.txt";
    let rewritten_file = "pre_tool_use_apply_patch_rewritten.txt";
    let original_patch = format!(
        r#"*** Begin Patch
*** Add File: {original_file}
+original
*** End Patch"#
    );
    let rewritten_patch = format!(
        r#"*** Begin Patch
*** Add File: {rewritten_file}
+rewritten
*** End Patch"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &original_patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "apply_patch rewritten"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let updated_input = serde_json::json!({ "command": rewritten_patch });
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            write_updating_pre_tool_use_hook(home, "^apply_patch$", &updated_input)
                .expect("failed to write updating pre tool use hook fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the rewritten patch").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].custom_tool_call_output(call_id);
    assert!(
        !test.workspace_path(original_file).exists(),
        "original patch should not create its target file"
    );
    assert_eq!(
        fs::read_to_string(test.workspace_path(rewritten_file))
            .context("read rewritten apply_patch file")?,
        "rewritten\n"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], original_patch);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_blocks_apply_patch_with_write_alias() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-apply-patch-write";
    let file_name = "pre_tool_use_apply_patch_write.txt";
    let patch = format!(
        r#"*** Begin Patch
*** Add File: {file_name}
+blocked
*** End Patch"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "apply_patch blocked by Write alias"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^Write$"), "json_deny", "blocked write alias")
                .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the patch blocked by Write alias")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].custom_tool_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked write alias"),
        "blocked apply_patch output should surface the hook reason",
    );
    assert!(
        !test.workspace_path(file_name).exists(),
        "blocked apply_patch should not create the file"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_name"], "apply_patch");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], patch);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_blocks_local_function_tool_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-local-function-tool";
    let args = serde_json::json!({});
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "test_sync_tool", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "local function hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let reason = "blocked local function pre hook";
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_pre_build_hook(|home| {
            write_pre_tool_use_hook(home, Some("^test_sync_tool$"), "json_deny", reason)
                .expect("failed to write pre tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("call the local function tool with the pre hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("blocked local function tool output string");
    assert!(
        output.contains(&format!(
            "Tool call blocked by PreToolUse hook: {reason}. Tool: test_sync_tool"
        )),
        "blocked local function output should surface the hook reason and tool name",
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["hook_event_name"], "PreToolUse");
    assert_eq!(hook_inputs[0]["tool_name"], "test_sync_tool");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"], args);

    Ok(())
}

#[tokio::test]
async fn pre_tool_use_rewrites_local_function_tool_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-local-function-tool-rewrite";
    let original_args = serde_json::json!({
        "barrier": {
            "id": "pretooluse-local-function-invalid-barrier",
            "participants": 0,
        }
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "test_sync_tool",
                    &serde_json::to_string(&original_args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "local function hook rewrote it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let updated_input = serde_json::json!({});
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_pre_build_hook(move |home| {
            write_updating_pre_tool_use_hook(home, "^test_sync_tool$", &updated_input)
                .expect("failed to write updating pre tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("call the local function tool with the pre hook rewrite")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("rewritten local function tool output string");
    assert_eq!(output, "ok");

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_input"], original_args);

    Ok(())
}

#[tokio::test]
async fn post_tool_use_records_additional_context_for_exec_command() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-exec-command";
    let command = "printf post-tool-output".to_string();
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "post hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let post_context = "Remember the bash post-tool note.";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_post_tool_use_hook(home, Some("^Bash$"), "context", post_context)
                .expect("failed to write post tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("run the shell command with post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&post_context.to_string()),
        "follow-up request should include post tool use additional context",
    );
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert!(
        output.contains("post-tool-output"),
        "shell command output should still reach the model",
    );

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["hook_event_name"], "PostToolUse");
    assert_eq!(hook_inputs[0]["tool_name"], "Bash");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);
    assert_eq!(
        hook_inputs[0]["tool_response"],
        Value::String("post-tool-output".to_string())
    );
    let transcript_path = hook_inputs[0]["transcript_path"]
        .as_str()
        .expect("post tool use hook transcript_path");
    assert!(
        !transcript_path.is_empty(),
        "post tool use hook should receive a non-empty transcript_path",
    );
    assert!(
        Path::new(transcript_path).exists(),
        "post tool use hook transcript_path should be materialized on disk",
    );
    assert!(
        hook_inputs[0]["turn_id"]
            .as_str()
            .is_some_and(|turn_id| !turn_id.is_empty())
    );

    Ok(())
}

#[tokio::test]
async fn post_tool_use_block_decision_replaces_exec_command_output_with_reason() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-exec-command-block";
    let command = "printf blocked-output".to_string();
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "post hook feedback observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let reason = "bash output looked sketchy";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_post_tool_use_hook(home, Some("^Bash$"), "decision_block", reason)
                .expect("failed to write post tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("run the shell command with blocking post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert_eq!(output, reason);

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_response"],
        Value::String("blocked-output".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn post_tool_use_continue_false_replaces_exec_command_output_with_stop_reason() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-exec-command-stop";
    let command = "printf stop-output".to_string();
    let args = serde_json::json!({ "cmd": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "post hook stop observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let stop_reason = "Execution halted by post-tool hook";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_post_tool_use_hook(home, Some("^Bash$"), "continue_false", stop_reason)
                .expect("failed to write post tool use hook test fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("run the shell command with stop-style post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell command output string");
    assert_eq!(output, stop_reason);

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_response"],
        Value::String("stop-output".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn post_tool_use_exit_two_replaces_one_shot_exec_command_output_with_feedback() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-exec-command";
    let command = "printf post-hook-output".to_string();
    let args = serde_json::json!({ "cmd": command, "tty": false });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "post hook blocked the exec result"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_post_tool_use_hook(home, Some("^Bash$"), "exit_2", "blocked by post hook")
                .expect("failed to write post tool use hook test fixture");
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("run the exec command with post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("exec command output string");
    assert_eq!(output, "blocked by post hook");

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], command);
    assert_eq!(
        hook_inputs[0]["tool_response"],
        Value::String("post-hook-output".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn post_tool_use_spills_large_feedback_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-large-feedback";
    let command = "printf post-hook-output".to_string();
    let args = serde_json::json!({ "cmd": command, "tty": false });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "post hook blocked the exec result"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let feedback = "blocked by post hook ".repeat(800);

    let mut builder = test_codex()
        .with_pre_build_hook({
            let feedback = feedback.clone();
            move |home| {
                write_post_tool_use_hook(home, Some("^Bash$"), "exit_2", &feedback)
                    .expect("failed to write post tool use hook test fixture");
            }
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("run the exec command with long post-hook feedback")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("exec command output string");
    assert!(output.contains("tokens truncated"));
    let path = spilled_hook_output_path(output).context("spill path")?;
    assert_eq!(fs::read_to_string(path)?, feedback.trim());

    Ok(())
}

#[tokio::test]
async fn post_tool_use_blocks_when_exec_session_completes_via_write_stdin() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_host_windows!(Ok(()));

    let server = start_mock_server().await;
    let start_call_id = "posttooluse-exec-session-start";
    let poll_call_id = "posttooluse-exec-session-poll";
    let command = "sleep 1; printf session-post-hook-output".to_string();
    let start_args = serde_json::json!({
        "cmd": command,
        "shell": "/bin/sh",
        "login": false,
        "tty": false,
        "yield_time_ms": 250,
    });
    let poll_args = serde_json::json!({
        "session_id": 1000,
        "chars": "",
        "yield_time_ms": 5_000,
    });
    let feedback = "blocked by session post hook";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    start_call_id,
                    "exec_command",
                    &serde_json::to_string(&start_args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                core_test_support::responses::ev_function_call(
                    poll_call_id,
                    "write_stdin",
                    &serde_json::to_string(&poll_args)?,
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "session post hook observed"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_logging_pre_and_blocking_post_tool_use_hooks(home, feedback)
                .expect("failed to write tool use hook test fixture");
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("run the exec command session with post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let output_item = requests[2].function_call_output(poll_call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("write_stdin output string");
    assert_eq!(output, feedback);

    let pre_hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(pre_hook_inputs.len(), 1);
    assert_eq!(pre_hook_inputs[0]["tool_name"], "Bash");
    assert_eq!(pre_hook_inputs[0]["tool_use_id"], start_call_id);
    assert_eq!(pre_hook_inputs[0]["tool_input"]["command"], command);

    let post_hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(post_hook_inputs.len(), 1);
    assert_eq!(post_hook_inputs[0]["hook_event_name"], "PostToolUse");
    assert_eq!(post_hook_inputs[0]["tool_name"], "Bash");
    assert_eq!(post_hook_inputs[0]["tool_use_id"], start_call_id);
    assert_eq!(post_hook_inputs[0]["tool_input"]["command"], command);
    assert!(
        post_hook_inputs[0]["tool_response"]
            .as_str()
            .is_some_and(|tool_response| tool_response.contains("session-post-hook-output")),
        "PostToolUse should see the final session output, got {:?}",
        post_hook_inputs[0]["tool_response"]
    );

    Ok(())
}

#[tokio::test]
async fn post_tool_use_records_additional_context_for_apply_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-apply-patch";
    let file_name = "post_tool_use_apply_patch.txt";
    let patch = format!(
        r#"*** Begin Patch
*** Add File: {file_name}
+patched
*** End Patch"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "apply_patch post hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let post_context = "Remember the apply_patch post-tool note.";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_post_tool_use_hook(home, Some("^apply_patch$"), "context", post_context)
                .expect("failed to write post tool use hook test fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the patch with post hook").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&post_context.to_string()),
        "follow-up request should include apply_patch post tool use context",
    );
    assert!(
        test.workspace_path(file_name).exists(),
        "apply_patch should create the file"
    );

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_name"], "apply_patch");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], patch);
    let tool_response = hook_inputs[0]["tool_response"]
        .as_str()
        .context("apply_patch tool_response should be a string")?;
    assert!(tool_response.starts_with("Exit code: 0"));
    assert!(tool_response.contains("Success. Updated the following files:"));
    assert!(tool_response.contains("A post_tool_use_apply_patch.txt"));

    Ok(())
}

#[tokio::test]
async fn post_tool_use_records_apply_patch_context_with_edit_alias() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-apply-patch-edit";
    let file_name = "post_tool_use_apply_patch_edit.txt";
    let patch = format!(
        r#"*** Begin Patch
*** Add File: {file_name}
+patched
*** End Patch"#
    );
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "apply_patch edit hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let post_context = "Remember the edit alias post-tool note.";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_post_tool_use_hook(home, Some("^Edit$"), "context", post_context)
                .expect("failed to write post tool use hook test fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the patch with edit alias post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&post_context.to_string()),
        "follow-up request should include apply_patch post tool use context",
    );
    assert!(
        test.workspace_path(file_name).exists(),
        "apply_patch should create the file"
    );

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(hook_inputs[0]["tool_name"], "apply_patch");
    assert_eq!(hook_inputs[0]["tool_use_id"], call_id);
    assert_eq!(hook_inputs[0]["tool_input"]["command"], patch);

    Ok(())
}
