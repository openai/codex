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

use responses::ev_assistant_message;
use responses::ev_completed;
use responses::sse;
use responses::start_mock_server;
use std::time::Duration;
use std::time::Instant;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "flaky on ubuntu-24.04-arm - aarch64-unknown-linux-gnu"]
// The notify script gets far enough to create (and therefore surface) the file,
// but hasn’t flushed the JSON yet. Reading an empty file produces EOF while parsing
// a value at line 1 column 0. May be caused by a slow runner.
async fn summarize_context_three_requests_and_instructions() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let sse1 = sse(vec![ev_assistant_message("m1", "Done"), ev_completed("r1")]);

    responses::mount_sse_once(&server, sse1).await;

    let notify_dir = TempDir::new()?;
    // write a script to the notify that touches a file next to it
    let notify_script = notify_dir.path().join("notify.sh");
    std::fs::write(
        &notify_script,
        r#"#!/bin/bash
set -e
echo "${@: -1}" >> $(dirname "${0}")/notify.txt"#,
    )?;
    std::fs::set_permissions(&notify_script, std::fs::Permissions::from_mode(0o755))?;

    let notify_file = notify_dir.path().join("notify.txt");
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
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // We fork the notify script, so we need to wait for it to write to the file.
    fs_wait::wait_for_path_exists(&notify_file, Duration::from_secs(5)).await?;
    let payloads = {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let notify_payload_raw = tokio::fs::read_to_string(&notify_file).await?;
            let lines = notify_payload_raw
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();
            if !lines.is_empty()
                && let Ok(payloads) = lines
                    .iter()
                    .map(|line| serde_json::from_str::<Value>(line))
                    .collect::<Result<Vec<_>, _>>()
                    && payloads
                        .iter()
                        .any(|payload| payload["type"] == json!("agent-turn-complete"))
                    {
                        break payloads;
                    }
            if Instant::now() >= deadline {
                let notify_payload_raw = tokio::fs::read_to_string(&notify_file).await?;
                let payloads = notify_payload_raw
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                    .collect::<Vec<_>>();
                if !payloads.is_empty() {
                    break payloads;
                }
                let payload: Value = serde_json::from_str(&notify_payload_raw)?;
                break vec![payload];
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let start_payload = payloads
        .iter()
        .find(|payload| payload["type"] == json!("agent-turn-start"))
        .expect("agent-turn-start payload");
    let complete_payload = payloads
        .iter()
        .find(|payload| payload["type"] == json!("agent-turn-complete"))
        .expect("agent-turn-complete payload");

    assert_eq!(start_payload["input-messages"], json!(["hello world"]));
    assert_eq!(complete_payload["input-messages"], json!(["hello world"]));
    assert_eq!(complete_payload["last-assistant-message"], json!("Done"));

    Ok(())
}
