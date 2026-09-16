//! Exercises the compression trigger through the public app-server API.

use std::time::Duration;

use anyhow::Result;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RolloutCompressResponse;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);

#[tokio::test]
async fn rollout_compress_runs_after_startup_with_compression_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_args(&["-c", "features.local_thread_store_compression=false"])
        .build_initialized()
        .await?;

    let filename_ts = "2025-01-05T12-00-00";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        /*model_provider*/ None,
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home.path(), filename_ts, &thread_id);
    let original = std::fs::read_to_string(&path)?;
    let compressed_path = path.with_extension("jsonl.zst");

    let response: RolloutCompressResponse = app_server
        .request(|request_id| ClientRequest::RolloutCompress {
            request_id,
            params: None,
        })
        .await?;
    assert_eq!(response, RolloutCompressResponse {});
    timeout(READ_TIMEOUT, async {
        while path.exists() || !compressed_path.exists() {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;

    let mut reader = codex_rollout::open_rollout_line_reader(&path).await?;
    let mut lines = Vec::new();
    while let Some(line) = reader.next_line().await? {
        lines.push(line);
    }
    assert_eq!(lines.join("\n") + "\n", original);
    Ok(())
}

#[tokio::test]
async fn rollout_compress_requires_experimental_capability() -> Result<()> {
    let mut app_server = TestAppServer::builder().build().await?;
    let initialization = app_server
        .initialize_with_capabilities(
            ClientInfo {
                name: DEFAULT_CLIENT_NAME.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: false,
                ..Default::default()
            }),
        )
        .await?;
    assert!(matches!(initialization, JSONRPCMessage::Response(_)));

    let request_id = app_server
        .send_raw_request("rollout/compress", /*params*/ None)
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error,
        JSONRPCErrorError {
            code: -32600,
            message: "rollout/compress requires experimentalApi capability".to_string(),
            data: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn rollout_compress_rejects_non_local_thread_stores() -> Result<()> {
    let mut app_server = TestAppServer::builder()
        .with_args(&[
            "-c",
            "experimental_thread_store={type=\"in_memory\",id=\"rollout-compress-test\"}",
        ])
        .build_initialized()
        .await?;
    let request_id = app_server
        .send_raw_request("rollout/compress", /*params*/ None)
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error,
        JSONRPCErrorError {
            code: -32601,
            message: "rollout/compress is not supported yet".to_string(),
            data: None,
        }
    );
    Ok(())
}
