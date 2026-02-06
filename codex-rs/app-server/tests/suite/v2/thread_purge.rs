use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadPurgeParams;
use codex_app_server_protocol::ThreadPurgeResponse;
use codex_app_server_protocol::ThreadPurgeResult;
use codex_app_server_protocol::ThreadPurgeStatus;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::find_archived_thread_path_by_id_str;
use codex_core::find_thread_path_by_id_str;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_purge_batch_deletes_archived_threads_and_reports_per_item_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let archived_thread = start_thread(&mut mcp).await?;
    let active_thread = start_thread(&mut mcp).await?;

    let archive_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: archived_thread.id.clone(),
        })
        .await?;
    let archive_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(archive_id)),
    )
    .await??;
    let _: ThreadArchiveResponse = to_response::<ThreadArchiveResponse>(archive_resp)?;

    let archived_path = find_archived_thread_path_by_id_str(codex_home.path(), &archived_thread.id)
        .await?
        .expect("expected archived rollout path for thread id");
    assert!(archived_path.exists());

    let missing_thread_id = ThreadId::new().to_string();
    let invalid_thread_id = "not-a-thread-id".to_string();
    let purge_id = mcp
        .send_thread_purge_request(ThreadPurgeParams {
            thread_ids: vec![
                archived_thread.id.clone(),
                active_thread.id.clone(),
                missing_thread_id.clone(),
                invalid_thread_id.clone(),
            ],
        })
        .await?;
    let purge_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(purge_id)),
    )
    .await??;
    let ThreadPurgeResponse { data } = to_response::<ThreadPurgeResponse>(purge_resp)?;
    assert_eq!(data.len(), 4);
    assert_eq!(
        data[0],
        ThreadPurgeResult {
            thread_id: archived_thread.id.clone(),
            status: ThreadPurgeStatus::Purged,
            message: None,
        }
    );
    assert_eq!(
        data[1],
        ThreadPurgeResult {
            thread_id: active_thread.id.clone(),
            status: ThreadPurgeStatus::InUse,
            message: Some("thread is currently loaded in memory".to_string()),
        }
    );
    assert_eq!(
        data[2],
        ThreadPurgeResult {
            thread_id: missing_thread_id,
            status: ThreadPurgeStatus::NotFound,
            message: None,
        }
    );
    assert_eq!(
        data[3].thread_id, invalid_thread_id,
        "expected invalid input id to be echoed in per-item result"
    );
    assert_eq!(data[3].status, ThreadPurgeStatus::Failed);
    assert!(
        data[3]
            .message
            .as_deref()
            .is_some_and(|message| message.contains("invalid thread id"))
    );

    assert!(!archived_path.exists());
    let active_rollout_path = find_thread_path_by_id_str(codex_home.path(), &active_thread.id)
        .await?
        .expect("expected active rollout path for thread id");
    assert!(active_rollout_path.exists());

    Ok(())
}

#[tokio::test]
async fn thread_purge_returns_in_use_for_loaded_thread_with_archived_rollout_path() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread = start_thread(&mut mcp).await?;
    let rollout_path = find_thread_path_by_id_str(codex_home.path(), &thread.id)
        .await?
        .expect("expected rollout path for thread id");
    let archived_dir = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    std::fs::create_dir_all(&archived_dir)?;
    let archived_rollout_path = archived_dir.join(
        rollout_path
            .file_name()
            .expect("expected rollout path to include filename"),
    );
    std::fs::rename(&rollout_path, &archived_rollout_path)?;

    let purge_id = mcp
        .send_thread_purge_request(ThreadPurgeParams {
            thread_ids: vec![thread.id.clone()],
        })
        .await?;
    let purge_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(purge_id)),
    )
    .await??;
    let ThreadPurgeResponse { data } = to_response::<ThreadPurgeResponse>(purge_resp)?;
    assert_eq!(
        data,
        vec![ThreadPurgeResult {
            thread_id: thread.id,
            status: ThreadPurgeStatus::InUse,
            message: Some("thread is currently loaded in memory".to_string()),
        }]
    );
    assert!(archived_rollout_path.exists());

    Ok(())
}

async fn start_thread(mcp: &mut McpProcess) -> Result<codex_app_server_protocol::Thread> {
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
    Ok(thread)
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
