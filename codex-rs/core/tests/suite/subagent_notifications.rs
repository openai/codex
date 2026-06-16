use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::ThreadConfigSnapshot;
use codex_core::config::AgentRoleConfig;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
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
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
use test_case::test_case;
use tokio::time::Instant;
use tokio::time::sleep;
use wiremock::MockServer;

const SPAWN_CALL_ID: &str = "spawn-call-1";
const MULTI_AGENT_V1_NAMESPACE: &str = "multi_agent_v1";
const TURN_0_FORK_PROMPT: &str = "seed fork context";
const TURN_1_PROMPT: &str = "spawn a child and continue";
const TURN_2_NO_WAIT_PROMPT: &str = "follow up without wait";
const CHILD_PROMPT: &str = "child: do work";
const INHERITED_MODEL: &str = "gpt-5.3-codex";
const INHERITED_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::XHigh;
const REQUESTED_MODEL: &str = "gpt-5.4";
const REQUESTED_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Low;
const ROLE_MODEL: &str = "gpt-5.4";
const ROLE_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::High;
const SUBAGENT_START_CONTEXT: &str = "subagent start context reaches child";
const SUBAGENT_STOP_CONTINUATION: &str = "continue only the child";
const INTERNAL_SUBAGENT_PROMPT: &str = "internal subagent: review";

fn body_contains(req: &wiremock::Request, text: &str) -> bool {
    let is_zstd = req
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    let bytes = if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&req.body)).ok()
    } else {
        Some(req.body.clone())
    };
    bytes
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn has_subagent_notification(req: &ResponsesRequest) -> bool {
    req.message_input_texts("user")
        .iter()
        .any(|text| text.contains("<subagent_notification>"))
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

    #[allow(clippy::expect_used)]
    let mut builder = configure_test(test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config.model = Some(INHERITED_MODEL.to_string());
        config.model_reasoning_effort = Some(INHERITED_REASONING_EFFORT);
    }));
    let test = builder.build(server).await?;
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
            if let Err(error) =
                write_subagent_lifecycle_hooks(home, /*stop_prompts*/ &[], "worker")
            {
                panic!("failed to write subagent hook fixture: {error}");
            }
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
            if let Err(error) = write_subagent_lifecycle_hooks(
                home,
                /*stop_prompts*/ &[SUBAGENT_STOP_CONTINUATION],
                "",
            ) {
                panic!("failed to write subagent hook fixture: {error}");
            }
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
        .start_thread_with_options(StartThreadOptions {
            config: test.config.clone(),
            initial_history: InitialHistory::New,
            session_source: Some(SessionSource::SubAgent(SubAgentSource::Review)),
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
            thread_extension_init: Default::default(),
        })
        .await?;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.cwd_path());
    internal_thread
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: INTERNAL_SUBAGENT_PROMPT.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                model: Some(internal_thread.session_configured.model.clone()),
                ..Default::default()
            },
        })
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
    assert!(turn2_requests.iter().any(has_subagent_notification));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_child_receives_forked_parent_context() -> Result<()> {
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

    let _child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, CHILD_PROMPT),
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

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    test.submit_turn(TURN_0_FORK_PROMPT).await?;
    let _ = seed_turn.single_request();

    test.submit_turn(TURN_1_PROMPT).await?;
    let _ = spawn_turn.single_request();

    let deadline = Instant::now() + Duration::from_secs(2);
    let child_request = loop {
        if let Some(request) = server
            .received_requests()
            .await
            .unwrap_or_default()
            .into_iter()
            .find(|request| {
                body_contains(request, CHILD_PROMPT) && !body_contains(request, SPAWN_CALL_ID)
            })
        {
            break request;
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for forked child request");
        }
        sleep(Duration::from_millis(10)).await;
    };
    assert!(body_contains(&child_request, TURN_0_FORK_PROMPT));
    assert!(!body_contains(&child_request, SPAWN_CALL_ID));

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
        |builder| builder,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_multi_agent_v2_spawn_sends_agent_message_to_child() -> Result<()> {
    let server = start_mock_server().await;
    let encrypted_message = "opaque-encrypted-message";
    let spawn_args = serde_json::to_string(&json!({
        "message": encrypted_message,
        "task_name": "worker",
    }))?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-parent-1"),
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, "\"type\":\"agent_message\""),
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, SPAWN_CALL_ID) && !body_contains(req, "\"type\":\"agent_message\"")
        },
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_model("koffing").with_config(|config| {
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

    test.submit_turn(TURN_1_PROMPT).await?;

    let child_request = wait_for_requests(&child_request_log)
        .await?
        .pop()
        .expect("child request");
    assert_eq!(
        child_request.inputs_of_type("agent_message"),
        vec![json!({
            "type": "agent_message",
            "author": "/root",
            "recipient": "/root/worker",
            "content": [{
                "type": "encrypted_content",
                "encrypted_content": encrypted_message,
            }],
        })]
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum CompletionScenario {
    Completed,
    TerminalError,
}

#[test_case(CompletionScenario::Completed ; "completed")]
#[test_case(CompletionScenario::TerminalError ; "terminal_error")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plaintext_multi_agent_v2_completion_sends_agent_message(
    scenario: CompletionScenario,
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
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
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
        |req: &wiremock::Request| body_contains(req, "\"type\":\"agent_message\""),
        sse_response(sse(child_events)).set_delay(Duration::from_secs(1)),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, SPAWN_CALL_ID) && !body_contains(req, "<subagent_notification>")
        },
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "parent done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;
    let error = "stream disconnected before completion: stream closed before response.completed";
    let (status, next_action, expected_text) = match scenario {
        CompletionScenario::Completed => (json!({"completed": "child done"}), None, "child done"),
        CompletionScenario::TerminalError => (
            json!({"errored": error}),
            Some(
                "This agent's turn failed. If you still need this agent, use the available collaboration tools to give it another task.",
            ),
            error,
        ),
    };
    let mut notification_body = json!({
        "agent_path": "/root/worker",
        "status": status,
    });
    if let Some(next_action) = next_action {
        notification_body["next_action"] = json!(next_action);
    }
    let notification =
        format!("<subagent_notification>\n{notification_body}\n</subagent_notification>");
    // If the child is still running when the parent turn starts, wait_agent blocks
    // until mailbox delivery. The follow-up request must then contain that delivery.
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, TURN_2_NO_WAIT_PROMPT)
                && !body_contains(req, "<subagent_notification>")
        },
        sse(vec![
            ev_response_created("resp-parent-3"),
            ev_function_call("wait-agent-call", "wait_agent", "{}"),
            ev_completed("resp-parent-3"),
        ]),
    )
    .await;
    let agent_request = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            body_contains(req, TURN_2_NO_WAIT_PROMPT) && body_contains(req, expected_text)
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
        .build(&server)
        .await?;

    test.submit_turn(TURN_1_PROMPT).await?;
    let _ = wait_for_requests(&child_request).await?;
    test.submit_turn(TURN_2_NO_WAIT_PROMPT).await?;

    let request = wait_for_requests(&agent_request)
        .await?
        .pop()
        .expect("agent message request");
    assert_eq!(
        request.inputs_of_type("agent_message"),
        vec![json!({
            "type": "agent_message",
            "author": "/root/worker",
            "recipient": "/root",
            "content": [{
                "type": "input_text",
                "text": notification,
            }],
        })]
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
            if let Err(err) = write_home_skill(home, "demo", "demo-skill", "demo skill") {
                panic!("write home skill: {err}");
            }
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
        .unwrap_or_else(|| {
            panic!("expected tool_search to return multi_agent_v1.spawn_agent: {output:?}")
        });
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
