use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

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
