use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
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

/// Find the most recently updated thread id for a thread name, if any.
pub async fn find_thread_id_by_name(
    codex_home: &Path,
    name: &str,
) -> std::io::Result<Option<ThreadId>> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let path = session_index_path(codex_home);
    if !path.exists() {
        return Ok(None);
    }
    let name = name.to_string();
    let entry = tokio::task::spawn_blocking(move || scan_index_from_end_by_name(&path, &name))
        .await
        .map_err(std::io::Error::other)??;
    Ok(entry.map(|entry| entry.id))
}

/// Locate a recorded thread rollout file by thread name using newest-first ordering.
/// Returns `Ok(Some(path))` if found, `Ok(None)` if not present.
pub async fn find_thread_path_by_name_str(
    codex_home: &Path,
    name: &str,
) -> std::io::Result<Option<PathBuf>> {
    let Some(thread_id) = find_thread_id_by_name(codex_home, name).await? else {
        return Ok(None);
    };
    super::list::find_thread_path_by_id_str(codex_home, &thread_id.to_string()).await
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

fn scan_index_from_end_by_name(
    path: &Path,
    name: &str,
) -> std::io::Result<Option<SessionIndexEntry>> {
    scan_index_from_end(path, |entry| entry.thread_name == name)
}

fn scan_index_from_end<F>(
    path: &Path,
    mut predicate: F,
) -> std::io::Result<Option<SessionIndexEntry>>
where
    F: FnMut(&SessionIndexEntry) -> bool,
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
                if let Some(entry) = parse_line_from_rev(&mut line_rev, &mut predicate)? {
                    return Ok(Some(entry));
                }
                continue;
            }
            line_rev.push(byte);
        }
    }

    if let Some(entry) = parse_line_from_rev(&mut line_rev, &mut predicate)? {
        return Ok(Some(entry));
    }

    Ok(None)
}

fn parse_line_from_rev<F>(
    line_rev: &mut Vec<u8>,
    predicate: &mut F,
) -> std::io::Result<Option<SessionIndexEntry>>
where
    F: FnMut(&SessionIndexEntry) -> bool,
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
    if predicate(&entry) {
        return Ok(Some(entry));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    fn write_index(path: &Path, lines: &[SessionIndexEntry]) -> std::io::Result<()> {
        let mut out = String::new();
        for entry in lines {
            out.push_str(&serde_json::to_string(entry).unwrap());
            out.push('\n');
        }
        std::fs::write(path, out)
    }

    #[test]
    fn find_thread_id_by_name_prefers_latest_entry() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let path = session_index_path(temp.path());
        let id1 = ThreadId::new();
        let id2 = ThreadId::new();
        let lines = vec![
            SessionIndexEntry {
                id: id1,
                thread_name: "same".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionIndexEntry {
                id: id2,
                thread_name: "same".to_string(),
                updated_at: "2024-01-02T00:00:00Z".to_string(),
            },
        ];
        write_index(&path, &lines)?;

        let found = scan_index_from_end_by_name(&path, "same")?;
        assert_eq!(found.map(|entry| entry.id), Some(id2));
        Ok(())
    }

    #[test]
    fn find_thread_name_by_id_prefers_latest_entry() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let path = session_index_path(temp.path());
        let id = ThreadId::new();
        let lines = vec![
            SessionIndexEntry {
                id,
                thread_name: "first".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionIndexEntry {
                id,
                thread_name: "second".to_string(),
                updated_at: "2024-01-02T00:00:00Z".to_string(),
            },
        ];
        write_index(&path, &lines)?;

        let found = scan_index_from_end_by_id(&path, &id)?;
        assert_eq!(
            found.map(|entry| entry.thread_name),
            Some("second".to_string())
        );
        Ok(())
    }

    #[test]
    fn scan_index_returns_none_when_entry_missing() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let path = session_index_path(temp.path());
        let id = ThreadId::new();
        let lines = vec![SessionIndexEntry {
            id,
            thread_name: "present".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }];
        write_index(&path, &lines)?;

        let missing_name = scan_index_from_end_by_name(&path, "missing")?;
        assert_eq!(missing_name, None);

        let missing_id = scan_index_from_end_by_id(&path, &ThreadId::new())?;
        assert_eq!(missing_id, None);
        Ok(())
    }

    #[test]
    fn scan_index_finds_latest_match_among_mixed_entries() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let path = session_index_path(temp.path());
        let id_target = ThreadId::new();
        let id_other = ThreadId::new();
        let expected = SessionIndexEntry {
            id: id_target,
            thread_name: "target".to_string(),
            updated_at: "2024-01-03T00:00:00Z".to_string(),
        };
        let expected_other = SessionIndexEntry {
            id: id_other,
            thread_name: "target".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        };
        // Resolution is based on append order (scan from end), not updated_at.
        let lines = vec![
            SessionIndexEntry {
                id: id_target,
                thread_name: "target".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            expected_other.clone(),
            expected.clone(),
            SessionIndexEntry {
                id: ThreadId::new(),
                thread_name: "another".to_string(),
                updated_at: "2024-01-04T00:00:00Z".to_string(),
            },
        ];
        write_index(&path, &lines)?;

        let found_by_name = scan_index_from_end_by_name(&path, "target")?;
        assert_eq!(found_by_name, Some(expected.clone()));

        let found_by_id = scan_index_from_end_by_id(&path, &id_target)?;
        assert_eq!(found_by_id, Some(expected));

        let found_other_by_id = scan_index_from_end_by_id(&path, &id_other)?;
        assert_eq!(found_other_by_id, Some(expected_other));
        Ok(())
    }
}
