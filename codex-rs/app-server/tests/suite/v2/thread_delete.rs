use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadDeleteResponse;
use codex_app_server_protocol::ThreadDeletedNotification;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::find_thread_path_by_id_str;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use codex_state::StateRuntime;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_delete_deletes_spawned_descendants() -> Result<()> {
    let codex_home = TempDir::new()?;

    let parent_id = create_delete_test_rollout(codex_home.path(), /*minute*/ 0, "parent")?;
    let child_id = create_delete_test_rollout(codex_home.path(), /*minute*/ 1, "child")?;
    let grandchild_id =
        create_delete_test_rollout(codex_home.path(), /*minute*/ 2, "grandchild")?;

    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "mock_provider".into()).await?;
    let parent_thread_id = ThreadId::from_string(&parent_id)?;
    let child_thread_id = ThreadId::from_string(&child_id)?;
    let grandchild_thread_id = ThreadId::from_string(&grandchild_id)?;

    for (parent, child, status) in [
        (
            parent_thread_id,
            child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Closed,
        ),
        (
            child_thread_id,
            grandchild_thread_id,
            DirectionalThreadSpawnEdgeStatus::Open,
        ),
    ] {
        state_db
            .upsert_thread_spawn_edge(parent, child, status)
            .await?;
    }

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let _: ThreadDeleteResponse = mcp
        .request(|request_id| ClientRequest::ThreadDelete {
            request_id,
            params: ThreadDeleteParams {
                thread_id: parent_id.clone(),
            },
        })
        .await?;

    let mut deleted_ids = Vec::new();
    for _ in 0..3 {
        let deleted_notification: ThreadDeletedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("thread/deleted"),
        )
        .await??;
        deleted_ids.push(deleted_notification.thread_id);
    }
    assert_eq!(deleted_ids, vec![grandchild_id, child_id, parent_id]);

    for thread_id in [parent_thread_id, child_thread_id, grandchild_thread_id] {
        let rollout_path = find_thread_path_by_id_str(
            codex_home.path(),
            &thread_id.to_string(),
            /*state_db_ctx*/ None,
        )
        .await?;
        assert!(
            rollout_path.is_none(),
            "expected active rollout for {thread_id} to be deleted"
        );
    }
    assert_eq!(
        state_db
            .list_thread_spawn_descendants(parent_thread_id)
            .await?,
        Vec::<ThreadId>::new()
    );
    Ok(())
}

#[tokio::test]
async fn thread_delete_preflights_external_fork_references_for_spawned_subtrees() -> Result<()> {
    let codex_home = TempDir::new()?;

    let parent_id = create_delete_test_rollout(codex_home.path(), /*minute*/ 0, "parent")?;
    let child_id = create_delete_test_rollout(codex_home.path(), /*minute*/ 1, "child")?;
    let external_id = create_delete_test_rollout(codex_home.path(), /*minute*/ 2, "external")?;
    let parent_thread_id = ThreadId::from_string(&parent_id)?;
    let child_thread_id = ThreadId::from_string(&child_id)?;
    let external_thread_id = ThreadId::from_string(&external_id)?;
    let parent_path = find_thread_path_by_id_str(
        codex_home.path(),
        &parent_thread_id.to_string(),
        /*state_db_ctx*/ None,
    )
    .await?
    .expect("parent rollout path");
    let external_path = find_thread_path_by_id_str(
        codex_home.path(),
        &external_thread_id.to_string(),
        /*state_db_ctx*/ None,
    )
    .await?
    .expect("external rollout path");
    let mut external_meta: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(external_path.as_path())?
            .lines()
            .next()
            .expect("external session metadata"),
    )?;
    external_meta["payload"]["history_base"] = serde_json::to_value(HistoryPosition {
        thread_id: parent_thread_id,
        end_ordinal_exclusive: 1,
        end_byte_offset: std::fs::metadata(parent_path.as_path())?.len(),
    })?;
    std::fs::write(external_path.as_path(), format!("{external_meta}\n"))?;

    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "mock_provider".into()).await?;
    state_db
        .upsert_thread_spawn_edge(
            parent_thread_id,
            child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Closed,
        )
        .await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let delete_id = mcp
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: parent_id.clone(),
        })
        .await?;
    let delete_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(delete_id)),
    )
    .await??;
    assert_eq!(
        delete_err.error.message,
        format!("cannot delete thread {parent_thread_id}: forked history still references it")
    );

    for thread_id in [parent_thread_id, child_thread_id, external_thread_id] {
        assert!(
            find_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_some(),
            "expected rollout for {thread_id} to remain"
        );
    }
    assert_eq!(
        state_db
            .list_thread_spawn_descendants(parent_thread_id)
            .await?,
        vec![child_thread_id]
    );
    Ok(())
}

fn create_delete_test_rollout(codex_home: &Path, minute: u8, preview: &str) -> Result<String> {
    create_fake_rollout(
        codex_home,
        &format!("2025-01-01T00-{minute:02}-00"),
        &format!("2025-01-01T00:{minute:02}:00Z"),
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
    )
}

#[tokio::test]
async fn thread_delete_handles_live_threads_before_rollout_exists() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let persisted_thread = mcp.start_thread(ThreadStartParams::default()).await?.thread;
    let rollout_path = find_thread_path_by_id_str(
        codex_home.path(),
        &persisted_thread.id,
        /*state_db_ctx*/ None,
    )
    .await?;
    assert_eq!(rollout_path, None);

    let _: ThreadDeleteResponse = mcp
        .request(|request_id| ClientRequest::ThreadDelete {
            request_id,
            params: ThreadDeleteParams {
                thread_id: persisted_thread.id,
            },
        })
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

    let delete_id = mcp
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let delete_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(delete_id)),
    )
    .await??;
    let expected_message = format!(
        "thread is not persisted and cannot be deleted: {}",
        thread.id
    );
    assert_eq!(delete_err.error.message, expected_message);

    let ThreadLoadedListResponse { mut data, .. } = mcp
        .request(|request_id| ClientRequest::ThreadLoadedList {
            request_id,
            params: ThreadLoadedListParams::default(),
        })
        .await?;
    data.sort();
    assert_eq!(data, vec![thread.id]);

    Ok(())
}
