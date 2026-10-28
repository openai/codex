#![cfg(not(target_os = "windows"))]

use codex_core::TurnInputRequest;

use codex_protocol::models::ImageReference;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::EventMsg;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn notify_receives_user_text_while_the_model_receives_attachments() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let sse1 = sse(vec![ev_assistant_message("m1", "Done"), ev_completed("r1")]);

    let response = responses::mount_sse_once(&server, sse1).await;

    let notify_dir = TempDir::new()?;
    // write a script to the notify that touches a file next to it
    let notify_script = notify_dir.path().join("notify.sh");
    codex_utils_cargo_bin::write_executable(
        &notify_script,
        r#"#!/bin/bash
set -e
payload_path="$(dirname "${0}")/notify.txt"
tmp_path="${payload_path}.tmp"
echo -n "${@: -1}" > "${tmp_path}"
mv "${tmp_path}" "${payload_path}""#,
    )?;

    let notify_file = notify_dir.path().join("notify.txt");
    let notify_script_str = notify_script.to_str().unwrap().to_string();

    let TestCodex { codex, .. } = test_codex()
        .with_model_info_override("gpt-5.5", |info| {
            info.input_modalities.push(InputModality::Audio);
        })
        .with_config(move |cfg| cfg.notify = Some(vec![notify_script_str]))
        .build(&server)
        .await?;

    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    let audio_url = "data:audio/wav;base64,AAAA";
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            UserInput::Text {
                text: "hello world".into(),
                text_elements: Vec::new(),
            },
            UserInput::Image {
                image: ImageReference::Inline {
                    image_url: image_url.into(),
                },
                detail: None,
            },
            UserInput::Audio {
                audio_url: audio_url.into(),
            },
        ]))
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    let request = response.single_request();
    assert_eq!(request.message_input_image_urls("user"), [image_url]);
    assert_eq!(request.message_input_audio_urls("user"), [audio_url]);

    // We fork the notify script, so we need to wait for it to write to the file.
    fs_wait::wait_for_path_exists(&notify_file, Duration::from_secs(5)).await?;
    let notify_payload_raw = tokio::fs::read_to_string(&notify_file).await?;
    let payload: Value = serde_json::from_str(&notify_payload_raw)?;

    assert_eq!(payload["type"], json!("agent-turn-complete"));
    assert_eq!(payload["input-messages"], json!(["hello world"]));
    assert_eq!(payload["last-assistant-message"], json!("Done"));

    Ok(())
}
