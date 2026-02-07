use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_core::find_archived_thread_path_by_id_str;
use codex_core::find_thread_path_by_id_str;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use serde_json::json;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[tokio::test]
async fn thread_unarchive_moves_rollout_back_into_sessions_directory() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

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

    let rollout_path = thread.path.clone().expect("thread path");
    write_minimal_rollout(&thread.id, &rollout_path)?;

    let found_rollout_path = find_thread_path_by_id_str(codex_home.path(), &thread.id)
        .await?
        .expect("expected rollout path for thread id to exist");
    assert_paths_match_on_disk(&found_rollout_path, &rollout_path)?;

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

    let archived_path = find_archived_thread_path_by_id_str(codex_home.path(), &thread.id)
        .await?
        .expect("expected archived rollout path for thread id to exist");
    let archived_path_display = archived_path.display();
    assert!(
        archived_path.exists(),
        "expected {archived_path_display} to exist"
    );
    let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    let old_timestamp = old_time
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("old timestamp")
        .as_secs() as i64;
    let times = FileTimes::new().set_modified(old_time);
    OpenOptions::new()
        .append(true)
        .open(&archived_path)?
        .set_times(times)?;

    let unarchive_id = mcp
        .send_thread_unarchive_request(ThreadUnarchiveParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let unarchive_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unarchive_id)),
    )
    .await??;
    let ThreadUnarchiveResponse {
        thread: unarchived_thread,
    } = to_response::<ThreadUnarchiveResponse>(unarchive_resp)?;
    assert!(
        unarchived_thread.updated_at > old_timestamp,
        "expected updated_at to be bumped on unarchive"
    );

    let rollout_path_display = rollout_path.display();
    assert!(
        rollout_path.exists(),
        "expected rollout path {rollout_path_display} to be restored"
    );
    assert!(
        !archived_path.exists(),
        "expected archived rollout path {archived_path_display} to be moved"
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

fn write_minimal_rollout(thread_id: &str, rollout_path: &Path) -> Result<()> {
    let parent = rollout_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("rollout path should have parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let timestamp = "2026-01-01T00:00:00Z";
    let meta = SessionMeta {
        id: ThreadId::from_string(thread_id)?,
        forked_from_id: None,
        timestamp: timestamp.to_string(),
        cwd: PathBuf::from("/"),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::Cli,
        model_provider: Some("openai".to_string()),
        base_instructions: None,
        dynamic_tools: None,
    };
    let payload = serde_json::to_value(SessionMetaLine { meta, git: None })?;
    let line = json!({
        "timestamp": timestamp,
        "type": "session_meta",
        "payload": payload,
    });
    std::fs::write(rollout_path, format!("{line}\n"))?;
    Ok(())
}
