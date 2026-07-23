use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::find_archived_thread_path_by_id_str;
use codex_core::find_thread_path_by_id_str;
use codex_protocol::ThreadId;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_archive_rejects_owned_unmaterialized_paginated_descendant() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let parent_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T00-00-00",
        "2025-01-01T00:00:00Z",
        "parent",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let parent_thread_id = ThreadId::from_string(&parent_id)?;
    let mut owner = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread: child, .. } = owner
        .start_thread(ThreadStartParams {
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let child_thread_id = ThreadId::from_string(&child.id)?;
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .upsert_thread_spawn_edge(
            parent_thread_id,
            child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Open,
        )
        .await?;

    let mut other = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let request_id = other
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: parent_id.clone(),
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        other.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        format!("thread {} already has an active writer", child.id)
    );
    timeout(DEFAULT_READ_TIMEOUT, owner.shutdown_gracefully()).await??;
    let _: ThreadArchiveResponse = other
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: parent_id,
            },
        })
        .await?;
    Ok(())
}

#[tokio::test]
async fn thread_archive_requires_materialized_rollout() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    // Start a thread.
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    assert!(!thread.id.is_empty());

    let rollout_path = thread.path.clone().expect("thread path");
    assert!(
        !rollout_path.exists(),
        "fresh thread rollout should not exist yet at {}",
        rollout_path.display()
    );
    assert!(
        find_thread_path_by_id_str(codex_home.path(), &thread.id, /*state_db_ctx*/ None)
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

    // Materialize rollout via a real user turn and confirm archive succeeds.
    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "materialize".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    assert!(
        rollout_path.exists(),
        "expected rollout path {} to exist after first user message",
        rollout_path.display()
    );

    let discovered_path =
        find_thread_path_by_id_str(codex_home.path(), &thread.id, /*state_db_ctx*/ None)
            .await?
            .expect("expected rollout path for thread id to exist after materialization");
    assert_paths_match_on_disk(&discovered_path, &rollout_path)?;

    let _: ThreadArchiveResponse = mcp
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: thread.id.clone(),
            },
        })
        .await?;
    let archived_notification: ThreadArchivedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("thread/archived"),
    )
    .await??;
    assert_eq!(archived_notification.thread_id, thread.id);

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

#[tokio::test]
async fn thread_archive_archives_spawned_descendants() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

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

    let parent_thread_id = ThreadId::from_string(&parent_id)?;
    let child_thread_id = ThreadId::from_string(&child_id)?;
    let grandchild_thread_id = ThreadId::from_string(&grandchild_id)?;
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let _: ThreadArchiveResponse = mcp
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: parent_id.clone(),
            },
        })
        .await?;

    let mut archived_ids = Vec::new();
    for _ in 0..3 {
        let archived_notification: ThreadArchivedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("thread/archived"),
        )
        .await??;
        archived_ids.push(archived_notification.thread_id);
    }
    assert_eq!(archived_ids, vec![parent_id, grandchild_id, child_id]);

    for thread_id in [parent_thread_id, child_thread_id, grandchild_thread_id] {
        assert!(
            find_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_none(),
            "expected active rollout for {thread_id} to be archived"
        );
        assert!(
            find_archived_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_some(),
            "expected archived rollout for {thread_id} to exist"
        );
    }

    Ok(())
}

#[tokio::test]
async fn thread_archive_succeeds_when_descendant_archive_fails() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

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

    let parent_thread_id = ThreadId::from_string(&parent_id)?;
    let child_thread_id = ThreadId::from_string(&child_id)?;
    let grandchild_thread_id = ThreadId::from_string(&grandchild_id)?;
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
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

    let child_rollout_path =
        find_thread_path_by_id_str(codex_home.path(), &child_id, /*state_db_ctx*/ None)
            .await?
            .expect("child rollout path");
    let archived_child_path = codex_home
        .path()
        .join(ARCHIVED_SESSIONS_SUBDIR)
        .join(child_rollout_path.file_name().expect("rollout file name"));
    std::fs::create_dir_all(&archived_child_path)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let _: ThreadArchiveResponse = mcp
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: parent_id.clone(),
            },
        })
        .await?;

    let mut archived_ids = Vec::new();
    for _ in 0..2 {
        let archived_notification: ThreadArchivedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("thread/archived"),
        )
        .await??;
        archived_ids.push(archived_notification.thread_id);
    }
    assert_eq!(archived_ids, vec![parent_id, grandchild_id]);

    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/archived"),
        )
        .await
        .is_err()
    );

    assert!(
        child_rollout_path.exists(),
        "child should stay active after descendant archive failure"
    );
    assert!(
        archived_child_path.is_dir(),
        "test conflict should remain in archived sessions"
    );
    for thread_id in [parent_thread_id, grandchild_thread_id] {
        assert!(
            find_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_none(),
            "expected active rollout for {thread_id} to be archived"
        );
        assert!(
            find_archived_thread_path_by_id_str(
                codex_home.path(),
                &thread_id.to_string(),
                /*state_db_ctx*/ None,
            )
            .await?
            .is_some(),
            "expected archived rollout for {thread_id} to exist"
        );
    }

    Ok(())
}

#[tokio::test]
async fn thread_archive_succeeds_when_spawned_descendant_is_missing() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let parent_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-01T00-00-00",
        "2025-01-01T00:00:00Z",
        "parent",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let parent_thread_id = ThreadId::from_string(&parent_id)?;
    let missing_child_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000901")?;

    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;
    state_db
        .upsert_thread_spawn_edge(
            parent_thread_id,
            missing_child_thread_id,
            DirectionalThreadSpawnEdgeStatus::Closed,
        )
        .await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let _: ThreadArchiveResponse = mcp
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: parent_id.clone(),
            },
        })
        .await?;

    let archived_notification: ThreadArchivedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("thread/archived"),
    )
    .await??;
    assert_eq!(archived_notification.thread_id, parent_id);

    assert!(
        find_thread_path_by_id_str(codex_home.path(), &parent_id, /*state_db_ctx*/ None)
            .await?
            .is_none(),
        "parent should be archived even when a descendant is missing"
    );
    assert!(
        find_archived_thread_path_by_id_str(
            codex_home.path(),
            &parent_id,
            /*state_db_ctx*/ None,
        )
        .await?
        .is_some(),
        "parent should be moved into archived sessions"
    );

    Ok(())
}

#[tokio::test]
async fn thread_archive_clears_stale_subscriptions_before_resume() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = primary
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let _: TurnStartResponse = primary
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "materialize".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    primary.clear_message_buffer();

    let mut secondary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let _: ThreadArchiveResponse = primary
        .request(|request_id| ClientRequest::ThreadArchive {
            request_id,
            params: ThreadArchiveParams {
                thread_id: thread.id.clone(),
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("thread/archived"),
    )
    .await??;

    let _: ThreadUnarchiveResponse = primary
        .request(|request_id| ClientRequest::ThreadUnarchive {
            request_id,
            params: ThreadUnarchiveParams {
                thread_id: thread.id.clone(),
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("thread/unarchived"),
    )
    .await??;
    primary.clear_message_buffer();

    let resume: ThreadResumeResponse = secondary
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread.id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resume.thread.status, ThreadStatus::Idle);
    primary.clear_message_buffer();
    secondary.clear_message_buffer();

    let _: TurnStartResponse = secondary
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id,
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "secondary turn".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            primary.read_stream_until_notification_message("turn/started"),
        )
        .await
        .is_err()
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

fn assert_paths_match_on_disk(actual: &Path, expected: &Path) -> std::io::Result<()> {
    let actual = actual.canonicalize()?;
    let expected = expected.canonicalize()?;
    assert_eq!(actual, expected);
    Ok(())
}
