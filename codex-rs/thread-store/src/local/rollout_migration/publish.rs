//! Filesystem helpers for safely publishing migrated rollouts.
//!
//! This module owns temporary rollout paths, durable `.pending` journals, compressed rollout
//! staging, cleanup, and parent-directory syncs. The migration orchestrator decides when to call
//! these helpers; this module keeps the low-level file operations in one place.
//!
//! A migration can crash between publishing JSONL and finishing SQLite metadata. The journal is
//! the durable handoff between those steps, so cleanup must only remove it once the paginated
//! rollout and SQLite state are both complete.

use std::collections::HashSet;
use std::fs::Permissions;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use codex_protocol::ThreadId;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;

use super::migration_error;
use crate::ThreadStoreResult;

const MIGRATION_JOURNAL_DIRECTORY: &str = "rollout-migrations";

pub(super) fn migration_journal_path(codex_home: &Path, thread_id: ThreadId) -> PathBuf {
    codex_home
        .join(MIGRATION_JOURNAL_DIRECTORY)
        .join(format!("{thread_id}.pending"))
}

pub(super) async fn pending_migration_thread_ids(
    codex_home: &Path,
) -> ThreadStoreResult<HashSet<ThreadId>> {
    let mut entries = match tokio::fs::read_dir(codex_home.join(MIGRATION_JOURNAL_DIRECTORY)).await
    {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(error) => return Err(migration_error(error)),
    };
    let mut thread_ids = HashSet::new();
    while let Some(entry) = entries.next_entry().await.map_err(migration_error)? {
        if !entry.file_type().await.map_err(migration_error)?.is_file() {
            continue;
        }
        let filename = entry.file_name();
        let Some(thread_id) = filename
            .to_str()
            .and_then(|filename| filename.strip_suffix(".pending"))
            .and_then(|thread_id| ThreadId::from_string(thread_id).ok())
        else {
            continue;
        };
        thread_ids.insert(thread_id);
    }
    Ok(thread_ids)
}

pub(super) fn staged_rollout_path(rollout_path: &Path) -> ThreadStoreResult<PathBuf> {
    staged_path(rollout_path, "paginated")
}

pub(super) fn decompressed_staged_rollout_path(rollout_path: &Path) -> ThreadStoreResult<PathBuf> {
    staged_path(rollout_path, "decompressed")
}

pub(super) fn compressed_staged_rollout_path(rollout_path: &Path) -> ThreadStoreResult<PathBuf> {
    staged_path(rollout_path, "paginated.zst")
}

fn staged_path(rollout_path: &Path, suffix: &str) -> ThreadStoreResult<PathBuf> {
    let filename = rollout_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .ok_or_else(|| migration_error("rollout path has no valid filename"))?;
    Ok(rollout_path.with_file_name(format!(".{filename}.{suffix}.tmp")))
}

pub(super) async fn decompress_rollout_to_path(
    compressed_path: &Path,
    plain_path: &Path,
) -> ThreadStoreResult<()> {
    let compressed_path = compressed_path.to_path_buf();
    let plain_path = plain_path.to_path_buf();
    tokio::task::spawn_blocking(move || -> io::Result<()> {
        let input = std::fs::File::open(compressed_path)?;
        let mut decoder = zstd::stream::read::Decoder::new(input)?;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut output = options.open(plain_path)?;
        #[cfg(unix)]
        output.set_permissions(Permissions::from_mode(0o600))?;
        io::copy(&mut decoder, &mut output)?;
        output.flush()
    })
    .await
    .map_err(migration_error)?
    .map_err(migration_error)
}

pub(super) async fn compress_rollout_to_path(
    plain_path: &Path,
    compressed_path: &Path,
    permissions: Permissions,
    modified_at: Option<SystemTime>,
) -> ThreadStoreResult<()> {
    let plain_path = plain_path.to_path_buf();
    let compressed_path = compressed_path.to_path_buf();
    tokio::task::spawn_blocking(move || -> io::Result<()> {
        let mut input = std::fs::File::open(plain_path)?;
        let output = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&compressed_path)?;
        let mut encoder = zstd::stream::write::Encoder::new(output, 3)?;
        io::copy(&mut input, &mut encoder)?;
        let output = encoder.finish()?;
        output.set_permissions(permissions)?;
        if let Some(modified_at) = modified_at {
            output.set_times(std::fs::FileTimes::new().set_modified(modified_at))?;
        }
        output.sync_all()?;

        let input = std::fs::File::open(compressed_path)?;
        let mut decoder = zstd::stream::read::Decoder::new(input)?;
        io::copy(&mut decoder, &mut io::sink())?;
        Ok(())
    })
    .await
    .map_err(migration_error)?
    .map_err(migration_error)
}

pub(super) fn rewritten_staged_rollout_path(staged_path: &Path) -> ThreadStoreResult<PathBuf> {
    let filename = staged_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .ok_or_else(|| migration_error("staged rollout path has no valid filename"))?;
    Ok(staged_path.with_file_name(format!("{filename}.head.tmp")))
}

pub(super) async fn rewrite_subagent_history_boundary(
    staged_path: &Path,
    boundary: u64,
) -> ThreadStoreResult<()> {
    let rewritten_path = rewritten_staged_rollout_path(staged_path)?;
    let permissions = tokio::fs::metadata(staged_path)
        .await
        .map_err(migration_error)?
        .permissions();
    let source = File::open(staged_path).await.map_err(migration_error)?;
    let mut source = BufReader::new(source);
    let mut head_bytes = Vec::new();
    source
        .read_until(b'\n', &mut head_bytes)
        .await
        .map_err(migration_error)?;
    let mut head = serde_json::from_slice::<RolloutLine>(&head_bytes).map_err(migration_error)?;
    let RolloutItem::SessionMeta(session_meta) = &mut head.item else {
        return Err(migration_error(
            "staged rollout head is not session metadata",
        ));
    };
    session_meta.meta.subagent_history_start_ordinal = Some(boundary);

    let rewritten = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&rewritten_path)
        .await
        .map_err(migration_error)?;
    rewritten
        .set_permissions(permissions)
        .await
        .map_err(migration_error)?;
    let mut rewritten = BufWriter::new(rewritten);
    rewritten
        .write_all(
            serde_json::to_string(&head)
                .map_err(migration_error)?
                .as_bytes(),
        )
        .await
        .map_err(migration_error)?;
    rewritten.write_all(b"\n").await.map_err(migration_error)?;
    tokio::io::copy(&mut source, &mut rewritten)
        .await
        .map_err(migration_error)?;
    rewritten.flush().await.map_err(migration_error)?;
    rewritten
        .get_ref()
        .sync_all()
        .await
        .map_err(migration_error)?;
    drop(rewritten);
    tokio::fs::rename(rewritten_path, staged_path)
        .await
        .map_err(migration_error)
}

pub(super) async fn remove_file_if_present(path: &Path) -> ThreadStoreResult<()> {
    if tokio::fs::try_exists(path).await.map_err(migration_error)? {
        tokio::fs::remove_file(path)
            .await
            .map_err(migration_error)?;
    }
    Ok(())
}

pub(super) async fn write_migration_journal(path: &Path) -> ThreadStoreResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| migration_error("rollout migration journal has no parent directory"))?;
    let parent_already_exists = tokio::fs::try_exists(parent)
        .await
        .map_err(migration_error)?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(migration_error)?;
    if !parent_already_exists {
        sync_parent_directory(parent).await?;
    }
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .await
        .map_err(migration_error)?;
    file.sync_all().await.map_err(migration_error)?;
    drop(file);
    sync_parent_directory(path).await
}

pub(super) async fn sync_parent_directory(path: &Path) -> ThreadStoreResult<()> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .ok_or_else(|| migration_error("rollout path has no parent directory"))?
            .to_path_buf();
        tokio::task::spawn_blocking(move || std::fs::File::open(parent)?.sync_all())
            .await
            .map_err(migration_error)?
            .map_err(migration_error)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
