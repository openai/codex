//! Regression coverage for metadata reads without SQLite.

use std::fs;
use std::fs::FileTimes;

use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadSource;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::LocalThreadStore;
use super::test_support::test_config;
use crate::ReadThreadParams;
use crate::ThreadStore;

#[tokio::test]
async fn empty_archived_reads_without_sqlite_preserve_source_and_file_time()
-> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::new();
    let timestamp = "2026-07-09T00:00:00Z";
    let created_at = DateTime::parse_from_rfc3339(timestamp)?.with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339("2026-07-10T00:00:00Z")?.with_timezone(&Utc);
    let directory = home.path().join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR);
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("rollout-2026-07-09T00-00-00-{thread_id}.jsonl"));
    let line = RolloutLine {
        timestamp: timestamp.to_string(),
        ordinal: None,
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                id: thread_id,
                session_id: thread_id.into(),
                timestamp: timestamp.to_string(),
                thread_source: Some(ThreadSource::User),
                ..SessionMeta::default()
            },
            git: None,
        }),
    };
    fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;
    fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_times(FileTimes::new().set_modified(updated_at.into()))?;

    let by_id = store
        .read_thread(ReadThreadParams {
            thread_id,
            include_archived: true,
            include_history: false,
        })
        .await?;
    let by_path = store
        .read_thread_by_rollout_path(
            path, /*include_archived*/ true, /*include_history*/ false,
        )
        .await?;
    for thread in [by_id, by_path] {
        assert_eq!(
            (
                thread.thread_id,
                thread.thread_source,
                thread.preview,
                thread.created_at,
                thread.updated_at,
                thread.recency_at,
                thread.archived_at
            ),
            (
                thread_id,
                Some(ThreadSource::User),
                String::new(),
                created_at,
                updated_at,
                updated_at,
                Some(updated_at)
            ),
        );
    }
    Ok(())
}
