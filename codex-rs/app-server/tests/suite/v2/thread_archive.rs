use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
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
async fn thread_archive_requires_materialized_rollout() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;

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

    let rollout_path = thread.path.clone().expect("thread path");
    assert!(
        !rollout_path.exists(),
        "fresh thread rollout should not exist yet at {}",
        rollout_path.display()
    );
    assert!(
        find_thread_path_by_id_str(codex_home.path(), &thread.id)
            .await?
            .is_none(),
        "thread id should not be discoverable before rollout materialization"
    );

    // Archive should fail before the rollout is materialized.
    let archive_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let archive_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(archive_id)),
    )
    .await??;
    assert!(
        archive_err
            .error
            .message
            .contains("no rollout found for thread id"),
        "unexpected archive error: {}",
        archive_err.error.message
    );

    // Materialize a rollout file and confirm archive succeeds.
    std::fs::create_dir_all(
        rollout_path
            .parent()
            .expect("rollout path should have parent directory"),
    )?;
    std::fs::write(&rollout_path, "{}\n")?;

    let discovered_path = find_thread_path_by_id_str(codex_home.path(), &thread.id)
        .await?
        .expect("expected rollout path for thread id to exist after materialization");
    assert_paths_match_on_disk(&discovered_path, &rollout_path)?;

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

    // Verify file moved.
    let archived_directory = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    // The archived file keeps the original filename (rollout-...-<id>.jsonl).
    let archived_rollout_path =
        archived_directory.join(rollout_path.file_name().expect("rollout file name"));
    assert!(
        !rollout_path.exists(),
        "expected rollout path {} to be moved",
        rollout_path.display()
    );
    assert!(
        archived_rollout_path.exists(),
        "expected archived rollout path {} to exist",
        archived_rollout_path.display()
    );

    Ok(())
}

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, config_contents())
}

fn config_contents() -> &'static str {
    r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
"#
}

fn assert_paths_match_on_disk(actual: &Path, expected: &Path) -> std::io::Result<()> {
    let actual = actual.canonicalize()?;
    let expected = expected.canonicalize()?;
    assert_eq!(actual, expected);
    Ok(())
}
