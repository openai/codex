use super::*;
use crate::config::ConfigBuilder;
use chrono::TimeZone;
use codex_features::Feature;
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
