//! Store session names in an append-only index and serialize index rewrites.
//! Writers must share the filesystem lock namespace and use this locking protocol.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use crate::reverse_jsonl_scanner::ReverseJsonlScanner;
use crate::reverse_jsonl_scanner::ScanOutcome;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use serde::Deserialize;
use serde::Serialize;

const SESSION_INDEX_FILE: &str = "session_index.jsonl";
const SESSION_INDEX_LOCK_FILE: &str = "session_index.jsonl.lock";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionIndexEntry {
    pub id: ThreadId,
    pub thread_name: String,
    pub updated_at: String,
}

/// Append a thread name update to the session index.
/// Name updates are append-only; the most recent entry wins when resolving names or ids.
pub async fn append_thread_name(
    codex_home: &Path,
    thread_id: ThreadId,
    name: &str,
) -> std::io::Result<()> {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    let updated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let entry = SessionIndexEntry {
        id: thread_id,
        thread_name: name.to_string(),
        updated_at,
    };
    append_session_index_entry(codex_home, entry).await
}

/// Append a raw session index entry to `session_index.jsonl`.
/// Consumers scan from the end to find the newest match.
pub async fn append_session_index_entry(
    codex_home: &Path,
    entry: SessionIndexEntry,
) -> std::io::Result<()> {
    let path = session_index_path(codex_home);
    with_session_index_lock(codex_home, move || {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let mut line = serde_json::to_string(&entry).map_err(std::io::Error::other)?;
        line.push('\n');
        file.write_all(line.as_bytes())?;
        file.flush()?;
        Ok(())
    })
    .await
}

/// Remove all recorded names for a thread from the session index.
pub async fn remove_thread_name_entries(
    codex_home: &Path,
    thread_id: ThreadId,
) -> std::io::Result<()> {
    let path = session_index_path(codex_home);
    if !path.try_exists()? {
        return Ok(());
    }
    with_session_index_lock(codex_home, move || {
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };
        let mut removed = false;
        let mut remaining = String::with_capacity(contents.len());
        for line in contents.lines() {
            let should_remove = serde_json::from_str::<SessionIndexEntry>(line.trim())
                .is_ok_and(|entry| entry.id == thread_id);
            if should_remove {
                removed = true;
            } else {
                remaining.push_str(line);
                remaining.push('\n');
            }
        }
        if !removed {
            return Ok(());
        }
        let mut temporary = tempfile::Builder::new()
            .prefix("session-index-")
            .tempfile_in(path.parent().unwrap_or(Path::new(".")))?;
        temporary
            .as_file()
            .set_permissions(std::fs::metadata(&path)?.permissions())?;
        temporary.write_all(remaining.as_bytes())?;
        temporary.persist(&path).map_err(|error| error.error)?;
        Ok(())
    })
    .await
}

/// Serialize index mutations within and across processes, including cancelled updates.
async fn with_session_index_lock(
    codex_home: &Path,
    update: impl FnOnce() -> std::io::Result<()> + Send + 'static,
) -> std::io::Result<()> {
    let lock_path = codex_home.join(SESSION_INDEX_LOCK_FILE);
    tokio::task::spawn_blocking(move || {
        // Keep this file in place. If we replace it, another process can lock a new
        // file while this update still holds the old file's lock.
        let file_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        let deadline = Instant::now() + Duration::from_secs(/*secs*/ 30);
        loop {
            match file_lock.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(/*millis*/ 50));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(std::io::Error::new(
                        ErrorKind::TimedOut,
                        "timed out waiting for the session index lock; another Codex process may be suspended on this or another host",
                    ));
                }
                Err(std::fs::TryLockError::Error(err)) => return Err(err),
            }
        }
        update()
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Find the latest thread name for a thread id, if any.
pub async fn find_thread_name_by_id(
    codex_home: &Path,
    thread_id: &ThreadId,
) -> std::io::Result<Option<String>> {
    let path = session_index_path(codex_home);
    if !path.exists() {
        return Ok(None);
    }
    let id = *thread_id;
    let entry = tokio::task::spawn_blocking(move || scan_index_from_end_by_id(&path, &id))
        .await
        .map_err(std::io::Error::other)??;
    Ok(entry.map(|entry| entry.thread_name))
}

/// Find the latest thread names for a batch of thread ids.
pub async fn find_thread_names_by_ids(
    codex_home: &Path,
    thread_ids: &HashSet<ThreadId>,
) -> std::io::Result<HashMap<ThreadId, String>> {
    if thread_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let path = session_index_path(codex_home);
    let mut remaining_ids = thread_ids.clone();
    tokio::task::spawn_blocking(move || {
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let mut scanner = ReverseJsonlScanner::new(File::open(path)?)?;
        let mut names = HashMap::with_capacity(remaining_ids.len());
        while let Some(outcome) = scanner.scan_next::<SessionIndexEntry>()? {
            let ScanOutcome::Parsed(entry) = outcome else {
                continue;
            };
            let name = entry.thread_name.trim();
            // The first nonempty name seen for an id is its latest usable name.
            if !name.is_empty() && remaining_ids.remove(&entry.id) {
                names.insert(entry.id, name.to_string());
                if remaining_ids.is_empty() {
                    break;
                }
            }
        }
        Ok(names)
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Locate the readable rollout with the newest modification time for a recorded thread name.
pub async fn find_thread_meta_by_name_str(
    codex_home: &Path,
    name: &str,
    state_db_ctx: Option<&codex_state::StateRuntime>,
) -> std::io::Result<Option<(PathBuf, SessionMetaLine)>> {
    Ok(find_thread_meta_candidates_by_name_str(
        codex_home,
        name,
        state_db_ctx,
        /*allowed_sources*/ &[],
        /*allowed_model_providers*/ &[],
    )
    .await?
    .into_iter()
    .next())
}

/// Locate readable rollouts for a recorded thread name, newest modification time first.
///
/// Empty filter slices disable their respective filters. Provider filtering only rejects explicit
/// mismatches because older rollouts omitted provider metadata. Filtering happens before ranking,
/// so an ineligible newer duplicate cannot hide an older usable session.
pub async fn find_thread_meta_candidates_by_name_str(
    codex_home: &Path,
    name: &str,
    state_db_ctx: Option<&codex_state::StateRuntime>,
    allowed_sources: &[SessionSource],
    allowed_model_providers: &[String],
) -> std::io::Result<Vec<(PathBuf, SessionMetaLine)>> {
    if name.trim().is_empty() {
        return Ok(Vec::new());
    }
    let path = session_index_path(codex_home);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let name = name.to_string();
    // Stream matching ids newest-first instead of stopping at the first name hit: the newest entry
    // may point at a thread whose rollout was never materialized.
    let scan =
        tokio::task::spawn_blocking(move || stream_thread_ids_from_end_by_name(&path, &name, tx));

    let mut candidates = Vec::new();
    while let Some(thread_id) = rx.recv().await {
        // Keep walking until a matching id resolves to a loadable rollout so an unsaved or partial
        // rename cannot shadow an older persisted session with the same name.
        if let Some(path) = super::list::find_thread_path_by_id_str(
            codex_home,
            &thread_id.to_string(),
            state_db_ctx,
        )
        .await?
            && let Ok(session_meta) = super::list::read_session_meta_line(&path).await
            && !session_meta.meta.attached_thread
            && (allowed_sources.is_empty() || allowed_sources.contains(&session_meta.meta.source))
            && session_meta
                .meta
                .model_provider
                .as_ref()
                .is_none_or(|provider| {
                    allowed_model_providers.is_empty() || allowed_model_providers.contains(provider)
                })
        {
            let modified = tokio::fs::metadata(&path)
                .await
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((modified, path, session_meta));
        }
    }
    scan.await.map_err(std::io::Error::other)??;
    candidates.sort_by(|(left, _, _), (right, _, _)| right.cmp(left));

    Ok(candidates
        .into_iter()
        .map(|(_, path, session_meta)| (path, session_meta))
        .collect())
}

fn session_index_path(codex_home: &Path) -> PathBuf {
    codex_home.join(SESSION_INDEX_FILE)
}

fn scan_index_from_end_by_id(
    path: &Path,
    thread_id: &ThreadId,
) -> std::io::Result<Option<SessionIndexEntry>> {
    scan_index_from_end(path, |entry| entry.id == *thread_id)
}

fn stream_thread_ids_from_end_by_name(
    path: &Path,
    name: &str,
    tx: tokio::sync::mpsc::Sender<ThreadId>,
) -> std::io::Result<()> {
    let mut seen = HashSet::new();
    scan_index_from_end_for_each(path, |entry| {
        // The first row seen for an id is its latest name. Ignore older rows for that id so a
        // historical name cannot be treated as the current one after the thread is renamed.
        if seen.insert(entry.id) && entry.thread_name == name && tx.blocking_send(entry.id).is_err()
        {
            return Ok(Some(entry.clone()));
        }
        Ok(None)
    })?;
    Ok(())
}

fn scan_index_from_end<F>(
    path: &Path,
    mut predicate: F,
) -> std::io::Result<Option<SessionIndexEntry>>
where
    F: FnMut(&SessionIndexEntry) -> bool,
{
    scan_index_from_end_for_each(path, |entry| {
        if predicate(entry) {
            return Ok(Some(entry.clone()));
        }
        Ok(None)
    })
}

fn scan_index_from_end_for_each<F>(
    path: &Path,
    mut visit_entry: F,
) -> std::io::Result<Option<SessionIndexEntry>>
where
    F: FnMut(&SessionIndexEntry) -> std::io::Result<Option<SessionIndexEntry>>,
{
    let mut scanner = ReverseJsonlScanner::new(File::open(path)?)?;
    while let Some(outcome) = scanner.scan_next::<SessionIndexEntry>()? {
        let ScanOutcome::Parsed(entry) = outcome else {
            continue;
        };
        if let Some(entry) = visit_entry(&entry)? {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "session_index_tests.rs"]
mod tests;
