use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::find_thread_path_by_id_str;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_archive_moves_rollout_into_archived_directory() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a thread.
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;
    assert!(!thread.id.is_empty());

    // Archive the thread.
    let archive_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let archive_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(archive_id)),
    )
    .await??;
    let _: ThreadArchiveResponse = to_response::<ThreadArchiveResponse>(archive_resp)?;

    // Locate the rollout path recorded for this thread id.
    let rollout_path = find_thread_path_by_id_str(codex_home.path(), &thread.id).await?;
    assert!(
        rollout_path.is_none(),
        "archived thread should no longer have an active rollout path"
    );

    // Verify file moved.
    let archived_directory = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    let archived_rollout_path =
        find_archived_rollout_by_thread_id(&archived_directory, &thread.id)?;
    assert!(
        archived_rollout_path.exists(),
        "expected archived rollout path {} to exist",
        archived_rollout_path.display()
    );

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, config_contents(server_uri))
}

fn config_contents(server_uri: &str) -> String {
    format!(
        r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
    )
}

fn find_archived_rollout_by_thread_id(
    archived_directory: &Path,
    thread_id: &str,
) -> std::io::Result<std::path::PathBuf> {
    let entries = std::fs::read_dir(archived_directory)?;
    for entry in entries {
        let path = entry?.path();
        if let Some(file_name) = path.file_name().and_then(|name| name.to_str())
            && file_name.ends_with(&format!("{thread_id}.jsonl"))
        {
            return Ok(path);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("no archived rollout found for thread id {thread_id}"),
    ))
}
