use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

use super::RolloutReferenceIndex;

#[tokio::test]
async fn scans_active_archived_and_compressed_history_bases() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let source_id = thread_id(Uuid::from_u128(1))?;
    let active_base = history_position(source_id);
    let archived_base = history_position(source_id);
    let compressed_base = history_position(source_id);
    let active_child_id = thread_id(Uuid::from_u128(2))?;
    let archived_child_id = thread_id(Uuid::from_u128(3))?;
    let compressed_child_id = thread_id(Uuid::from_u128(4))?;

    write_rollout(
        active_rollout_path(home.path(), Uuid::from_u128(2)),
        active_child_id,
        Some(active_base),
    )?;
    write_rollout(
        archived_rollout_path(home.path(), Uuid::from_u128(3)),
        archived_child_id,
        Some(archived_base),
    )?;
    let compressed_path = archived_rollout_path(home.path(), Uuid::from_u128(4));
    write_rollout(
        compressed_path.clone(),
        compressed_child_id,
        Some(compressed_base),
    )?;
    compress_now(compressed_path.as_path())?;

    let index = RolloutReferenceIndex::scan(home.path()).await?;

    assert_eq!(index.reference_count(source_id), 3);
    assert_eq!(index.history_base(active_child_id), Some(&active_base));
    assert_eq!(index.history_base(archived_child_id), Some(&archived_base));
    assert_eq!(
        index.history_base(compressed_child_id),
        Some(&compressed_base)
    );
    Ok(())
}

#[tokio::test]
async fn active_duplicate_wins_without_double_counting() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let active_source_id = thread_id(Uuid::from_u128(8))?;
    let archived_source_id = thread_id(Uuid::from_u128(9))?;
    let child_uuid = Uuid::from_u128(10);
    let child_id = thread_id(Uuid::from_u128(10))?;
    let active_base = history_position(active_source_id);
    write_rollout(
        active_rollout_path(home.path(), child_uuid),
        child_id,
        Some(active_base),
    )?;
    write_rollout(
        archived_rollout_path(home.path(), child_uuid),
        child_id,
        Some(history_position(archived_source_id)),
    )?;

    let index = RolloutReferenceIndex::scan(home.path()).await?;

    assert_eq!(index.history_base(child_id), Some(&active_base));
    assert_eq!(index.reference_count(active_source_id), 1);
    assert_eq!(index.reference_count(archived_source_id), 0);
    Ok(())
}

#[tokio::test]
async fn self_history_base_does_not_count_as_reference() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let thread_id = thread_id(Uuid::from_u128(11))?;
    let history_base = history_position(thread_id);
    write_rollout(
        active_rollout_path(home.path(), Uuid::from_u128(11)),
        thread_id,
        Some(history_base),
    )?;

    let index = RolloutReferenceIndex::scan(home.path()).await?;

    assert_eq!(index.history_base(thread_id), Some(&history_base));
    assert_eq!(index.reference_count(thread_id), 0);
    Ok(())
}

#[tokio::test]
async fn expired_deadline_returns_none() -> anyhow::Result<()> {
    let home = TempDir::new()?;

    let index =
        RolloutReferenceIndex::scan_until(home.path(), Instant::now(), Duration::ZERO).await?;

    assert!(index.is_none());
    Ok(())
}

fn active_rollout_path(home: &Path, uuid: Uuid) -> PathBuf {
    home.join("sessions/2025/01/03")
        .join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"))
}

fn archived_rollout_path(home: &Path, uuid: Uuid) -> PathBuf {
    home.join("archived_sessions")
        .join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"))
}

fn write_rollout(
    path: PathBuf,
    thread_id: ThreadId,
    history_base: Option<HistoryPosition>,
) -> anyhow::Result<()> {
    fs::create_dir_all(path.parent().expect("rollout parent"))?;
    let session_meta = serde_json::json!({
        "timestamp": "2025-01-03T12:00:00Z",
        "type": "session_meta",
        "payload": {
            "id": thread_id,
            "timestamp": "2025-01-03T12:00:00Z",
            "cwd": path.parent().expect("rollout parent"),
            "originator": "test",
            "cli_version": "test",
            "source": "cli",
            "model_provider": "test-provider",
            "history_mode": "paginated",
            "history_base": history_base,
        },
    });
    fs::write(path, format!("{session_meta}\n"))?;
    Ok(())
}

fn history_position(thread_id: ThreadId) -> HistoryPosition {
    HistoryPosition {
        thread_id,
        end_ordinal_exclusive: 2,
        end_byte_offset: 100,
    }
}

fn thread_id(uuid: Uuid) -> anyhow::Result<ThreadId> {
    Ok(ThreadId::from_string(&uuid.to_string())?)
}

fn compress_now(path: &Path) -> anyhow::Result<()> {
    let compressed_path = path.with_extension("jsonl.zst");
    let input = fs::File::open(path)?;
    let output = fs::File::create(compressed_path)?;
    let mut encoder = zstd::stream::write::Encoder::new(output, 3)?;
    std::io::copy(&mut std::io::BufReader::new(input), &mut encoder)?;
    encoder.finish()?;
    fs::remove_file(path)?;
    Ok(())
}
