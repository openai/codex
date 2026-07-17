use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadDeleteResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(20);

#[tokio::test]
async fn archive_runs_session_end_before_moving_transcript() -> Result<()> {
    run_removal_session_end_test("archive").await
}

#[tokio::test]
async fn delete_runs_session_end_before_removing_transcript() -> Result<()> {
    run_removal_session_end_test("delete").await
}

async fn run_removal_session_end_test(operation: &str) -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("persisted answer").await;
    let codex_home = TempDir::new()?;
    let log_path = write_config_and_hook(codex_home.path(), &server.uri())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;
    let thread_id = start_thread(&mut app_server).await?;

    let turn_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "persist this before removal".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let response = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(response)?;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    if operation == "archive" {
        let request_id = app_server
            .send_thread_archive_request(ThreadArchiveParams {
                thread_id: thread_id.clone(),
            })
            .await?;
        let response: JSONRPCResponse = timeout(
            READ_TIMEOUT,
            app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let _: ThreadArchiveResponse = to_response(response)?;
    } else {
        let request_id = app_server
            .send_thread_delete_request(ThreadDeleteParams {
                thread_id: thread_id.clone(),
            })
            .await?;
        let response: JSONRPCResponse = timeout(
            READ_TIMEOUT,
            app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let _: ThreadDeleteResponse = to_response(response)?;
    }

    let payloads = read_hook_log(&log_path)?;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["session_id"], thread_id);
    assert_eq!(payloads[0]["hook_event_name"], "SessionEnd");
    assert_eq!(payloads[0]["reason"], "other");
    assert_eq!(payloads[0]["transcript_exists"], true);
    let transcript = payloads[0]["transcript_text"]
        .as_str()
        .expect("session end transcript text");
    assert!(transcript.contains("persist this before removal"));
    assert!(transcript.contains("persisted answer"));
    Ok(())
}

#[tokio::test]
async fn app_server_shutdown_runs_session_end_for_all_loaded_threads() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let log_path = write_config_and_hook(codex_home.path(), &server.uri())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;
    let first = start_thread(&mut app_server).await?;
    let second = start_thread(&mut app_server).await?;

    let status = timeout(READ_TIMEOUT, app_server.shutdown_gracefully()).await??;
    assert!(status.success(), "app-server did not exit successfully");

    let mut actual = read_hook_log(&log_path)?
        .into_iter()
        .map(|payload| {
            (
                payload["session_id"].as_str().unwrap().to_string(),
                payload["reason"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = vec![(first, "other".to_string()), (second, "other".to_string())];
    expected.sort();
    assert_eq!(actual, expected);
    Ok(())
}

async fn start_thread(app_server: &mut TestAppServer) -> Result<String> {
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            config: Some(HashMap::from([(
                "bypass_hook_trust".to_string(),
                json!(true),
            )])),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    Ok(to_response::<ThreadStartResponse>(response)?.thread.id)
}

fn write_config_and_hook(codex_home: &Path, server_uri: &str) -> Result<std::path::PathBuf> {
    let log_path = codex_home.join("session-end.jsonl");
    let script_path = codex_home.join("session-end.py");
    std::fs::write(
        &script_path,
        format!(
            r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
transcript_path = payload.get("transcript_path")
transcript = Path(transcript_path) if transcript_path else None
payload["transcript_exists"] = bool(transcript and transcript.exists())
payload["transcript_text"] = transcript.read_text(encoding="utf-8") if transcript and transcript.exists() else ""
with Path(r"{}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
"#,
            log_path.display()
        ),
    )?;
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
model_provider = "mock_provider"

[features]
hooks = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[[hooks.SessionEnd]]
matcher = "other"

[[hooks.SessionEnd.hooks]]
type = "command"
command = "python3 {script_path}"
timeout = 3
"#,
            script_path = script_path.display(),
        ),
    )?;
    Ok(log_path)
}

fn read_hook_log(log_path: &Path) -> Result<Vec<Value>> {
    std::fs::read_to_string(log_path)
        .with_context(|| format!("read SessionEnd log {}", log_path.display()))?
        .lines()
        .map(|line| serde_json::from_str(line).context("parse SessionEnd log line"))
        .collect()
}
