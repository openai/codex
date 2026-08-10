use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use codex_rollout::RolloutRecorder;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::super::publish::migration_journal_path;
use super::super::publish::write_migration_journal;
use super::super::thread_history;
use super::LocalThreadStore;
use crate::local::test_support::test_config;

const TIMESTAMP: &str = "2025-01-03T12:00:00Z";

fn write_rollout(home: &Path, thread_id: ThreadId, history_mode: ThreadHistoryMode) -> PathBuf {
    let directory = home.join("sessions/2025/01/03");
    fs::create_dir_all(&directory).expect("create rollout directory");
    let path = directory.join(format!("rollout-2025-01-03T12-00-00-{thread_id}.jsonl"));
    let mut file = fs::File::create(&path).expect("create legacy rollout");
    let paginated = history_mode == ThreadHistoryMode::Paginated;
    let metadata = SessionMeta {
        session_id: thread_id.into(),
        id: thread_id,
        timestamp: TIMESTAMP.to_string(),
        cwd: home.to_path_buf(),
        originator: "test-originator".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::Cli,
        model_provider: Some("test-provider".to_string()),
        history_mode,
        ..SessionMeta::default()
    };
    for (ordinal, item) in [
        RolloutItem::SessionMeta(SessionMetaLine {
            meta: metadata,
            git: None,
        }),
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "question".to_string(),
            ..UserMessageEvent::default()
        })),
    ]
    .into_iter()
    .enumerate()
    {
        let line = RolloutLine {
            timestamp: TIMESTAMP.to_string(),
            ordinal: paginated.then_some(ordinal as u64),
            item,
        };
        writeln!(
            file,
            "{}",
            serde_json::to_string(&line).expect("serialize legacy record")
        )
        .expect("write legacy record");
    }
    path
}

fn move_to_timestamp(
    home: &Path,
    path: PathBuf,
    session_day: &str,
    filename_timestamp: &str,
) -> PathBuf {
    let directory = home.join(format!("sessions/{session_day}"));
    fs::create_dir_all(&directory).expect("create rollout directory");
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".jsonl"))
        .expect("rollout filename");
    let thread_id = stem
        .get(stem.len().checked_sub(36).expect("thread id offset")..)
        .expect("thread id in rollout filename");
    let moved_path = directory.join(format!("rollout-{filename_timestamp}-{thread_id}.jsonl"));
    fs::rename(path, &moved_path).expect("move rollout timestamp");
    moved_path
}

async fn indexed_store(home: &Path) -> LocalThreadStore {
    let config = test_config(home);
    let rollout_config = RolloutConfig {
        codex_home: config.codex_home.clone(),
        sqlite: config.sqlite.clone(),
        cwd: home.to_path_buf(),
        model_provider_id: config.default_model_provider_id.clone(),
        generate_memories: false,
    };
    let state_db = codex_rollout::state_db::try_init(&rollout_config)
        .await
        .expect("backfill legacy thread metadata");
    LocalThreadStore::new(config, Some(state_db))
}

#[tokio::test]
async fn records_and_advances_checked_thread() {
    let home = TempDir::new().expect("create Codex home");
    let legacy_thread_id = ThreadId::new();
    let legacy_path = write_rollout(home.path(), legacy_thread_id, ThreadHistoryMode::Legacy);
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts_on_startup()
        .await
        .expect("migrate startup rollouts");
    assert_eq!(
        codex_rollout::read_session_meta_line(&legacy_path)
            .await
            .expect("read migrated metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );

    let newer_thread_id = ThreadId::new();
    move_to_timestamp(
        home.path(),
        write_rollout(home.path(), newer_thread_id, ThreadHistoryMode::Paginated),
        "2025/01/04",
        "2025-01-04T12-00-00",
    );
    store
        .migrate_rollouts_on_startup()
        .await
        .expect("check newer paginated rollout");

    let state = store
        .state_db()
        .await
        .expect("state db")
        .get_rollout_migration_state(super::LEGACY_TO_PAGINATED_MIGRATION_ID)
        .await
        .expect("read migration state")
        .expect("migration state");
    assert_eq!(
        state
            .last_checked_thread
            .expect("last checked thread")
            .thread_id,
        newer_thread_id.to_string()
    );
}

#[tokio::test]
async fn checks_rollouts_within_the_cursor_lookback() {
    let home = TempDir::new().expect("create Codex home");
    let older_thread_id = ThreadId::new();
    let older_path = move_to_timestamp(
        home.path(),
        write_rollout(home.path(), older_thread_id, ThreadHistoryMode::Legacy),
        "2025/01/02",
        "2025-01-02T12-00-00",
    );
    let newer_path = move_to_timestamp(
        home.path(),
        write_rollout(home.path(), ThreadId::new(), ThreadHistoryMode::Paginated),
        "2025/01/03",
        "2025-01-03T12-00-00",
    );
    let store = indexed_store(home.path()).await;
    let cursor = super::thread_creation_cursor(&newer_path).expect("newer rollout cursor");
    store
        .state_db()
        .await
        .expect("state db")
        .advance_rollout_migration_state(super::LEGACY_TO_PAGINATED_MIGRATION_ID, Some(&cursor))
        .await
        .expect("seed migration cursor");

    store
        .migrate_rollouts_on_startup()
        .await
        .expect("check rollout behind cursor");

    assert_eq!(
        codex_rollout::read_session_meta_line(&older_path)
            .await
            .expect("read migrated metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );
}

#[tokio::test]
async fn recovers_pending_migrations_behind_the_checked_thread() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(home.path(), thread_id, ThreadHistoryMode::Legacy);
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts_on_startup()
        .await
        .expect("migrate and advance startup cursor");
    thread_history::delete_thread(&store, thread_id)
        .await
        .expect("simulate missing projection");
    let journal_path = migration_journal_path(home.path(), thread_id);
    write_migration_journal(&journal_path)
        .await
        .expect("simulate pending migration marker");

    store
        .migrate_rollouts_on_startup()
        .await
        .expect("recover pending migration behind cursor");

    assert!(!journal_path.exists());
    assert!(
        thread_history::projection_state(&store, thread_id)
            .await
            .expect("read repaired projection")
            .is_some()
    );
    assert_eq!(
        codex_rollout::read_session_meta_line(&path)
            .await
            .expect("read migrated metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );
}

#[tokio::test]
async fn waits_for_a_live_writer_before_migrating() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(home.path(), thread_id, ThreadHistoryMode::Legacy);
    let store = indexed_store(home.path()).await;
    let live_writer_guard = store.live_writer_locks.lock(thread_id).await;
    let migration_store = store.clone();
    let mut migration = tokio::spawn(async move {
        migration_store
            .migrate_rollouts_on_startup()
            .await
            .expect("migrate startup rollouts");
    });

    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut migration)
            .await
            .is_err(),
        "migration should wait for the live writer"
    );
    drop(live_writer_guard);
    migration.await.expect("join startup migration");

    assert_eq!(
        codex_rollout::read_session_meta_line(&path)
            .await
            .expect("read migrated metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );
}

#[tokio::test]
async fn rechecks_changed_empty_rollouts() {
    let home = TempDir::new().expect("create Codex home");
    write_rollout(home.path(), ThreadId::new(), ThreadHistoryMode::Legacy);
    let empty_thread_id = ThreadId::new();
    let empty_path = move_to_timestamp(
        home.path(),
        write_rollout(home.path(), empty_thread_id, ThreadHistoryMode::Legacy),
        "2025/01/04",
        "2025-01-04T12-00-00",
    );
    let restored_contents = fs::read(&empty_path).expect("read rollout before emptying");
    fs::write(&empty_path, []).expect("empty rollout");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts_on_startup()
        .await
        .expect("record empty rollout");
    fs::write(&empty_path, restored_contents).expect("restore rollout");
    let (items, _, _) = RolloutRecorder::load_rollout_items(&empty_path)
        .await
        .expect("load restored rollout");
    let metadata = codex_rollout::builder_from_items(items.as_slice(), &empty_path)
        .expect("build restored metadata")
        .build("test-provider");
    store
        .state_db()
        .await
        .expect("state db")
        .upsert_thread(&metadata)
        .await
        .expect("seed restored metadata");

    store
        .migrate_rollouts_on_startup()
        .await
        .expect("migrate changed rollout");

    assert_eq!(
        codex_rollout::read_session_meta_line(&empty_path)
            .await
            .expect("read migrated metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );
    assert!(
        store
            .state_db()
            .await
            .expect("state db")
            .list_rollout_migration_skipped_rollouts(super::LEGACY_TO_PAGINATED_MIGRATION_ID)
            .await
            .expect("read skipped rollouts")
            .is_empty()
    );
}
