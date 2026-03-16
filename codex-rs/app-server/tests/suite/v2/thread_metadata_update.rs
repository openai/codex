use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::GitInfo;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::state_db::reconcile_rollout;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GitInfo as RolloutGitInfo;
use codex_state::StateRuntime;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_metadata_update_patches_git_branch_and_returns_updated_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

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

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread.id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/sidebar-pr".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let update_result = update_resp.result.clone();
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread.id);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/sidebar-pr".to_string()),
            origin_url: None,
        })
    );
    assert_eq!(updated.status, ThreadStatus::Idle);
    let updated_thread_json = update_result
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/metadata/update result.thread must be an object");
    let updated_git_info_json = updated_thread_json
        .get("gitInfo")
        .and_then(Value::as_object)
        .expect("thread/metadata/update must serialize `thread.gitInfo` on the wire");
    assert_eq!(
        updated_git_info_json.get("branch").and_then(Value::as_str),
        Some("feature/sidebar-pr")
    );

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread: read } = to_response::<ThreadReadResponse>(read_resp)?;

    assert_eq!(
        read.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/sidebar-pr".to_string()),
            origin_url: None,
        })
    );
    assert_eq!(read.status, ThreadStatus::Idle);

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_rejects_empty_git_info_patch() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

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

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread.id,
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: None,
                origin_url: None,
            }),
        })
        .await?;
    let update_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(update_id)),
    )
    .await??;

    assert_eq!(
        update_err.error.message,
        "gitInfo must include at least one field"
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_repairs_missing_sqlite_row_for_stored_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let _state_db = init_state_db(codex_home.path()).await?;

    let preview = "Stored thread preview";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/stored-thread".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id);
    assert_eq!(updated.preview, preview);
    assert_eq!(updated.created_at, 1736078400);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/stored-thread".to_string()),
            origin_url: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_repairs_loaded_thread_without_resetting_summary() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let state_db = init_state_db(codex_home.path()).await?;

    let preview = "Loaded thread preview";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T08-30-00",
        "2025-01-06T08:30:00Z",
        preview,
        Some("mock_provider"),
        None,
    )?;
    let thread_uuid = ThreadId::from_string(&thread_id)?;
    let rollout_path = rollout_path(codex_home.path(), "2025-01-06T08-30-00", &thread_id);
    reconcile_rollout(
        Some(&state_db),
        rollout_path.as_path(),
        "mock_provider",
        None,
        &[],
        None,
        None,
    )
    .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let _: ThreadResumeResponse = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(state_db.delete_thread(thread_uuid).await?, 1);

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/loaded-thread".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id);
    assert_eq!(updated.preview, preview);
    assert_eq!(updated.created_at, 1736152200);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/loaded-thread".to_string()),
            origin_url: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_repairs_missing_sqlite_row_for_archived_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let _state_db = init_state_db(codex_home.path()).await?;

    let preview = "Archived thread preview";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-06T08-30-00",
        "2025-01-06T08:30:00Z",
        preview,
        Some("mock_provider"),
        None,
    )?;

    let archived_dir = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    fs::create_dir_all(&archived_dir)?;
    let archived_source = rollout_path(codex_home.path(), "2025-01-06T08-30-00", &thread_id);
    let archived_dest = archived_dir.join(
        archived_source
            .file_name()
            .expect("archived rollout should have a file name"),
    );
    fs::rename(&archived_source, &archived_dest)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/archived-thread".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id);
    assert_eq!(updated.preview, preview);
    assert_eq!(updated.created_at, 1736152200);
    assert_eq!(
        updated.git_info,
        Some(GitInfo {
            sha: None,
            branch: Some("feature/archived-thread".to_string()),
            origin_url: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn thread_metadata_update_can_clear_stored_git_fields() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-07T09-15-00",
        "2025-01-07T09:15:00Z",
        "Thread preview",
        Some("mock_provider"),
        Some(RolloutGitInfo {
            commit_hash: Some("abc123".to_string()),
            branch: Some("feature/sidebar-pr".to_string()),
            repository_url: Some("git@example.com:openai/codex.git".to_string()),
        }),
    )?;
    let _state_db = init_state_db(codex_home.path()).await?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: Some(None),
                branch: Some(None),
                origin_url: Some(None),
            }),
        })
        .await?;
    let update_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;
    let ThreadMetadataUpdateResponse { thread: updated } =
        to_response::<ThreadMetadataUpdateResponse>(update_resp)?;

    assert_eq!(updated.id, thread_id.clone());
    assert_eq!(updated.git_info, None);

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id,
            include_turns: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread: read } = to_response::<ThreadReadResponse>(read_resp)?;

    assert_eq!(read.git_info, None);

    Ok(())
}

async fn init_state_db(codex_home: &Path) -> Result<Arc<StateRuntime>> {
    let state_db = StateRuntime::init(codex_home.to_path_buf(), "mock_provider".into()).await?;
    state_db.mark_backfill_complete(None).await?;
    Ok(state_db)
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"
suppress_unstable_features_warning = true

[features]
sqlite = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
