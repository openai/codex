//! Functionality to persist a Codex conversation *rollout* – a linear list of
//! [`ResponseItem`] objects exchanged during a session – to disk so that
//! sessions can be replayed or inspected later (mirrors the behaviour of the
//! upstream TypeScript implementation).

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::io::ErrorKind;

use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{self};
use uuid::Uuid;

use crate::config::codex_dir;
use crate::models::ResponseItem;

/// Folder inside `~/.codex` that holds saved rollouts.
const SESSIONS_SUBDIR: &str = "sessions";

#[derive(Serialize)]
struct SessionMeta {
    id: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
}

/// Records all [`ResponseItem`]s for a session and flushes them to disk after
/// every update.
///
/// Rollouts are recorded as JSONL and can be inspected with tools such as:
///
/// ```ignore
/// $ jq -C . ~/.codex/sessions/rollout-2025-05-07-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// $ fx ~/.codex/sessions/rollout-2025-05-07-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// ```
#[derive(Clone)]
pub(crate) struct RolloutRecorder {
    tx: Sender<String>,
}

impl RolloutRecorder {
    /// Attempt to create a new [`RolloutRecorder`]. If the sessions directory
    /// cannot be created or the rollout file cannot be opened we return the
    /// error so the caller can decide whether to disable persistence.
    pub async fn new(instructions: Option<String>) -> std::io::Result<Self> {
        let LogFileInfo {
            file,
            session_id,
            timestamp,
        } = create_log_file()?;

        // Build the static session metadata JSON first.
        let timestamp_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        let timestamp = timestamp.format(timestamp_format).map_err(|e| {
            IoError::new(ErrorKind::Other, format!("failed to format timestamp: {e}"))
        })?;

        let meta = SessionMeta {
            timestamp,
            id: session_id.to_string(),
            instructions,
        };

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller’s thread.
        let (tx, mut rx) = mpsc::channel::<String>(256);

        // Spawn a Tokio task that owns the file handle and performs async
        // writes. Using `tokio::fs::File` keeps everything on the async I/O
        // driver instead of blocking the runtime.
        tokio::task::spawn(async move {
            let mut file = tokio::fs::File::from_std(file);

            while let Some(line) = rx.recv().await {
                // Write line + newline, then flush to disk.
                if let Err(e) = file.write_all(line.as_bytes()).await {
                    tracing::warn!("rollout writer: failed to write line: {e}");
                    break;
                }
                if let Err(e) = file.write_all(b"\n").await {
                    tracing::warn!("rollout writer: failed to write newline: {e}");
                    break;
                }
                if let Err(e) = file.flush().await {
                    tracing::warn!("rollout writer: failed to flush: {e}");
                    break;
                }
            }
        });

        let recorder = Self { tx };
        // Ensure SessionMeta is the first item in the file.
        recorder.record_item(&meta).await?;
        Ok(recorder)
    }

    /// Append `items` to the rollout file.
    pub(crate) async fn record_items(&self, items: &[ResponseItem]) -> std::io::Result<()> {
        for item in items {
            match item {
                // Note that function calls may look a bit strange if they are
                // "fully qualified MCP tool calls," so we could consider
                // reformatting them in that case.
                ResponseItem::Message { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::FunctionCallOutput { .. } => {}
                ResponseItem::Other => {
                    // These should never be serialized.
                    continue;
                }
            }
            self.record_item(item).await?;
        }
        Ok(())
    }

    async fn record_item(&self, item: &impl Serialize) -> std::io::Result<()> {
        // Serialize the item to JSON first so that the writer thread only has
        // to perform the actual write.
        let json = serde_json::to_string(item).map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!("failed to serialize response items: {e}"),
            )
        })?;

        self.tx.send(json).await.map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!("failed to queue rollout item: {e}"),
            )
        })
    }
}

struct LogFileInfo {
    /// Opened file handle to the rollout file.
    file: File,

    /// Session ID (also embedded in filename).
    session_id: Uuid,

    /// Timestamp for the start of the session.
    timestamp: OffsetDateTime,
}

fn create_log_file() -> std::io::Result<LogFileInfo> {
    // Resolve ~/.codex/sessions and create it if missing.
    let mut dir = codex_dir()?;
    dir.push(SESSIONS_SUBDIR);
    fs::create_dir_all(&dir)?;

    // Generate a v4 UUID – matches the JS CLI implementation.
    let session_id = Uuid::new_v4();
    let timestamp = OffsetDateTime::now_utc();

    // Custom format for YYYY-MM-DD.
    let format: &[FormatItem] = format_description!("[year]-[month]-[day]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::new(ErrorKind::Other, format!("failed to format timestamp: {e}")))?;

    let filename = format!("rollout-{date_str}-{session_id}.jsonl");

    let path = dir.join(filename);
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;

    Ok(LogFileInfo {
        file,
        session_id,
        timestamp,
    })
}
