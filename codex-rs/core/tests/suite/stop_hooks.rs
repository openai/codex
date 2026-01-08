#![cfg(not(target_os = "windows"))]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

use responses::ev_assistant_message;
use responses::ev_completed;
use responses::ev_response_created;
use responses::sse;
use responses::start_mock_server;

fn write_hook_script(path: &Path, contents: &str) -> anyhow::Result<()> {
    std::fs::write(path, contents)?;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

fn write_hooks_json(path: &Path, script_path: &Path) -> anyhow::Result<()> {
    let hooks_json = json!({
        "hooks": {
            "Stop": [
                {
                    "type": "command",
                    "command": script_path.to_string_lossy()
                }
            ]
        }
    });
    std::fs::write(path, serde_json::to_string(&hooks_json)?)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_hook_block_reinjects_prompt() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "First response"),
        ev_completed("resp-1"),
    ]);
    let second = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-2", "Second response"),
        ev_completed("resp-2"),
    ]);
    let response_mock = responses::mount_sse_sequence(&server, vec![first, second]).await;

    let fixture = test_codex().with_model("gpt-5.1").build(&server).await?;

    let hooks_dir = fixture.cwd_path().join(".codex").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let script_path = fixture.cwd_path().join("stop-hook.sh");
    write_hook_script(
        &script_path,
        r#"#!/bin/sh
set -e
FLAG=.codex/stop-hook.once
if [ -f "$FLAG" ]; then
  echo '{"decision":"approve"}'
  exit 0
fi
mkdir -p .codex
touch "$FLAG"
echo '{"decision":"block","reason":"repeat","systemMessage":"loop"}'
"#,
    )?;

    write_hooks_json(&hooks_dir.join("hooks.json"), &script_path)?;

    fixture
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await?;

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let second_request = requests.last().expect("second request");
    let user_texts = second_request.message_input_texts("user");
    assert!(user_texts.iter().any(|text| text == "repeat"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_hook_approve_allows_completion() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "Done"),
        ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_sequence(&server, vec![first]).await;

    let fixture = test_codex().with_model("gpt-5.1").build(&server).await?;

    let hooks_dir = fixture.cwd_path().join(".codex").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let script_path = fixture.cwd_path().join("stop-hook-approve.sh");
    write_hook_script(
        &script_path,
        r#"#!/bin/sh
echo '{"decision":"approve"}'
"#,
    )?;

    write_hooks_json(&hooks_dir.join("hooks.json"), &script_path)?;

    fixture
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await?;

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 1);

    Ok(())
}
