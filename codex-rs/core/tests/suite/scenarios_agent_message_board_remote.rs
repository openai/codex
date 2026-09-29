//! Runs existing board tools against a research-provisioned remote board.

use super::BoardClock;
use super::configure;
use super::done;
use super::tool;
use anyhow::Context;
use codex_features::RemoteMessageBoardConfigToml;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_board_uses_the_existing_tools_and_session_identity() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let board = responses::start_mock_server().await;
    let token = "research-board-credential-for-runtime-test";
    let url = board.uri();
    let root = test_codex()
        .with_config(move |config| {
            configure(config);
            config.ephemeral = true;
            config.current_time_reminder = Some(codex_core::config::CurrentTimeReminderConfig {
                clock_source: codex_features::CurrentTimeSource::External,
                ..Default::default()
            });
            config.multi_agent_v2.message_board_remote = Some(RemoteMessageBoardConfigToml {
                url,
                bearer_token: Some(token.into()),
                bearer_token_env_var: None,
            });
        })
        .with_external_time_provider(Arc::new(BoardClock::Available))
        .build_with_auto_env(&server)
        .await?;
    let root_id = root.session_configured.thread_id;
    let post = json!({
        "message_id": "00000000-0000-4000-8000-000000000001",
        "thread_id": "00000000-0000-4000-8000-000000000001",
        "author": "/root", "channel_name": "design", "created_at": "2026-09-18T12:00:00Z",
    });
    Mock::given(method("POST"))
        .and(path(format!("/v1/boards/{root_id}/call")))
        .and(header("authorization", format!("Bearer {token}")))
        .and(body_partial_json(json!({
            "caller": root_id,
            "timestamp": "2026-09-18T12:00:00Z",
            "method": "post",
            "params": {
                "destination": {"NewChannel": "design"},
                "text": "A remote decision.", "agents_to_notify": []
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(&post))
        .expect(1)
        .mount(&board)
        .await;
    let model = responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "remote-post",
                "post",
                json!({"new_channel_name":"design", "text":"A remote decision."}),
            ),
            done(),
        ],
    )
    .await;
    root.submit_turn("Post the remote decision.").await?;
    let requests = model.requests();
    let output: Value = serde_json::from_str(
        &requests
            .last()
            .context("model request")?
            .function_call_output_text("remote-post")
            .context("tool output")?,
    )?;
    assert_eq!(output, post);
    // Normalize tool JSON key order across Cargo and Bazel feature sets.
    let mut bodies = requests
        .iter()
        .map(responses::ResponsesRequest::body_json)
        .collect::<Vec<_>>();
    for body in &mut bodies {
        for item in body["input"].as_array_mut().context("request input")? {
            if item["type"] == "function_call_output" {
                let mut output: Value =
                    serde_json::from_str(item["output"].as_str().context("output")?)?;
                output.sort_all_objects();
                item["output"] = Value::String(output.to_string());
            }
        }
    }
    insta::assert_snapshot!(
        "remote_board_tools",
        context_snapshot::format_context_snapshot(
            "Existing board tools use a provisioned remote board.",
            &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    root.codex.shutdown_and_wait().await?;
    assert_eq!(
        board
            .received_requests()
            .await
            .context("board requests")?
            .len(),
        1
    );
    Ok(())
}
