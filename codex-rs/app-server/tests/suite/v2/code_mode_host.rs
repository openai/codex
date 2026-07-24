use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;

#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_shares_flag_selected_code_mode_host_across_threads() -> Result<()> {
    let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut websocket_host = Command::new(host_program)
        .args(["--listen", "ws://127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .context("failed to start websocket code-mode host")?;
    let stdout = websocket_host
        .stdout
        .take()
        .context("websocket code-mode host stdout was not captured")?;
    let mut lines = BufReader::new(stdout).lines();
    let websocket_url = timeout(DEFAULT_READ_TIMEOUT, lines.next_line())
        .await
        .context("timed out waiting for websocket code-mode host URL")??
        .context("websocket code-mode host exited before publishing its URL")?;

    let model_server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &model_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_custom_tool_call(
                    "first-remote-cell",
                    "exec",
                    "text('remote app-server host')",
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-3"),
                responses::ev_custom_tool_call(
                    "second-remote-cell",
                    "exec",
                    "text('remote app-server host')",
                ),
                responses::ev_completed("resp-3"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-2", "Done"),
                responses::ev_completed("resp-4"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&model_server.uri())
        .enable_feature(Feature::CodeModeOnly)
        .write(codex_home.path())?;
    let original_config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_args(&["--code-mode-host", &websocket_url])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    for prompt in ["run the first remote cell", "run the second remote cell"] {
        let thread = app_server
            .start_thread(ThreadStartParams::default())
            .await?;
        let completed = timeout(
            DEFAULT_READ_TIMEOUT,
            app_server.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.thread.id,
                input: vec![UserInput::Text {
                    text: prompt.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;

        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    for (request, call_id) in [
        (&requests[1], "first-remote-cell"),
        (&requests[3], "second-remote-cell"),
    ] {
        let output = request.custom_tool_call_output(call_id);
        assert_eq!(
            output["output"]
                .as_array()
                .and_then(|items| items.last())
                .cloned(),
            Some(json!({
                "type": "input_text",
                "text": "remote app-server host",
            }))
        );
    }
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join("config.toml"))?,
        original_config
    );

    Ok(())
}
