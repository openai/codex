use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::NetworkConstraints;
use codex_config::NetworkRequirementsToml;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_windows;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
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
codex_hooks = true

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

fn write_session_start_hook_recording_transcript(home: &Path) -> Result<()> {
    let script_path = home.join("session_start_hook.py");
    let log_path = home.join("session_start_hook_log.jsonl");
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
transcript_path = payload.get("transcript_path")
record = {{
    "transcript_path": transcript_path,
    "exists": Path(transcript_path).exists() if transcript_path else False,
}}

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(record) + "\n")
"#,
        log_path = log_path.display(),
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

fn rollout_hook_prompt_texts(text: &str) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = serde_json::from_str(trimmed).context("parse rollout line")?;
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) = rollout.item
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
    let body: Value = match serde_json::from_slice(body) {
        Ok(body) => body,
        Err(error) => panic!("parse request body: {error}"),
    };
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) = write_stop_hook(
                home,
                &[FIRST_CONTINUATION_PROMPT, SECOND_CONTINUATION_PROMPT],
            ) {
                panic!("failed to write stop hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) = write_session_start_hook_recording_transcript(home) {
                panic!("failed to write session start hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;

    let hook_inputs = read_session_start_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]
            .get("transcript_path")
            .and_then(Value::as_str)
            .map(str::is_empty),
        Some(false)
    );
    assert_eq!(hook_inputs[0].get("exists"), Some(&Value::Bool(true)));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) = write_stop_hook(home, &[FIRST_CONTINUATION_PROMPT]) {
                panic!("failed to write stop hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let initial = initial_builder.build(&server).await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

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

    let mut resume_builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::CodexHooks)
            .expect("test config should allow feature update");
    });
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;

    resumed.submit_turn("and now continue").await?;

    let resumed_request = resumed_response.single_request();
    assert_eq!(
        request_hook_prompt_texts(&resumed_request),
        vec![FIRST_CONTINUATION_PROMPT.to_string()],
        "resumed request should keep the persisted continuation prompt in user history",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) = write_parallel_stop_hooks(
                home,
                &[FIRST_CONTINUATION_PROMPT, SECOND_CONTINUATION_PROMPT],
            ) {
                panic!("failed to write parallel stop hook fixtures: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) =
                write_user_prompt_submit_hook(home, "blocked first prompt", BLOCKED_PROMPT_CONTEXT)
            {
                panic!("failed to write user prompt submit hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) =
                write_user_prompt_submit_hook(home, "blocked queued prompt", BLOCKED_PROMPT_CONTEXT)
            {
                panic!("failed to write user prompt submit hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build_with_streaming_server(&server).await?;

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "initial prompt".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::AgentMessageContentDelta(_))
    })
    .await;

    for text in ["accepted queued prompt", "blocked queued prompt"] {
        test.codex
            .submit(Op::UserInput {
                environments: None,
                items: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
            })
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_request_hook_allows_shell_command_without_user_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-shell-command";
    let marker = std::env::temp_dir().join("permissionrequest-shell-command-marker");
    let command = format!("rm -f {}", marker.display());
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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
            if let Err(error) = install_allow_permission_request_hook(home) {
                panic!("failed to write permission request hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    fs::write(&marker, "seed").context("create permission request marker")?;

    test.submit_turn_with_policies(
        "run the shell command after hook approval",
        AskForApproval::OnRequest,
        codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
                ev_apply_patch_function_call(call_id, &patch),
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
            if let Err(error) = write_permission_request_hook(
                home,
                Some("^Write$"),
                "allow",
                PERMISSION_REQUEST_ALLOW_REASON,
            ) {
                panic!("failed to write permission request hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.include_apply_patch_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    let target_path = test.workspace_path(&patch_path);

    test.submit_turn_with_policies(
        "apply the patch after hook approval",
        AskForApproval::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        },
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].function_call_output(call_id);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) = install_allow_permission_request_hook(home) {
                panic!("failed to write permission request hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    fs::write(&marker, "seed").context("create exec command permission request marker")?;

    test.submit_turn_with_policies(
        "run the exec command after hook approval",
        AskForApproval::OnRequest,
        codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_request_hook_allows_network_approval_without_prompt() -> Result<()> {
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
    let call_id = "permissionrequest-network-approval";
    let command = r#"python3 -c "import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); print('OK:' + opener.open('http://codex-network-test.invalid', timeout=2).read().decode(errors='replace'))""#;
    let args = serde_json::json!({ "command": command });
    let _responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "permission request hook allowed network access"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let approval_policy = AskForApproval::OnFailure;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: true,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();
    let test = test_codex()
        .with_home(Arc::clone(&home))
        .with_pre_build_hook(|home| {
            if let Err(error) = install_allow_permission_request_hook(home) {
                panic!("failed to write permission request hook test fixture: {error}");
            }
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
            config.permissions.approval_policy = Constrained::allow_any(approval_policy);
            config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
            let layers = config
                .config_layer_stack
                .get_layers(
                    ConfigLayerStackOrdering::LowestPrecedenceFirst,
                    /*include_disabled*/ true,
                )
                .into_iter()
                .cloned()
                .collect();
            let mut requirements = config.config_layer_stack.requirements().clone();
            requirements.network = Some(Sourced::new(
                NetworkConstraints {
                    enabled: Some(true),
                    allow_local_binding: Some(true),
                    ..Default::default()
                },
                RequirementSource::CloudRequirements,
            ));
            let mut requirements_toml = config.config_layer_stack.requirements_toml().clone();
            requirements_toml.network = Some(NetworkRequirementsToml {
                enabled: Some(true),
                allow_local_binding: Some(true),
                ..Default::default()
            });
            config.config_layer_stack =
                ConfigLayerStack::new(layers, requirements, requirements_toml)
                    .expect("rebuild config layer stack with network requirements");
        })
        .build(&server)
        .await?;
    assert!(
        test.config.managed_network_requirements_enabled(),
        "expected managed network requirements to be enabled"
    );
    assert!(
        test.config.permissions.network.is_some(),
        "expected managed network proxy config to be present"
    );
    test.session_configured
        .network_proxy
        .as_ref()
        .expect("expected runtime managed network proxy addresses");

    test.submit_turn_with_policies(
        "run the shell command after network hook approval",
        approval_policy,
        sandbox_policy,
    )
    .await?;

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

    assert_single_permission_request_hook_input(
        test.codex_home_path(),
        command,
        Some("network-access http://codex-network-test.invalid:80"),
    )?;

    test.codex.submit(Op::Shutdown {}).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_request_hook_sees_retry_context_after_sandbox_denial() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-retry-shell-command";
    let marker = "permissionrequest_retry_marker.txt";
    let command = format!("printf retry > {marker}");
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "permission request hook allowed retry"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            if let Err(error) = install_allow_permission_request_hook(home) {
                panic!("failed to write permission request hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    let marker_path = test.workspace_path(marker);
    let _ = fs::remove_file(&marker_path);

    test.submit_turn_with_policies(
        "retry the shell command after sandbox denial",
        AskForApproval::OnFailure,
        codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    requests[1].function_call_output(call_id);
    assert_eq!(
        fs::read_to_string(&marker_path).context("read retry marker")?,
        "retry"
    );

    assert_single_permission_request_hook_input(
        test.codex_home_path(),
        &command,
        /*description*/ None,
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_shell_command_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-shell-command";
    let marker = std::env::temp_dir().join("pretooluse-shell-command-marker");
    let command = format!("printf blocked > {}", marker.display());
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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
            if let Err(error) =
                write_pre_tool_use_hook(home, Some("^Bash$"), "json_deny", "blocked by pre hook")
            {
                panic!("failed to write pre tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover pre tool use marker")?;
    }

    test.submit_turn_with_policy(
        "run the blocked shell command",
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
        output.contains("Command blocked by PreToolUse hook: blocked by pre hook"),
        "blocked tool output should surface the hook reason",
    );
    assert!(
        output.contains(&format!("Command: {command}")),
        "blocked tool output should surface the blocked command",
    );
    assert!(
        !marker.exists(),
        "blocked command should not create marker file"
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

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_shell_when_defined_in_config_toml() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-config-toml";
    let marker = std::env::temp_dir().join("pretooluse-config-toml-marker");
    let command = format!("printf blocked > {}", marker.display());
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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

    let mut builder = test_codex().with_pre_build_hook(|home| {
        if let Err(error) = write_pre_tool_use_hook_toml(
            home,
            "pre_tool_use_config_hook.py",
            "pre_tool_use_config_hook_log.jsonl",
            Some("^Bash$"),
            "json_deny",
            "blocked by config toml hook",
        ) {
            panic!("failed to write config.toml hook test fixture: {error}");
        }
    });
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover config.toml marker")?;
    }

    test.submit_turn_with_policy(
        "run the blocked shell command from config toml",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_merges_hooks_json_and_config_toml() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-merged-sources";
    let command = "printf merged-hooks".to_string();
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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

    let mut builder = test_codex().with_pre_build_hook(|home| {
        if let Err(error) = write_pre_tool_use_hook(home, Some("^Bash$"), "allow", "unused") {
            panic!("failed to write hooks.json hook fixture: {error}");
        }
        if let Err(error) = write_pre_tool_use_hook_toml(
            home,
            "pre_tool_use_toml_hook.py",
            "pre_tool_use_toml_hook_log.jsonl",
            Some("^Bash$"),
            "allow",
            "unused",
        ) {
            panic!("failed to write config.toml hook fixture: {error}");
        }
    });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_local_shell_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-local-shell";
    let marker = std::env::temp_dir().join("pretooluse-local-shell-marker");
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        format!("printf blocked > {}", marker.display()),
    ];
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_local_shell_call(
                    call_id,
                    "completed",
                    command.iter().map(String::as_str).collect(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "local shell blocked"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            if let Err(error) =
                write_pre_tool_use_hook(home, Some("^Bash$"), "json_deny", "blocked local shell")
            {
                panic!("failed to write pre tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    if marker.exists() {
        fs::remove_file(&marker).context("remove leftover local shell marker")?;
    }

    test.submit_turn("run the blocked local shell command")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("local shell output string");
    assert!(
        output.contains("Command blocked by PreToolUse hook: blocked local shell"),
        "blocked local shell output should surface the hook reason",
    );
    assert!(
        output.contains(&format!(
            "Command: {}",
            codex_shell_command::parse_command::shlex_join(&command)
        )),
        "blocked local shell output should surface the blocked command",
    );
    assert!(
        !marker.exists(),
        "blocked local shell command should not execute"
    );

    let hook_inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_input"]["command"],
        codex_shell_command::parse_command::shlex_join(&command),
    );
    assert!(
        hook_inputs[0]["turn_id"]
            .as_str()
            .is_some_and(|turn_id| !turn_id.is_empty())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) =
                write_pre_tool_use_hook(home, Some("^Bash$"), "exit_2", "blocked exec command")
            {
                panic!("failed to write pre tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
                ev_apply_patch_function_call(call_id, &patch),
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
            if let Err(error) = write_pre_tool_use_hook(
                home,
                Some("^apply_patch$"),
                "json_deny",
                "blocked apply_patch",
            ) {
                panic!("failed to write pre tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.include_apply_patch_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the blocked patch").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
                ev_apply_patch_function_call(call_id, &patch),
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
            if let Err(error) =
                write_pre_tool_use_hook(home, Some("^Write$"), "json_deny", "blocked write alias")
            {
                panic!("failed to write pre tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.include_apply_patch_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("apply the patch blocked by Write alias")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_does_not_fire_for_plan_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-update-plan";
    let args = serde_json::json!({
        "plan": [{
            "step": "watch the tide",
            "status": "pending",
        }]
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "update_plan",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "plan updated"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            if let Err(error) =
                write_pre_tool_use_hook(home, /*matcher*/ None, "json_deny", "should not fire")
            {
                panic!("failed to write pre tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("update the plan").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("update plan output string");
    assert!(
        !output.contains("should not fire"),
        "non-shell tool output should not be blocked by PreToolUse",
    );

    let hook_log_path = test.codex_home_path().join("pre_tool_use_hook_log.jsonl");
    assert!(
        !hook_log_path.exists(),
        "plan tool should not trigger pre tool use hooks",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_records_additional_context_for_shell_command() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-shell-command";
    let command = "printf post-tool-output".to_string();
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^Bash$"), "context", post_context)
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_block_decision_replaces_shell_command_output_with_reason() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-shell-command-block";
    let command = "printf blocked-output".to_string();
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^Bash$"), "decision_block", reason)
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_continue_false_replaces_shell_command_output_with_stop_reason() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-shell-command-stop";
    let command = "printf stop-output".to_string();
    let args = serde_json::json!({ "command": command });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "shell_command",
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
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^Bash$"), "continue_false", stop_reason)
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_records_additional_context_for_local_shell() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-local-shell";
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf local-post-tool-output".to_string(),
    ];
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_local_shell_call(
                    call_id,
                    "completed",
                    command.iter().map(String::as_str).collect(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "local shell post hook context observed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let post_context = "Remember the local shell post-tool note.";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^Bash$"), "context", post_context)
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("run the local shell command with post hook")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&post_context.to_string()),
        "follow-up request should include local shell post tool use additional context",
    );
    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_input"]["command"],
        codex_shell_command::parse_command::shlex_join(&command),
    );
    assert_eq!(
        hook_inputs[0]["tool_response"],
        Value::String("local-post-tool-output".to_string()),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^Bash$"), "exit_2", "blocked by post hook")
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_blocks_when_exec_session_completes_via_write_stdin() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

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
            if let Err(error) = write_logging_pre_and_blocking_post_tool_use_hooks(home, feedback) {
                panic!("failed to write tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
                ev_apply_patch_function_call(call_id, &patch),
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
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^apply_patch$"), "context", post_context)
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.include_apply_patch_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
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
    let mut parsed_tool_response = serde_json::from_str::<Value>(tool_response)?;
    if let Some(metadata) = parsed_tool_response
        .get_mut("metadata")
        .and_then(Value::as_object_mut)
    {
        let _ = metadata.remove("duration_seconds");
    }
    assert_eq!(
        parsed_tool_response,
        serde_json::json!({
            "output": "Success. Updated the following files:\nA post_tool_use_apply_patch.txt\n",
            "metadata": {
                "exit_code": 0,
            },
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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
                ev_apply_patch_function_call(call_id, &patch),
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
            if let Err(error) =
                write_post_tool_use_hook(home, Some("^Edit$"), "context", post_context)
            {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config.include_apply_patch_tool = true;
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_does_not_fire_for_plan_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-update-plan";
    let args = serde_json::json!({
        "plan": [{
            "step": "watch the tide",
            "status": "pending",
        }]
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                core_test_support::responses::ev_function_call(
                    call_id,
                    "update_plan",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "plan updated"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            if let Err(error) = write_post_tool_use_hook(
                home,
                /*matcher*/ None,
                "decision_block",
                "should not fire",
            ) {
                panic!("failed to write post tool use hook test fixture: {error}");
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("update the plan").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("update plan output string");
    assert!(
        !output.contains("should not fire"),
        "non-shell tool output should not be affected by PostToolUse",
    );

    let hook_log_path = test.codex_home_path().join("post_tool_use_hook_log.jsonl");
    assert!(
        !hook_log_path.exists(),
        "plan tool should not trigger post tool use hooks",
    );

    Ok(())
}
