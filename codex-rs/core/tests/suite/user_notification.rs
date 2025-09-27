#![cfg(not(target_os = "windows"))]

use std::os::unix::fs::PermissionsExt;

use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::matchers::any;

use responses::ev_assistant_message;
use responses::ev_completed;
use responses::sse;
use responses::start_mock_server;
use tokio::time::Duration;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn summarize_context_three_requests_and_instructions() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let sse1 = sse(vec![ev_assistant_message("m1", "Done"), ev_completed("r1")]);

    responses::mount_sse_once(&server, any(), sse1).await;

    let notify_dir = TempDir::new()?;
    // write a script to the notify that touches a file next to it
    let notify_script = notify_dir.path().join("notify.sh");
    std::fs::write(
        &notify_script,
        r#"#!/bin/bash
set -e
echo -n "${@: -1}" > $(dirname "${0}")/notify.txt"#,
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
            items: vec![InputItem::Text {
                text: "hello world".into(),
            }],
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // We fork the notify script, so we need to wait for it to write to the file.
    for _ in 0..100u32 {
        if notify_file.exists() {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(notify_file.exists());

    Ok(())
}
