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
use std::io::Result;
use std::io::Write;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::config::Config;
use crate::config_types::HistoryPersistence;

use codex_protocol::ConversationId;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Filename that stores the message history inside `~/.codex`.
const HISTORY_FILENAME: &str = "history.jsonl";

const MAX_RETRIES: usize = 10;
const RETRY_SLEEP: Duration = Duration::from_millis(100);

#[derive(Serialize, Deserialize, Debug, Clone)]
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

    // Open in append-only mode.
    let mut options = OpenOptions::new();
    options.append(true).read(true).create(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }

    let mut history_file = options.open(&path)?;

    // Ensure permissions.
    ensure_owner_only_permissions(&history_file).await?;

    // Perform a blocking write under an advisory write lock using std::fs.
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Retry a few times to avoid indefinite blocking when contended.
        for _ in 0..MAX_RETRIES {
            match history_file.try_lock() {
                Ok(()) => {
                    // While holding the exclusive lock, write the full line.
                    history_file.write_all(line.as_bytes())?;
                    history_file.flush()?;
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

/// Asynchronously fetch the history file's *identifier* (inode on Unix) and
/// the current number of entries by counting newline characters.
pub(crate) async fn history_metadata(config: &Config) -> (u64, usize) {
    let path = history_filepath(config);

    #[cfg(unix)]
    let log_id = {
        use std::os::unix::fs::MetadataExt;
        // Obtain metadata (async) to get the identifier.
        let meta = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (0, 0),
            Err(_) => return (0, 0),
        };
        meta.ino()
    };
    #[cfg(not(unix))]
    let log_id = 0u64;

    // Open the file.
    let mut file = match fs::File::open(&path).await {
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

/// Given a `log_id` (on Unix this is the file's inode number) and a zero-based
/// `offset`, return the corresponding `HistoryEntry` if the identifier matches
/// the current history file **and** the requested offset exists. Any I/O or
/// parsing errors are logged and result in `None`.
///
/// Note this function is not async because it uses a sync advisory file
/// locking API.
#[cfg(unix)]
pub(crate) fn lookup(log_id: u64, offset: usize, config: &Config) -> Option<HistoryEntry> {
    use std::io::BufRead;
    use std::io::BufReader;
    use std::os::unix::fs::MetadataExt;

    let path = history_filepath(config);
    let file: File = match OpenOptions::new().read(true).open(&path) {
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

    if metadata.ino() != log_id {
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

/// Fallback stub for non-Unix systems: currently always returns `None`.
#[cfg(not(unix))]
pub(crate) fn lookup(log_id: u64, offset: usize, config: &Config) -> Option<HistoryEntry> {
    let _ = (log_id, offset, config);
    None
}

/// On Unix systems ensure the file permissions are `0o600` (rw-------). If the
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

#[cfg(not(unix))]
async fn ensure_owner_only_permissions(_file: &File) -> Result<()> {
    // For now, on non-Unix, simply succeed.
    Ok(())
}
