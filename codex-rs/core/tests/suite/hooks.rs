use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::sleep;

const FIRST_CONTINUATION_PROMPT: &str = "Retry with exactly the phrase meow meow meow.";
const SECOND_CONTINUATION_PROMPT: &str = "Now tighten it to just: meow.";
const BLOCKED_PROMPT_CONTEXT: &str = "Remember the blocked lighthouse note.";

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

fn write_user_prompt_submit_hook(
    home: &Path,
    blocked_prompt: &str,
    additional_context: &str,
) -> Result<()> {
    let script_path = home.join("user_prompt_submit_hook.py");
    let blocked_prompt_json =
        serde_json::to_string(blocked_prompt).context("serialize blocked prompt for test")?;
    let additional_context_json = serde_json::to_string(additional_context)
        .context("serialize user prompt submit additional context for test")?;
    let script = format!(
        r#"import json
import sys

payload = json.load(sys.stdin)

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

fn rollout_developer_texts(text: &str) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = serde_json::from_str(trimmed).context("parse rollout line")?;
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) = rollout.item
            && role == "developer"
        {
            for item in content {
                if let ContentItem::InputText { text } = item {
                    texts.push(text);
                }
            }
        }
    }
    Ok(texts)
}

fn read_stop_hook_inputs(home: &Path) -> Result<Vec<serde_json::Value>> {
    fs::read_to_string(home.join("stop_hook_log.jsonl"))
        .context("read stop hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse stop hook log line"))
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
    assert!(
        requests[1]
            .message_input_texts("developer")
            .contains(&FIRST_CONTINUATION_PROMPT.to_string()),
        "second request should include the first continuation prompt",
    );
    assert!(
        requests[2]
            .message_input_texts("developer")
            .contains(&FIRST_CONTINUATION_PROMPT.to_string()),
        "third request should retain the first continuation prompt from history",
    );
    assert!(
        requests[2]
            .message_input_texts("developer")
            .contains(&SECOND_CONTINUATION_PROMPT.to_string()),
        "third request should include the second continuation prompt",
    );

    let hook_inputs = read_stop_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 3);
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
    let developer_texts = rollout_developer_texts(&rollout_text)?;
    assert!(
        developer_texts.contains(&FIRST_CONTINUATION_PROMPT.to_string()),
        "rollout should persist the first continuation prompt",
    );
    assert!(
        developer_texts.contains(&SECOND_CONTINUATION_PROMPT.to_string()),
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
    assert!(
        resumed_request
            .message_input_texts("developer")
            .contains(&FIRST_CONTINUATION_PROMPT.to_string()),
        "resumed request should keep the persisted continuation prompt in history",
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
        .with_model("gpt-5.1")
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
            items: vec![UserInput::Text {
                text: "initial prompt".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::AgentMessageContentDelta(_))
    })
    .await;

    for text in ["accepted queued prompt", "blocked queued prompt"] {
        test.codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
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

    server.shutdown().await;
    Ok(())
}
