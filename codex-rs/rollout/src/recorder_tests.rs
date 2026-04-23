#![allow(warnings, clippy::all)]

use super::*;
use crate::config::RolloutConfig;
use chrono::TimeZone;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

fn test_config(codex_home: &Path) -> RolloutConfig {
    RolloutConfig {
        codex_home: codex_home.to_path_buf(),
        sqlite_home: codex_home.to_path_buf(),
        cwd: codex_home.to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: true,
    }
}

fn write_session_file(root: &Path, ts: &str, uuid: Uuid) -> std::io::Result<PathBuf> {
    let day_dir = root.join("sessions/2025/01/03");
    fs::create_dir_all(&day_dir)?;
    let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
    let mut file = File::create(&path)?;
    let meta = serde_json::json!({
        "timestamp": ts,
        "type": "session_meta",
        "payload": {
            "id": uuid,
            "timestamp": ts,
            "cwd": ".",
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "cli",
            "model_provider": "test-provider",
        },
    });
    writeln!(file, "{meta}")?;
    let user_event = serde_json::json!({
        "timestamp": ts,
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": "Hello from user",
            "kind": "plain",
        },
    });
    writeln!(file, "{user_event}")?;
    Ok(path)
}

#[tokio::test]
async fn recorder_materializes_on_flush_with_pending_items() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let thread_id = ThreadId::new();
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            thread_id,
            /*forked_from_id*/ None,
            SessionSource::Exec,
            BaseInstructions::default(),
            Vec::new(),
            EventPersistenceMode::Limited,
        ),
        /*state_db_ctx*/ None,
        /*state_builder*/ None,
    )
    .await?;

    let rollout_path = recorder.rollout_path().to_path_buf();
    assert!(
        !rollout_path.exists(),
        "rollout file should not exist before the first recordable item"
    );

    recorder
        .record_items(&[RolloutItem::EventMsg(EventMsg::AgentMessage(
            AgentMessageEvent {
                message: "buffered-event".to_string(),
                phase: None,
                memory_citation: None,
            },
        ))])
        .await?;
    recorder.flush().await?;
    assert!(
        rollout_path.exists(),
        "flush with pending items should materialize the rollout"
    );

    recorder
        .record_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(
            UserMessageEvent {
                message: "first-user-message".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        ))])
        .await?;
    recorder.flush().await?;

    recorder.persist().await?;
    // Second call verifies `persist()` is idempotent after materialization.
    recorder.persist().await?;
    assert!(rollout_path.exists(), "rollout file should be materialized");

    let text = std::fs::read_to_string(&rollout_path)?;
    assert!(
        text.contains("\"type\":\"session_meta\""),
        "expected session metadata in rollout"
    );
    let buffered_idx = text
        .find("buffered-event")
        .expect("buffered event in rollout");
    let user_idx = text
        .find("first-user-message")
        .expect("first user message in rollout");
    assert!(
        buffered_idx < user_idx,
        "buffered items should preserve ordering"
    );
    let text_after_second_persist = std::fs::read_to_string(&rollout_path)?;
    assert_eq!(text_after_second_persist, text);

    recorder.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn persist_reports_filesystem_error_and_retries_buffered_items() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let thread_id = ThreadId::new();
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            thread_id,
            /*forked_from_id*/ None,
            SessionSource::Exec,
            BaseInstructions::default(),
            Vec::new(),
            EventPersistenceMode::Limited,
        ),
        /*state_db_ctx*/ None,
        /*state_builder*/ None,
    )
    .await?;
    let rollout_path = recorder.rollout_path().to_path_buf();

    recorder
        .record_items(&[RolloutItem::EventMsg(EventMsg::AgentMessage(
            AgentMessageEvent {
                message: "buffered-before-persist".to_string(),
                phase: None,
                memory_citation: None,
            },
        ))])
        .await?;
    let sessions_blocker_path = home.path().join("sessions");
    File::create(&sessions_blocker_path)?;

    let err = recorder
        .persist()
        .await
        .expect_err("blocked sessions directory should fail persist");
    assert_ne!(err.kind(), std::io::ErrorKind::Interrupted);
    assert!(
        !rollout_path.exists(),
        "failed persist should keep the rollout deferred"
    );

    fs::remove_file(sessions_blocker_path)?;
    recorder.flush().await?;
    let text = std::fs::read_to_string(&rollout_path)?;
    assert!(
        text.contains("buffered-before-persist"),
        "retry should preserve items buffered before the failed persist"
    );

    recorder.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn writer_state_retries_write_error_before_reporting_flush_success() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let rollout_path = home.path().join("rollout.jsonl");
    File::create(&rollout_path)?;
    let read_only_file = std::fs::OpenOptions::new().read(true).open(&rollout_path)?;
    let mut state = RolloutWriterState::new(
        Some(tokio::fs::File::from_std(read_only_file)),
        /*deferred_log_file_info*/ None,
        /*meta*/ None,
        home.path().to_path_buf(),
        rollout_path.clone(),
        /*state_db_ctx*/ None,
        /*state_builder*/ None,
        config.model_provider_id.clone(),
        config.generate_memories,
    );
    state.add_items(vec![RolloutItem::EventMsg(EventMsg::AgentMessage(
        AgentMessageEvent {
            message: "queued-after-writer-error".to_string(),
            phase: None,
            memory_citation: None,
        },
    ))]);

    state.flush().await?;
    let text_after_retry = std::fs::read_to_string(&rollout_path)?;
    assert!(
        text_after_retry.contains("queued-after-writer-error"),
        "flush should retry after reopening and write buffered items"
    );
    Ok(())
}

#[tokio::test]
async fn metadata_irrelevant_events_touch_state_db_updated_at() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let state_db = StateRuntime::init(home.path().to_path_buf(), config.model_provider_id.clone())
        .await
        .expect("state db should initialize");
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");

    let thread_id = ThreadId::new();
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            thread_id,
            /*forked_from_id*/ None,
            SessionSource::Cli,
            BaseInstructions::default(),
            Vec::new(),
            EventPersistenceMode::Limited,
        ),
        Some(state_db.clone()),
        /*state_builder*/ None,
    )
    .await?;

    recorder
        .record_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(
            UserMessageEvent {
                message: "first-user-message".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        ))])
        .await?;
    recorder.persist().await?;
    recorder.flush().await?;
    let initial_thread = state_db
        .get_thread(thread_id)
        .await
        .expect("thread should load")
        .expect("thread should exist");
    let initial_updated_at = initial_thread.updated_at;
    let initial_title = initial_thread.title.clone();
    let initial_first_user_message = initial_thread.first_user_message.clone();

    tokio::time::sleep(Duration::from_secs(1)).await;

    recorder
        .record_items(&[RolloutItem::EventMsg(EventMsg::AgentMessage(
            AgentMessageEvent {
                message: "assistant text".to_string(),
                phase: None,
                memory_citation: None,
            },
        ))])
        .await?;
    recorder.flush().await?;

    let updated_thread = state_db
        .get_thread(thread_id)
        .await
        .expect("thread should load after agent message")
        .expect("thread should still exist");

    assert!(updated_thread.updated_at > initial_updated_at);
    assert_eq!(updated_thread.title, initial_title);
    assert_eq!(
        updated_thread.first_user_message,
        initial_first_user_message
    );

    recorder.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn metadata_irrelevant_events_fall_back_to_upsert_when_thread_missing() -> std::io::Result<()>
{
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let state_db = StateRuntime::init(home.path().to_path_buf(), config.model_provider_id.clone())
        .await
        .expect("state db should initialize");
    let thread_id = ThreadId::new();
    let rollout_path = home.path().join("rollout.jsonl");
    let builder = ThreadMetadataBuilder::new(
        thread_id,
        rollout_path.clone(),
        Utc::now(),
        SessionSource::Cli,
    );
    let items = vec![RolloutItem::EventMsg(EventMsg::AgentMessage(
        AgentMessageEvent {
            message: "assistant text".to_string(),
            phase: None,
            memory_citation: None,
        },
    ))];

    sync_thread_state_after_write(
        Some(state_db.as_ref()),
        rollout_path.as_path(),
        Some(&builder),
        items.as_slice(),
        config.model_provider_id.as_str(),
        /*new_thread_memory_mode*/ None,
    )
    .await;

    let thread = state_db
        .get_thread(thread_id)
        .await
        .expect("thread should load after fallback")
        .expect("thread should be inserted after fallback");
    assert_eq!(thread.id, thread_id);

    Ok(())
}

#[tokio::test]
async fn list_threads_db_disabled_does_not_skip_paginated_items() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let newest = write_session_file(home.path(), "2025-01-03T12-00-00", Uuid::from_u128(9001))?;
    let middle = write_session_file(home.path(), "2025-01-02T12-00-00", Uuid::from_u128(9002))?;
    let _oldest = write_session_file(home.path(), "2025-01-01T12-00-00", Uuid::from_u128(9003))?;

    let default_provider = config.model_provider_id.clone();
    let page1 = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 1,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        default_provider.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].path, newest);
    let cursor = page1.next_cursor.clone().expect("cursor should be present");

    let page2 = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 1,
        Some(&cursor),
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        default_provider.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].path, middle);
    Ok(())
}

#[tokio::test]
async fn list_threads_db_enabled_drops_missing_rollout_paths() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let uuid = Uuid::from_u128(9010);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let stale_path = home.path().join(format!(
        "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
    ));

    let runtime = codex_state::StateRuntime::init(
        home.path().to_path_buf(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");
    let created_at = chrono::Utc
        .with_ymd_and_hms(2025, 1, 3, 13, 0, 0)
        .single()
        .expect("valid datetime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        stale_path,
        created_at,
        SessionSource::Cli,
    );
    builder.model_provider = Some(config.model_provider_id.clone());
    builder.cwd = home.path().to_path_buf();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.first_user_message = Some("Hello from user".to_string());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("state db upsert should succeed");

    let default_provider = config.model_provider_id.clone();
    let page = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        default_provider.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(page.items.len(), 0);
    let stored_path = runtime
        .find_rollout_path_by_id(thread_id, Some(false))
        .await
        .expect("state db lookup should succeed");
    assert_eq!(stored_path, None);
    Ok(())
}

#[tokio::test]
async fn list_threads_db_enabled_repairs_stale_rollout_paths() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let uuid = Uuid::from_u128(9011);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let real_path = write_session_file(home.path(), "2025-01-03T13-00-00", uuid)?;
    let stale_path = home.path().join(format!(
        "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
    ));

    let runtime = codex_state::StateRuntime::init(
        home.path().to_path_buf(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");
    let created_at = chrono::Utc
        .with_ymd_and_hms(2025, 1, 3, 13, 0, 0)
        .single()
        .expect("valid datetime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        stale_path,
        created_at,
        SessionSource::Cli,
    );
    builder.model_provider = Some(config.model_provider_id.clone());
    builder.cwd = home.path().to_path_buf();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.first_user_message = Some("Hello from user".to_string());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("state db upsert should succeed");

    let default_provider = config.model_provider_id.clone();
    let page = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 1,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        default_provider.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].path, real_path);

    let repaired_path = runtime
        .find_rollout_path_by_id(thread_id, Some(false))
        .await
        .expect("state db lookup should succeed");
    assert_eq!(repaired_path, Some(real_path));
    Ok(())
}

#[tokio::test]
async fn list_threads_state_db_only_skips_jsonl_repair_scan() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let runtime = codex_state::StateRuntime::init(
        home.path().to_path_buf(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");

    let uuid = Uuid::from_u128(9012);
    let ts = "2025-01-03T14-00-00";
    let day_dir = home.path().join("sessions/2025/01/03");
    fs::create_dir_all(&day_dir)?;
    let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
    let mut file = File::create(&path)?;
    let meta = serde_json::json!({
        "timestamp": ts,
        "type": "session_meta",
        "payload": {
            "id": uuid,
            "timestamp": ts,
            "cwd": home.path().display().to_string(),
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "cli",
            "model_provider": "test-provider",
        },
    });
    writeln!(file, "{meta}")?;
    let user_event = serde_json::json!({
        "timestamp": ts,
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": "Hello from user",
            "kind": "plain",
        },
    });
    writeln!(file, "{user_event}")?;

    let cwd_filters = [home.path().to_path_buf()];
    let state_db_only_page = RolloutRecorder::list_threads_from_state_db(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ Some(cwd_filters.as_slice()),
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(state_db_only_page.items.len(), 0);

    let repaired_page = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ Some(cwd_filters.as_slice()),
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(repaired_page.items.len(), 1);

    let repaired_state_db_only_page = RolloutRecorder::list_threads_from_state_db(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ Some(cwd_filters.as_slice()),
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(repaired_state_db_only_page.items.len(), 1);
    Ok(())
}

#[tokio::test]
async fn list_threads_default_filter_returns_filesystem_scan_results() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let uuid = Uuid::from_u128(9013);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let real_path = write_session_file(home.path(), "2025-01-03T13-00-00", uuid)?;
    let stale_cwd = home.path().join("stale-cwd");

    let runtime = codex_state::StateRuntime::init(
        home.path().to_path_buf(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");
    let created_at = chrono::Utc
        .with_ymd_and_hms(2025, 1, 3, 13, 0, 0)
        .single()
        .expect("valid datetime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        real_path,
        created_at,
        SessionSource::Cli,
    );
    builder.model_provider = Some(config.model_provider_id.clone());
    builder.cwd = stale_cwd.clone();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.first_user_message = Some("Hello from user".to_string());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("state db upsert should succeed");

    let cwd_filters = [stale_cwd];
    let state_db_only_page = RolloutRecorder::list_threads_from_state_db(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ Some(cwd_filters.as_slice()),
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(state_db_only_page.items.len(), 1);

    let scanned_page = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ Some(cwd_filters.as_slice()),
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(scanned_page.items.len(), 0);

    let repaired_state_db_only_page = RolloutRecorder::list_threads_from_state_db(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ Some(cwd_filters.as_slice()),
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;
    assert_eq!(repaired_state_db_only_page.items.len(), 0);
    Ok(())
}

#[tokio::test]
async fn list_threads_metadata_filter_overlays_state_db_list_metadata() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let uuid = Uuid::from_u128(9015);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let rollout_path = write_session_file(home.path(), "2025-01-03T16-00-00", uuid)?;

    let runtime = codex_state::StateRuntime::init(
        home.path().to_path_buf(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");
    let created_at = chrono::Utc
        .with_ymd_and_hms(2025, 1, 3, 16, 0, 0)
        .single()
        .expect("valid datetime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        rollout_path,
        created_at,
        SessionSource::Cli,
    );
    builder.model_provider = Some(config.model_provider_id.clone());
    builder.cwd = home.path().to_path_buf();
    builder.git_branch = Some("sqlite-branch".to_string());
    builder.git_sha = Some("sqlite-sha".to_string());
    builder.git_origin_url = Some("https://example.com/repo.git".to_string());
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.first_user_message = Some("Hello from user".to_string());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("state db upsert should succeed");

    let page = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[SessionSource::Cli],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        config.model_provider_id.as_str(),
        /*search_term*/ None,
    )
    .await?;

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].git_branch.as_deref(), Some("sqlite-branch"));
    assert_eq!(page.items[0].git_sha.as_deref(), Some("sqlite-sha"));
    assert_eq!(
        page.items[0].git_origin_url.as_deref(),
        Some("https://example.com/repo.git")
    );
    Ok(())
}

#[test]
fn fill_missing_thread_item_metadata_preserves_filesystem_identity() {
    let filesystem_thread_id = ThreadId::new();
    let state_thread_id = ThreadId::new();
    let filesystem_path = PathBuf::from("/tmp/filesystem-rollout.jsonl");
    let state_path = PathBuf::from("/tmp/state-rollout.jsonl");
    let mut item = ThreadItem {
        path: filesystem_path.clone(),
        thread_id: Some(filesystem_thread_id),
        first_user_message: Some("filesystem message".to_string()),
        cwd: None,
        git_branch: None,
        git_sha: None,
        git_origin_url: None,
        source: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: None,
        cli_version: None,
        created_at: None,
        updated_at: None,
    };
    let state_item = ThreadItem {
        path: state_path,
        thread_id: Some(state_thread_id),
        first_user_message: Some("state message".to_string()),
        cwd: Some(PathBuf::from("/tmp/state-cwd")),
        git_branch: Some("state-branch".to_string()),
        git_sha: Some("state-sha".to_string()),
        git_origin_url: Some("https://example.com/state.git".to_string()),
        source: Some(SessionSource::Exec),
        agent_nickname: Some("state-agent".to_string()),
        agent_role: Some("state-role".to_string()),
        model_provider: Some("state-provider".to_string()),
        cli_version: Some("state-version".to_string()),
        created_at: Some("2025-01-03T16:00:00Z".to_string()),
        updated_at: Some("2025-01-03T16:01:02.003Z".to_string()),
    };

    fill_missing_thread_item_metadata(&mut item, state_item);

    assert_eq!(item.path, filesystem_path);
    assert_eq!(item.thread_id, Some(filesystem_thread_id));
    assert_eq!(
        item.first_user_message.as_deref(),
        Some("filesystem message")
    );
    assert_eq!(item.cwd.as_deref(), Some(Path::new("/tmp/state-cwd")));
    assert_eq!(item.git_branch.as_deref(), Some("state-branch"));
    assert_eq!(item.git_sha.as_deref(), Some("state-sha"));
    assert_eq!(
        item.git_origin_url.as_deref(),
        Some("https://example.com/state.git")
    );
    assert_eq!(item.source, Some(SessionSource::Exec));
    assert_eq!(item.agent_nickname.as_deref(), Some("state-agent"));
    assert_eq!(item.agent_role.as_deref(), Some("state-role"));
    assert_eq!(item.model_provider.as_deref(), Some("state-provider"));
    assert_eq!(item.cli_version.as_deref(), Some("state-version"));
    assert_eq!(item.created_at.as_deref(), Some("2025-01-03T16:00:00Z"));
    assert_eq!(item.updated_at.as_deref(), Some("2025-01-03T16:01:02.003Z"));
}

#[tokio::test]
async fn list_threads_search_repairs_stale_state_db_hits_before_returning() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());

    let uuid = Uuid::from_u128(9014);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let real_path = write_session_file(home.path(), "2025-01-03T15-00-00", uuid)?;

    let runtime = codex_state::StateRuntime::init(
        home.path().to_path_buf(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be complete");
    let created_at = chrono::Utc
        .with_ymd_and_hms(2025, 1, 3, 15, 0, 0)
        .single()
        .expect("valid datetime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        real_path,
        created_at,
        SessionSource::Cli,
    );
    builder.model_provider = Some(config.model_provider_id.clone());
    builder.cwd = home.path().to_path_buf();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.title = "needle stale title".to_string();
    metadata.first_user_message = Some("stale first user".to_string());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("state db upsert should succeed");

    let stale_state_db_only_page = RolloutRecorder::list_threads_from_state_db(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        config.model_provider_id.as_str(),
        Some("needle"),
    )
    .await?;
    assert_eq!(stale_state_db_only_page.items.len(), 1);

    let scanned_page = RolloutRecorder::list_threads(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        config.model_provider_id.as_str(),
        Some("needle"),
    )
    .await?;
    assert_eq!(scanned_page.items.len(), 0);

    let repaired_state_db_only_page = RolloutRecorder::list_threads_from_state_db(
        &config,
        /*page_size*/ 10,
        /*cursor*/ None,
        ThreadSortKey::CreatedAt,
        SortDirection::Desc,
        &[],
        /*model_providers*/ None,
        /*cwd_filters*/ None,
        config.model_provider_id.as_str(),
        Some("needle"),
    )
    .await?;
    assert_eq!(repaired_state_db_only_page.items.len(), 0);
    Ok(())
}

#[tokio::test]
async fn resume_candidate_matches_cwd_reads_latest_turn_context() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let stale_cwd = home.path().join("stale");
    let latest_cwd = home.path().join("latest");
    fs::create_dir_all(&stale_cwd)?;
    fs::create_dir_all(&latest_cwd)?;

    let path = write_session_file(home.path(), "2025-01-03T13-00-00", Uuid::from_u128(9012))?;
    let mut file = std::fs::OpenOptions::new().append(true).open(&path)?;
    let turn_context = RolloutLine {
        timestamp: "2025-01-03T13:00:01Z".to_string(),
        item: RolloutItem::TurnContext(TurnContextItem {
            turn_id: Some("turn-1".to_string()),
            trace_id: None,
            cwd: latest_cwd.clone(),
            current_date: None,
            timezone: None,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            network: None,
            file_system_sandbox_policy: None,
            model: "test-model".to_string(),
            personality: None,
            collaboration_mode: None,
            realtime_active: None,
            effort: None,
            summary: ReasoningSummaryConfig::Auto,
            user_instructions: None,
            developer_instructions: None,
            final_output_json_schema: None,
            truncation_policy: None,
        }),
    };
    writeln!(file, "{}", serde_json::to_string(&turn_context)?)?;

    assert!(
        resume_candidate_matches_cwd(
            path.as_path(),
            Some(stale_cwd.as_path()),
            latest_cwd.as_path(),
            "test-provider",
        )
        .await
    );
    Ok(())
}
