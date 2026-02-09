#![cfg(not(target_os = "windows"))]

use anyhow::Context;
use anyhow::Result;
use codex_core::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use sha2::Digest;
use sha2::Sha256;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_user_message_is_stored_as_discoverable_item() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request_log = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.model_context_window = Some(100);
    });
    let test = builder.build(&server).await?;

    let large_prompt = "oversized user prompt ".repeat(800);
    test.codex
        .submit(codex_core::protocol::Op::UserInput {
            items: vec![UserInput::Text {
                text: large_prompt.clone(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = request_log.single_request();
    let mut hasher = Sha256::new();
    hasher.update(large_prompt.as_bytes());
    let expected_checksum = format!("{:x}", hasher.finalize());
    let discoverable_root = test.codex_home_path().join("discovarable_items");
    let expected_path = std::fs::read_dir(&discoverable_root)
        .context("read discoverable_items dir")?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("user_message").join(&expected_checksum))
        .find(|candidate| candidate.is_file())
        .context("expected discoverable file was not created")?;
    assert!(expected_path.is_file(), "discoverable file should exist");

    let expected = format!(
        "User message was too large. Read it from <{}>",
        expected_path.display()
    );
    let actual = request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.contains(&expected_checksum))
        .context("missing discoverable replacement text in user input")?;

    assert_eq!(actual, expected);

    Ok(())
}
