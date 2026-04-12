use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMetaLine;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;

const SESSION_INDEX_FILE: &str = "session_index.jsonl";
const READ_CHUNK_SIZE: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionIndexEntry {
    pub id: ThreadId,
    pub thread_name: String,
    pub updated_at: String,
}

/// Append a thread name update to the session index.
/// The index is append-only; the most recent entry wins when resolving names or ids.
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
    append_session_index_entry(codex_home, &entry).await
}

/// Append a raw session index entry to `session_index.jsonl`.
/// The file is append-only; consumers scan from the end to find the newest match.
pub async fn append_session_index_entry(
    codex_home: &Path,
    entry: &SessionIndexEntry,
) -> std::io::Result<()> {
    let path = session_index_path(codex_home);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    let mut line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
    line.push('\n');
    file.write_all(line.as_bytes()).await?;
    file.flush().await?;
    Ok(())
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
    let path = session_index_path(codex_home);
    if thread_ids.is_empty() || !path.exists() {
        return Ok(HashMap::new());
    }

    let file = tokio::fs::File::open(&path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut names = HashMap::with_capacity(thread_ids.len());

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(trimmed) else {
            continue;
        };
        let name = entry.thread_name.trim();
        if !name.is_empty() && thread_ids.contains(&entry.id) {
            names.insert(entry.id, name.to_string());
        }
    }

    Ok(names)
}

/// Find all thread ids whose latest indexed thread name exactly matches `name`.
pub async fn find_thread_ids_by_name(
    codex_home: &Path,
    name: &str,
) -> std::io::Result<Vec<ThreadId>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(Vec::new());
    }
    let path = session_index_path(codex_home);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = tokio::fs::File::open(&path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut latest_names = HashMap::<ThreadId, String>::new();

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(trimmed) else {
            continue;
        };
        let thread_name = entry.thread_name.trim();
        if thread_name.is_empty() {
            continue;
        }
        latest_names.insert(entry.id, thread_name.to_string());
    }

    let mut ids = latest_names
        .into_iter()
        .filter_map(|(thread_id, thread_name)| (thread_name == name).then_some(thread_id))
        .collect::<Vec<_>>();
    ids.sort_by_key(ToString::to_string);
    Ok(ids)
}

/// Locate a recorded thread rollout and read its session metadata by thread name.
/// Returns the newest indexed name that still has a readable rollout header.
pub async fn find_thread_meta_by_name_str(
    codex_home: &Path,
    name: &str,
) -> std::io::Result<Option<(PathBuf, SessionMetaLine)>> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let path = session_index_path(codex_home);
    if !path.exists() {
        return Ok(None);
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let name = name.to_string();
    // Stream matching ids newest-first instead of stopping at the first name hit: the newest entry
    // may point at a thread whose rollout was never materialized.
    let scan =
        tokio::task::spawn_blocking(move || stream_thread_ids_from_end_by_name(&path, &name, tx));

    while let Some(thread_id) = rx.recv().await {
        // Keep walking until a matching id resolves to a loadable rollout so an unsaved or partial
        // rename cannot shadow an older persisted session with the same name.
        if let Some(path) =
            super::list::find_thread_path_by_id_str(codex_home, &thread_id.to_string()).await?
            && let Ok(session_meta) = super::list::read_session_meta_line(&path).await
        {
            drop(rx);
            scan.await.map_err(std::io::Error::other)??;
            return Ok(Some((path, session_meta)));
        }
    }
    scan.await.map_err(std::io::Error::other)??;

    Ok(None)
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
    let mut file = File::open(path)?;
    let mut remaining = file.metadata()?.len();
    let mut line_rev: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; READ_CHUNK_SIZE];

    while remaining > 0 {
        let read_size = usize::try_from(remaining.min(READ_CHUNK_SIZE as u64))
            .map_err(std::io::Error::other)?;
        remaining -= read_size as u64;
        file.seek(SeekFrom::Start(remaining))?;
        file.read_exact(&mut buf[..read_size])?;

        for &byte in buf[..read_size].iter().rev() {
            if byte == b'\n' {
                if let Some(entry) = parse_line_from_rev(&mut line_rev, &mut visit_entry)? {
                    return Ok(Some(entry));
                }
                continue;
            }
            line_rev.push(byte);
        }
    }

    if let Some(entry) = parse_line_from_rev(&mut line_rev, &mut visit_entry)? {
        return Ok(Some(entry));
    }

    Ok(None)
}

fn parse_line_from_rev<F>(
    line_rev: &mut Vec<u8>,
    visit_entry: &mut F,
) -> std::io::Result<Option<SessionIndexEntry>>
where
    F: FnMut(&SessionIndexEntry) -> std::io::Result<Option<SessionIndexEntry>>,
{
    if line_rev.is_empty() {
        return Ok(None);
    }
    line_rev.reverse();
    let line = std::mem::take(line_rev);
    let Ok(mut line) = String::from_utf8(line) else {
        return Ok(None);
    };
    if line.ends_with('\r') {
        line.pop();
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(trimmed) else {
        return Ok(None);
    };
    visit_entry(&entry)
}

#[cfg(test)]
#[path = "session_index_tests.rs"]
mod tests;
