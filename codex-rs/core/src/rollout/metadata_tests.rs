use super::*;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_state::BackfillStatus;
use codex_state::ThreadMetadataBuilder;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn extract_metadata_from_rollout_uses_session_meta() {
    let dir = tempdir().expect("tempdir");
    let uuid = Uuid::new_v4();
    let id = ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let path = dir
        .path()
        .join(format!("rollout-2026-01-27T12-34-56-{uuid}.jsonl"));

    let session_meta = SessionMeta {
        id,
        forked_from_id: None,
        timestamp: "2026-01-27T12:34:56Z".to_string(),
        cwd: dir.path().to_path_buf(),
        originator: "cli".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::default(),
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: Some("openai".to_string()),
        base_instructions: None,
        dynamic_tools: None,
        memory_mode: None,
        approved_command_prefixes: None,
    };
    let session_meta_line = SessionMetaLine {
        meta: session_meta,
        git: None,
    };
    let rollout_line = RolloutLine {
        timestamp: "2026-01-27T12:34:56Z".to_string(),
        item: RolloutItem::SessionMeta(session_meta_line.clone()),
    };
    let json = serde_json::to_string(&rollout_line).expect("rollout json");
    let mut file = File::create(&path).expect("create rollout");
    writeln!(file, "{json}").expect("write rollout");

    let outcome = extract_metadata_from_rollout(&path, "openai")
        .await
        .expect("extract");

    let builder = builder_from_session_meta(&session_meta_line, path.as_path()).expect("builder");
    let mut expected = builder.build("openai");
    apply_rollout_item(&mut expected, &rollout_line.item, "openai");
    expected.updated_at = file_modified_time_utc(&path).await.expect("mtime");

    assert_eq!(outcome.metadata, expected);
    assert_eq!(outcome.memory_mode, None);
    assert_eq!(outcome.parse_errors, 0);
}

#[tokio::test]
async fn extract_metadata_from_rollout_returns_latest_memory_mode() {
    let dir = tempdir().expect("tempdir");
    let uuid = Uuid::new_v4();
    let id = ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let path = dir
        .path()
        .join(format!("rollout-2026-01-27T12-34-56-{uuid}.jsonl"));

    let session_meta = SessionMeta {
        id,
        forked_from_id: None,
        timestamp: "2026-01-27T12:34:56Z".to_string(),
        cwd: dir.path().to_path_buf(),
        originator: "cli".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::default(),
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: Some("openai".to_string()),
        base_instructions: None,
        dynamic_tools: None,
        memory_mode: None,
        approved_command_prefixes: None,
    };
    let polluted_meta = SessionMeta {
        memory_mode: Some("polluted".to_string()),
        approved_command_prefixes: None,
        ..session_meta.clone()
    };
    let lines = vec![
        RolloutLine {
            timestamp: "2026-01-27T12:34:56Z".to_string(),
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta,
                git: None,
            }),
        },
        RolloutLine {
            timestamp: "2026-01-27T12:35:00Z".to_string(),
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: polluted_meta,
                git: None,
            }),
        },
    ];
    let mut file = File::create(&path).expect("create rollout");
    for line in lines {
        writeln!(
            file,
            "{}",
            serde_json::to_string(&line).expect("serialize rollout line")
        )
        .expect("write rollout line");
    }

    let outcome = extract_metadata_from_rollout(&path, "openai")
        .await
        .expect("extract");

    assert_eq!(outcome.memory_mode.as_deref(), Some("polluted"));
}

#[test]
fn builder_from_items_falls_back_to_filename() {
    let dir = tempdir().expect("tempdir");
    let uuid = Uuid::new_v4();
    let path = dir
        .path()
        .join(format!("rollout-2026-01-27T12-34-56-{uuid}.jsonl"));
    let items = vec![RolloutItem::Compacted(CompactedItem {
        message: "noop".to_string(),
        replacement_history: None,
    })];

    let builder = builder_from_items(items.as_slice(), path.as_path()).expect("builder");
    let naive = NaiveDateTime::parse_from_str("2026-01-27T12-34-56", "%Y-%m-%dT%H-%M-%S")
        .expect("timestamp");
    let created_at = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
        .with_nanosecond(0)
        .expect("nanosecond");
    let expected = ThreadMetadataBuilder::new(
        ThreadId::from_string(&uuid.to_string()).expect("thread id"),
        path,
        created_at,
        SessionSource::default(),
    );

    assert_eq!(builder, expected);
}

#[tokio::test]
async fn backfill_sessions_resumes_from_watermark_and_marks_complete() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().to_path_buf();
    let first_uuid = Uuid::new_v4();
    let second_uuid = Uuid::new_v4();
    let first_path = write_rollout_in_sessions(
        codex_home.as_path(),
        "2026-01-27T12-34-56",
        "2026-01-27T12:34:56Z",
        first_uuid,
        None,
    );
    let second_path = write_rollout_in_sessions(
        codex_home.as_path(),
        "2026-01-27T12-35-56",
        "2026-01-27T12:35:56Z",
        second_uuid,
        None,
    );

    let runtime = codex_state::StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");
    let first_watermark = backfill_watermark_for_path(codex_home.as_path(), first_path.as_path());
    runtime.mark_backfill_running().await.expect("mark running");
    runtime
        .checkpoint_backfill(first_watermark.as_str())
        .await
        .expect("checkpoint first watermark");
    tokio::time::sleep(std::time::Duration::from_secs(
        (BACKFILL_LEASE_SECONDS + 1) as u64,
    ))
    .await;

    let mut config = crate::config::test_config();
    config.codex_home = codex_home.clone();
    config.model_provider_id = "test-provider".to_string();
    backfill_sessions(runtime.as_ref(), &config).await;

    let first_id = ThreadId::from_string(&first_uuid.to_string()).expect("first thread id");
    let second_id = ThreadId::from_string(&second_uuid.to_string()).expect("second thread id");
    assert_eq!(
        runtime
            .get_thread(first_id)
            .await
            .expect("get first thread"),
        None
    );
    assert!(
        runtime
            .get_thread(second_id)
            .await
            .expect("get second thread")
            .is_some()
    );

    let state = runtime
        .get_backfill_state()
        .await
        .expect("get backfill state");
    assert_eq!(state.status, BackfillStatus::Complete);
    assert_eq!(
        state.last_watermark,
        Some(backfill_watermark_for_path(
            codex_home.as_path(),
            second_path.as_path()
        ))
    );
    assert!(state.last_success_at.is_some());
}

#[tokio::test]
async fn backfill_sessions_preserves_existing_git_branch_and_fills_missing_git_fields() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().to_path_buf();
    let thread_uuid = Uuid::new_v4();
    let rollout_path = write_rollout_in_sessions(
        codex_home.as_path(),
        "2026-01-27T12-34-56",
        "2026-01-27T12:34:56Z",
        thread_uuid,
        Some(GitInfo {
            commit_hash: Some("rollout-sha".to_string()),
            branch: Some("rollout-branch".to_string()),
            repository_url: Some("git@example.com:openai/codex.git".to_string()),
        }),
    );

    let runtime = codex_state::StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");
    let thread_id = ThreadId::from_string(&thread_uuid.to_string()).expect("thread id");
    let mut existing = extract_metadata_from_rollout(&rollout_path, "test-provider")
        .await
        .expect("extract")
        .metadata;
    existing.git_sha = None;
    existing.git_branch = Some("sqlite-branch".to_string());
    existing.git_origin_url = None;
    runtime
        .upsert_thread(&existing)
        .await
        .expect("existing metadata upsert");

    let mut config = crate::config::test_config();
    config.codex_home = codex_home.clone();
    config.model_provider_id = "test-provider".to_string();
    backfill_sessions(runtime.as_ref(), &config).await;

    let persisted = runtime
        .get_thread(thread_id)
        .await
        .expect("get thread")
        .expect("thread exists");
    assert_eq!(persisted.git_sha.as_deref(), Some("rollout-sha"));
    assert_eq!(persisted.git_branch.as_deref(), Some("sqlite-branch"));
    assert_eq!(
        persisted.git_origin_url.as_deref(),
        Some("git@example.com:openai/codex.git")
    );
}

#[tokio::test]
async fn backfill_sessions_normalizes_cwd_before_upsert() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().to_path_buf();
    let thread_uuid = Uuid::new_v4();
    let session_cwd = codex_home.join(".");
    let rollout_path = write_rollout_in_sessions_with_cwd(
        codex_home.as_path(),
        "2026-01-27T12-34-56",
        "2026-01-27T12:34:56Z",
        thread_uuid,
        session_cwd.clone(),
        None,
    );

    let runtime = codex_state::StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");

    let mut config = crate::config::test_config();
    config.codex_home = codex_home.clone();
    config.model_provider_id = "test-provider".to_string();
    backfill_sessions(runtime.as_ref(), &config).await;

    let thread_id = ThreadId::from_string(&thread_uuid.to_string()).expect("thread id");
    let stored = runtime
        .get_thread(thread_id)
        .await
        .expect("get thread")
        .expect("thread should be backfilled");

    assert_eq!(stored.rollout_path, rollout_path);
    assert_eq!(stored.cwd, normalize_cwd_for_state_db(&session_cwd));
}

fn write_rollout_in_sessions(
    codex_home: &Path,
    filename_ts: &str,
    event_ts: &str,
    thread_uuid: Uuid,
    git: Option<GitInfo>,
) -> PathBuf {
    write_rollout_in_sessions_with_cwd(
        codex_home,
        filename_ts,
        event_ts,
        thread_uuid,
        codex_home.to_path_buf(),
        git,
    )
}

fn write_rollout_in_sessions_with_cwd(
    codex_home: &Path,
    filename_ts: &str,
    event_ts: &str,
    thread_uuid: Uuid,
    cwd: PathBuf,
    git: Option<GitInfo>,
) -> PathBuf {
    let id = ThreadId::from_string(&thread_uuid.to_string()).expect("thread id");
    let sessions_dir = codex_home.join("sessions");
    std::fs::create_dir_all(sessions_dir.as_path()).expect("create sessions dir");
    let path = sessions_dir.join(format!("rollout-{filename_ts}-{thread_uuid}.jsonl"));
    let session_meta = SessionMeta {
        id,
        forked_from_id: None,
        timestamp: event_ts.to_string(),
        cwd,
        originator: "cli".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::default(),
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: Some("test-provider".to_string()),
        base_instructions: None,
        dynamic_tools: None,
        memory_mode: None,
        approved_command_prefixes: None,
    };
    let session_meta_line = SessionMetaLine {
        meta: session_meta,
        git,
    };
    let rollout_line = RolloutLine {
        timestamp: event_ts.to_string(),
        item: RolloutItem::SessionMeta(session_meta_line),
    };
    let json = serde_json::to_string(&rollout_line).expect("serialize rollout");
    let mut file = File::create(&path).expect("create rollout");
    writeln!(file, "{json}").expect("write rollout");
    path
}
