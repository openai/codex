use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::to_response;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadDeleteResponse;
use codex_app_server_protocol::ThreadDeletedNotification;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::find_archived_thread_path_by_id_str;
use codex_core::find_thread_path_by_id_str;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use codex_state::StateRuntime;
use codex_state::ThreadGoalStatus;
use codex_state::ThreadMetadataBuilder;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_delete_deletes_spawned_descendants_and_metadata() -> Result<()> {
    let codex_home = TempDir::new()?;

    let parent_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T00-00-00",
        "2025-01-01T00:00:00Z",
        "parent",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let child_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T00-01-00",
        "2025-01-01T00:01:00Z",
        "child",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let grandchild_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T00-02-00",
        "2025-01-01T00:02:00Z",
        "grandchild",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "mock_provider".into()).await?;
    let parent_thread_id = seed_thread_metadata(
        &state_db,
        codex_home.path(),
        &parent_id,
        "2025-01-01T00-00-00",
        "2025-01-01T00:00:00Z",
    )
    .await?;
    let child_thread_id = seed_thread_metadata(
        &state_db,
        codex_home.path(),
        &child_id,
        "2025-01-01T00-01-00",
        "2025-01-01T00:01:00Z",
    )
    .await?;
    let grandchild_thread_id = seed_thread_metadata(
        &state_db,
        codex_home.path(),
        &grandchild_id,
        "2025-01-01T00-02-00",
        "2025-01-01T00:02:00Z",
    )
    .await?;

    state_db
        .upsert_thread_spawn_edge(
            parent_thread_id,
            child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Closed,
        )
        .await?;
    state_db
        .upsert_thread_spawn_edge(
            child_thread_id,
            grandchild_thread_id,
            DirectionalThreadSpawnEdgeStatus::Open,
        )
        .await?;
    state_db
        .thread_goals()
        .replace_thread_goal(
            parent_thread_id,
            "parent goal",
            ThreadGoalStatus::Active,
            /*token_budget*/ None,
        )
        .await?;
    state_db
        .thread_goals()
        .replace_thread_goal(
            child_thread_id,
            "child goal",
            ThreadGoalStatus::Active,
            /*token_budget*/ None,
        )
        .await?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let delete_id = mcp
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: parent_id.clone(),
        })
        .await?;
    let delete_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(delete_id)),
    )
    .await??;
    let _: ThreadDeleteResponse = to_response::<ThreadDeleteResponse>(delete_resp)?;

    let mut deleted_ids = Vec::new();
    for _ in 0..3 {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("thread/deleted"),
        )
        .await??;
        let deleted_notification: ThreadDeletedNotification = serde_json::from_value(
            notification
                .params
                .expect("thread/deleted notification params"),
        )?;
        deleted_ids.push(deleted_notification.thread_id);
    }
    assert_eq!(deleted_ids, vec![parent_id, grandchild_id, child_id]);

    for thread_id in [parent_thread_id, child_thread_id, grandchild_thread_id] {
        assert!(
            find_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_none(),
            "expected active rollout for {thread_id} to be deleted"
        );
        assert!(
            find_archived_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_none(),
            "expected archived rollout for {thread_id} to be absent"
        );
        assert!(
            state_db.get_thread(thread_id).await?.is_none(),
            "expected sqlite metadata for {thread_id} to be deleted"
        );
    }
    assert!(
        state_db
            .list_thread_spawn_descendants(parent_thread_id)
            .await?
            .is_empty()
    );
    assert!(
        state_db
            .thread_goals()
            .get_thread_goal(parent_thread_id)
            .await?
            .is_none()
    );
    assert!(
        state_db
            .thread_goals()
            .get_thread_goal(child_thread_id)
            .await?
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn thread_delete_rejects_live_ephemeral_thread_without_unloading() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let thread_id = start_ephemeral_thread(&mut mcp).await?;

    let delete_id = mcp
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let delete_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(delete_id)),
    )
    .await??;
    assert_eq!(
        delete_err.error.message,
        format!("thread is not persisted and cannot be deleted: {thread_id}")
    );

    let loaded_thread_ids = loaded_thread_ids(&mut mcp).await?;
    assert_eq!(loaded_thread_ids, vec![thread_id]);

    Ok(())
}

async fn seed_thread_metadata(
    state_db: &StateRuntime,
    codex_home: &Path,
    thread_id: &str,
    filename_ts: &str,
    meta_rfc3339: &str,
) -> Result<ThreadId> {
    let id = ThreadId::from_string(thread_id)?;
    let created_at = DateTime::parse_from_rfc3339(meta_rfc3339)?.with_timezone(&Utc);
    let mut builder = ThreadMetadataBuilder::new(
        id,
        rollout_path(codex_home, filename_ts, thread_id),
        created_at,
        SessionSource::Cli,
    );
    builder.updated_at = Some(created_at);
    builder.cwd = codex_home.to_path_buf();
    let metadata = builder.build("mock_provider");
    state_db.upsert_thread(&metadata).await?;
    Ok(id)
}

async fn start_ephemeral_thread(mcp: &mut McpProcess) -> Result<String> {
    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.2".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(response)?;
    Ok(thread.id)
}

async fn loaded_thread_ids(mcp: &mut McpProcess) -> Result<Vec<String>> {
    let request_id = mcp
        .send_thread_loaded_list_request(ThreadLoadedListParams::default())
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadLoadedListResponse { mut data, .. } =
        to_response::<ThreadLoadedListResponse>(response)?;
    data.sort();
    Ok(data)
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
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
        ),
    )
}
