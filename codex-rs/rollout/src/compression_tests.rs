use codex_utils_absolute_path::test_support::PathExt;
use std::fs;
use std::fs::FileTimes;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use std::time::SystemTime;

use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

use super::*;
use crate::RolloutConfig;
use crate::RolloutRecorder;
use crate::RolloutRecorderParams;
use crate::append_rollout_item_to_path;
use crate::read_session_meta_line;
use crate::search_rollout_matches;

#[tokio::test]
async fn load_rollout_items_reads_compressed_rollout() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(1);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "hello compressed")?;
    compress_now(&rollout_path)?;

    let (items, loaded_thread_id, parse_errors) =
        RolloutRecorder::load_rollout_items(&rollout_path).await?;

    assert_eq!(loaded_thread_id, Some(thread_id));
    assert_eq!(parse_errors, 0);
    assert_eq!(items.len(), 2);
    assert!(!rollout_path.exists());
    assert!(compressed_rollout_path(&rollout_path).exists());
    Ok(())
}

#[tokio::test]
async fn read_session_meta_line_stops_before_invalid_utf8_tail() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(16);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "ignored tail")?;
    let mut contents = fs::read(&rollout_path)?;
    contents.resize(contents.len() + 64 * 1024, 0xff);
    fs::write(&rollout_path, contents)?;

    let session_meta = read_session_meta_line(&rollout_path).await?;

    assert_eq!(session_meta.meta.id, thread_id);
    Ok(())
}

#[tokio::test]
async fn read_session_meta_line_stops_before_invalid_utf8_compressed_tail() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(17);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "ignored compressed tail")?;
    let mut contents = fs::read(&rollout_path)?;
    contents.resize(contents.len() + 64 * 1024, 0xff);
    fs::write(&rollout_path, contents)?;
    compress_now(&rollout_path)?;

    let session_meta = read_session_meta_line(&rollout_path).await?;

    assert_eq!(session_meta.meta.id, thread_id);
    assert!(!rollout_path.exists());
    assert!(compressed_rollout_path(&rollout_path).exists());
    Ok(())
}

#[tokio::test]
async fn read_session_meta_line_preserves_pre_header_and_error_semantics() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(18);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "after metadata")?;
    let contents = fs::read_to_string(&rollout_path)?;
    let (session_meta, tail) = contents.split_once('\n').expect("session metadata line");
    let mut session_meta = serde_json::from_str::<serde_json::Value>(session_meta)?;
    session_meta["payload"]
        .as_object_mut()
        .expect("session metadata payload")
        .remove("session_id");
    let contents = format!("{session_meta}\n{tail}");
    let ignored_event = serde_json::json!({
        "timestamp": "2025-01-03T12:00:00Z",
        "type": "event_msg",
        "payload": {"type": "user_message", "message": "before metadata", "kind": "plain"}
    });
    fs::write(
        &rollout_path,
        format!("\nnot json\n{ignored_event}\n{contents}"),
    )?;

    let session_meta = read_session_meta_line(&rollout_path).await?;

    assert_eq!(session_meta.meta.id, thread_id);

    let response_item = serde_json::json!({
        "timestamp": "2025-01-03T12:00:00Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "before metadata"}]
        }
    });
    fs::write(&rollout_path, format!("{response_item}\n{contents}"))?;

    let error = read_session_meta_line(&rollout_path)
        .await
        .expect_err("response item before metadata should fail");

    assert_eq!(
        error.to_string(),
        format!(
            "rollout at {} does not start with session metadata",
            rollout_path.display()
        )
    );

    let unknown_history_mode = serde_json::json!({
        "timestamp": "2025-01-03T12:00:00Z",
        "type": "session_meta",
        "payload": {
            "session_id": uuid.to_string(),
            "id": uuid.to_string(),
            "timestamp": "2025-01-03T12:00:00Z",
            "cwd": ".",
            "originator": "test",
            "cli_version": "test",
            "source": "cli",
            "model_provider": "test-provider",
            "history_mode": "future",
        },
    });
    fs::write(&rollout_path, format!("{unknown_history_mode}\n{contents}"))?;

    let error = read_session_meta_line(&rollout_path)
        .await
        .expect_err("unknown history mode before valid metadata should fail");

    assert!(
        error
            .to_string()
            .starts_with("invalid session metadata history_mode:"),
        "{error}"
    );

    fs::write(&rollout_path, "\nnot json\n")?;

    let error = read_session_meta_line(&rollout_path)
        .await
        .expect_err("rollout without selected records should fail");

    assert_eq!(
        error.to_string(),
        format!("rollout at {} is empty", rollout_path.display())
    );
    Ok(())
}

#[test]
fn rollout_file_from_path_normalizes_compressed_file_names() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(7);
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    let compressed_path = compressed_rollout_path(&rollout_path);

    assert_eq!(
        RolloutFile::from_path(compressed_path.clone()),
        Some(RolloutFile {
            path: compressed_path,
            plain_file_name: format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"),
        })
    );
    Ok(())
}

#[test]
fn rollout_file_from_path_hides_compressed_sibling_when_plain_exists() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(8);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "plain wins")?;

    assert_eq!(
        RolloutFile::from_path(compressed_rollout_path(&rollout_path)),
        None
    );
    Ok(())
}

#[tokio::test]
async fn append_rollout_item_materializes_compressed_rollout() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(2);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "hello before append")?;
    compress_now(&rollout_path)?;

    append_rollout_item_to_path(
        &rollout_path,
        &RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "hello after append".to_string(),
            ..Default::default()
        })),
    )
    .await?;

    assert!(rollout_path.exists());
    assert!(!compressed_rollout_path(&rollout_path).exists());
    let (items, loaded_thread_id, parse_errors) =
        RolloutRecorder::load_rollout_items(&rollout_path).await?;
    assert_eq!(loaded_thread_id, Some(thread_id));
    assert_eq!(parse_errors, 0);
    assert_eq!(items.len(), 3);
    Ok(())
}

#[tokio::test]
async fn search_rollout_matches_uses_logical_path_for_compressed_rollout() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(15);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "targeted search term")?;
    compress_now(&rollout_path)?;

    let matches = search_rollout_matches(
        std::path::Path::new("missing-rg-for-test"),
        home.path(),
        /*archived*/ false,
        "search term",
    )
    .await?;

    assert_eq!(
        matches.get(rollout_path.as_path()),
        Some(&Some("targeted search term".to_string()))
    );
    Ok(())
}

#[tokio::test]
async fn worker_compresses_old_active_and_archived_rollouts() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let active_uuid = Uuid::from_u128(3);
    let active_id = ThreadId::from_string(&active_uuid.to_string())?;
    let active_path = rollout_path(home.path(), "2025-01-03T12-00-00", active_uuid);
    write_rollout(&active_path, active_id, "old active")?;
    set_old_mtime(&active_path)?;

    let archived_uuid = Uuid::from_u128(4);
    let archived_id = ThreadId::from_string(&archived_uuid.to_string())?;
    let archived_path = archived_rollout_path(home.path(), "2025-01-04T12-00-00", archived_uuid);
    write_rollout(&archived_path, archived_id, "old archived")?;
    set_old_mtime(&archived_path)?;

    let fresh_uuid = Uuid::from_u128(5);
    let fresh_id = ThreadId::from_string(&fresh_uuid.to_string())?;
    let fresh_path = rollout_path(home.path(), "2025-01-05T12-00-00", fresh_uuid);
    write_rollout(&fresh_path, fresh_id, "fresh active")?;

    let stale_temp = active_path.with_file_name("rollout-stale.jsonl.zst.tmp");
    fs::write(&stale_temp, "stale temp")?;
    set_old_mtime(&stale_temp)?;

    let fresh_temp = active_path.with_file_name("rollout-fresh.jsonl.zst.tmp");
    fs::write(&fresh_temp, "fresh temp")?;

    worker::run(home.path().to_path_buf()).await?;

    assert!(!active_path.exists());
    assert!(compressed_rollout_path(&active_path).exists());
    assert!(!archived_path.exists());
    assert!(compressed_rollout_path(&archived_path).exists());
    assert!(fresh_path.exists());
    assert!(!compressed_rollout_path(&fresh_path).exists());
    assert!(!stale_temp.exists());
    assert!(fresh_temp.exists());
    assert!(
        home.path()
            .join(".tmp")
            .join("rollout-compression.lock")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn worker_skips_archived_paginated_fork_pointer_chain() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let source_uuid = Uuid::from_u128(16);
    let source_id = ThreadId::from_string(&source_uuid.to_string())?;
    let source_path = rollout_path(home.path(), "2025-01-03T12-00-00", source_uuid);
    write_rollout(&source_path, source_id, "referenced source")?;
    set_old_mtime(&source_path)?;

    let child_uuid = Uuid::from_u128(17);
    let child_id = ThreadId::from_string(&child_uuid.to_string())?;
    let child_path = archived_rollout_path(home.path(), "2025-01-03T12-00-01", child_uuid);
    write_rollout(&child_path, child_id, "fork child")?;
    set_history_base(
        child_path.as_path(),
        HistoryPosition {
            thread_id: source_id,
            end_ordinal_exclusive: 2,
            end_byte_offset: std::fs::metadata(source_path.as_path())?.len(),
        },
    )?;
    set_old_mtime(&child_path)?;

    worker::run(home.path().to_path_buf()).await?;

    assert!(source_path.exists());
    assert!(child_path.exists());
    Ok(())
}

#[tokio::test]
async fn worker_skips_source_referenced_by_archived_compressed_rollout() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let source_uuid = Uuid::from_u128(18);
    let source_id = ThreadId::from_string(&source_uuid.to_string())?;
    let source_path = rollout_path(home.path(), "2025-01-03T12-00-00", source_uuid);
    write_rollout(&source_path, source_id, "referenced source")?;
    set_old_mtime(&source_path)?;

    let child_uuid = Uuid::from_u128(19);
    let child_id = ThreadId::from_string(&child_uuid.to_string())?;
    let child_path = archived_rollout_path(home.path(), "2025-01-03T12-00-01", child_uuid);
    write_rollout(&child_path, child_id, "fork child")?;
    set_history_base(
        child_path.as_path(),
        HistoryPosition {
            thread_id: source_id,
            end_ordinal_exclusive: 2,
            end_byte_offset: std::fs::metadata(source_path.as_path())?.len(),
        },
    )?;
    compress_now(child_path.as_path())?;

    worker::run(home.path().to_path_buf()).await?;

    assert!(source_path.exists());
    Ok(())
}

#[tokio::test]
async fn worker_skips_unreadable_metadata_without_blocking_other_compression() -> anyhow::Result<()>
{
    let home = TempDir::new()?;
    let source_uuid = Uuid::from_u128(20);
    let source_id = ThreadId::from_string(&source_uuid.to_string())?;
    let source_path = rollout_path(home.path(), "2025-01-03T12-00-00", source_uuid);
    write_rollout(&source_path, source_id, "candidate")?;
    set_old_mtime(&source_path)?;

    let unreadable_path = rollout_path(home.path(), "2025-01-03T12-00-01", Uuid::from_u128(21));
    fs::write(unreadable_path.as_path(), "{not json}\n")?;

    worker::run(home.path().to_path_buf()).await?;

    assert!(!source_path.exists());
    assert!(compressed_rollout_path(&source_path).exists());
    assert!(unreadable_path.exists());
    Ok(())
}

#[tokio::test]
async fn resume_materializes_compressed_rollout_path() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let config = RolloutConfig {
        codex_home: home.path().to_path_buf(),
        sqlite: codex_state::SqliteConfig::new_for_testing(home.path().abs()),
        cwd: home.path().to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: true,
    };
    let uuid = Uuid::from_u128(3);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "hello before resume")?;
    compress_now(&rollout_path)?;
    let compressed_path = compressed_rollout_path(&rollout_path);

    let InitialHistory::Resumed(history) =
        RolloutRecorder::get_rollout_history(compressed_path.as_path()).await?
    else {
        panic!("expected compressed rollout to load as resumed history");
    };
    assert_eq!(history.rollout_path, Some(rollout_path.clone()));

    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::resume(compressed_path.clone()),
    )
    .await?;

    assert_eq!(recorder.rollout_path(), rollout_path.as_path());
    assert!(rollout_path.exists());
    assert!(!compressed_path.exists());
    recorder
        .record_canonical_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(
            UserMessageEvent {
                message: "hello after resume".to_string(),
                ..Default::default()
            },
        ))])
        .await?;
    recorder.flush().await?;
    recorder.shutdown().await?;

    let (items, loaded_thread_id, parse_errors) =
        RolloutRecorder::load_rollout_items(&rollout_path).await?;
    assert_eq!(loaded_thread_id, Some(thread_id));
    assert_eq!(parse_errors, 0);
    assert_eq!(items.len(), 3);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn compression_preserves_rollout_permissions() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(6);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = archived_rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "restricted transcript")?;
    fs::set_permissions(&rollout_path, fs::Permissions::from_mode(0o600))?;
    set_old_mtime(&rollout_path)?;

    worker::run(home.path().to_path_buf()).await?;

    let compressed_path = compressed_rollout_path(&rollout_path);
    assert!(!rollout_path.exists());
    assert_eq!(
        fs::metadata(&compressed_path)?.permissions().mode() & 0o777,
        0o600
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn append_materialization_preserves_compressed_rollout_permissions() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(6);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "restricted transcript")?;
    compress_now(&rollout_path)?;
    let compressed_path = compressed_rollout_path(&rollout_path);
    fs::set_permissions(&compressed_path, fs::Permissions::from_mode(0o600))?;

    append_rollout_item_to_path(
        &rollout_path,
        &RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "materialize restricted transcript".to_string(),
            ..Default::default()
        })),
    )
    .await?;

    assert!(rollout_path.exists());
    assert!(!compressed_path.exists());
    assert_eq!(
        fs::metadata(&rollout_path)?.permissions().mode() & 0o777,
        0o600
    );
    Ok(())
}

#[test]
fn persist_temp_file_noclobber_installs_completed_temp() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let temp_path = home.path().join("rollout.jsonl.tmp");
    let destination = home.path().join("rollout.jsonl");
    fs::write(&temp_path, "completed rollout")?;

    persist_temp_file_noclobber(&temp_path, &destination)?;

    assert!(!temp_path.exists());
    assert_eq!(fs::read_to_string(destination)?, "completed rollout");
    Ok(())
}

#[test]
fn persist_temp_file_noclobber_does_not_replace_existing_destination() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let temp_path = home.path().join("rollout.jsonl.tmp");
    let destination = home.path().join("rollout.jsonl");
    fs::write(&temp_path, "candidate rollout")?;
    fs::write(&destination, "existing rollout")?;

    persist_temp_file_noclobber(&temp_path, &destination)?;

    assert!(!temp_path.exists());
    assert_eq!(fs::read_to_string(destination)?, "existing rollout");
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn compression_preserves_read_only_rollout_permissions() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(7);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = archived_rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "read-only transcript")?;
    set_old_mtime(&rollout_path)?;
    fs::set_permissions(&rollout_path, fs::Permissions::from_mode(0o400))?;
    let source_modified = fs::metadata(&rollout_path)?.modified()?;

    worker::run(home.path().to_path_buf()).await?;

    let compressed_path = compressed_rollout_path(&rollout_path);
    let compressed_metadata = fs::metadata(&compressed_path)?;
    assert!(!rollout_path.exists());
    assert_eq!(compressed_metadata.permissions().mode() & 0o777, 0o400);
    assert_eq!(compressed_metadata.modified()?, source_modified);
    Ok(())
}

#[tokio::test]
async fn worker_skips_existing_compressed_archived_rollouts() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(10);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = archived_rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "already compressed")?;
    compress_now(&rollout_path)?;
    let compressed_path = compressed_rollout_path(&rollout_path);
    set_old_mtime(&compressed_path)?;

    worker::run(home.path().to_path_buf()).await?;

    assert!(!rollout_path.exists());
    assert!(compressed_path.exists());
    let (items, loaded_thread_id, parse_errors) =
        RolloutRecorder::load_rollout_items(&rollout_path).await?;
    assert_eq!(loaded_thread_id, Some(thread_id));
    assert_eq!(parse_errors, 0);
    assert_eq!(items.len(), 2);
    Ok(())
}

#[tokio::test]
async fn worker_skips_when_fresh_run_marker_exists() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(11);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = archived_rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "throttled worker")?;
    set_old_mtime(&rollout_path)?;
    let marker_dir = home.path().join(".tmp");
    fs::create_dir_all(marker_dir.as_path())?;
    fs::write(marker_dir.join("rollout-compression.lock"), "recent run")?;

    worker::run(home.path().to_path_buf()).await?;

    assert!(rollout_path.exists());
    assert!(!compressed_rollout_path(&rollout_path).exists());
    Ok(())
}

#[test]
fn run_marker_is_removed_unless_persisted() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let marker_path = home.path().join(".tmp").join("rollout-compression.lock");

    {
        let marker = worker::CompressionRunMarker::try_claim(home.path())?;
        assert!(marker.is_some());
    }
    assert!(!marker_path.exists());

    let marker = worker::CompressionRunMarker::try_claim(home.path())?;
    let Some(marker) = marker else {
        panic!("expected run marker claim");
    };
    marker.persist();
    assert!(marker_path.exists());
    assert!(worker::CompressionRunMarker::try_claim(home.path())?.is_none());
    Ok(())
}

#[tokio::test]
async fn find_thread_path_by_id_handles_compressed_rollout_filenames() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(8);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid);
    write_rollout(&rollout_path, thread_id, "compressed filename lookup")?;
    compress_now(&rollout_path)?;
    let compressed_path = compressed_rollout_path(&rollout_path);

    assert_eq!(
        crate::find_thread_path_by_id_str(
            home.path(),
            &uuid.to_string(),
            /*state_db_ctx*/ None
        )
        .await?,
        Some(compressed_path)
    );
    assert_eq!(
        crate::find_thread_path_by_id_str(home.path(), "not-a-uuid", /*state_db_ctx*/ None).await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn find_thread_path_by_id_ignores_compression_temp_matches() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let uuid = Uuid::from_u128(9);
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let temp_path = rollout_path(home.path(), "2025-01-03T12-00-00", uuid).with_file_name(format!(
        "rollout-2025-01-03T12-00-00-{uuid}.jsonl.zst.compress.1.0.tmp"
    ));
    write_rollout(&temp_path, thread_id, "temporary file should not resolve")?;

    assert_eq!(
        crate::find_thread_path_by_id_str(
            home.path(),
            &uuid.to_string(),
            /*state_db_ctx*/ None
        )
        .await?,
        None
    );
    Ok(())
}

fn rollout_path(home: &std::path::Path, ts: &str, uuid: Uuid) -> std::path::PathBuf {
    home.join("sessions/2025/01/03")
        .join(format!("rollout-{ts}-{uuid}.jsonl"))
}

fn archived_rollout_path(home: &std::path::Path, ts: &str, uuid: Uuid) -> std::path::PathBuf {
    home.join("archived_sessions")
        .join(format!("rollout-{ts}-{uuid}.jsonl"))
}

fn write_rollout(path: &std::path::Path, thread_id: ThreadId, message: &str) -> anyhow::Result<()> {
    let parent = path.parent().expect("rollout path should have parent");
    fs::create_dir_all(parent)?;
    let session_meta_line = SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            forked_from_id: None,
            parent_thread_id: None,
            timestamp: "2025-01-03T12:00:00Z".to_string(),
            cwd: parent.to_path_buf(),
            originator: "test".to_string(),
            cli_version: "test".to_string(),
            source: SessionSource::Cli,
            thread_source: None,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
            model_provider: None,
            base_instructions: None,
            dynamic_tools: None,
            selected_capability_roots: Vec::new(),
            memory_mode: None,
            history_mode: Default::default(),
            history_base: None,
            subagent_history_start_ordinal: None,
            multi_agent_version: None,
            context_window: None,
        },
        git: None,
    };
    let lines = [
        RolloutLine {
            timestamp: "2025-01-03T12:00:00Z".to_string(),
            ordinal: None,
            item: RolloutItem::SessionMeta(session_meta_line),
        },
        RolloutLine {
            timestamp: "2025-01-03T12:00:01Z".to_string(),
            ordinal: None,
            item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: message.to_string(),
                ..Default::default()
            })),
        },
    ];
    let jsonl = lines
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    fs::write(path, format!("{jsonl}\n"))?;
    Ok(())
}

fn set_history_base(path: &std::path::Path, history_base: HistoryPosition) -> anyhow::Result<()> {
    let contents = fs::read_to_string(path)?;
    let mut lines = contents.lines();
    let mut head: serde_json::Value = serde_json::from_str(lines.next().expect("session meta"))?;
    head["payload"]["history_base"] = serde_json::to_value(history_base)?;
    let mut updated = serde_json::to_string(&head)?;
    for line in lines {
        updated.push('\n');
        updated.push_str(line);
    }
    updated.push('\n');
    fs::write(path, updated)?;
    Ok(())
}

fn compress_now(path: &std::path::Path) -> anyhow::Result<()> {
    let compressed_path = compressed_rollout_path(path);
    let input = fs::File::open(path)?;
    let output = fs::File::create(compressed_path)?;
    let mut encoder = zstd::stream::write::Encoder::new(output, 3)?;
    let mut input = std::io::BufReader::new(input);
    std::io::copy(&mut input, &mut encoder)?;
    encoder.finish()?;
    fs::remove_file(path)?;
    Ok(())
}

fn set_old_mtime(path: &std::path::Path) -> anyhow::Result<()> {
    let old = SystemTime::now()
        .checked_sub(Duration::from_secs(8 * 24 * 60 * 60))
        .expect("old timestamp should be representable");
    let times = FileTimes::new().set_modified(old);
    fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(times)?;
    Ok(())
}
