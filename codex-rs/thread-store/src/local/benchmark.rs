use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadMemoryMode;
use tempfile::TempDir;

use super::LoadThreadHistoryParams;
use super::LocalThreadStore;
use super::ReadThreadParams;
use super::ResumeThreadParams;
use super::read_thread;
use super::test_support::test_config;
use super::test_support::write_session_file;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStore;

const BENCHMARK_HISTORY_ITEMS: usize = 10_000;
const BENCHMARK_RUNS: usize = 20;

fn write_thread_history_benchmark_fixture(
    root: &std::path::Path,
    uuid: uuid::Uuid,
) -> std::path::PathBuf {
    let timestamp = "2025-01-04T10-00-00";
    let rollout_path = write_session_file(root, timestamp, uuid).expect("write benchmark session");
    let message = "x".repeat(1_024);
    let event = serde_json::json!({
        "timestamp": timestamp,
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": message,
            "kind": "plain",
        },
    });
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&rollout_path)
        .expect("open benchmark session");
    for _ in 0..BENCHMARK_HISTORY_ITEMS {
        writeln!(file, "{event}").expect("append benchmark history item");
    }
    rollout_path
}

fn print_thread_history_benchmark(
    label: &str,
    mut durations: Vec<Duration>,
    rollout_bytes: u64,
    read_work: &read_thread::read_work::ReadWork,
) {
    durations.sort_unstable();
    let p50 = durations[durations.len() / 2];
    let p95 = durations[durations.len() * 95 / 100];
    let max = *durations.last().expect("benchmark duration");
    eprintln!(
        "{BENCHMARK_HISTORY_ITEMS}-item thread history {label} benchmark: rollout_bytes={rollout_bytes} p50={p50:?} p95={p95:?} max={max:?} work={read_work:?}"
    );
}

fn thread_metadata() -> ThreadPersistenceMetadata {
    ThreadPersistenceMetadata {
        cwd: Some(std::env::current_dir().expect("cwd")),
        model_provider: "test-provider".to_string(),
        memory_mode: ThreadMemoryMode::Enabled,
    }
}

#[tokio::test]
#[ignore = "release benchmark"]
async fn thread_history_benchmark_10_000_items() {
    let home = TempDir::new().expect("temp dir");
    let uuid = uuid::Uuid::from_u128(408);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let rollout_path = write_thread_history_benchmark_fixture(home.path(), uuid);
    let rollout_bytes = std::fs::metadata(&rollout_path)
        .expect("benchmark rollout metadata")
        .len();

    let config = test_config(home.path());
    let runtime = codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        rollout_path.clone(),
        chrono::Utc::now(),
        SessionSource::Cli,
    );
    builder.model_provider = Some(config.default_model_provider_id.clone());
    builder.cwd = home.path().to_path_buf();
    runtime
        .upsert_thread(&builder.build(config.default_model_provider_id.as_str()))
        .await
        .expect("state db upsert should succeed");
    let sqlite_store = LocalThreadStore::new(config.clone(), Some(runtime));
    sqlite_store
        .read_thread(ReadThreadParams {
            thread_id,
            include_archived: false,
            include_history: true,
        })
        .await
        .expect("warm SQLite history read");

    let mut sqlite_durations = Vec::with_capacity(BENCHMARK_RUNS);
    let mut sqlite_work = None;
    for _ in 0..BENCHMARK_RUNS {
        let started = Instant::now();
        let (thread, read_work) =
            read_thread::read_work::measure(sqlite_store.read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: true,
            }))
            .await;
        assert_eq!(
            thread
                .expect("read SQLite-backed thread")
                .history
                .expect("SQLite-backed history")
                .items
                .len(),
            BENCHMARK_HISTORY_ITEMS + 2
        );
        sqlite_durations.push(started.elapsed());
        sqlite_work = Some(read_work);
    }
    print_thread_history_benchmark(
        "SQLite",
        sqlite_durations,
        rollout_bytes,
        &sqlite_work.expect("SQLite read work"),
    );

    let live_store = LocalThreadStore::new(config, /*state_db*/ None);
    live_store
        .resume_thread(ResumeThreadParams {
            thread_id,
            rollout_path: Some(rollout_path),
            history: None,
            include_archived: true,
            metadata: thread_metadata(),
        })
        .await
        .expect("resume benchmark thread");
    live_store
        .load_history(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await
        .expect("warm active-writer history read");

    let mut live_durations = Vec::with_capacity(BENCHMARK_RUNS);
    let mut live_work = None;
    for _ in 0..BENCHMARK_RUNS {
        let started = Instant::now();
        let (history, read_work) =
            read_thread::read_work::measure(live_store.load_history(LoadThreadHistoryParams {
                thread_id,
                include_archived: false,
            }))
            .await;
        assert_eq!(
            history.expect("load active-writer history").items.len(),
            BENCHMARK_HISTORY_ITEMS + 2
        );
        live_durations.push(started.elapsed());
        live_work = Some(read_work);
    }
    print_thread_history_benchmark(
        "active writer",
        live_durations,
        rollout_bytes,
        &live_work.expect("active-writer read work"),
    );
}
