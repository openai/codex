#![cfg(not(target_os = "windows"))]

use codex_core::CodexAuth;
use codex_core::features::Feature;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_body_is_zstd_compressed_for_codex_backend_when_enabled() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request_log = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let base_url = format!("{}/backend-api/codex/v1", server.uri());
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.features.enable(Feature::EnableRequestCompression);
            config.model_provider.base_url = Some(base_url);
        });
    let codex = builder.build(&server).await?.codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "compress me".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    // Wait until the task completes so the request definitely hit the server.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = request_log.single_request();
    assert_eq!(request.header("content-encoding").as_deref(), Some("zstd"));

    let decompressed = zstd::stream::decode_all(std::io::Cursor::new(request.body_bytes()))?;
    let json: serde_json::Value = serde_json::from_slice(&decompressed)?;
    assert!(
        json.get("input").is_some(),
        "expected request body to decode as Responses API JSON"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_body_is_not_compressed_for_api_key_auth_even_when_enabled() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request_log = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let base_url = format!("{}/backend-api/codex/v1", server.uri());
    let mut builder = test_codex().with_config(move |config| {
        config.features.enable(Feature::EnableRequestCompression);
        config.model_provider.base_url = Some(base_url);
    });
    let codex = builder.build(&server).await?.codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "do not compress".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = request_log.single_request();
    assert!(
        request.header("content-encoding").is_none(),
        "did not expect request compression for API-key auth"
    );

    let json: serde_json::Value = serde_json::from_slice(&request.body_bytes())?;
    assert!(
        json.get("input").is_some(),
        "expected request body to be plain Responses API JSON"
    );

    Ok(())
}
