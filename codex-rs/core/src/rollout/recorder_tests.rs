use super::*;
use crate::config::ConfigBuilder;
use chrono::TimeZone;
use codex_features::Feature;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::fs::{self};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

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

fn test_rollout_agent_message(message: &str) -> RolloutItem {
    let mut events = AgentMessageItem::new(&[AgentMessageContent::Text {
        text: message.to_string(),
    }])
    .as_legacy_events();
    RolloutItem::EventMsg(events.pop().expect("single agent message event"))
}

async fn queue_test_rollout_agent_message(
    tx: &mpsc::Sender<RolloutCmd>,
    message: &str,
) -> Result<(), mpsc::error::SendError<RolloutCmd>> {
    tx.send(RolloutCmd::AddItems(vec![test_rollout_agent_message(
        message,
    )]))
    .await
}

async fn flush_test_rollout_writer(tx: &mpsc::Sender<RolloutCmd>) -> std::io::Result<()> {
    let (flush_tx, flush_rx) = oneshot::channel();
    tx.send(RolloutCmd::Flush { ack: flush_tx })
        .await
        .map_err(|e| IoError::other(format!("flush should queue: {e}")))?;
    flush_rx
        .await
        .map_err(|e| IoError::other(format!("flush ack should be sent: {e}")))?
}

async fn open_read_only_rollout_writer(path: &Path) -> std::io::Result<JsonlWriter> {
    std::fs::write(path, "")?;
    let file = tokio::fs::OpenOptions::new().read(true).open(path).await?;
    Ok(JsonlWriter::new(file))
}

fn assert_rollout_message_count(text: &str, message: &str, expected: usize) {
    assert_eq!(
        text.matches(message).count(),
        expected,
        "unexpected rollout count for {message:?}"
    );
}

#[tokio::test]
async fn recorder_materializes_only_after_explicit_persist() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let thread_id = ThreadId::new();
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            thread_id,
            None,
            SessionSource::Exec,
            BaseInstructions::default(),
            Vec::new(),
            EventPersistenceMode::Limited,
        ),
        None,
        None,
    )
    .await?;

    let rollout_path = recorder.rollout_path().to_path_buf();
    assert!(
        !rollout_path.exists(),
        "rollout file should not exist before first user message"
    );

    recorder
        .record_items(&[test_rollout_agent_message("buffered-event")])
        .await?;
    recorder.flush().await?;
    assert!(
        !rollout_path.exists(),
        "rollout file should remain deferred before first user message"
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
    assert!(
        !rollout_path.exists(),
        "user-message-like items should not materialize without explicit persist"
    );

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
async fn rollout_writer_reopens_after_initial_write_error_and_retries_pending_items()
-> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let rollout_path = home.path().join("rollout.jsonl");
    let writer = open_read_only_rollout_writer(&rollout_path).await?;
    let (tx, rx) = mpsc::channel::<RolloutCmd>(256);
    let writer_task = tokio::spawn(rollout_writer(
        Some(writer),
        None,
        rx,
        None,
        home.path().to_path_buf(),
        rollout_path.clone(),
        None,
        None,
        "test-provider".to_string(),
        false,
    ));

    queue_test_rollout_agent_message(&tx, "first-write-fails")
        .await
        .expect("first write should queue");
    queue_test_rollout_agent_message(&tx, "second-write-succeeds")
        .await
        .expect("second write should queue after reopen");
    flush_test_rollout_writer(&tx).await?;

    drop(tx);

    let text = std::fs::read_to_string(&rollout_path)?;
    assert!(
        text.contains("second-write-succeeds"),
        "expected the message sent after a failure to show up in the rollout"
    );
    assert!(
        text.contains("first-write-fails"),
        "expected the message from the failed batch to be retried into the rollout"
    );

    writer_task
        .await
        .expect("writer task should join cleanly")
        .expect("writer task should exit cleanly after channel closes");

    Ok(())
}

#[tokio::test]
async fn recorder_retries_persist_after_materialization_failure() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            ThreadId::new(),
            None,
            SessionSource::Exec,
            BaseInstructions::default(),
            Vec::new(),
            EventPersistenceMode::Limited,
        ),
        None,
        None,
    )
    .await?;
    let rollout_path = recorder.rollout_path().to_path_buf();
    std::fs::create_dir_all(
        rollout_path
            .parent()
            .expect("rollout path should have a parent directory"),
    )?;
    std::fs::create_dir(&rollout_path)?;

    recorder
        .record_items(&[test_rollout_agent_message("buffered-before-failure")])
        .await?;
    let persist_error = recorder
        .persist()
        .await
        .expect_err("materialization should fail while the rollout path is a directory");
    assert!(
        persist_error.to_string().contains("Is a directory"),
        "expected a real open failure, got: {persist_error}"
    );

    std::fs::remove_dir(&rollout_path)?;

    recorder
        .record_items(&[test_rollout_agent_message("buffered-after-failure")])
        .await?;
    recorder.persist().await?;
    recorder.flush().await?;

    let text = std::fs::read_to_string(&rollout_path)?;
    assert_rollout_message_count(&text, "buffered-before-failure", 1);
    assert_rollout_message_count(&text, "buffered-after-failure", 1);

    recorder.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn metadata_irrelevant_events_touch_state_db_updated_at() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    config
        .features
        .enable(Feature::Sqlite)
        .expect("test config should allow sqlite");

    let state_db = StateRuntime::init(home.path().to_path_buf(), config.model_provider_id.clone())
        .await
        .expect("state db should initialize");
    state_db
        .mark_backfill_complete(None)
        .await
        .expect("backfill should be complete");

    let thread_id = ThreadId::new();
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            thread_id,
            None,
            SessionSource::Cli,
            BaseInstructions::default(),
            Vec::new(),
            EventPersistenceMode::Limited,
        ),
        Some(state_db.clone()),
        None,
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
        .record_items(&[test_rollout_agent_message("assistant text")])
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
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    config
        .features
        .enable(Feature::Sqlite)
        .expect("test config should allow sqlite");

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
    let items = vec![test_rollout_agent_message("assistant text")];

    sync_thread_state_after_write(
        Some(state_db.as_ref()),
        rollout_path.as_path(),
        Some(&builder),
        items.as_slice(),
        config.model_provider_id.as_str(),
        None,
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
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    config
        .features
        .disable(Feature::Sqlite)
        .expect("test config should allow sqlite to be disabled");

    let newest = write_session_file(home.path(), "2025-01-03T12-00-00", Uuid::from_u128(9001))?;
    let middle = write_session_file(home.path(), "2025-01-02T12-00-00", Uuid::from_u128(9002))?;
    let _oldest = write_session_file(home.path(), "2025-01-01T12-00-00", Uuid::from_u128(9003))?;

    let default_provider = config.model_provider_id.clone();
    let page1 = RolloutRecorder::list_threads(
        &config,
        1,
        None,
        ThreadSortKey::CreatedAt,
        &[],
        None,
        default_provider.as_str(),
        None,
    )
    .await?;
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].path, newest);
    let cursor = page1.next_cursor.clone().expect("cursor should be present");

    let page2 = RolloutRecorder::list_threads(
        &config,
        1,
        Some(&cursor),
        ThreadSortKey::CreatedAt,
        &[],
        None,
        default_provider.as_str(),
        None,
    )
    .await?;
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].path, middle);
    Ok(())
}

#[tokio::test]
async fn list_threads_db_enabled_drops_missing_rollout_paths() -> std::io::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    config
        .features
        .enable(Feature::Sqlite)
        .expect("test config should allow sqlite");

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
        .mark_backfill_complete(None)
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
        10,
        None,
        ThreadSortKey::CreatedAt,
        &[],
        None,
        default_provider.as_str(),
        None,
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
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    config
        .features
        .enable(Feature::Sqlite)
        .expect("test config should allow sqlite");

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
        .mark_backfill_complete(None)
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
        1,
        None,
        ThreadSortKey::CreatedAt,
        &[],
        None,
        default_provider.as_str(),
        None,
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
            network: None,
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
