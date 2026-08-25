use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::ThreadConfigSnapshot;
use codex_core::TurnInputRequest;
use codex_core::config::AgentRoleConfig;
use codex_core::config::CurrentTimeReminderConfig;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_models_manager::bundled_models_response;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::items::SubAgentActivityItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::MultiAgentMessages;
use codex_protocol::openai_models::MultiAgentRoleMessages;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentActivityKind;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::assert_parent_turn;
use core_test_support::responses::assert_root_turn;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::strip_metadata_from_json;
use core_test_support::responses::strip_response_item_ids_from_json;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use test_case::test_case;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tracing::Level;
use tracing_test::internal::MockWriter;
use wiremock::MockServer;

const SPAWN_CALL_ID: &str = "spawn-call-1";
const MULTI_AGENT_V1_NAMESPACE: &str = "multi_agent_v1";
const MULTI_AGENT_V2_NAMESPACE: &str = "collaboration";
const TURN_0_FORK_PROMPT: &str = "seed fork context";
const TURN_1_PROMPT: &str = "spawn a child and continue";
const TURN_2_NO_WAIT_PROMPT: &str = "follow up without wait";
const CHILD_PROMPT: &str = "child: do work";
const INHERITED_MODEL: &str = "gpt-5.2";
const INHERITED_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::XHigh;
const REQUESTED_MODEL: &str = "gpt-5.4";
const REQUESTED_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Low;
const V2_DEFAULT_MODEL: &str = "gpt-5.6-terra";
const V2_DEFAULT_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::High;
const V2_REQUESTED_MODEL: &str = "gpt-5.6-sol";
const V2_REQUESTED_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Low;
const ROLE_MODEL: &str = "gpt-5.4";
const ROLE_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::High;
const SUBAGENT_START_CONTEXT: &str = "subagent start context reaches child";
const SUBAGENT_STOP_CONTINUATION: &str = "continue only the child";
const INTERNAL_SUBAGENT_PROMPT: &str = "internal subagent: review";
const FULL_HISTORY_MULTI_AGENT_MODE_HINT: &str = "Delegate independent work to another agent.";
const FULL_HISTORY_SUBAGENT_DEVELOPER_INSTRUCTIONS: &str =
    "Child-only developer instructions preserve their classification.";
const FULL_HISTORY_SHARED_USAGE_HINT: &str = "Shared delegation guidance.";
const FULL_HISTORY_PROACTIVE_PROMPT: &str = "switch to proactive delegation";
const FULL_HISTORY_EXPLICIT_PROMPT: &str = "restore explicit-only delegation";
const FULL_HISTORY_PROACTIVE_POLICY: &str = "Proactive multi-agent delegation is active.";
const FULL_HISTORY_EXPLICIT_POLICY: &str = "Do not spawn sub-agents unless the user or applicable AGENTS.md/skill instructions explicitly ask";

fn body_contains(req: &wiremock::Request, text: &str) -> bool {
    decoded_body(req)
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn request_has_input_type(req: &wiremock::Request, ty: &str) -> bool {
    decoded_body(req)
        .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
        .and_then(|body| body.get("input").and_then(Value::as_array).cloned())
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("type").and_then(Value::as_str) == Some(ty))
        })
}

fn decoded_body(req: &wiremock::Request) -> Option<Vec<u8>> {
    let is_zstd = req
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&req.body)).ok()
    } else {
        Some(req.body.clone())
    }
}

fn log_field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    line.split_ascii_whitespace()
        .find_map(|field| field.strip_prefix(&prefix))
        .map(|value| value.trim_matches('"'))
}

fn tool_parameter_description(tool: &Value, parameter_name: &str) -> Option<String> {
    tool.get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get(parameter_name))
        .and_then(|parameter| parameter.get("description"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn role_block(description: &str, role_name: &str) -> Option<String> {
    let role_header = format!("{role_name}: {{");
    let mut lines = description.lines().skip_while(|line| *line != role_header);
    let first_line = lines.next()?;
    let mut block = vec![first_line];
    for line in lines {
        if line.ends_with(": {") {
            break;
        }
        block.push(line);
    }
    Some(block.join("\n"))
}

fn write_home_skill(codex_home: &Path, dir: &str, name: &str, description: &str) -> Result<()> {
    let skill_dir = codex_home.join("skills").join(dir);
    fs::create_dir_all(&skill_dir)?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), contents)?;
    Ok(())
}

fn write_subagent_lifecycle_hooks(
    home: &Path,
    stop_prompts: &[&str],
    subagent_stop_matcher: &str,
) -> Result<()> {
    let session_start_script_path = home.join("session_start_hook.py");
    let session_start_log_path = home.join("session_start_hook_log.jsonl");
    let session_start_script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{session_start_log_path}")
payload = json.load(sys.stdin)
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
"#,
        session_start_log_path = session_start_log_path.display(),
    );

    let start_script_path = home.join("subagent_start_hook.py");
    let start_log_path = home.join("subagent_start_hook_log.jsonl");
    let start_script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{start_log_path}")
payload = json.load(sys.stdin)
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
print(json.dumps({{"hookSpecificOutput": {{"hookEventName": "SubagentStart", "additionalContext": {SUBAGENT_START_CONTEXT:?}}}}}))
"#,
        start_log_path = start_log_path.display(),
    );

    let user_prompt_submit_script_path = home.join("user_prompt_submit_hook.py");
    let user_prompt_submit_log_path = home.join("user_prompt_submit_hook_log.jsonl");
    let user_prompt_submit_script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{user_prompt_submit_log_path}")
payload = json.load(sys.stdin)
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
"#,
        user_prompt_submit_log_path = user_prompt_submit_log_path.display(),
    );

    let subagent_stop_script_path = home.join("subagent_stop_hook.py");
    let subagent_stop_log_path = home.join("subagent_stop_hook_log.jsonl");
    let prompts_json = serde_json::to_string(stop_prompts)?;
    let subagent_stop_script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{subagent_stop_log_path}")
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
    print(json.dumps({{"systemMessage": f"subagent stop pass {{invocation_index + 1}} complete"}}))
"#,
        subagent_stop_log_path = subagent_stop_log_path.display(),
        prompts_json = prompts_json,
    );

    let stop_script_path = home.join("stop_hook.py");
    let stop_log_path = home.join("stop_hook_log.jsonl");
    let stop_script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{stop_log_path}")
payload = json.load(sys.stdin)
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
print(json.dumps({{"systemMessage": "root stop complete"}}))
"#,
        stop_log_path = stop_log_path.display(),
    );

    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "matcher": "startup",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", session_start_script_path.display()),
                }]
            }],
            "SubagentStart": [{
                "matcher": "worker",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", start_script_path.display()),
                }]
            }],
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", user_prompt_submit_script_path.display()),
                }]
            }],
            "SubagentStop": [{
                "matcher": subagent_stop_matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", subagent_stop_script_path.display()),
                }]
            }],
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", stop_script_path.display()),
                }]
            }]
        }
    });

    fs::write(&session_start_script_path, session_start_script)?;
    fs::write(&start_script_path, start_script)?;
    fs::write(&user_prompt_submit_script_path, user_prompt_submit_script)?;
    fs::write(&subagent_stop_script_path, subagent_stop_script)?;
    fs::write(&stop_script_path, stop_script)?;
    fs::write(home.join("hooks.json"), hooks.to_string())?;
    Ok(())
}

fn read_hook_log(home: &Path, filename: &str) -> Result<Vec<serde_json::Value>> {
    let path = home.join(filename);
    if !path.exists() {
        return Ok(Vec::new());
    }
    fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

async fn wait_for_hook_log(
    home: &Path,
    filename: &str,
    expected_len: usize,
) -> Result<Vec<serde_json::Value>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let inputs = read_hook_log(home, filename)?;
        if inputs.len() >= expected_len {
            return Ok(inputs);
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "expected at least {expected_len} entries in {filename}, got {}",
                inputs.len()
            );
        }
        sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_spawned_thread_id(test: &TestCodex) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let ids = test.thread_manager.list_thread_ids().await;
        if let Some(spawned_id) = ids
            .iter()
            .find(|id| **id != test.session_configured.thread_id)
        {
            return Ok(spawned_id.to_string());
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for spawned thread id");
        }
        sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_requests(
    mock: &core_test_support::responses::ResponseMock,
) -> Result<Vec<ResponsesRequest>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let requests = mock.requests();
        if !requests.is_empty() {
            return Ok(requests);
        }
        if Instant::now() >= deadline {
            anyhow::bail!("expected at least 1 request, got {}", requests.len());
        }
        sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_request_with_model(
    mock: &core_test_support::responses::ResponseMock,
    model: &str,
) -> Result<ResponsesRequest> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(request) = mock
            .requests()
            .into_iter()
            .find(|request| request.body_json()["model"] == model)
        {
            return Ok(request);
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for request using model {model}");
        }
        sleep(Duration::from_millis(10)).await;
    }
}

async fn setup_turn_one_with_spawned_child(
    server: &MockServer,
    child_response_delay: Option<Duration>,
) -> Result<(TestCodex, String)> {
    let (test, spawned_id, _child_request_log) = setup_turn_one_with_custom_spawned_child(
        server,
        json!({
            "message": CHILD_PROMPT,
        }),
        child_response_delay,
        /*wait_for_parent_notification*/ true,
        |builder| builder,
    )
    .await?;
    Ok((test, spawned_id))
}

async fn setup_turn_one_with_custom_spawned_child(
    server: &MockServer,
    spawn_args: serde_json::Value,
    child_response_delay: Option<Duration>,
    wait_for_parent_notification: bool,
    configure_test: impl FnOnce(
        core_test_support::test_codex::TestCodexBuilder,
    ) -> core_test_support::test_codex::TestCodexBuilder,
) -> Result<(
    TestCodex,
    String,
    core_test_support::responses::ResponseMock,
)> {
    let spawn_args = serde_json::to_string(&spawn_args)?;

    mount_sse_once_match(
        server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let child_sse = sse(vec![
        ev_response_created("resp-child-1"),
        ev_assistant_message("msg-child-1", "child done"),
        ev_completed("resp-child-1"),
    ]);
    let child_request_log = if let Some(delay) = child_response_delay {
        mount_response_once_match(
            server,
            |req: &wiremock::Request| {
                body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
            },
            sse_response(child_sse).set_delay(delay),
        )
        .await
    } else {
        mount_sse_once_match(
            server,
            |req: &wiremock::Request| {
                body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
            },
            child_sse,
        )
        .await
    };

    let _turn1_followup = mount_sse_once_match(
        server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let mut builder = configure_test(test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config.model = Some(INHERITED_MODEL.to_string());
        config.model_reasoning_effort = Some(INHERITED_REASONING_EFFORT);
    }));
    let test = builder.build_with_auto_env(server).await?;
    test.submit_turn(TURN_1_PROMPT).await?;
    if child_response_delay.is_none() && wait_for_parent_notification {
        let _ = wait_for_requests(&child_request_log).await?;
        let rollout_path = test
            .codex
            .rollout_path()
            .ok_or_else(|| anyhow::anyhow!("expected parent rollout path"))?;
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            let has_notification = tokio::fs::read_to_string(&rollout_path)
                .await
                .is_ok_and(|rollout| rollout.contains("<subagent_notification>"));
            if has_notification {
                break;
            }
            if Instant::now() >= deadline {
                anyhow::bail!(
                    "timed out waiting for parent rollout to include subagent notification"
                );
            }
            sleep(Duration::from_millis(10)).await;
        }
    }
    let spawned_id = wait_for_spawned_thread_id(&test).await?;

    Ok((test, spawned_id, child_request_log))
}

async fn spawn_child_and_capture_snapshot(
    server: &MockServer,
    spawn_args: serde_json::Value,
    configure_test: impl FnOnce(
        core_test_support::test_codex::TestCodexBuilder,
    ) -> core_test_support::test_codex::TestCodexBuilder,
) -> Result<ThreadConfigSnapshot> {
    let (test, spawned_id, _child_request_log) = setup_turn_one_with_custom_spawned_child(
        server,
        spawn_args,
        /*child_response_delay*/ None,
        /*wait_for_parent_notification*/ false,
        configure_test,
    )
    .await?;
    let thread_id = ThreadId::from_string(&spawned_id)?;
    Ok(test
        .thread_manager
        .get_thread(thread_id)
        .await?
        .config_snapshot()
        .await)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_start_replaces_session_start_and_injects_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "task_name": "child",
        "agent_type": "worker",
    }))?;

    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, CHILD_PROMPT)
                && body_contains(req, SUBAGENT_START_CONTEXT)
                && !body_contains(req, "<subagent_notification>")
                && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;

    let _turn1_followup = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_subagent_lifecycle_hooks(home, /*stop_prompts*/ &[], "worker")
                .expect("failed to write subagent hook fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;

    test.submit_turn(TURN_1_PROMPT).await?;
    let _ = wait_for_requests(&child_request_log).await?;

    let start_inputs = wait_for_hook_log(
        test.codex_home_path(),
        "subagent_start_hook_log.jsonl",
        /*expected_len*/ 1,
    )
    .await?;
    assert_eq!(start_inputs.len(), 1);
    assert_eq!(start_inputs[0]["agent_type"].as_str(), Some("worker"));
    let spawned_id = wait_for_spawned_thread_id(&test).await?;
    assert_eq!(
        start_inputs[0]["agent_id"].as_str(),
        Some(spawned_id.as_str())
    );

    let user_prompt_submit_inputs = wait_for_hook_log(
        test.codex_home_path(),
        "user_prompt_submit_hook_log.jsonl",
        /*expected_len*/ 2,
    )
    .await?;
    let parent_prompt_input = user_prompt_submit_inputs
        .iter()
        .find(|input| input["prompt"].as_str() == Some(TURN_1_PROMPT))
        .expect("parent prompt submit hook input should be logged");
    assert_eq!(parent_prompt_input.get("agent_id"), None);
    assert_eq!(parent_prompt_input.get("agent_type"), None);

    let child_prompt_input = user_prompt_submit_inputs
        .iter()
        .find(|input| input["prompt"].as_str() == Some(CHILD_PROMPT))
        .expect("child prompt submit hook input should be logged");
    assert_eq!(
        child_prompt_input["agent_id"].as_str(),
        Some(spawned_id.as_str())
    );
    assert_eq!(child_prompt_input["agent_type"].as_str(), Some("worker"));

    let session_start_inputs = wait_for_hook_log(
        test.codex_home_path(),
        "session_start_hook_log.jsonl",
        /*expected_len*/ 1,
    )
    .await?;
    assert_eq!(session_start_inputs.len(), 1);
    assert_eq!(session_start_inputs[0]["source"].as_str(), Some("startup"));
    assert_ne!(
        session_start_inputs[0]["session_id"].as_str(),
        Some(spawned_id.as_str())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_stop_replaces_stop_and_skips_internal_subagents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "task_name": "child",
        "agent_type": "worker",
    }))?;

    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let first_child_request = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done first"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    let second_child_request = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, SUBAGENT_STOP_CONTINUATION) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-2"),
            ev_assistant_message("msg-child-2", "child done final"),
            ev_completed("resp-child-2"),
        ]),
    )
    .await;

    let _turn1_followup = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;
    let internal_request = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, INTERNAL_SUBAGENT_PROMPT),
        sse(vec![
            ev_response_created("resp-internal-1"),
            ev_assistant_message("msg-internal-1", "internal subagent done"),
            ev_completed("resp-internal-1"),
        ]),
    )
    .await;

    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_subagent_lifecycle_hooks(
                home,
                /*stop_prompts*/ &[SUBAGENT_STOP_CONTINUATION],
                "",
            )
            .expect("failed to write subagent hook fixture");
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;

    test.submit_turn(TURN_1_PROMPT).await?;
    let _ = wait_for_requests(&first_child_request).await?;
    let _ = wait_for_requests(&second_child_request).await?;

    let subagent_stop_inputs = wait_for_hook_log(
        test.codex_home_path(),
        "subagent_stop_hook_log.jsonl",
        /*expected_len*/ 2,
    )
    .await?;
    assert_eq!(subagent_stop_inputs.len(), 2);
    assert_eq!(
        subagent_stop_inputs
            .iter()
            .map(|input| input["stop_hook_active"].as_bool())
            .collect::<Vec<_>>(),
        vec![Some(false), Some(true)]
    );
    assert_eq!(
        subagent_stop_inputs[0]["agent_type"].as_str(),
        Some("worker")
    );
    let parent_transcript_path = subagent_stop_inputs[0]["transcript_path"]
        .as_str()
        .expect("SubagentStop should include parent transcript_path");
    let agent_transcript_path = subagent_stop_inputs[0]["agent_transcript_path"]
        .as_str()
        .expect("SubagentStop should include agent_transcript_path");
    assert_ne!(parent_transcript_path, agent_transcript_path);
    assert_eq!(
        subagent_stop_inputs[1]["transcript_path"].as_str(),
        Some(parent_transcript_path)
    );
    assert_eq!(
        subagent_stop_inputs[1]["agent_transcript_path"].as_str(),
        Some(agent_transcript_path)
    );
    assert_eq!(
        subagent_stop_inputs[0]["last_assistant_message"].as_str(),
        Some("child done first")
    );

    let stop_inputs = read_hook_log(test.codex_home_path(), "stop_hook_log.jsonl")?;
    assert!(
        stop_inputs
            .iter()
            .all(|input| input["last_assistant_message"].as_str() != Some("child done first")),
        "child completion should not invoke the normal Stop hook"
    );
    let stop_input_count = stop_inputs.len();

    // This matcher would catch the old synthetic "review" SubagentStop target
    // because the SubagentStop hook above intentionally matches all agent types.
    let internal_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::Review)),
            environments: Some(Vec::new()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.cwd_path());
    internal_thread
        .thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: INTERNAL_SUBAGENT_PROMPT.to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                model: Some(internal_thread.session_configured.model.clone()),
                ..Default::default()
            }),
        )
        .await?;
    let turn_id = wait_for_event_match(internal_thread.thread.as_ref(), |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    wait_for_event_match(internal_thread.thread.as_ref(), |event| match event {
        EventMsg::TurnComplete(event) if event.turn_id == turn_id => Some(()),
        _ => None,
    })
    .await;
    let requests = wait_for_requests(&internal_request).await?;
    assert_eq!(requests.len(), 1);

    let subagent_stop_inputs_after_internal =
        read_hook_log(test.codex_home_path(), "subagent_stop_hook_log.jsonl")?;
    assert_eq!(subagent_stop_inputs_after_internal, subagent_stop_inputs);

    let stop_inputs_after_internal = read_hook_log(test.codex_home_path(), "stop_hook_log.jsonl")?;
    assert_eq!(stop_inputs_after_internal.len(), stop_input_count);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_notification_is_included_without_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let (test, _spawned_id) =
        setup_turn_one_with_spawned_child(&server, /*child_response_delay*/ None).await?;

    let turn2 = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_2_NO_WAIT_PROMPT),
        sse(vec![
            ev_response_created("resp-turn2-1"),
            ev_assistant_message("msg-turn2-1", "no wait path"),
            ev_completed("resp-turn2-1"),
        ]),
    )
    .await;
    test.submit_turn(TURN_2_NO_WAIT_PROMPT).await?;

    let turn2_requests = wait_for_requests(&turn2).await?;
    assert!(
        turn2_requests
            .iter()
            .any(|request| request.has_content_kinds(&["multi_agent.subagent_notification"]))
    );

    Ok(())
}

#[test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_child_receives_forked_parent_context(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let seed_turn = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_0_FORK_PROMPT),
        sse(vec![
            ev_response_created("resp-seed-1"),
            ev_assistant_message("msg-seed-1", "seeded"),
            ev_completed("resp-seed-1"),
        ]),
    )
    .await;

    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "fork_context": true,
    }))?;
    let spawn_turn = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;

    let _turn1_followup = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_history_mode(history_mode)
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config.model = Some(INHERITED_MODEL.to_string());
            config.model_reasoning_effort = Some(INHERITED_REASONING_EFFORT);
            config.agent_default_subagent_model = Some(REQUESTED_MODEL.to_string());
            config.agent_default_subagent_reasoning_effort = Some(REQUESTED_REASONING_EFFORT);
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn(TURN_0_FORK_PROMPT).await?;
    let _ = seed_turn.single_request();

    test.submit_turn(TURN_1_PROMPT).await?;
    let parent_body = spawn_turn.single_request().body_json();

    let child_request = wait_for_request_with_model(&child_request_log, REQUESTED_MODEL).await?;
    assert!(child_request.body_contains_text(TURN_0_FORK_PROMPT));
    let child_body = child_request.body_json();
    let child_metadata: serde_json::Value = serde_json::from_str(
        child_body["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("child turn metadata"),
    )?;
    assert_eq!(child_metadata["thread_source"], "subagent");
    let original_parent_turn_id = parent_body["client_metadata"]["turn_id"]
        .as_str()
        .expect("legacy spawn parent turn id");
    assert_parent_turn(&parent_body, /*expected*/ None)?;
    assert_parent_turn(&child_body, Some(original_parent_turn_id))?;
    assert_root_turn(&parent_body, Some(original_parent_turn_id))?;
    assert_root_turn(&child_body, Some(original_parent_turn_id))?;
    assert_eq!(
        (
            child_body["model"].clone(),
            child_body["reasoning"]["effort"].clone(),
        ),
        (
            json!(REQUESTED_MODEL),
            json!(REQUESTED_REASONING_EFFORT.to_string()),
        )
    );
    let child_thread_id = ThreadId::from_string(
        child_body["client_metadata"]["thread_id"]
            .as_str()
            .expect("legacy child thread id"),
    )?;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while !matches!(child_thread.agent_status().await, AgentStatus::Completed(_)) {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let args = serde_json::to_string(&json!({
        "target": child_thread_id.to_string(),
        "message": "legacy child follow-up",
    }))?;
    let parent = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "reuse the legacy child"),
        sse(vec![
            ev_response_created("resp-legacy-reuse"),
            ev_function_call_with_namespace(
                "legacy-reuse-call",
                MULTI_AGENT_V1_NAMESPACE,
                "send_input",
                &args,
            ),
            ev_completed("resp-legacy-reuse"),
        ]),
    )
    .await;
    let followup = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("resp-legacy-child-reuse")]),
            sse(vec![ev_completed("resp-legacy-reuse-complete")]),
        ],
    )
    .await;

    test.submit_turn("reuse the legacy child").await?;
    let followup_parent_body = parent.single_request().body_json();
    let reused_child_body = wait_for_request_with_model(&followup, REQUESTED_MODEL)
        .await?
        .body_json();
    let followup_parent_turn_id = followup_parent_body["client_metadata"]["turn_id"]
        .as_str()
        .expect("legacy follow-up parent turn id");
    assert_ne!(followup_parent_turn_id, original_parent_turn_id);
    let metadata = &reused_child_body["client_metadata"];
    assert_eq!(metadata["thread_id"], json!(child_thread_id));
    assert_parent_turn(&followup_parent_body, /*expected*/ None)?;
    assert_parent_turn(&reused_child_body, Some(followup_parent_turn_id))?;
    assert_root_turn(&followup_parent_body, Some(followup_parent_turn_id))?;
    assert_root_turn(&reused_child_body, Some(followup_parent_turn_id))?;
    Ok(())
}

#[derive(Clone, Copy)]
enum FullHistoryV2ModelSelection {
    ConfiguredDefault,
    ExplicitOverride,
    WorldStateIdentity,
    CurrentTimeReminders,
    MultiAgentModeInstructions,
    MultiAgentModeTransitions,
}

#[test_case(FullHistoryV2ModelSelection::ConfiguredDefault; "configured default with omitted fork_turns")]
#[test_case(FullHistoryV2ModelSelection::ExplicitOverride; "explicit override with fork_turns all")]
#[test_case(FullHistoryV2ModelSelection::WorldStateIdentity; "world state appends context window when agent identity changes")]
#[test_case(FullHistoryV2ModelSelection::CurrentTimeReminders; "full fork drops inherited current-time reminders")]
#[test_case(FullHistoryV2ModelSelection::MultiAgentModeInstructions; "full fork drops inherited multi-agent mode instructions")]
#[test_case(FullHistoryV2ModelSelection::MultiAgentModeTransitions; "full fork restores explicit policy after proactive transition")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_full_history_v2_child_uses_model_precedence_without_dropping_context(
    selection: FullHistoryV2ModelSelection,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let seed_turn = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_0_FORK_PROMPT),
        sse(vec![
            ev_response_created("resp-seed-1"),
            ev_assistant_message("msg-seed-1", "seeded"),
            ev_completed("resp-seed-1"),
        ]),
    )
    .await;
    let (spawn_args, expected_model, expected_reasoning_effort) = match selection {
        FullHistoryV2ModelSelection::ConfiguredDefault
        | FullHistoryV2ModelSelection::WorldStateIdentity
        | FullHistoryV2ModelSelection::CurrentTimeReminders
        | FullHistoryV2ModelSelection::MultiAgentModeInstructions
        | FullHistoryV2ModelSelection::MultiAgentModeTransitions => (
            json!({
                "message": CHILD_PROMPT,
                "task_name": "worker",
            }),
            V2_DEFAULT_MODEL,
            V2_DEFAULT_REASONING_EFFORT,
        ),
        FullHistoryV2ModelSelection::ExplicitOverride => (
            json!({
                "message": CHILD_PROMPT,
                "task_name": "worker",
                "fork_turns": "all",
                "model": V2_REQUESTED_MODEL,
                "reasoning_effort": V2_REQUESTED_REASONING_EFFORT,
            }),
            V2_REQUESTED_MODEL,
            V2_REQUESTED_REASONING_EFFORT,
        ),
    };
    let spawn_args = serde_json::to_string(&spawn_args)?;
    let mode_transition_turns = if matches!(
        selection,
        FullHistoryV2ModelSelection::MultiAgentModeTransitions
    ) {
        Some((
            mount_sse_once_match(
                &server,
                |req: &wiremock::Request| body_contains(req, FULL_HISTORY_PROACTIVE_PROMPT),
                sse(vec![
                    ev_response_created("resp-proactive-1"),
                    ev_assistant_message("msg-proactive-1", "proactive done"),
                    ev_completed("resp-proactive-1"),
                ]),
            )
            .await,
            mount_sse_once_match(
                &server,
                |req: &wiremock::Request| body_contains(req, FULL_HISTORY_EXPLICIT_PROMPT),
                sse(vec![
                    ev_response_created("resp-explicit-1"),
                    ev_assistant_message("msg-explicit-1", "explicit done"),
                    ev_completed("resp-explicit-1"),
                ]),
            )
            .await,
        ))
    } else {
        None
    };
    let spawn_turn = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;
    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    let _turn1_followup = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;
    let mut builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        let model_catalog = config.model_catalog.get_or_insert_with(|| {
            bundled_models_response().expect("bundled models.json should parse")
        });
        for model in [INHERITED_MODEL, V2_DEFAULT_MODEL, V2_REQUESTED_MODEL] {
            let model_info = model_catalog
                .models
                .iter_mut()
                .find(|model_info| model_info.slug == model)
                .unwrap_or_else(|| panic!("{model} should exist in bundled models.json"));
            let multi_agent = model_info
                .model_messages
                .as_mut()
                .expect("bundled model should include model messages")
                .multi_agent
                .get_or_insert_with(MultiAgentMessages::default);
            multi_agent.role = Some(MultiAgentRoleMessages {
                root: Some(format!("{model} root role.")),
                subagent: Some(format!("{model} subagent role.")),
            });
            if matches!(
                selection,
                FullHistoryV2ModelSelection::MultiAgentModeTransitions
            ) && model == INHERITED_MODEL
            {
                model_info
                    .supported_reasoning_levels
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffort::Ultra,
                        description: "Ultra".to_string(),
                    });
            }
        }
        if matches!(selection, FullHistoryV2ModelSelection::WorldStateIdentity) {
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow feature update");
            config.model_context_window = Some(128_000);
        }
        if matches!(selection, FullHistoryV2ModelSelection::ConfiguredDefault) {
            config.developer_instructions = None;
            config.multi_agent_v2.subagent_developer_instructions =
                Some(FULL_HISTORY_SUBAGENT_DEVELOPER_INSTRUCTIONS.to_string());
        }
        if matches!(selection, FullHistoryV2ModelSelection::CurrentTimeReminders) {
            config
                .features
                .enable(Feature::CurrentTimeReminder)
                .expect("test config should allow feature update");
            config.current_time_reminder = Some(CurrentTimeReminderConfig {
                reminder_interval_seconds: 0,
                ..CurrentTimeReminderConfig::default()
            });
        }
        if matches!(
            selection,
            FullHistoryV2ModelSelection::MultiAgentModeInstructions
        ) {
            config.multi_agent_v2.multi_agent_mode_hint_text =
                Some(FULL_HISTORY_MULTI_AGENT_MODE_HINT.to_string());
        }
        if matches!(
            selection,
            FullHistoryV2ModelSelection::MultiAgentModeTransitions
        ) {
            config.multi_agent_v2.root_agent_usage_hint_text =
                Some(FULL_HISTORY_SHARED_USAGE_HINT.to_string());
            config.multi_agent_v2.subagent_usage_hint_text =
                Some(FULL_HISTORY_SHARED_USAGE_HINT.to_string());
        }
        config.model = Some(INHERITED_MODEL.to_string());
        config.model_reasoning_effort = Some(INHERITED_REASONING_EFFORT);
        config.agent_default_subagent_model = Some(V2_DEFAULT_MODEL.to_string());
        config.agent_default_subagent_reasoning_effort = Some(V2_DEFAULT_REASONING_EFFORT);
    });
    if matches!(selection, FullHistoryV2ModelSelection::WorldStateIdentity) {
        builder = builder.with_history_mode(ThreadHistoryMode::Paginated);
    }
    let test = builder.build(&server).await?;
    if matches!(selection, FullHistoryV2ModelSelection::WorldStateIdentity) {
        test.codex.submit(Op::Compact).await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    test.submit_turn(TURN_0_FORK_PROMPT).await?;
    let _ = seed_turn.single_request();
    if let Some((proactive_turn, explicit_turn)) = mode_transition_turns {
        for (prompt, effort, approval_policy) in [
            (
                FULL_HISTORY_PROACTIVE_PROMPT,
                ReasoningEffort::Ultra,
                Some(AskForApproval::OnRequest),
            ),
            (FULL_HISTORY_EXPLICIT_PROMPT, ReasoningEffort::High, None),
        ] {
            test.codex
                .start_or_steer_turn(
                    TurnInputRequest::user_input(vec![UserInput::Text {
                        text: prompt.to_string(),
                        text_elements: Vec::new(),
                    }])
                    .with_thread_settings(ThreadSettingsOverrides {
                        effort: Some(Some(effort)),
                        approval_policy,
                        ..Default::default()
                    }),
                )
                .await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
        }
        let proactive_request = proactive_turn.single_request();
        let proactive_developer_messages = proactive_request.message_input_text_groups("developer");
        assert!(
            proactive_developer_messages.iter().any(|message| {
                message.len() > 1
                    && message
                        .iter()
                        .any(|text| text.contains(FULL_HISTORY_PROACTIVE_POLICY))
            }),
            "proactive policy should share a developer message with unrelated context: {proactive_developer_messages:?}"
        );
        let explicit_request = explicit_turn.single_request();
        assert!(
            explicit_request
                .message_input_texts("developer")
                .iter()
                .any(|text| text.contains(FULL_HISTORY_EXPLICIT_POLICY)),
            "restored parent policy should require an explicit delegation request"
        );
    }
    test.submit_turn(TURN_1_PROMPT).await?;
    let parent_request = spawn_turn.single_request();

    let child_request = wait_for_request_with_model(&child_request_log, expected_model).await?;
    assert!(child_request.body_contains_text(TURN_0_FORK_PROMPT));
    let misaligned_child_messages = child_request
        .inputs_of_type("message")
        .into_iter()
        .filter(|message| {
            message["internal_chat_message_metadata_passthrough"]["content_item_kinds"]
                .as_array()
                .is_some_and(|kinds| {
                    message["content"]
                        .as_array()
                        .is_none_or(|content| content.len() != kinds.len())
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(misaligned_child_messages, Vec::<Value>::new());
    let child_developer_messages = child_request.message_input_texts("developer");
    if matches!(selection, FullHistoryV2ModelSelection::ConfiguredDefault) {
        assert_eq!(
            (
                parent_request.body_contains_text(FULL_HISTORY_SUBAGENT_DEVELOPER_INSTRUCTIONS),
                child_request.has_content_kinds(&["generic.developer_instructions"]),
                child_developer_messages
                    .iter()
                    .filter(|text| text.as_str() == FULL_HISTORY_SUBAGENT_DEVELOPER_INSTRUCTIONS)
                    .count(),
            ),
            (false, true, 1)
        );
    }
    if !matches!(
        selection,
        FullHistoryV2ModelSelection::MultiAgentModeTransitions
    ) {
        assert!(child_request.has_content_kinds(&["multi_agent.role_instructions"]));
        assert_eq!(
            child_developer_messages
                .iter()
                .filter(|message| message.contains(&format!("{expected_model} subagent role.")))
                .count(),
            1
        );
    }
    assert!(!child_developer_messages.iter().any(|message| {
        message.contains(&format!("{INHERITED_MODEL} root role."))
            || message.contains(&format!("{INHERITED_MODEL} subagent role."))
    }));
    if matches!(
        selection,
        FullHistoryV2ModelSelection::MultiAgentModeInstructions
    ) {
        let mode_instruction_count = |request: &ResponsesRequest| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| text.starts_with(MULTI_AGENT_MODE_OPEN_TAG))
                .count()
        };
        assert_eq!(
            (
                mode_instruction_count(&parent_request),
                mode_instruction_count(&child_request),
            ),
            (1, 1)
        );
    }
    if matches!(
        selection,
        FullHistoryV2ModelSelection::MultiAgentModeTransitions
    ) {
        assert_eq!(
            (
                child_developer_messages
                    .iter()
                    .filter(|message| message.starts_with(MULTI_AGENT_MODE_OPEN_TAG))
                    .count(),
                child_developer_messages
                    .iter()
                    .filter(|message| message.contains(FULL_HISTORY_EXPLICIT_POLICY))
                    .count(),
                child_developer_messages
                    .iter()
                    .filter(|message| message.contains(FULL_HISTORY_PROACTIVE_POLICY))
                    .count(),
                child_developer_messages
                    .iter()
                    .filter(|message| message.contains(FULL_HISTORY_SHARED_USAGE_HINT))
                    .count(),
            ),
            (1, 1, 0, 1)
        );
    }
    if matches!(selection, FullHistoryV2ModelSelection::CurrentTimeReminders) {
        let reminder_count = |request: &ResponsesRequest| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| text.starts_with("<current_time_reminder>"))
                .count()
        };
        assert_eq!(reminder_count(&parent_request), 2);
        assert_eq!(reminder_count(&child_request), 1);
    }
    let child_body = child_request.body_json();
    if matches!(selection, FullHistoryV2ModelSelection::WorldStateIdentity) {
        let child_thread_id = ThreadId::from_string(
            child_body["client_metadata"]["thread_id"]
                .as_str()
                .expect("child thread id"),
        )?;
        let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
        child_thread.flush_rollout().await?;
        let child_rollout = codex_rollout::RolloutRecorder::get_rollout_history(
            &child_thread
                .rollout_path()
                .expect("child rollout should exist"),
        )
        .await?;
        let context_window_snapshots = child_rollout
            .get_rollout_items()
            .iter()
            .filter_map(|item| match item {
                RolloutItem::WorldState(world_state) => {
                    world_state.state.get("context_window").cloned()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            context_window_snapshots,
            vec![json!("/root"), json!("/root/worker")]
        );
        let context_windows = child_request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text.starts_with("<context_window>\n"))
            .collect::<Vec<_>>();
        let identities = context_windows
            .iter()
            .map(|text| text.lines().nth(1).expect("agent identity"))
            .collect::<Vec<_>>();
        assert_eq!(
            identities,
            ["Agent name: /root", "Agent name: /root/worker"]
        );
        let window_ids = context_windows
            .iter()
            .map(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("Current context window id: "))
                    .expect("context window id")
            })
            .collect::<Vec<_>>();
        assert_ne!(window_ids[0], window_ids[1]);
        let checkpoint = child_rollout
            .get_rollout_items()
            .iter()
            .find_map(|item| match item {
                RolloutItem::Compacted(checkpoint) => Some(checkpoint),
                _ => None,
            })
            .expect("inherited compaction checkpoint");
        assert!(
            checkpoint
                .replacement_history
                .as_ref()
                .is_some_and(|history| !history.is_empty())
        );
        assert_eq!(
            (
                checkpoint.window_number,
                checkpoint.first_window_id.as_deref(),
                checkpoint.previous_window_id.as_deref(),
                checkpoint.window_id.as_deref(),
            ),
            (Some(0), Some(window_ids[1]), None, Some(window_ids[1]))
        );
        assert!(
            child_request.has_message_with_input_texts("developer", |message| {
                matches!(
                    message,
                    [text] if text.starts_with("<context_window>\nAgent name: /root/worker\n")
                )
            })
        );
    }
    assert_eq!(
        (
            child_body["model"].clone(),
            child_body["reasoning"]["effort"].clone(),
        ),
        (
            json!(expected_model),
            json!(expected_reasoning_effort.to_string()),
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_requested_model_and_reasoning_override_inherited_settings_without_role()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let child_snapshot = spawn_child_and_capture_snapshot(
        &server,
        json!({
            "message": CHILD_PROMPT,
            "model": REQUESTED_MODEL,
            "reasoning_effort": REQUESTED_REASONING_EFFORT,
        }),
        |builder| {
            builder.with_config(|config| {
                config.agent_default_subagent_model = Some(INHERITED_MODEL.to_string());
                config.agent_default_subagent_reasoning_effort = Some(ReasoningEffort::High);
            })
        },
    )
    .await?;

    assert_eq!(child_snapshot.model, REQUESTED_MODEL);
    assert_eq!(
        child_snapshot.reasoning_effort,
        Some(REQUESTED_REASONING_EFFORT)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_uses_configured_subagent_defaults() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let child_snapshot =
        spawn_child_and_capture_snapshot(&server, json!({ "message": CHILD_PROMPT }), |builder| {
            builder.with_config(|config| {
                config.agent_default_subagent_model = Some(REQUESTED_MODEL.to_string());
                config.agent_default_subagent_reasoning_effort = Some(REQUESTED_REASONING_EFFORT);
            })
        })
        .await?;

    assert_eq!(
        (child_snapshot.model, child_snapshot.reasoning_effort),
        (
            REQUESTED_MODEL.to_string(),
            Some(REQUESTED_REASONING_EFFORT)
        )
    );
    Ok(())
}

#[test_case(
    Some(REQUESTED_MODEL),
    None,
    REQUESTED_MODEL,
    Some(ReasoningEffort::Medium);
    "model only"
)]
#[test_case(
    None,
    Some(REQUESTED_REASONING_EFFORT),
    INHERITED_MODEL,
    Some(REQUESTED_REASONING_EFFORT);
    "reasoning effort only"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_uses_independent_configured_subagent_defaults(
    default_model: Option<&str>,
    default_reasoning_effort: Option<ReasoningEffort>,
    expected_model: &str,
    expected_reasoning_effort: Option<ReasoningEffort>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let default_model = default_model.map(str::to_string);
    let child_snapshot =
        spawn_child_and_capture_snapshot(&server, json!({ "message": CHILD_PROMPT }), |builder| {
            builder.with_config(move |config| {
                config.agent_default_subagent_model = default_model;
                config.agent_default_subagent_reasoning_effort = default_reasoning_effort;
            })
        })
        .await?;

    assert_eq!(
        (child_snapshot.model, child_snapshot.reasoning_effort),
        (expected_model.to_string(), expected_reasoning_effort)
    );
    Ok(())
}

#[test_case(true, false; "unsupported child")]
#[test_case(false, true; "supported child")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_agent_uses_summary_support_for_final_model(
    parent_supports_summary: bool,
    child_supports_summary: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut model_catalog = bundled_models_response().expect("bundled models.json should parse");
    for (slug, supports_summary) in [
        (INHERITED_MODEL, parent_supports_summary),
        (REQUESTED_MODEL, child_supports_summary),
    ] {
        let model = model_catalog
            .models
            .iter_mut()
            .find(|model| model.slug == slug)
            .unwrap_or_else(|| panic!("{slug} should exist in bundled models.json"));
        model.supports_reasoning_summary_parameter = supports_summary;
    }

    let (_test, _spawned_id, child_request_log) = setup_turn_one_with_custom_spawned_child(
        &server,
        json!({
            "message": CHILD_PROMPT,
            "model": REQUESTED_MODEL,
        }),
        /*child_response_delay*/ Some(Duration::from_secs(1)),
        /*wait_for_parent_notification*/ false,
        move |builder| {
            builder.with_config(move |config| {
                config.model_catalog = Some(model_catalog);
                config.model_reasoning_summary = Some(ReasoningSummary::Detailed);
                config
                    .features
                    .enable(Feature::ConcurrentReasoningSummaries)
                    .expect("test config should allow feature update");
            })
        },
    )
    .await?;

    let deadline = Instant::now() + Duration::from_secs(2);
    let child_body = loop {
        if let Some(body) = child_request_log
            .requests()
            .iter()
            .map(ResponsesRequest::body_json)
            .find(|body| body["model"] == REQUESTED_MODEL)
        {
            break body;
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for the child request");
        }
        sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(child_body["model"], json!(REQUESTED_MODEL));
    let expected_reasoning = if child_supports_summary {
        json!({"effort": "medium", "summary": "detailed"})
    } else {
        json!({"effort": "medium"})
    };
    assert_eq!(child_body["reasoning"], expected_reasoning);
    assert_eq!(
        child_body.get("stream_options").is_some(),
        child_supports_summary
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_multi_agent_v2_child_inherits_parent_developer_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "task_name": "worker",
    }))?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;

    let _turn1_followup = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        config.developer_instructions = Some("Parent developer instructions.".to_string());
    });
    let test = builder.build(&server).await?;

    test.submit_turn(TURN_1_PROMPT).await?;

    let child_requests = wait_for_requests(&child_request_log).await?;
    let child_request = child_requests
        .last()
        .expect("child request log should capture at least one request");
    assert!(child_request.body_contains_text("Parent developer instructions."));
    assert!(child_request.body_contains_text(CHILD_PROMPT));

    Ok(())
}

#[test_case(None, false; "encrypted")]
#[test_case(None, true; "plaintext")]
#[test_case(Some("gpt-5.6-luna"), false; "luna encrypted leaf")]
#[test_case(Some("gpt-5.5"), false; "legacy encrypted leaf")]
#[tokio::test]
async fn multi_agent_v2_spawn_sends_agent_message_to_child(
    model: Option<&str>,
    plaintext: bool,
) -> Result<()> {
    let output: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .with_writer(MockWriter::new(output))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let server = start_mock_server().await;
    let message = if plaintext {
        "plaintext delegated task"
    } else {
        "opaque-encrypted-message"
    };
    let mut spawn_args = json!({
        "message": message,
        "task_name": "worker",
    });
    if let Some(model) = model {
        spawn_args["model"] = json!(model);
        if model == "gpt-5.5" {
            spawn_args["fork_turns"] = json!("none");
        }
    }
    let spawn_args = serde_json::to_string(&spawn_args)?;
    let mut spawn_event = ev_function_call_with_namespace(
        SPAWN_CALL_ID,
        MULTI_AGENT_V2_NAMESPACE,
        "spawn_agent",
        &spawn_args,
    );
    if plaintext {
        spawn_event["item"]["encrypted_function_args"] = json!([]);
    }
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-parent-1"),
            spawn_event,
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| request_has_input_type(req, "agent_message"),
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    let parent_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, SPAWN_CALL_ID) && !request_has_input_type(req, "agent_message")
        },
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;

    let parent_model = if model.is_some() {
        "gpt-5.6-sol"
    } else {
        "koffing"
    };
    let mut builder = test_codex().with_model(parent_model).with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let root_thread_id = test.session_configured.thread_id;

    test.submit_turn(TURN_1_PROMPT).await?;

    // The response mock records candidate requests before its request matcher runs, so wait for
    // the child request instead of assuming the latest recorded request is already it.
    let deadline = Instant::now() + Duration::from_secs(2);
    let child_request = loop {
        if let Some(request) = child_request_log
            .requests()
            .into_iter()
            .find(|request| !request.inputs_of_type("agent_message").is_empty())
        {
            break request;
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for child agent message request");
        }
        sleep(Duration::from_millis(10)).await;
    };
    let content = if plaintext {
        vec![json!({
            "type": "input_text",
            "text": format!(
                "Message Type: NEW_TASK\nTask name: /root/worker\nSender: /root\nPayload:\n{message}"
            ),
        })]
    } else {
        vec![
            json!({
                "type": "input_text",
                "text": "Message Type: NEW_TASK\nTask name: /root/worker\nSender: /root\nPayload:\n",
            }),
            json!({
                "type": "encrypted_content",
                "encrypted_content": message,
            }),
        ]
    };
    assert_eq!(
        strip_response_item_ids_from_json(strip_metadata_from_json(Value::Array(
            child_request.inputs_of_type("agent_message"),
        ))),
        Value::Array(vec![json!({
            "type": "agent_message",
            "author": "/root",
            "recipient": "/root/worker",
            "content": content,
        })])
    );
    if let Some(model) = model {
        assert_eq!(child_request.body_json()["model"], json!(model));
        assert!(
            !child_request
                .body_json()
                .to_string()
                .contains("\"name\":\"collaboration\""),
            "leaf workers must not receive collaboration tools",
        );
    }
    if plaintext {
        assert!(
            parent_request_log.requests().into_iter().any(|request| {
                request.input().iter().any(|item| {
                    item["call_id"].as_str() == Some(SPAWN_CALL_ID)
                        && item["encrypted_function_args"] == json!([])
                })
            }),
            "plaintext function-call metadata should survive replay"
        );
    }

    let child_thread_id = test
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|thread_id| *thread_id != root_thread_id)
        .expect("child thread ID");
    let logs = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let logs = String::from_utf8(output.lock().expect("buffer lock").clone())
                .expect("logs should be UTF-8");
            if logs.contains("kind=\"spawn\"") && logs.contains("state=\"receive\"") {
                break logs;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("spawn communication logs should be emitted");
    let send = logs
        .lines()
        .find(|line| line.contains("kind=\"spawn\"") && line.contains("state=\"send\""))
        .expect("spawn send event");
    assert!(send.contains(&format!("sender_thread_id={root_thread_id}")));
    assert!(send.contains(&format!("receiver_thread_id={child_thread_id}")));
    let logged_message = if plaintext { "[plaintext]" } else { message };
    assert!(send.contains(&format!("content=\"{logged_message}\"")));

    let communication_id = log_field(send, "communication_id").expect("communication ID");
    logs.lines()
        .find(|line| {
            line.contains("state=\"receive\"")
                && log_field(line, "communication_id") == Some(communication_id)
        })
        .expect("correlated receive event");

    Ok(())
}

#[derive(Clone, Copy)]
enum CompletionScenario {
    Completed,
    TerminalError,
}

#[test_case(
    CompletionScenario::Completed,
    ThreadHistoryMode::Paginated;
    "completed_paginated"
)]
#[test_case(
    CompletionScenario::Completed,
    ThreadHistoryMode::Legacy;
    "completed_legacy"
)]
#[test_case(
    CompletionScenario::TerminalError,
    ThreadHistoryMode::Paginated;
    "terminal_error"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plaintext_multi_agent_v2_completion_sends_agent_message(
    scenario: CompletionScenario,
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": "opaque-encrypted-message",
        "task_name": "worker",
    }))?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-parent-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let child_events = match scenario {
        CompletionScenario::Completed => vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ],
        CompletionScenario::TerminalError => vec![ev_response_created("resp-child-1")],
    };
    let child_request = mount_response_once_match(
        &server,
        |req: &wiremock::Request| {
            request_has_input_type(req, "agent_message")
                && decoded_body(req)
                    .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
                    .and_then(|body| {
                        body["client_metadata"]["x-codex-turn-metadata"]
                            .as_str()
                            .and_then(|metadata| serde_json::from_str::<Value>(metadata).ok())
                    })
                    .is_some_and(|metadata| {
                        metadata.get("parent_turn_id").is_some_and(Value::is_string)
                    })
        },
        sse_response(sse(child_events)).set_delay(Duration::from_secs(1)),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, SPAWN_CALL_ID)
                && !request_has_input_type(req, "agent_message")
                && !body_contains(req, "Message Type: FINAL_ANSWER")
        },
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "parent done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;
    let error = "stream disconnected before completion: stream closed before response.completed";
    let (payload, expected_text) = match scenario {
        CompletionScenario::Completed => ("child done".to_string(), "child done"),
        CompletionScenario::TerminalError => (
            format!(
                "Agent errored: {error}\n\nThis agent's turn failed. If you still need this agent, use the available collaboration tools to give it another task."
            ),
            error,
        ),
    };
    let notification = format!(
        "Message Type: FINAL_ANSWER\nTask name: /root\nSender: /root/worker\nPayload:\n{payload}"
    );
    // If the child is still running when the parent turn starts, wait_agent blocks
    // until mailbox delivery. The follow-up request must then contain that delivery.
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, TURN_2_NO_WAIT_PROMPT)
                && !body_contains(req, "Message Type: FINAL_ANSWER")
        },
        sse(vec![
            ev_response_created("resp-parent-3"),
            ev_function_call_with_namespace(
                "wait-agent-call",
                MULTI_AGENT_V2_NAMESPACE,
                "wait_agent",
                "{}",
            ),
            ev_completed("resp-parent-3"),
        ]),
    )
    .await;
    let agent_request = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, TURN_2_NO_WAIT_PROMPT)
                && body_contains(req, "Message Type: FINAL_ANSWER")
                && body_contains(req, expected_text)
        },
        sse(vec![
            ev_response_created("resp-parent-4"),
            ev_assistant_message("msg-parent-4", "done"),
            ev_completed("resp-parent-4"),
        ]),
    )
    .await;
    let test = test_codex()
        .with_model("koffing")
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(0);
            config.model_provider.supports_websockets = false;
        })
        .with_history_mode(history_mode)
        .build(&server)
        .await?;

    test.submit_turn(TURN_1_PROMPT).await?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let (child_request, child_turn_metadata) = loop {
        let child_request = child_request.requests().into_iter().find_map(|request| {
            let body = request.body_json();
            let turn_metadata: Value =
                serde_json::from_str(body["client_metadata"]["x-codex-turn-metadata"].as_str()?)
                    .ok()?;
            turn_metadata
                .get("parent_turn_id")
                .and_then(Value::as_str)?;
            Some((request, turn_metadata))
        });
        if let Some(child_request) = child_request {
            break child_request;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for child request"
        );
        sleep(Duration::from_millis(10)).await;
    };
    let expected_completed_activity = if matches!(scenario, CompletionScenario::Completed) {
        let child_body = child_request.body_json();
        let parent_turn_id = child_turn_metadata["parent_turn_id"]
            .as_str()
            .expect("child parent turn ID")
            .to_string();
        let child_turn_id = child_body["client_metadata"]["turn_id"]
            .as_str()
            .expect("child turn ID")
            .to_string();
        let child_thread_id = ThreadId::from_string(
            child_body["client_metadata"]["thread_id"]
                .as_str()
                .expect("child thread ID"),
        )?;
        Some((parent_turn_id, child_turn_id, child_thread_id))
    } else {
        None
    };
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: TURN_2_NO_WAIT_PROMPT.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let (completed_activity_started, completed_activity_completed) =
        timeout(Duration::from_secs(5), async {
            let mut active_turn_id = None;
            let mut completed_activity_started = None;
            let mut completed_activity_completed = None;
            loop {
                let event = test
                    .codex
                    .next_event()
                    .await
                    .expect("event stream should remain open");
                match event.msg {
                    EventMsg::TurnStarted(event) => {
                        active_turn_id = Some(event.turn_id);
                    }
                    EventMsg::ItemStarted(event)
                        if matches!(
                            &event.item,
                            TurnItem::SubAgentActivity(SubAgentActivityItem {
                                kind: SubAgentActivityKind::Completed,
                                ..
                            })
                        ) =>
                    {
                        completed_activity_started = Some(event);
                    }
                    EventMsg::ItemCompleted(event)
                        if matches!(
                            &event.item,
                            TurnItem::SubAgentActivity(SubAgentActivityItem {
                                kind: SubAgentActivityKind::Completed,
                                ..
                            })
                        ) =>
                    {
                        completed_activity_completed = Some(event);
                    }
                    EventMsg::TurnComplete(event)
                        if active_turn_id.as_deref() == Some(event.turn_id.as_str()) =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
            (completed_activity_started, completed_activity_completed)
        })
        .await
        .expect("timed out waiting for parent turn completion");

    let request = wait_for_requests(&agent_request)
        .await?
        .pop()
        .expect("agent message request");
    assert_eq!(
        strip_response_item_ids_from_json(strip_metadata_from_json(Value::Array(
            request.inputs_of_type("agent_message"),
        ))),
        Value::Array(vec![json!({
            "type": "agent_message",
            "author": "/root/worker",
            "recipient": "/root",
            "content": [{
                "type": "input_text",
                "text": notification,
            }],
        })])
    );

    if let Some((parent_turn_id, child_turn_id, child_thread_id)) = expected_completed_activity {
        let started = completed_activity_started.expect("completed activity start event");
        let completed_event =
            completed_activity_completed.expect("completed activity completion event");
        let TurnItem::SubAgentActivity(started_item) = &started.item else {
            panic!("expected started sub-agent activity");
        };
        let TurnItem::SubAgentActivity(completed_event_item) = &completed_event.item else {
            panic!("expected completed sub-agent activity");
        };
        assert_eq!(
            (
                started.thread_id,
                &started.turn_id,
                started_item,
                Some(started.started_at_ms),
            ),
            (
                completed_event.thread_id,
                &completed_event.turn_id,
                completed_event_item,
                completed_event.started_at_ms,
            )
        );
        assert_eq!(started.turn_id, parent_turn_id);

        test.codex.ensure_rollout_materialized().await;
        test.codex.flush_rollout().await?;
        let rollout = codex_rollout::RolloutRecorder::get_rollout_history(
            &test.codex.rollout_path().expect("parent rollout path"),
        )
        .await?;
        assert!(
            !rollout.get_rollout_items().iter().any(|item| matches!(
                item,
                RolloutItem::EventMsg(EventMsg::SubAgentActivity(activity))
                    if activity.kind == SubAgentActivityKind::Completed
            )),
            "legacy completed activity should not be persisted without its parent turn ID"
        );
        let completed = rollout
            .get_rollout_items()
            .iter()
            .find_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::ItemCompleted(completed))
                    if matches!(
                        &completed.item,
                        TurnItem::SubAgentActivity(SubAgentActivityItem {
                            kind: SubAgentActivityKind::Completed,
                            ..
                        })
                    ) =>
                {
                    Some(completed)
                }
                _ => None,
            })
            .expect("persisted completed sub-agent activity");

        assert_eq!(completed.turn_id, parent_turn_id);
        let TurnItem::SubAgentActivity(completed_item) = &completed.item else {
            panic!("expected completed sub-agent activity");
        };
        assert_eq!(
            completed_item,
            &SubAgentActivityItem {
                id: format!("subagent-completed-{child_turn_id}"),
                kind: SubAgentActivityKind::Completed,
                agent_thread_id: child_thread_id,
                agent_path: codex_protocol::AgentPath::root()
                    .join("worker")
                    .expect("worker path"),
            }
        );
    } else {
        assert!(completed_activity_started.is_none());
        assert!(completed_activity_completed.is_none());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_v2_peer_followup_completion_notifies_initiating_turn() -> Result<()> {
    const SPAWN_WORKER_PROMPT: &str = "spawn the completion-routing worker";
    const SPAWN_REQUESTER_PROMPT: &str = "spawn the completion-routing requester";
    const READ_RESULT_PROMPT: &str = "read the completion-routing worker result";
    const WORKER_INITIAL_TASK: &str = "finish the worker initial task";
    const REQUESTER_TASK: &str = "ask the sibling worker to do more";
    const WORKER_FOLLOWUP_TASK: &str = "finish the peer-requested worker task";
    const WORKER_CALL_ID: &str = "spawn-routing-worker";
    const REQUESTER_CALL_ID: &str = "spawn-routing-requester";
    const FOLLOWUP_CALL_ID: &str = "request-peer-followup";

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(|config| {
            for feature in [Feature::Collab, Feature::MultiAgentV2] {
                config
                    .features
                    .enable(feature)
                    .expect("test config should allow feature update");
            }
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(0);
            config.model_provider.supports_websockets = false;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let root_thread_id = test.session_configured.thread_id;
    let mut created_threads = test.thread_manager.subscribe_thread_created();

    let worker_spawn_args = serde_json::to_string(&json!({
        "message": WORKER_INITIAL_TASK,
        "task_name": "worker",
        "fork_turns": "none",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, SPAWN_WORKER_PROMPT) && !body_contains(request, WORKER_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-spawn-routing-worker"),
            ev_function_call_with_namespace(
                WORKER_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &worker_spawn_args,
            ),
            ev_completed("resp-spawn-routing-worker"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, WORKER_INITIAL_TASK)
                && request_has_input_type(request, "agent_message")
                && !body_contains(request, WORKER_FOLLOWUP_TASK)
        },
        sse(vec![
            ev_response_created("resp-routing-worker-initial"),
            ev_assistant_message("msg-routing-worker-initial", "initial worker finished"),
            ev_completed("resp-routing-worker-initial"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, WORKER_CALL_ID)
                && body_contains(request, SPAWN_WORKER_PROMPT)
                && !body_contains(request, SPAWN_REQUESTER_PROMPT)
        },
        sse(vec![
            ev_response_created("resp-routing-worker-spawned"),
            ev_assistant_message("msg-routing-worker-spawned", "worker spawned"),
            ev_completed("resp-routing-worker-spawned"),
        ]),
    )
    .await;

    test.submit_turn(SPAWN_WORKER_PROMPT).await?;
    let worker_thread_id = created_threads.recv().await?;
    let worker_thread = test.thread_manager.get_thread(worker_thread_id).await?;
    wait_for_event(worker_thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requester_spawn_args = serde_json::to_string(&json!({
        "message": REQUESTER_TASK,
        "task_name": "requester",
        "fork_turns": "none",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, SPAWN_REQUESTER_PROMPT)
                && !body_contains(request, REQUESTER_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-spawn-routing-requester"),
            ev_function_call_with_namespace(
                REQUESTER_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &requester_spawn_args,
            ),
            ev_completed("resp-spawn-routing-requester"),
        ]),
    )
    .await;
    let followup_args = serde_json::to_string(&json!({
        "target": "/root/worker",
        "message": WORKER_FOLLOWUP_TASK,
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, REQUESTER_TASK)
                && request_has_input_type(request, "agent_message")
                && !body_contains(request, SPAWN_REQUESTER_PROMPT)
                && !body_contains(request, FOLLOWUP_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-routing-requester"),
            ev_function_call_with_namespace(
                FOLLOWUP_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "followup_task",
                &followup_args,
            ),
            ev_completed("resp-routing-requester"),
        ]),
    )
    .await;
    let worker_followup_request = mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, WORKER_FOLLOWUP_TASK)
                && request_has_input_type(request, "agent_message")
                && !body_contains(request, FOLLOWUP_CALL_ID)
        },
        sse_response(sse(vec![
            ev_response_created("resp-routing-worker-followup"),
            ev_assistant_message("msg-routing-worker-followup", "peer follow-up finished"),
            ev_completed("resp-routing-worker-followup"),
        ]))
        .set_delay(Duration::from_millis(250)),
    )
    .await;
    let mut collaboration_responses = Vec::new();
    for (call_id, prompt, response_id) in [
        (
            REQUESTER_CALL_ID,
            SPAWN_REQUESTER_PROMPT,
            "resp-routing-requester-spawned",
        ),
        (
            FOLLOWUP_CALL_ID,
            REQUESTER_TASK,
            "resp-routing-followup-requested",
        ),
    ] {
        collaboration_responses.push(
            mount_sse_once_match(
                &server,
                move |request: &wiremock::Request| {
                    body_contains(request, call_id) && body_contains(request, prompt)
                },
                sse(vec![
                    ev_response_created(response_id),
                    ev_assistant_message(response_id, "request accepted"),
                    ev_completed(response_id),
                ]),
            )
            .await,
        );
    }

    test.submit_turn(SPAWN_REQUESTER_PROMPT).await?;
    let requester_thread_id = created_threads.recv().await?;
    let requester_thread = test.thread_manager.get_thread(requester_thread_id).await?;
    let requester_turn_id = wait_for_event_match(requester_thread.as_ref(), |event| match event {
        EventMsg::TurnStarted(started) => Some(started.turn_id.clone()),
        _ => None,
    })
    .await;
    wait_for_event(requester_thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let followup_output = collaboration_responses[1]
        .function_call_output_text(FOLLOWUP_CALL_ID)
        .expect("requester follow-up tool output");
    assert_eq!(followup_output, "");
    wait_for_event(worker_thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let worker_followup_turn_id = worker_followup_request
        .requests()
        .into_iter()
        .find_map(|request| {
            let body = request.body_json();
            (body["client_metadata"]["thread_id"] == json!(worker_thread_id)
                && request.body_contains_text(WORKER_FOLLOWUP_TASK))
            .then(|| {
                body["client_metadata"]["turn_id"]
                    .as_str()
                    .expect("worker follow-up turn ID")
                    .to_string()
            })
        })
        .expect("worker follow-up model request");
    let completed = timeout(
        Duration::from_secs(5),
        wait_for_event_match(requester_thread.as_ref(), |event| match event {
            EventMsg::ItemCompleted(completed)
                if matches!(
                    &completed.item,
                    TurnItem::SubAgentActivity(activity)
                        if activity.kind == SubAgentActivityKind::Completed
                            && activity.agent_thread_id == worker_thread_id
                ) =>
            {
                Some(completed.clone())
            }
            _ => None,
        }),
    )
    .await?;
    let TurnItem::SubAgentActivity(completed_item) = completed.item else {
        unreachable!("completion event should contain sub-agent activity");
    };
    assert_eq!(
        (completed.thread_id, completed.turn_id, completed_item),
        (
            requester_thread_id,
            requester_turn_id,
            SubAgentActivityItem {
                id: format!("subagent-completed-{worker_followup_turn_id}"),
                kind: SubAgentActivityKind::Completed,
                agent_thread_id: worker_thread_id,
                agent_path: codex_protocol::AgentPath::root()
                    .join("worker")
                    .expect("worker path"),
            },
        )
    );

    // Fresh turn input is sampled before queued mail is drained. Let that first
    // request complete successfully so the next request can include the result.
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, READ_RESULT_PROMPT)
                && !body_contains(request, "peer follow-up finished")
        },
        sse(vec![ev_completed("resp-routing-root-before-mail")]),
    )
    .await;
    let root_result_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, READ_RESULT_PROMPT)
                && body_contains(request, "Sender: /root/worker")
                && body_contains(request, "peer follow-up finished")
        },
        sse(vec![
            ev_response_created("resp-routing-root-result"),
            ev_assistant_message("msg-routing-root-result", "result received"),
            ev_completed("resp-routing-root-result"),
        ]),
    )
    .await;
    test.submit_turn(READ_RESULT_PROMPT).await?;
    let root_request = root_result_request
        .requests()
        .into_iter()
        .find(|request| {
            request.body_json()["client_metadata"]["thread_id"] == json!(root_thread_id)
                && request.body_contains_text(READ_RESULT_PROMPT)
                && request.body_contains_text("peer follow-up finished")
        })
        .expect("root result request");
    assert!(
        root_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| {
                item["author"] == "/root/worker"
                    && item["recipient"] == "/root"
                    && item.to_string().contains("peer follow-up finished")
            })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skills_toggle_skips_instructions_for_parent_and_spawned_child() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "task_name": "worker",
    }))?;
    let spawn_turn = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;

    let _turn1_followup = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_home_skill(home, "demo", "demo-skill", "demo skill").expect("write home skill");
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            config.include_skill_instructions = false;
        });
    let test = builder.build(&server).await?;

    test.submit_turn(TURN_1_PROMPT).await?;
    let parent_request = spawn_turn.single_request();
    assert!(!parent_request.body_contains_text("<skills_instructions>"));
    assert!(!parent_request.body_contains_text("demo-skill"));

    let child_requests = wait_for_requests(&child_request_log).await?;
    let child_request = child_requests
        .last()
        .expect("child request log should capture at least one request");
    assert!(!child_request.body_contains_text("<skills_instructions>"));
    assert!(!child_request.body_contains_text("demo-skill"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_role_overrides_requested_model_and_reasoning_settings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let child_snapshot = spawn_child_and_capture_snapshot(
        &server,
        json!({
            "message": CHILD_PROMPT,
            "agent_type": "custom",
            "model": REQUESTED_MODEL,
            "reasoning_effort": REQUESTED_REASONING_EFFORT,
        }),
        |builder| {
            builder.with_config(|config| {
                let role_path = config.codex_home.join("custom-role.toml");
                std::fs::write(
                    &role_path,
                    format!(
                        "model = \"{ROLE_MODEL}\"\nmodel_reasoning_effort = \"{ROLE_REASONING_EFFORT}\"\n",
                    ),
                )
                .expect("write role config");
                config.agent_roles.insert(
                    "custom".to_string(),
                    AgentRoleConfig {
                        description: Some("Custom role".to_string()),
                        config_file: Some(role_path.to_path_buf()),
                        nickname_candidates: None,
                    },
                );
            })
        },
    )
    .await?;

    assert_eq!(child_snapshot.model, ROLE_MODEL);
    assert_eq!(child_snapshot.reasoning_effort, Some(ROLE_REASONING_EFFORT));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_preserves_configured_defaults_through_unrelated_role() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let child_snapshot = spawn_child_and_capture_snapshot(
        &server,
        json!({
            "message": CHILD_PROMPT,
            "agent_type": "custom",
        }),
        |builder| {
            builder.with_config(|config| {
                let role_path = config.codex_home.join("instructions-only-role.toml");
                std::fs::write(&role_path, "developer_instructions = \"Stay focused\"\n")
                    .expect("write role config");
                config.agent_roles.insert(
                    "custom".to_string(),
                    AgentRoleConfig {
                        description: Some("Custom role".to_string()),
                        config_file: Some(role_path.to_path_buf()),
                        nickname_candidates: None,
                    },
                );
                config.agent_default_subagent_model = Some(REQUESTED_MODEL.to_string());
                config.agent_default_subagent_reasoning_effort = Some(REQUESTED_REASONING_EFFORT);
            })
        },
    )
    .await?;

    assert_eq!(
        (child_snapshot.model, child_snapshot.reasoning_effort),
        (
            REQUESTED_MODEL.to_string(),
            Some(REQUESTED_REASONING_EFFORT)
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_rejects_reasoning_effort_unsupported_by_role_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "agent_type": "custom",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;
    let tool_output = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            let role_path = config.codex_home.join("model-only-role.toml");
            std::fs::write(&role_path, format!("model = \"{ROLE_MODEL}\"\n"))
                .expect("write role config");
            config.agent_roles.insert(
                "custom".to_string(),
                AgentRoleConfig {
                    description: Some("Custom role".to_string()),
                    config_file: Some(role_path.to_path_buf()),
                    nickname_candidates: None,
                },
            );
            config.agent_default_subagent_model = Some("gpt-5.6-sol".to_string());
            config.agent_default_subagent_reasoning_effort = Some(ReasoningEffort::Ultra);
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn(TURN_1_PROMPT).await?;

    let (output, _) = tool_output
        .single_request()
        .function_call_output_content_and_success(SPAWN_CALL_ID)
        .expect("spawn_agent output");
    assert_eq!(
        output.as_deref(),
        Some(
            "Reasoning effort `ultra` is not supported for model `gpt-5.4`. Supported reasoning efforts: low, medium, high, xhigh"
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_tool_description_mentions_role_locked_settings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "tool-search-spawn-agent";
    let resp_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-turn1-1"),
                ev_tool_search_call(
                    call_id,
                    &json!({
                        "query": "spawn agent custom role",
                        "limit": 1,
                    }),
                ),
                ev_completed("resp-turn1-1"),
            ]),
            sse(vec![
                ev_response_created("resp-turn1-2"),
                ev_assistant_message("msg-turn1-2", "done"),
                ev_completed("resp-turn1-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config.multi_agent_v2.hide_spawn_agent_metadata = false;
        let role_path = config.codex_home.join("custom-role.toml");
        std::fs::write(
            &role_path,
            format!(
                "developer_instructions = \"Stay focused\"\nmodel = \"{ROLE_MODEL}\"\nmodel_reasoning_effort = \"{ROLE_REASONING_EFFORT}\"\n",
            ),
        )
        .expect("write role config");
        config.agent_roles.insert(
            "custom".to_string(),
            AgentRoleConfig {
                description: Some("Custom role".to_string()),
                config_file: Some(role_path.to_path_buf()),
                nickname_candidates: None,
            },
        );
    });
    let test = builder.build(&server).await?;

    test.submit_turn(TURN_1_PROMPT).await?;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].tool_search_output(call_id);
    let spawn_agent = namespace_child_tool(&output, "multi_agent_v1", "spawn_agent")
        .expect("tool_search should return multi_agent_v1.spawn_agent");
    let agent_type_description = tool_parameter_description(spawn_agent, "agent_type")
        .expect("spawn_agent agent_type description");
    let custom_role_description =
        role_block(&agent_type_description, "custom").expect("custom role description");
    assert_eq!(
        custom_role_description,
        "custom: {\nCustom role\n- This role's model is set to `gpt-5.4` and its reasoning effort is set to `high`. These settings cannot be changed.\n}"
    );

    Ok(())
}
