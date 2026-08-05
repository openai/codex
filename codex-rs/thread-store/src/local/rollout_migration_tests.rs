use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::RolloutConfig;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::LocalThreadStore;
use super::RolloutMigrationMode;
use super::RolloutMigrationOptions;
use super::RolloutMigrationStatus;
#[cfg(unix)]
use super::decompress_rollout_to_path;
use super::migration_journal_path;
use super::thread_history;
use super::write_migration_journal;
use crate::ListTurnsParams;
use crate::SortDirection;
use crate::StoredTurnItemsView;
use crate::local::test_support::test_config;

const TIMESTAMP: &str = "2025-01-03T12:00:00Z";

fn write_rollout(
    home: &Path,
    thread_id: ThreadId,
    source: SessionSource,
    items: Vec<RolloutItem>,
) -> PathBuf {
    write_rollout_with_fork(home, thread_id, source, /*forked_from_id*/ None, items)
}

fn write_rollout_with_fork(
    home: &Path,
    thread_id: ThreadId,
    source: SessionSource,
    forked_from_id: Option<ThreadId>,
    items: Vec<RolloutItem>,
) -> PathBuf {
    let directory = home.join("sessions/2025/01/03");
    fs::create_dir_all(&directory).expect("create rollout directory");
    let path = directory.join(format!("rollout-2025-01-03T12-00-00-{thread_id}.jsonl"));
    let mut file = fs::File::create(&path).expect("create legacy rollout");
    let metadata = SessionMeta {
        session_id: thread_id.into(),
        id: thread_id,
        forked_from_id,
        timestamp: TIMESTAMP.to_string(),
        cwd: home.to_path_buf(),
        originator: "test-originator".to_string(),
        cli_version: "0.0.0".to_string(),
        source,
        model_provider: Some("test-provider".to_string()),
        ..SessionMeta::default()
    };
    let items = std::iter::once(RolloutItem::SessionMeta(SessionMetaLine {
        meta: metadata,
        git: None,
    }))
    .chain(items);
    for item in items {
        let line = RolloutLine {
            timestamp: TIMESTAMP.to_string(),
            ordinal: None,
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

fn user_message(text: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        message: text.to_string(),
        ..UserMessageEvent::default()
    }))
}

fn agent_message(text: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
        message: text.to_string(),
        phase: None,
        memory_citation: None,
    }))
}

fn read_rollout(path: &Path) -> Vec<RolloutLine> {
    fs::read_to_string(path)
        .expect("read migrated rollout")
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse migrated rollout"))
        .collect()
}

fn compress_rollout(path: PathBuf) -> PathBuf {
    let compressed_path = path.with_extension("jsonl.zst");
    let input = fs::File::open(&path).expect("open rollout for compression");
    let compressed = zstd::stream::encode_all(input, /*level*/ 3).expect("compress rollout");
    fs::write(&compressed_path, compressed).expect("write compressed rollout");
    fs::remove_file(path).expect("remove plain rollout");
    compressed_path
}

fn apply_options() -> RolloutMigrationOptions {
    RolloutMigrationOptions {
        mode: RolloutMigrationMode::Apply,
        max_mib_per_second: 1024,
        ..RolloutMigrationOptions::default()
    }
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
async fn migration_publishes_canonical_projected_history_and_is_idempotent() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("first question"),
            agent_message("first answer"),
        ],
    );
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy rollout");
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);

    let lines = read_rollout(&path);
    assert_eq!(
        lines.iter().map(|line| line.ordinal).collect::<Vec<_>>(),
        (0..lines.len() as u64).map(Some).collect::<Vec<_>>()
    );
    assert!(matches!(
        &lines[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.history_mode == ThreadHistoryMode::Paginated
                && metadata.meta.id == thread_id
                && metadata.meta.history_base.is_none()
    ));
    assert_eq!(
        lines
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::EventMsg(EventMsg::ItemCompleted(_))))
            .count(),
        2
    );

    let turns = store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::Summary,
        })
        .await
        .expect("read projected turns");
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 2);

    let bytes = fs::read(&path).expect("read first migration");
    let second = store
        .migrate_rollouts(apply_options())
        .await
        .expect("rerun migration");
    assert_eq!(
        second.outcomes[0].status,
        RolloutMigrationStatus::AlreadyPaginated
    );
    assert_eq!(fs::read(&path).expect("read idempotent rollout"), bytes);
}

#[tokio::test]
async fn migration_preserves_valid_final_record_without_newline() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("question")],
    );
    let final_line = RolloutLine {
        timestamp: TIMESTAMP.to_string(),
        ordinal: None,
        item: agent_message("answer"),
    };
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open legacy rollout")
        .write_all(
            serde_json::to_string(&final_line)
                .expect("serialize final record")
                .as_bytes(),
        )
        .expect("append final record");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy rollout");

    assert_eq!(
        read_rollout(&path)
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::EventMsg(EventMsg::ItemCompleted(_))))
            .count(),
        2
    );
}

#[tokio::test]
async fn migration_preserves_copied_user_fork_history_without_creating_a_history_base() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let parent_id = ThreadId::new();
    let copied_metadata = SessionMeta {
        session_id: parent_id.into(),
        id: parent_id,
        timestamp: TIMESTAMP.to_string(),
        cwd: home.path().to_path_buf(),
        source: SessionSource::Cli,
        ..SessionMeta::default()
    };
    let copied_response = RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "copied parent history".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });
    let path = write_rollout_with_fork(
        home.path(),
        thread_id,
        SessionSource::Cli,
        Some(parent_id),
        vec![
            RolloutItem::SessionMeta(SessionMetaLine {
                meta: copied_metadata,
                git: None,
            }),
            copied_response,
            user_message("child question"),
            agent_message("child answer"),
        ],
    );
    let expected_responses = read_rollout(&path)
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::ResponseItem(item) => {
                Some(serde_json::to_value(item).expect("serialize copied response"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate copied user fork");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    let lines = read_rollout(&path);
    assert!(matches!(
        &lines[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.id == thread_id
                && metadata.meta.forked_from_id == Some(parent_id)
                && metadata.meta.history_mode == ThreadHistoryMode::Paginated
                && metadata.meta.history_base.is_none()
    ));
    assert!(matches!(
        &lines[1].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.id == parent_id
                && metadata.meta.history_mode == ThreadHistoryMode::Legacy
    ));
    assert_eq!(
        lines
            .into_iter()
            .filter_map(|line| match line.item {
                RolloutItem::ResponseItem(item) => {
                    Some(serde_json::to_value(item).expect("serialize migrated response"))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        expected_responses
    );
}

#[tokio::test]
async fn dry_run_reports_newest_first_and_skips_subagents() {
    let home = TempDir::new().expect("create Codex home");
    let root_id = ThreadId::new();
    let root = write_rollout(
        home.path(),
        root_id,
        SessionSource::Cli,
        vec![user_message("root question")],
    );
    let subagent_id = ThreadId::new();
    let subagent = write_rollout(
        home.path(),
        subagent_id,
        SessionSource::SubAgent(SubAgentSource::Other("test".to_string())),
        vec![user_message("subagent question")],
    );
    let compressed_id = ThreadId::new();
    let compressed = compress_rollout(write_rollout(
        home.path(),
        compressed_id,
        SessionSource::Cli,
        vec![user_message("compressed question")],
    ));
    let original = fs::read(&root).expect("read original root rollout");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);

    let report = store
        .migrate_rollouts(RolloutMigrationOptions::default())
        .await
        .expect("inspect legacy rollouts");

    let mut expected = vec![
        (root.clone(), root_id, RolloutMigrationStatus::Eligible),
        (
            subagent,
            subagent_id,
            RolloutMigrationStatus::SkippedSubagent,
        ),
        (compressed, compressed_id, RolloutMigrationStatus::Eligible),
    ];
    expected.sort_by(|(left, ..), (right, ..)| right.cmp(left));
    assert_eq!(
        report
            .outcomes
            .iter()
            .map(|outcome| (
                outcome.rollout_path.clone(),
                outcome.thread_id.expect("rollout thread ID"),
                outcome.status,
            ))
            .collect::<Vec<_>>(),
        expected,
    );
    assert_eq!(fs::read(&root).expect("read inspected rollout"), original);
}

#[tokio::test]
async fn migration_preserves_compressed_rollouts_during_publish_and_recovery() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let compressed_path = compress_rollout(write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("compressed question")],
    ));
    let plain_path = compressed_path.with_extension("");
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate compressed rollout");
    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!plain_path.exists());
    assert_eq!(
        codex_rollout::read_session_meta_line(&compressed_path)
            .await
            .expect("read compressed metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );

    thread_history::delete_thread(&store, thread_id)
        .await
        .expect("simulate missing projection");
    let journal_path = migration_journal_path(home.path(), thread_id);
    write_migration_journal(&journal_path)
        .await
        .expect("simulate pending migration journal");
    let recovered = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover compressed rollout");

    assert_eq!(
        recovered.outcomes[0].status,
        RolloutMigrationStatus::Migrated
    );
    assert!(recovered.outcomes[0].bytes_processed > 0);
    assert!(compressed_path.exists());
    assert!(!plain_path.exists());
    assert!(!journal_path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn decompression_temporaries_are_owner_only() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let compressed_path = compress_rollout(write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("compressed question")],
    ));
    let plain_path = home.path().join("decompressed.tmp");

    decompress_rollout_to_path(&compressed_path, &plain_path)
        .await
        .expect("decompress rollout");

    assert_eq!(
        fs::metadata(&plain_path)
            .expect("read temporary metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[tokio::test]
async fn migration_skips_threads_with_an_active_writer() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("active question")],
    );
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let _writer = store
        .writer_lock_coordinator
        .acquire(thread_id)
        .expect("acquire live writer lock");
    let original = fs::read(&path).expect("read active rollout");

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect active writer");

    assert_eq!(
        report.outcomes[0].status,
        RolloutMigrationStatus::SkippedBusy
    );
    assert_eq!(fs::read(&path).expect("read unmodified rollout"), original);
}

#[tokio::test]
async fn migration_recovers_a_published_rollout_with_missing_projection() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("recover question"),
            agent_message("recover answer"),
        ],
    );
    let store = indexed_store(home.path()).await;
    store
        .migrate_rollouts(apply_options())
        .await
        .expect("publish canonical rollout");
    thread_history::delete_thread(&store, thread_id)
        .await
        .expect("simulate interrupted projection");
    let journal_path = migration_journal_path(home.path(), thread_id);
    write_migration_journal(&journal_path)
        .await
        .expect("simulate pending migration journal");

    let writer = store
        .writer_lock_coordinator
        .acquire(thread_id)
        .expect("acquire live writer lock");
    let busy = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect busy published recovery");
    assert_eq!(busy.outcomes[0].status, RolloutMigrationStatus::SkippedBusy);
    assert!(journal_path.exists());
    drop(writer);

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover published rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!journal_path.exists());
    let projection = thread_history::projection_state(&store, thread_id)
        .await
        .expect("read repaired projection")
        .expect("projection was rebuilt");
    assert_eq!(
        projection.next_byte_offset,
        fs::metadata(&path).expect("read rollout metadata").len()
    );
}

#[tokio::test]
async fn migration_recovers_pending_rollouts_before_new_work() {
    let home = TempDir::new().expect("create Codex home");
    let pending_thread_id = ThreadId::new();
    write_rollout(
        home.path(),
        pending_thread_id,
        SessionSource::Cli,
        vec![user_message("pending question")],
    );
    let new_thread_id = ThreadId::new();
    let new_path = write_rollout(
        home.path(),
        new_thread_id,
        SessionSource::Cli,
        vec![user_message("new question")],
    );
    let newer_directory = home.path().join("sessions/2025/01/04");
    fs::create_dir_all(&newer_directory).expect("create newer rollout directory");
    fs::rename(
        &new_path,
        newer_directory.join(new_path.file_name().expect("rollout filename")),
    )
    .expect("move newer rollout");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(RolloutMigrationOptions {
            thread_ids: vec![pending_thread_id],
            ..apply_options()
        })
        .await
        .expect("publish pending rollout");
    thread_history::delete_thread(&store, pending_thread_id)
        .await
        .expect("simulate missing projection");
    write_migration_journal(&migration_journal_path(home.path(), pending_thread_id))
        .await
        .expect("simulate pending migration journal");

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover pending rollout before new work");

    assert_eq!(
        report
            .outcomes
            .iter()
            .map(|outcome| (
                outcome.thread_id.expect("rollout thread ID"),
                outcome.status
            ))
            .collect::<Vec<_>>(),
        vec![
            (pending_thread_id, RolloutMigrationStatus::Migrated),
            (new_thread_id, RolloutMigrationStatus::Migrated),
        ]
    );
}

#[tokio::test]
async fn failed_migration_preserves_the_legacy_rollout_and_can_be_retried() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("recoverable question")],
    );
    let valid_length = fs::metadata(&path)
        .expect("read valid rollout length")
        .len();
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open legacy rollout");
    writeln!(file, "{{not valid rollout json").expect("append malformed record");
    drop(file);
    let original = fs::read(&path).expect("read malformed legacy rollout");
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("report malformed legacy rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Failed);
    assert_eq!(
        fs::read(&path).expect("read preserved legacy rollout"),
        original
    );

    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open repaired legacy rollout")
        .set_len(valid_length)
        .expect("remove malformed tail");
    let retry = store
        .migrate_rollouts(apply_options())
        .await
        .expect("retry repaired legacy rollout");

    assert_eq!(retry.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!migration_journal_path(home.path(), thread_id).exists());
}
