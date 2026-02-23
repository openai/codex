#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::OsStr;
use std::fs::File;
use std::fs::FileTimes;
use std::fs::{self};
use std::io::Write;
use std::path::Path;

use chrono::TimeZone;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use time::Duration;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use crate::rollout::list::Cursor;
use crate::rollout::list::ThreadItem;
use crate::rollout::list::ThreadSortKey;
use crate::rollout::list::ThreadsPage;
use crate::rollout::list::get_threads;
use crate::rollout::list::read_head_for_summary;
use crate::rollout::rollout_date_parts;
use anyhow::Result;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;

const NO_SOURCE_FILTER: &[SessionSource] = &[];
const TEST_PROVIDER: &str = "test-provider";

fn provider_vec(providers: &[&str]) -> Vec<String> {
    providers
        .iter()
        .map(std::string::ToString::to_string)
        .collect()
}

fn thread_id_from_uuid(uuid: Uuid) -> ThreadId {
    ThreadId::from_string(&uuid.to_string()).expect("valid thread id")
}

async fn insert_state_db_thread(
    home: &Path,
    thread_id: ThreadId,
    rollout_path: &Path,
    archived: bool,
) {
    let runtime =
        codex_state::StateRuntime::init(home.to_path_buf(), TEST_PROVIDER.to_string(), None)
            .await
            .expect("state db should initialize");
    runtime
        .mark_backfill_complete(None)
        .await
        .expect("backfill should be complete");
    let created_at = chrono::Utc
        .with_ymd_and_hms(2025, 1, 3, 12, 0, 0)
        .single()
        .expect("valid datetime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        rollout_path.to_path_buf(),
        created_at,
        SessionSource::Cli,
    );
    builder.model_provider = Some(TEST_PROVIDER.to_string());
    builder.cwd = home.to_path_buf();
    if archived {
        builder.archived_at = Some(created_at);
    }
    let mut metadata = builder.build(TEST_PROVIDER);
    metadata.first_user_message = Some("Hello from user".to_string());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("state db upsert should succeed");
}

// TODO(jif) fix
// #[tokio::test]
// async fn list_threads_prefers_state_db_when_available() {
//     let temp = TempDir::new().unwrap();
//     let home = temp.path();
//     let fs_uuid = Uuid::from_u128(101);
//     write_session_file(
//         home,
//         "2025-01-03T13-00-00",
//         fs_uuid,
//         1,
//         Some(SessionSource::Cli),
//     )
//     .unwrap();
//
//     let db_uuid = Uuid::from_u128(102);
//     let db_thread_id = ThreadId::from_string(&db_uuid.to_string()).expect("valid thread id");
//     let db_rollout_path = home.join(format!(
//         "sessions/2025/01/03/rollout-2025-01-03T12-00-00-{db_uuid}.jsonl"
//     ));
//     insert_state_db_thread(home, db_thread_id, db_rollout_path.as_path(), false).await;
//
//     let page = RolloutRecorder::list_threads(
//         home,
//         10,
//         None,
//         ThreadSortKey::CreatedAt,
//         NO_SOURCE_FILTER,
//         None,
//         TEST_PROVIDER,
//     )
//     .await
//     .expect("thread listing should succeed");
//
//     assert_eq!(page.items.len(), 1);
//     assert_eq!(page.items[0].path, db_rollout_path);
//     assert_eq!(page.items[0].thread_id, Some(db_thread_id));
//     assert_eq!(page.items[0].cwd, Some(home.to_path_buf()));
//     assert_eq!(
//         page.items[0].first_user_message.as_deref(),
//         Some("Hello from user")
//     );
// }

// #[tokio::test]
// async fn list_threads_db_excludes_archived_entries() {
//     let temp = TempDir::new().unwrap();
//     let home = temp.path();
//     let sessions_root = home.join("sessions/2025/01/03");
//     let archived_root = home.join("archived_sessions");
//     fs::create_dir_all(&sessions_root).unwrap();
//     fs::create_dir_all(&archived_root).unwrap();
//
//     let active_uuid = Uuid::from_u128(211);
//     let active_thread_id =
//         ThreadId::from_string(&active_uuid.to_string()).expect("valid active thread id");
//     let active_rollout_path =
//         sessions_root.join(format!("rollout-2025-01-03T12-00-00-{active_uuid}.jsonl"));
//     insert_state_db_thread(home, active_thread_id, active_rollout_path.as_path(), false).await;
//
//     let archived_uuid = Uuid::from_u128(212);
//     let archived_thread_id =
//         ThreadId::from_string(&archived_uuid.to_string()).expect("valid archived thread id");
//     let archived_rollout_path =
//         archived_root.join(format!("rollout-2025-01-03T11-00-00-{archived_uuid}.jsonl"));
//     insert_state_db_thread(
//         home,
//         archived_thread_id,
//         archived_rollout_path.as_path(),
//         true,
//     )
//     .await;
//
//     let page = RolloutRecorder::list_threads(
//         home,
//         10,
//         None,
//         ThreadSortKey::CreatedAt,
//         NO_SOURCE_FILTER,
//         None,
//         TEST_PROVIDER,
//     )
//     .await
//     .expect("thread listing should succeed");
//
//     assert_eq!(page.items.len(), 1);
//     assert_eq!(page.items[0].path, active_rollout_path);
// }

// #[tokio::test]
// async fn list_threads_falls_back_to_files_when_state_db_is_unavailable() {
//     let temp = TempDir::new().unwrap();
//     let home = temp.path();
//     let fs_uuid = Uuid::from_u128(301);
//     write_session_file(
//         home,
//         "2025-01-03T13-00-00",
//         fs_uuid,
//         1,
//         Some(SessionSource::Cli),
//     )
//     .unwrap();
//
//     let page = RolloutRecorder::list_threads(
//         home,
//         10,
//         None,
//         ThreadSortKey::CreatedAt,
//         NO_SOURCE_FILTER,
//         None,
//         TEST_PROVIDER,
//     )
//     .await
//     .expect("thread listing should succeed");
//
//     assert_eq!(page.items.len(), 1);
//     let file_name = page.items[0]
//         .path
//         .file_name()
//         .and_then(|value| value.to_str())
//         .expect("rollout file name should be utf8");
//     assert!(
//         file_name.contains(&fs_uuid.to_string()),
//         "expected file path from filesystem listing, got: {file_name}"
//     );
// }

#[tokio::test]
async fn find_thread_path_falls_back_when_db_path_is_stale() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    let uuid = Uuid::from_u128(302);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let ts = "2025-01-03T13-00-00";
    write_session_file(home, ts, uuid, 1, Some(SessionSource::Cli)).unwrap();
    let fs_rollout_path = home.join(format!("sessions/2025/01/03/rollout-{ts}-{uuid}.jsonl"));

    let stale_db_path = home.join(format!(
        "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
    ));
    insert_state_db_thread(home, thread_id, stale_db_path.as_path(), false).await;

    let found = crate::rollout::find_thread_path_by_id_str(home, &uuid.to_string())
        .await
        .expect("lookup should succeed");
    assert_eq!(found, Some(fs_rollout_path.clone()));
    assert_state_db_rollout_path(home, thread_id, Some(fs_rollout_path.as_path())).await;
}

#[tokio::test]
async fn find_thread_path_repairs_missing_db_row_after_filesystem_fallback() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    let uuid = Uuid::from_u128(303);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
    let ts = "2025-01-03T13-00-00";
    write_session_file(home, ts, uuid, 1, Some(SessionSource::Cli)).unwrap();
    let fs_rollout_path = home.join(format!("sessions/2025/01/03/rollout-{ts}-{uuid}.jsonl"));

    // Create an empty state DB so lookup takes the DB-first path and then falls back to files.
    let _runtime =
        codex_state::StateRuntime::init(home.to_path_buf(), TEST_PROVIDER.to_string(), None)
            .await
            .expect("state db should initialize");
    _runtime
        .mark_backfill_complete(None)
        .await
        .expect("backfill should be complete");

    let found = crate::rollout::find_thread_path_by_id_str(home, &uuid.to_string())
        .await
        .expect("lookup should succeed");
    assert_eq!(found, Some(fs_rollout_path.clone()));
    assert_state_db_rollout_path(home, thread_id, Some(fs_rollout_path.as_path())).await;
}

#[test]
fn rollout_date_parts_extracts_directory_components() {
    let file_name = OsStr::new("rollout-2025-03-01T09-00-00-123.jsonl");
    let parts = rollout_date_parts(file_name);
    assert_eq!(
        parts,
        Some(("2025".to_string(), "03".to_string(), "01".to_string()))
    );
}

async fn assert_state_db_rollout_path(
    home: &Path,
    thread_id: ThreadId,
    expected_path: Option<&Path>,
) {
    let runtime =
        codex_state::StateRuntime::init(home.to_path_buf(), TEST_PROVIDER.to_string(), None)
            .await
            .expect("state db should initialize");
    let path = runtime
        .find_rollout_path_by_id(thread_id, Some(false))
        .await
        .expect("state db lookup should succeed");
    assert_eq!(path.as_deref(), expected_path);
}

fn write_session_file(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    num_records: usize,
    source: Option<SessionSource>,
) -> std::io::Result<(OffsetDateTime, Uuid)> {
    write_session_file_with_provider(
        root,
        ts_str,
        uuid,
        num_records,
        source,
        Some("test-provider"),
    )
}

fn write_session_file_with_provider(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    num_records: usize,
    source: Option<SessionSource>,
    model_provider: Option<&str>,
) -> std::io::Result<(OffsetDateTime, Uuid)> {
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let dt = PrimitiveDateTime::parse(ts_str, format)
        .unwrap()
        .assume_utc();
    let dir = root
        .join("sessions")
        .join(format!("{:04}", dt.year()))
        .join(format!("{:02}", u8::from(dt.month())))
        .join(format!("{:02}", dt.day()));
    fs::create_dir_all(&dir)?;

    let filename = format!("rollout-{ts_str}-{uuid}.jsonl");
    let file_path = dir.join(filename);
    let mut file = File::create(file_path)?;

    let mut payload = serde_json::json!({
        "id": uuid,
        "timestamp": ts_str,
        "cwd": ".",
        "originator": "test_originator",
        "cli_version": "test_version",
        "base_instructions": null,
    });

    if let Some(source) = source {
        payload["source"] = serde_json::to_value(source).unwrap();
    }
    if let Some(provider) = model_provider {
        payload["model_provider"] = serde_json::Value::String(provider.to_string());
    }

    let meta = serde_json::json!({
        "timestamp": ts_str,
        "type": "session_meta",
        "payload": payload,
    });
    writeln!(file, "{meta}")?;

    // Include at least one user message event to satisfy listing filters
    let user_event = serde_json::json!({
        "timestamp": ts_str,
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": "Hello from user",
            "kind": "plain"
        }
    });
    writeln!(file, "{user_event}")?;

    for i in 0..num_records {
        let rec = serde_json::json!({
            "record_type": "response",
            "index": i
        });
        writeln!(file, "{rec}")?;
    }
    let times = FileTimes::new().set_modified(dt.into());
    file.set_times(times)?;
    Ok((dt, uuid))
}

fn write_session_file_with_delayed_user_event(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    meta_lines_before_user: usize,
) -> std::io::Result<()> {
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let dt = PrimitiveDateTime::parse(ts_str, format)
        .unwrap()
        .assume_utc();
    let dir = root
        .join("sessions")
        .join(format!("{:04}", dt.year()))
        .join(format!("{:02}", u8::from(dt.month())))
        .join(format!("{:02}", dt.day()));
    fs::create_dir_all(&dir)?;

    let filename = format!("rollout-{ts_str}-{uuid}.jsonl");
    let file_path = dir.join(filename);
    let mut file = File::create(file_path)?;

    for i in 0..meta_lines_before_user {
        let id = if i == 0 {
            uuid
        } else {
            Uuid::from_u128(100 + i as u128)
        };
        let payload = serde_json::json!({
            "id": id,
            "timestamp": ts_str,
            "cwd": ".",
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "vscode",
            "model_provider": "test-provider",
        });
        let meta = serde_json::json!({
            "timestamp": ts_str,
            "type": "session_meta",
            "payload": payload,
        });
        writeln!(file, "{meta}")?;
    }

    let user_event = serde_json::json!({
        "timestamp": ts_str,
        "type": "event_msg",
        "payload": {"type": "user_message", "message": "Hello from user", "kind": "plain"}
    });
    writeln!(file, "{user_event}")?;

    let times = FileTimes::new().set_modified(dt.into());
    file.set_times(times)?;
    Ok(())
}

fn write_session_file_with_meta_payload(
    root: &Path,
    ts_str: &str,
    uuid: Uuid,
    payload: serde_json::Value,
) -> std::io::Result<()> {
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let dt = PrimitiveDateTime::parse(ts_str, format)
        .unwrap()
        .assume_utc();
    let dir = root
        .join("sessions")
        .join(format!("{:04}", dt.year()))
        .join(format!("{:02}", u8::from(dt.month())))
        .join(format!("{:02}", dt.day()));
    fs::create_dir_all(&dir)?;

    let filename = format!("rollout-{ts_str}-{uuid}.jsonl");
    let file_path = dir.join(filename);
    let mut file = File::create(file_path)?;

    let meta = serde_json::json!({
        "timestamp": ts_str,
        "type": "session_meta",
        "payload": payload,
    });
    writeln!(file, "{meta}")?;

    let user_event = serde_json::json!({
        "timestamp": ts_str,
        "type": "event_msg",
        "payload": {"type": "user_message", "message": "Hello from user", "kind": "plain"}
    });
    writeln!(file, "{user_event}")?;

    let times = FileTimes::new().set_modified(dt.into());
    file.set_times(times)?;

    Ok(())
}

#[tokio::test]
async fn test_list_conversations_latest_first() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Fixed UUIDs for deterministic expectations
    let u1 = Uuid::from_u128(1);
    let u2 = Uuid::from_u128(2);
    let u3 = Uuid::from_u128(3);

    // Create three sessions across three days
    write_session_file(
        home,
        "2025-01-01T12-00-00",
        u1,
        3,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-01-02T12-00-00",
        u2,
        3,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-01-03T12-00-00",
        u3,
        3,
        Some(SessionSource::VSCode),
    )
    .unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();

    // Build expected objects
    let p1 = home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("03")
        .join(format!("rollout-2025-01-03T12-00-00-{u3}.jsonl"));
    let p2 = home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("02")
        .join(format!("rollout-2025-01-02T12-00-00-{u2}.jsonl"));
    let p3 = home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("01")
        .join(format!("rollout-2025-01-01T12-00-00-{u1}.jsonl"));

    let updated_times: Vec<Option<String>> =
        page.items.iter().map(|i| i.updated_at.clone()).collect();

    let expected = ThreadsPage {
        items: vec![
            ThreadItem {
                path: p1,
                thread_id: Some(thread_id_from_uuid(u3)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-01-03T12-00-00".into()),
                updated_at: updated_times.first().cloned().flatten(),
            },
            ThreadItem {
                path: p2,
                thread_id: Some(thread_id_from_uuid(u2)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-01-02T12-00-00".into()),
                updated_at: updated_times.get(1).cloned().flatten(),
            },
            ThreadItem {
                path: p3,
                thread_id: Some(thread_id_from_uuid(u1)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-01-01T12-00-00".into()),
                updated_at: updated_times.get(2).cloned().flatten(),
            },
        ],
        next_cursor: None,
        num_scanned_files: 3,
        reached_scan_cap: false,
    };

    assert_eq!(page, expected);
}

#[tokio::test]
async fn test_pagination_cursor() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    // Fixed UUIDs for deterministic expectations
    let u1 = Uuid::from_u128(11);
    let u2 = Uuid::from_u128(22);
    let u3 = Uuid::from_u128(33);
    let u4 = Uuid::from_u128(44);
    let u5 = Uuid::from_u128(55);

    // Oldest to newest
    write_session_file(
        home,
        "2025-03-01T09-00-00",
        u1,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-02T09-00-00",
        u2,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-03T09-00-00",
        u3,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-04T09-00-00",
        u4,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-03-05T09-00-00",
        u5,
        1,
        Some(SessionSource::VSCode),
    )
    .unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page1 = get_threads(
        home,
        2,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let p5 = home
        .join("sessions")
        .join("2025")
        .join("03")
        .join("05")
        .join(format!("rollout-2025-03-05T09-00-00-{u5}.jsonl"));
    let p4 = home
        .join("sessions")
        .join("2025")
        .join("03")
        .join("04")
        .join(format!("rollout-2025-03-04T09-00-00-{u4}.jsonl"));
    let updated_page1: Vec<Option<String>> =
        page1.items.iter().map(|i| i.updated_at.clone()).collect();
    let expected_cursor1: Cursor =
        serde_json::from_str(&format!("\"2025-03-04T09-00-00|{u4}\"")).unwrap();
    let expected_page1 = ThreadsPage {
        items: vec![
            ThreadItem {
                path: p5,
                thread_id: Some(thread_id_from_uuid(u5)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-03-05T09-00-00".into()),
                updated_at: updated_page1.first().cloned().flatten(),
            },
            ThreadItem {
                path: p4,
                thread_id: Some(thread_id_from_uuid(u4)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-03-04T09-00-00".into()),
                updated_at: updated_page1.get(1).cloned().flatten(),
            },
        ],
        next_cursor: Some(expected_cursor1.clone()),
        num_scanned_files: 3, // scanned 05, 04, and peeked at 03 before breaking
        reached_scan_cap: false,
    };
    assert_eq!(page1, expected_page1);

    let page2 = get_threads(
        home,
        2,
        page1.next_cursor.as_ref(),
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let p3 = home
        .join("sessions")
        .join("2025")
        .join("03")
        .join("03")
        .join(format!("rollout-2025-03-03T09-00-00-{u3}.jsonl"));
    let p2 = home
        .join("sessions")
        .join("2025")
        .join("03")
        .join("02")
        .join(format!("rollout-2025-03-02T09-00-00-{u2}.jsonl"));
    let updated_page2: Vec<Option<String>> =
        page2.items.iter().map(|i| i.updated_at.clone()).collect();
    let expected_cursor2: Cursor =
        serde_json::from_str(&format!("\"2025-03-02T09-00-00|{u2}\"")).unwrap();
    let expected_page2 = ThreadsPage {
        items: vec![
            ThreadItem {
                path: p3,
                thread_id: Some(thread_id_from_uuid(u3)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-03-03T09-00-00".into()),
                updated_at: updated_page2.first().cloned().flatten(),
            },
            ThreadItem {
                path: p2,
                thread_id: Some(thread_id_from_uuid(u2)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some("2025-03-02T09-00-00".into()),
                updated_at: updated_page2.get(1).cloned().flatten(),
            },
        ],
        next_cursor: Some(expected_cursor2.clone()),
        num_scanned_files: 5, // scanned 05, 04 (anchor), 03, 02, and peeked at 01
        reached_scan_cap: false,
    };
    assert_eq!(page2, expected_page2);

    let page3 = get_threads(
        home,
        2,
        page2.next_cursor.as_ref(),
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let p1 = home
        .join("sessions")
        .join("2025")
        .join("03")
        .join("01")
        .join(format!("rollout-2025-03-01T09-00-00-{u1}.jsonl"));
    let updated_page3: Vec<Option<String>> =
        page3.items.iter().map(|i| i.updated_at.clone()).collect();
    let expected_page3 = ThreadsPage {
        items: vec![ThreadItem {
            path: p1,
            thread_id: Some(thread_id_from_uuid(u1)),
            first_user_message: Some("Hello from user".to_string()),
            cwd: Some(Path::new(".").to_path_buf()),
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: Some(SessionSource::VSCode),
            agent_nickname: None,
            agent_role: None,
            model_provider: Some(TEST_PROVIDER.to_string()),
            cli_version: Some("test_version".to_string()),
            created_at: Some("2025-03-01T09-00-00".into()),
            updated_at: updated_page3.first().cloned().flatten(),
        }],
        next_cursor: None,
        num_scanned_files: 5, // scanned 05, 04 (anchor), 03, 02 (anchor), 01
        reached_scan_cap: false,
    };
    assert_eq!(page3, expected_page3);
}

#[tokio::test]
async fn test_list_threads_scans_past_head_for_user_event() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let uuid = Uuid::from_u128(99);
    let ts = "2025-05-01T10-30-00";
    write_session_file_with_delayed_user_event(home, ts, uuid, 12).unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].thread_id, Some(thread_id_from_uuid(uuid)));
}

#[tokio::test]
async fn test_get_thread_contents() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let uuid = Uuid::new_v4();
    let ts = "2025-04-01T10-30-00";
    write_session_file(home, ts, uuid, 2, Some(SessionSource::VSCode)).unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        1,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let path = &page.items[0].path;

    let content = tokio::fs::read_to_string(path).await.unwrap();

    // Page equality (single item)
    let expected_path = home
        .join("sessions")
        .join("2025")
        .join("04")
        .join("01")
        .join(format!("rollout-2025-04-01T10-30-00-{uuid}.jsonl"));
    let expected_page = ThreadsPage {
        items: vec![ThreadItem {
            path: expected_path,
            thread_id: Some(thread_id_from_uuid(uuid)),
            first_user_message: Some("Hello from user".to_string()),
            cwd: Some(Path::new(".").to_path_buf()),
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: Some(SessionSource::VSCode),
            agent_nickname: None,
            agent_role: None,
            model_provider: Some(TEST_PROVIDER.to_string()),
            cli_version: Some("test_version".to_string()),
            created_at: Some(ts.into()),
            updated_at: page.items[0].updated_at.clone(),
        }],
        next_cursor: None,
        num_scanned_files: 1,
        reached_scan_cap: false,
    };
    assert_eq!(page, expected_page);

    // Entire file contents equality
    let meta = serde_json::json!({
        "timestamp": ts,
        "type": "session_meta",
        "payload": {
            "id": uuid,
            "timestamp": ts,
            "cwd": ".",
            "originator": "test_originator",
            "cli_version": "test_version",
            "base_instructions": null,
            "source": "vscode",
            "model_provider": "test-provider",
        }
    });
    let user_event = serde_json::json!({
        "timestamp": ts,
        "type": "event_msg",
        "payload": {"type": "user_message", "message": "Hello from user", "kind": "plain"}
    });
    let rec0 = serde_json::json!({"record_type": "response", "index": 0});
    let rec1 = serde_json::json!({"record_type": "response", "index": 1});
    let expected_content = format!("{meta}\n{user_event}\n{rec0}\n{rec1}\n");
    assert_eq!(content, expected_content);
}

#[tokio::test]
async fn test_base_instructions_missing_in_meta_defaults_to_null() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-04-02T10-30-00";
    let uuid = Uuid::from_u128(101);
    let payload = serde_json::json!({
        "id": uuid,
        "timestamp": ts,
        "cwd": ".",
        "originator": "test_originator",
        "cli_version": "test_version",
        "source": "vscode",
        "model_provider": "test-provider",
    });
    write_session_file_with_meta_payload(home, ts, uuid, payload).unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        1,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();

    let head = read_head_for_summary(&page.items[0].path)
        .await
        .expect("session meta head");
    let first = head.first().expect("first head entry");
    assert_eq!(
        first.get("base_instructions"),
        Some(&serde_json::Value::Null)
    );
}

#[tokio::test]
async fn test_base_instructions_present_in_meta_is_preserved() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-04-03T10-30-00";
    let uuid = Uuid::from_u128(102);
    let base_text = "Custom base instructions";
    let payload = serde_json::json!({
        "id": uuid,
        "timestamp": ts,
        "cwd": ".",
        "originator": "test_originator",
        "cli_version": "test_version",
        "source": "vscode",
        "model_provider": "test-provider",
        "base_instructions": {"text": base_text},
    });
    write_session_file_with_meta_payload(home, ts, uuid, payload).unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        1,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();

    let head = read_head_for_summary(&page.items[0].path)
        .await
        .expect("session meta head");
    let first = head.first().expect("first head entry");
    let base = first
        .get("base_instructions")
        .and_then(|value| value.get("text"))
        .and_then(serde_json::Value::as_str);
    assert_eq!(base, Some(base_text));
}

#[tokio::test]
async fn test_created_at_sort_uses_file_mtime_for_updated_at() -> Result<()> {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-06-01T08-00-00";
    let uuid = Uuid::from_u128(43);
    write_session_file(home, ts, uuid, 0, Some(SessionSource::VSCode)).unwrap();

    let created = PrimitiveDateTime::parse(
        ts,
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]"),
    )?
    .assume_utc();
    let updated = created + Duration::hours(2);
    let expected_updated = updated.format(&time::format_description::well_known::Rfc3339)?;

    let file_path = home
        .join("sessions")
        .join("2025")
        .join("06")
        .join("01")
        .join(format!("rollout-{ts}-{uuid}.jsonl"));
    let file = std::fs::OpenOptions::new().write(true).open(&file_path)?;
    let times = FileTimes::new().set_modified(updated.into());
    file.set_times(times)?;

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        1,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await?;

    let item = page.items.first().expect("conversation item");
    assert_eq!(item.created_at.as_deref(), Some(ts));
    assert_eq!(item.updated_at.as_deref(), Some(expected_updated.as_str()));

    Ok(())
}

#[tokio::test]
async fn test_updated_at_uses_file_mtime() -> Result<()> {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-06-01T08-00-00";
    let uuid = Uuid::from_u128(42);
    let day_dir = home.join("sessions").join("2025").join("06").join("01");
    fs::create_dir_all(&day_dir)?;
    let file_path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
    let mut file = File::create(&file_path)?;

    let conversation_id = ThreadId::from_string(&uuid.to_string())?;
    let meta_line = RolloutLine {
        timestamp: ts.to_string(),
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                id: conversation_id,
                forked_from_id: None,
                timestamp: ts.to_string(),
                cwd: ".".into(),
                originator: "test_originator".into(),
                cli_version: "test_version".into(),
                source: SessionSource::VSCode,
                agent_nickname: None,
                agent_role: None,
                model_provider: Some("test-provider".into()),
                base_instructions: None,
                dynamic_tools: None,
            },
            git: None,
        }),
    };
    writeln!(file, "{}", serde_json::to_string(&meta_line)?)?;

    let user_event_line = RolloutLine {
        timestamp: ts.to_string(),
        item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "hello".into(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
        })),
    };
    writeln!(file, "{}", serde_json::to_string(&user_event_line)?)?;

    let total_messages = 12usize;
    for idx in 0..total_messages {
        let response_line = RolloutLine {
            timestamp: format!("{ts}-{idx:02}"),
            item: RolloutItem::ResponseItem(ResponseItem::Message {
                id: None,
                role: "assistant".into(),
                content: vec![ContentItem::OutputText {
                    text: format!("reply-{idx}"),
                }],
                end_turn: None,
                phase: None,
            }),
        };
        writeln!(file, "{}", serde_json::to_string(&response_line)?)?;
    }
    drop(file);

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page = get_threads(
        home,
        1,
        None,
        ThreadSortKey::UpdatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await?;
    let item = page.items.first().expect("conversation item");
    assert_eq!(item.created_at.as_deref(), Some(ts));
    let updated = item
        .updated_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .expect("updated_at set from file mtime");
    let now = chrono::Utc::now();
    let age = now - updated;
    assert!(age.num_seconds().abs() < 30);

    Ok(())
}

#[tokio::test]
async fn test_stable_ordering_same_second_pagination() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let ts = "2025-07-01T00-00-00";
    let u1 = Uuid::from_u128(1);
    let u2 = Uuid::from_u128(2);
    let u3 = Uuid::from_u128(3);

    write_session_file(home, ts, u1, 0, Some(SessionSource::VSCode)).unwrap();
    write_session_file(home, ts, u2, 0, Some(SessionSource::VSCode)).unwrap();
    write_session_file(home, ts, u3, 0, Some(SessionSource::VSCode)).unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let page1 = get_threads(
        home,
        2,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();

    let p3 = home
        .join("sessions")
        .join("2025")
        .join("07")
        .join("01")
        .join(format!("rollout-2025-07-01T00-00-00-{u3}.jsonl"));
    let p2 = home
        .join("sessions")
        .join("2025")
        .join("07")
        .join("01")
        .join(format!("rollout-2025-07-01T00-00-00-{u2}.jsonl"));
    let updated_page1: Vec<Option<String>> =
        page1.items.iter().map(|i| i.updated_at.clone()).collect();
    let expected_cursor1: Cursor = serde_json::from_str(&format!("\"{ts}|{u2}\"")).unwrap();
    let expected_page1 = ThreadsPage {
        items: vec![
            ThreadItem {
                path: p3,
                thread_id: Some(thread_id_from_uuid(u3)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some(ts.to_string()),
                updated_at: updated_page1.first().cloned().flatten(),
            },
            ThreadItem {
                path: p2,
                thread_id: Some(thread_id_from_uuid(u2)),
                first_user_message: Some("Hello from user".to_string()),
                cwd: Some(Path::new(".").to_path_buf()),
                git_branch: None,
                git_sha: None,
                git_origin_url: None,
                source: Some(SessionSource::VSCode),
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(TEST_PROVIDER.to_string()),
                cli_version: Some("test_version".to_string()),
                created_at: Some(ts.to_string()),
                updated_at: updated_page1.get(1).cloned().flatten(),
            },
        ],
        next_cursor: Some(expected_cursor1.clone()),
        num_scanned_files: 3, // scanned u3, u2, peeked u1
        reached_scan_cap: false,
    };
    assert_eq!(page1, expected_page1);

    let page2 = get_threads(
        home,
        2,
        page1.next_cursor.as_ref(),
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let p1 = home
        .join("sessions")
        .join("2025")
        .join("07")
        .join("01")
        .join(format!("rollout-2025-07-01T00-00-00-{u1}.jsonl"));
    let updated_page2: Vec<Option<String>> =
        page2.items.iter().map(|i| i.updated_at.clone()).collect();
    let expected_page2 = ThreadsPage {
        items: vec![ThreadItem {
            path: p1,
            thread_id: Some(thread_id_from_uuid(u1)),
            first_user_message: Some("Hello from user".to_string()),
            cwd: Some(Path::new(".").to_path_buf()),
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: Some(SessionSource::VSCode),
            agent_nickname: None,
            agent_role: None,
            model_provider: Some(TEST_PROVIDER.to_string()),
            cli_version: Some("test_version".to_string()),
            created_at: Some(ts.to_string()),
            updated_at: updated_page2.first().cloned().flatten(),
        }],
        next_cursor: None,
        num_scanned_files: 3, // scanned u3, u2 (anchor), u1
        reached_scan_cap: false,
    };
    assert_eq!(page2, expected_page2);
}

#[tokio::test]
async fn test_source_filter_excludes_non_matching_sessions() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let interactive_id = Uuid::from_u128(42);
    let non_interactive_id = Uuid::from_u128(77);

    write_session_file(
        home,
        "2025-08-02T10-00-00",
        interactive_id,
        2,
        Some(SessionSource::Cli),
    )
    .unwrap();
    write_session_file(
        home,
        "2025-08-01T10-00-00",
        non_interactive_id,
        2,
        Some(SessionSource::Exec),
    )
    .unwrap();

    let provider_filter = provider_vec(&[TEST_PROVIDER]);
    let interactive_only = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        INTERACTIVE_SESSION_SOURCES,
        Some(provider_filter.as_slice()),
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let paths: Vec<_> = interactive_only
        .items
        .iter()
        .map(|item| item.path.as_path())
        .collect();

    assert_eq!(paths.len(), 1);
    assert!(paths.iter().all(|path| {
        path.ends_with("rollout-2025-08-02T10-00-00-00000000-0000-0000-0000-00000000002a.jsonl")
    }));

    let all_sessions = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        NO_SOURCE_FILTER,
        None,
        TEST_PROVIDER,
    )
    .await
    .unwrap();
    let all_paths: Vec<_> = all_sessions
        .items
        .into_iter()
        .map(|item| item.path)
        .collect();
    assert_eq!(all_paths.len(), 2);
    assert!(all_paths.iter().any(|path| {
        path.ends_with("rollout-2025-08-02T10-00-00-00000000-0000-0000-0000-00000000002a.jsonl")
    }));
    assert!(all_paths.iter().any(|path| {
        path.ends_with("rollout-2025-08-01T10-00-00-00000000-0000-0000-0000-00000000004d.jsonl")
    }));
}

#[tokio::test]
async fn test_model_provider_filter_selects_only_matching_sessions() -> Result<()> {
    let temp = TempDir::new().unwrap();
    let home = temp.path();

    let openai_id = Uuid::from_u128(1);
    let beta_id = Uuid::from_u128(2);
    let none_id = Uuid::from_u128(3);

    write_session_file_with_provider(
        home,
        "2025-09-01T12-00-00",
        openai_id,
        1,
        Some(SessionSource::VSCode),
        Some("openai"),
    )?;
    write_session_file_with_provider(
        home,
        "2025-09-01T11-00-00",
        beta_id,
        1,
        Some(SessionSource::VSCode),
        Some("beta"),
    )?;
    write_session_file_with_provider(
        home,
        "2025-09-01T10-00-00",
        none_id,
        1,
        Some(SessionSource::VSCode),
        None,
    )?;

    let openai_id_str = openai_id.to_string();
    let none_id_str = none_id.to_string();
    let openai_filter = provider_vec(&["openai"]);
    let openai_sessions = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        NO_SOURCE_FILTER,
        Some(openai_filter.as_slice()),
        "openai",
    )
    .await?;
    assert_eq!(openai_sessions.items.len(), 2);
    let openai_ids: Vec<_> = openai_sessions
        .items
        .iter()
        .filter_map(|item| item.thread_id.as_ref().map(ToString::to_string))
        .collect();
    assert!(openai_ids.contains(&openai_id_str));
    assert!(openai_ids.contains(&none_id_str));

    let beta_filter = provider_vec(&["beta"]);
    let beta_sessions = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        NO_SOURCE_FILTER,
        Some(beta_filter.as_slice()),
        "openai",
    )
    .await?;
    assert_eq!(beta_sessions.items.len(), 1);
    let beta_id_str = beta_id.to_string();
    let beta_head = beta_sessions
        .items
        .first()
        .and_then(|item| item.thread_id.as_ref().map(ToString::to_string));
    assert_eq!(beta_head.as_deref(), Some(beta_id_str.as_str()));

    let unknown_filter = provider_vec(&["unknown"]);
    let unknown_sessions = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        NO_SOURCE_FILTER,
        Some(unknown_filter.as_slice()),
        "openai",
    )
    .await?;
    assert!(unknown_sessions.items.is_empty());

    let all_sessions = get_threads(
        home,
        10,
        None,
        ThreadSortKey::CreatedAt,
        NO_SOURCE_FILTER,
        None,
        "openai",
    )
    .await?;
    assert_eq!(all_sessions.items.len(), 3);

    Ok(())
}
