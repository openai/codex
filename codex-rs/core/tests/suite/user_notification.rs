#![cfg(not(target_os = "windows"))]

use std::os::unix::fs::PermissionsExt;

use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::any;

use responses::ev_assistant_message;
use responses::ev_completed;
use responses::sse;
use responses::start_mock_server;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "flaky on ubuntu-24.04-arm - aarch64-unknown-linux-gnu"]
// The notify script gets far enough to create (and therefore surface) the file,
// but hasn’t flushed the JSON yet. Reading an empty file produces EOF while parsing
// a value at line 1 column 0. May be caused by a slow runner.
async fn summarize_context_three_requests_and_instructions() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let sse1 = sse(vec![ev_assistant_message("m1", "Done"), ev_completed("r1")]);

    responses::mount_sse_once_match(&server, any(), sse1).await;

    let notify_dir = TempDir::new()?;
    // write a script to the notify that touches a file next to it
    let notify_script = notify_dir.path().join("notify.sh");
    std::fs::write(
        &notify_script,
        r#"#!/bin/bash
set -e
payload="${@: -1}"
notify_dir=$(dirname "${0}")
if [[ "${payload}" == *"agent-turn-start"* ]]; then
  echo -n "${payload}" > "${notify_dir}/notify-start.txt"
else
  echo -n "${payload}" > "${notify_dir}/notify-complete.txt"
fi"#,
    )?;
    std::fs::set_permissions(&notify_script, std::fs::Permissions::from_mode(0o755))?;

    let notify_start_file = notify_dir.path().join("notify-start.txt");
    let notify_complete_file = notify_dir.path().join("notify-complete.txt");
    let notify_script_str = notify_script.to_str().unwrap().to_string();

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |cfg| cfg.notify = Some(vec![notify_script_str]))
        .build(&server)
        .await?;

    // 1) Normal user input – should hit server once.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello world".into(),
            }],
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // We fork the notify script, so we need to wait for it to write to the file.
    fs_wait::wait_for_path_exists(&notify_start_file, Duration::from_secs(5)).await?;
    fs_wait::wait_for_path_exists(&notify_complete_file, Duration::from_secs(5)).await?;
    let start_payload_raw = tokio::fs::read_to_string(&notify_start_file).await?;
    let start_payload: Value = serde_json::from_str(&start_payload_raw)?;
    assert_eq!(start_payload["type"], json!("agent-turn-start"));
    assert_eq!(start_payload["input-messages"], json!(["hello world"]));

    let notify_payload_raw = tokio::fs::read_to_string(&notify_complete_file).await?;
    let payload: Value = serde_json::from_str(&notify_payload_raw)?;
    assert_eq!(payload["type"], json!("agent-turn-complete"));
    assert_eq!(payload["input-messages"], json!(["hello world"]));
    assert_eq!(payload["last-assistant-message"], json!("Done"));

    Ok(())
}
