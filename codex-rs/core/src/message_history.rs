//! Persistence layer for the global, append-only *message history* file.
//!
//! The history is stored at `~/.codex/history.jsonl` with **one JSON object per
//! line** so that it can be efficiently appended to and parsed with standard
//! JSON-Lines tooling. Each record has the following schema:
//!
//! ````text
//! {"conversation_id":"<uuid>","ts":<unix_seconds>,"text":"<message>"}
//! ````
//!
//! To minimise the chance of interleaved writes when multiple processes are
//! appending concurrently, callers should *prepare the full line* (record +
//! trailing `\n`) and write it with a **single `write(2)` system call** while
//! the file descriptor is opened with the `O_APPEND` flag. POSIX guarantees
//! that writes up to `PIPE_BUF` bytes are atomic in that case.

use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Result;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::config::Config;
use crate::config::types::HistoryPersistence;

use codex_protocol::ConversationId;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Filename that stores the message history inside `~/.codex`.
const HISTORY_FILENAME: &str = "history.jsonl";

/// When history exceeds the hard cap, trim it down to this fraction of `max_bytes`.
const HISTORY_SOFT_CAP_RATIO: f64 = 0.8;

const MAX_RETRIES: usize = 10;
const RETRY_SLEEP: Duration = Duration::from_millis(100);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub session_id: String,
    pub ts: u64,
    pub text: String,
}

fn history_filepath(config: &Config) -> PathBuf {
    let mut path = config.codex_home.clone();
    path.push(HISTORY_FILENAME);
    path
}

/// Append a `text` entry associated with `conversation_id` to the history file. Uses
/// advisory file locking to ensure that concurrent writes do not interleave,
/// which entails a small amount of blocking I/O internally.
pub(crate) async fn append_entry(
    text: &str,
    conversation_id: &ConversationId,
    config: &Config,
) -> Result<()> {
    match config.history.persistence {
        HistoryPersistence::SaveAll => {
            // Save everything: proceed.
        }
        HistoryPersistence::None => {
            // No history persistence requested.
            return Ok(());
        }
    }

    // TODO: check `text` for sensitive patterns

    // Resolve `~/.codex/history.jsonl` and ensure the parent directory exists.
    let path = history_filepath(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Compute timestamp (seconds since the Unix epoch).
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| std::io::Error::other(format!("system clock before Unix epoch: {e}")))?
        .as_secs();

    // Construct the JSON line first so we can write it in a single syscall.
    let entry = HistoryEntry {
        session_id: conversation_id.to_string(),
        ts,
        text: text.to_string(),
    };
    let mut line = serde_json::to_string(&entry)
        .map_err(|e| std::io::Error::other(format!("failed to serialise history entry: {e}")))?;
    line.push('\n');

    // Open the history file for read/write access (append-only on Unix).
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.append(true);
        options.mode(0o600);
    }

    let mut history_file = options.open(&path)?;

    // Ensure permissions.
    ensure_owner_only_permissions(&history_file).await?;

    let history_max_bytes = config.history.max_bytes;

    // Perform a blocking write under an advisory write lock using std::fs.
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Retry a few times to avoid indefinite blocking when contended.
        for _ in 0..MAX_RETRIES {
            match history_file.try_lock() {
                Ok(()) => {
                    // While holding the exclusive lock, write the full line.
                    // We do not open the file with `append(true)` on Windows, so ensure the
                    // cursor is positioned at the end before writing.
                    history_file.seek(SeekFrom::End(0))?;
                    history_file.write_all(line.as_bytes())?;
                    history_file.flush()?;
                    enforce_history_limit(&mut history_file, history_max_bytes)?;
                    return Ok(());
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(RETRY_SLEEP);
                }
                Err(e) => return Err(e.into()),
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "could not acquire exclusive lock on history file after multiple attempts",
        ))
    })
    .await??;

    Ok(())
}

/// Trim the history file to honor `max_bytes`, dropping the oldest lines while holding
/// the write lock so the newest entry is always retained. When the file exceeds the
/// hard cap, it rewrites the remaining tail to a soft cap to avoid trimming again
/// immediately on the next write.
fn enforce_history_limit(file: &mut File, max_bytes: Option<usize>) -> Result<()> {
    let Some(max_bytes) = max_bytes else {
        return Ok(());
    };

    if max_bytes == 0 {
        return Ok(());
    }

    let max_bytes = match u64::try_from(max_bytes) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };

    let mut current_len = file.metadata()?.len();

    if current_len <= max_bytes {
        return Ok(());
    }

    let mut reader_file = file.try_clone()?;
    reader_file.seek(SeekFrom::Start(0))?;

    let mut buf_reader = BufReader::new(reader_file);
    let mut line_lengths = Vec::new();
    let mut line_buf = String::new();

    loop {
        line_buf.clear();

        let bytes = buf_reader.read_line(&mut line_buf)?;

        if bytes == 0 {
            break;
        }

        line_lengths.push(bytes as u64);
    }

    if line_lengths.is_empty() {
        return Ok(());
    }

    let last_index = line_lengths.len() - 1;
    let trim_target = trim_target_bytes(max_bytes, line_lengths[last_index]);

    let mut drop_bytes = 0u64;
    let mut idx = 0usize;

    while current_len > trim_target && idx < last_index {
        current_len = current_len.saturating_sub(line_lengths[idx]);
        drop_bytes += line_lengths[idx];
        idx += 1;
    }

    if drop_bytes == 0 {
        return Ok(());
    }

    let mut reader = buf_reader.into_inner();
    reader.seek(SeekFrom::Start(drop_bytes))?;

    let capacity = usize::try_from(current_len).unwrap_or(0);
    let mut tail = Vec::with_capacity(capacity);

    reader.read_to_end(&mut tail)?;

    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&tail)?;
    file.flush()?;

    Ok(())
}

fn trim_target_bytes(max_bytes: u64, newest_entry_len: u64) -> u64 {
    let soft_cap_bytes = ((max_bytes as f64) * HISTORY_SOFT_CAP_RATIO)
        .floor()
        .clamp(1.0, max_bytes as f64) as u64;

    soft_cap_bytes.max(newest_entry_len)
}

/// Asynchronously fetch the history file's *identifier* (inode on Unix) and
/// the current number of entries by counting newline characters.
pub(crate) async fn history_metadata(config: &Config) -> (u64, usize) {
    let path = history_filepath(config);
    history_metadata_for_file(&path).await
}

/// Given a `log_id` (on Unix this is the file's inode number,
/// on Windows this is the file's creation time) and a zero-based
/// `offset`, return the corresponding `HistoryEntry` if the identifier matches
/// the current history file **and** the requested offset exists. Any I/O or
/// parsing errors are logged and result in `None`.
///
/// Note this function is not async because it uses a sync advisory file
/// locking API.
pub(crate) fn lookup(log_id: u64, offset: usize, config: &Config) -> Option<HistoryEntry> {
    let path = history_filepath(config);
    lookup_history_entry(&path, log_id, offset)
}

/// On Unix systems, ensure the file permissions are `0o600` (rw-------). If the
/// permissions cannot be changed the error is propagated to the caller.
#[cfg(unix)]
async fn ensure_owner_only_permissions(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    let current_mode = metadata.permissions().mode() & 0o777;
    if current_mode != 0o600 {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let perms_clone = perms.clone();
        let file_clone = file.try_clone()?;
        tokio::task::spawn_blocking(move || file_clone.set_permissions(perms_clone)).await??;
    }
    Ok(())
}

#[cfg(windows)]
// On Windows, simply succeed.
async fn ensure_owner_only_permissions(_file: &File) -> Result<()> {
    Ok(())
}

async fn history_metadata_for_file(path: &Path) -> (u64, usize) {
    let log_id = match fs::metadata(path).await {
        Ok(metadata) => history_log_id(&metadata).unwrap_or(0),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (0, 0),
        Err(_) => return (0, 0),
    };

    // Open the file.
    let mut file = match fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return (log_id, 0),
    };

    // Count newline bytes.
    let mut buf = [0u8; 8192];
    let mut count = 0usize;
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                count += buf[..n].iter().filter(|&&b| b == b'\n').count();
            }
            Err(_) => return (log_id, 0),
        }
    }

    (log_id, count)
}

fn lookup_history_entry(path: &Path, log_id: u64, offset: usize) -> Option<HistoryEntry> {
    use std::io::BufRead;
    use std::io::BufReader;

    let file: File = match OpenOptions::new().read(true).open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, "failed to open history file");
            return None;
        }
    };

    let metadata = match file.metadata() {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to stat history file");
            return None;
        }
    };

    let current_log_id = history_log_id(&metadata)?;

    if log_id != 0 && current_log_id != log_id {
        return None;
    }

    // Open & lock file for reading using a shared lock.
    // Retry a few times to avoid indefinite blocking.
    for _ in 0..MAX_RETRIES {
        let lock_result = file.try_lock_shared();

        match lock_result {
            Ok(()) => {
                let reader = BufReader::new(&file);
                for (idx, line_res) in reader.lines().enumerate() {
                    let line = match line_res {
                        Ok(l) => l,
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to read line from history file");
                            return None;
                        }
                    };

                    if idx == offset {
                        match serde_json::from_str::<HistoryEntry>(&line) {
                            Ok(entry) => return Some(entry),
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to parse history entry");
                                return None;
                            }
                        }
                    }
                }
                // Not found at requested offset.
                return None;
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                std::thread::sleep(RETRY_SLEEP);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to acquire shared lock on history file");
                return None;
            }
        }
    }

    None
}

#[cfg(unix)]
fn history_log_id(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino())
}

#[cfg(windows)]
fn history_log_id(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::windows::fs::MetadataExt;
    Some(metadata.creation_time())
}

#[cfg(not(any(unix, windows)))]
fn history_log_id(_metadata: &std::fs::Metadata) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use codex_protocol::ConversationId;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn lookup_reads_history_entries() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let history_path = temp_dir.path().join(HISTORY_FILENAME);

        let entries = vec![
            HistoryEntry {
                session_id: "first-session".to_string(),
                ts: 1,
                text: "first".to_string(),
            },
            HistoryEntry {
                session_id: "second-session".to_string(),
                ts: 2,
                text: "second".to_string(),
            },
        ];

        let mut file = File::create(&history_path).expect("create history file");
        for entry in &entries {
            writeln!(
                file,
                "{}",
                serde_json::to_string(entry).expect("serialize history entry")
            )
            .expect("write history entry");
        }

        let (log_id, count) = history_metadata_for_file(&history_path).await;
        assert_eq!(count, entries.len());

        let second_entry =
            lookup_history_entry(&history_path, log_id, 1).expect("fetch second history entry");
        assert_eq!(second_entry, entries[1]);
    }

    #[tokio::test]
    async fn lookup_uses_stable_log_id_after_appends() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let history_path = temp_dir.path().join(HISTORY_FILENAME);

        let initial = HistoryEntry {
            session_id: "first-session".to_string(),
            ts: 1,
            text: "first".to_string(),
        };
        let appended = HistoryEntry {
            session_id: "second-session".to_string(),
            ts: 2,
            text: "second".to_string(),
        };

        let mut file = File::create(&history_path).expect("create history file");
        writeln!(
            file,
            "{}",
            serde_json::to_string(&initial).expect("serialize initial entry")
        )
        .expect("write initial entry");

        let (log_id, count) = history_metadata_for_file(&history_path).await;
        assert_eq!(count, 1);

        let mut append = std::fs::OpenOptions::new()
            .append(true)
            .open(&history_path)
            .expect("open history file for append");
        writeln!(
            append,
            "{}",
            serde_json::to_string(&appended).expect("serialize appended entry")
        )
        .expect("append history entry");

        let fetched =
            lookup_history_entry(&history_path, log_id, 1).expect("lookup appended history entry");
        assert_eq!(fetched, appended);
    }

    #[tokio::test]
    async fn append_entry_trims_history_when_beyond_max_bytes() {
        let codex_home = TempDir::new().expect("create temp dir");

        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load config");

        let conversation_id = ConversationId::new();

        let entry_one = "a".repeat(200);
        let entry_two = "b".repeat(200);

        let history_path = codex_home.path().join("history.jsonl");

        append_entry(&entry_one, &conversation_id, &config)
            .await
            .expect("write first entry");

        let first_len = std::fs::metadata(&history_path).expect("metadata").len();
        let limit_bytes = first_len + 10;

        config.history.max_bytes =
            Some(usize::try_from(limit_bytes).expect("limit should fit into usize"));

        append_entry(&entry_two, &conversation_id, &config)
            .await
            .expect("write second entry");

        let contents = std::fs::read_to_string(&history_path).expect("read history");

        let entries = contents
            .lines()
            .map(|line| serde_json::from_str::<HistoryEntry>(line).expect("parse entry"))
            .collect::<Vec<HistoryEntry>>();

        assert_eq!(
            entries.len(),
            1,
            "only one entry left because entry_one should be evicted"
        );
        assert_eq!(entries[0].text, entry_two);
        assert!(std::fs::metadata(&history_path).expect("metadata").len() <= limit_bytes);
    }

    #[tokio::test]
    async fn append_entry_trims_history_to_soft_cap() {
        let codex_home = TempDir::new().expect("create temp dir");

        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load config");

        let conversation_id = ConversationId::new();

        let short_entry = "a".repeat(200);
        let long_entry = "b".repeat(400);

        let history_path = codex_home.path().join("history.jsonl");

        append_entry(&short_entry, &conversation_id, &config)
            .await
            .expect("write first entry");

        let short_entry_len = std::fs::metadata(&history_path).expect("metadata").len();

        append_entry(&long_entry, &conversation_id, &config)
            .await
            .expect("write second entry");

        let two_entry_len = std::fs::metadata(&history_path).expect("metadata").len();

        let long_entry_len = two_entry_len
            .checked_sub(short_entry_len)
            .expect("second entry length should be larger than first entry length");

        config.history.max_bytes = Some(
            usize::try_from((2 * long_entry_len) + (short_entry_len / 2))
                .expect("max bytes should fit into usize"),
        );

        append_entry(&long_entry, &conversation_id, &config)
            .await
            .expect("write third entry");

        let contents = std::fs::read_to_string(&history_path).expect("read history");

        let entries = contents
            .lines()
            .map(|line| serde_json::from_str::<HistoryEntry>(line).expect("parse entry"))
            .collect::<Vec<HistoryEntry>>();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, long_entry);

        let pruned_len = std::fs::metadata(&history_path).expect("metadata").len();
        let max_bytes = config
            .history
            .max_bytes
            .expect("max bytes should be configured") as u64;

        assert!(pruned_len <= max_bytes);

        let soft_cap_bytes = ((max_bytes as f64) * HISTORY_SOFT_CAP_RATIO)
            .floor()
            .clamp(1.0, max_bytes as f64) as u64;
        let len_without_first = 2 * long_entry_len;

        assert!(
            len_without_first <= max_bytes,
            "dropping only the first entry would satisfy the hard cap"
        );
        assert!(
            len_without_first > soft_cap_bytes,
            "soft cap should require more aggressive trimming than the hard cap"
        );

        assert_eq!(pruned_len, long_entry_len);
        assert!(pruned_len <= soft_cap_bytes.max(long_entry_len));
    }
}
