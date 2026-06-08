use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::CodexThread;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::time::sleep;
use tokio::time::timeout;

const FIRST_PROMPT: &str = "start the detached hooks";
const SECOND_PROMPT: &str = "use the detached hook output";
const FIRST_CONTEXT: &str = "first detached hook context";
const SECOND_CONTEXT: &str = "second detached hook context";

fn write_gated_async_hooks(home: &Path) -> Result<()> {
    let script_path = home.join("async_hook.py");
    let script = format!(
        r#"import json
from pathlib import Path
import sys
import time

name = sys.argv[1]
if json.load(sys.stdin).get("prompt") != {FIRST_PROMPT:?}:
    raise SystemExit(0)
root = Path(__file__).parent
if name == "sync":
    completed = root / "first.completed"
    deadline = time.time() + 10
    while not completed.exists() and time.time() < deadline:
        time.sleep(0.01)
    if not completed.exists():
        raise RuntimeError("timed out waiting for first async hook")
    raise SystemExit(0)
release = root / f"{{name}}.release"
if name == "second":
    while not release.exists():
        time.sleep(0.01)
context = {{"first": {FIRST_CONTEXT:?}, "second": {SECOND_CONTEXT:?}}}[name]
print(json.dumps({{"hookSpecificOutput": {{
    "hookEventName": "UserPromptSubmit",
    "additionalContext": context,
}}}}))
(root / f"{{name}}.completed").write_text("done", encoding="utf-8")
"#,
    );
    let hooks = serde_json::json!({
        "hooks": {"UserPromptSubmit": [{"hooks": [
            {"type": "command", "command": format!("python3 {} first", script_path.display()), "async": true},
            {"type": "command", "command": format!("python3 {} second", script_path.display()), "async": true},
            {"type": "command", "command": format!("python3 {} sync", script_path.display())},
        ]}]}
    });
    fs::write(script_path, script).context("write async hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

async fn submit_user_turn(codex: &CodexThread, prompt: &str) -> Result<()> {
    codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    Ok(())
}

async fn hook_lifecycle_before_turn_complete(codex: &CodexThread) -> Result<(usize, usize)> {
    let mut started = 0;
    let mut completed = 0;
    loop {
        let event = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .context("timed out waiting for Codex event")??;
        match event.msg {
            EventMsg::HookStarted(_) => started += 1,
            EventMsg::HookCompleted(_) => completed += 1,
            EventMsg::TurnComplete(_) => return Ok((started, completed)),
            _ => {}
        }
    }
}

async fn wait_for_hook(home: &Path, name: &str) -> Result<()> {
    let completed = home.join(format!("{name}.completed"));
    timeout(Duration::from_secs(10), async {
        while !completed.exists() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .with_context(|| format!("timed out waiting for {name} async hook"))?;
    sleep(Duration::from_millis(100)).await;
    Ok(())
}

async fn release_hook(home: &Path, name: &str) -> Result<()> {
    fs::write(home.join(format!("{name}.release")), "release")?;
    wait_for_hook(home, name).await
}

fn message_index(input: &[Value], role: &str, text: &str) -> Option<usize> {
    input.iter().position(|item| {
        item.get("role").and_then(Value::as_str) == Some(role) && item.to_string().contains(text)
    })
}

#[tokio::test]
async fn async_command_hooks_deliver_ordered_output_on_the_next_user_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "first turn complete"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "async output received"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_pre_build_hook(|home| write_gated_async_hooks(home).unwrap())
        .with_config(trust_discovered_hooks)
        .build(&server)
        .await?;

    submit_user_turn(&test.codex, FIRST_PROMPT).await?;
    assert_eq!(
        hook_lifecycle_before_turn_complete(&test.codex).await?,
        (1, 1)
    );
    assert_eq!(responses.requests().len(), 1);
    wait_for_hook(test.codex_home_path(), "first").await?;
    assert!(!test.codex_home_path().join("second.completed").exists());

    release_hook(test.codex_home_path(), "second").await?;
    assert_eq!(
        responses.requests().len(),
        1,
        "async completion must not sample while the session is idle",
    );

    submit_user_turn(&test.codex, SECOND_PROMPT).await?;
    assert_eq!(
        hook_lifecycle_before_turn_complete(&test.codex).await?,
        (1, 1)
    );

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .message_input_texts("developer")
            .iter()
            .all(|text| !text.contains("<async_hook_outputs>"))
    );
    let messages = requests[1]
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.contains("<async_hook_outputs>"))
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].matches("<async_hook_output ").count(), 2);
    let first = messages[0]
        .find(FIRST_CONTEXT)
        .context("first async context")?;
    let second = messages[0]
        .find(SECOND_CONTEXT)
        .context("second async context")?;
    assert!(first < second);

    let input = requests[1].input();
    let user = message_index(&input, "user", SECOND_PROMPT).context("second user message")?;
    let developer =
        message_index(&input, "developer", "<async_hook_outputs>").context("async context")?;
    assert!(user < developer);
    Ok(())
}
