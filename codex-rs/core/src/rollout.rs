//! Functionality to persist a Codex conversation *rollout* – a linear list of
//! [`ResponseItem`] objects exchanged during a session – to disk so that
//! sessions can be replayed or inspected later (mirrors the behaviour of the
//! upstream TypeScript implementation).

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{self};
use uuid::Uuid;

use crate::config::Config;
use crate::models::ResponseItem;

/// Folder inside `~/.codex` that holds saved rollouts.
const SESSIONS_SUBDIR: &str = "sessions";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SessionMeta {
    pub id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SessionStateSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SavedSession {
    pub session: SessionMeta,
    #[serde(default)]
    pub items: Vec<ResponseItem>,
    #[serde(default)]
    pub state: SessionStateSnapshot,
}

/// Records all [`ResponseItem`]s for a session and flushes them to disk after
/// every update.
///
/// Rollouts are recorded as JSONL and can be inspected with tools such as:
///
/// ```ignore
/// $ jq -C . ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// $ fx ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// ```
#[derive(Clone)]
pub(crate) struct RolloutRecorder {
    tx: Sender<RolloutCmd>,
}

#[derive(Clone)]
enum RolloutCmd {
    AddItems(Vec<ResponseItem>),
    UpdateState(SessionStateSnapshot),
}

// ─────────────────────────────────────────────────────────────────────────────
// JSONL Write Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Write the in-memory `SavedSession` out as **JSONL**:
///  1. SessionMeta line
///  2. One line per `ResponseItem`
///  3. SessionStateSnapshot line
///
/// Each line is compact (no pretty formatting) so callers that do
/// `for line in content.lines()` can parse each line independently.
async fn write_session(file: &mut tokio::fs::File, data: &SavedSession) {
    // Start from scratch each time (simple & safe for small files).
    if file.seek(std::io::SeekFrom::Start(0)).await.is_err() {
        return;
    }
    if file.set_len(0).await.is_err() {
        return;
    }

    // Session meta
    if let Ok(json) = serde_json::to_string(&data.session) {
        let _ = file.write_all(json.as_bytes()).await;
        let _ = file.write_all(b"\n").await;
    }

    // Items
    for item in &data.items {
        if let Ok(json) = serde_json::to_string(item) {
            let _ = file.write_all(json.as_bytes()).await;
            let _ = file.write_all(b"\n").await;
        }
    }

    // State (always last)
    if let Ok(json) = serde_json::to_string(&data.state) {
        let _ = file.write_all(json.as_bytes()).await;
        let _ = file.write_all(b"\n").await;
    }

    let _ = file.flush().await;
}

impl RolloutRecorder {
    /// Attempt to create a new [`RolloutRecorder`]. If the sessions directory
    /// cannot be created or the rollout file cannot be opened we return the
    /// error so the caller can decide whether to disable persistence.
    pub async fn new(
        config: &Config,
        uuid: Uuid,
        instructions: Option<String>,
    ) -> std::io::Result<Self> {
        let LogFileInfo {
            file,
            session_id,
            timestamp,
        } = create_log_file(config, uuid)?;

        // Build the static session metadata JSON first.
        let timestamp_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        let timestamp = timestamp
            .format(timestamp_format)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

        let meta = SessionMeta {
            timestamp,
            id: session_id.to_string(),
            instructions,
        };

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller’s thread.
        let (tx, mut rx) = mpsc::channel::<RolloutCmd>(256);

        let mut data = SavedSession {
            session: meta,
            items: Vec::new(),
            state: SessionStateSnapshot::default(),
        };

        tokio::task::spawn(async move {
            let mut file = tokio::fs::File::from_std(file);
            write_session(&mut file, &data).await;

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RolloutCmd::AddItems(items) => data.items.extend(items),
                    RolloutCmd::UpdateState(state) => data.state = state,
                }
                write_session(&mut file, &data).await;
            }
        });

        Ok(Self { tx })
    }

    /// Append `items` to the rollout file.
    pub(crate) async fn record_items(&self, items: &[ResponseItem]) -> std::io::Result<()> {
        let mut filtered = Vec::new();
        for item in items {
            match item {
                ResponseItem::Message { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::FunctionCallOutput { .. } => filtered.push(item.clone()),
                ResponseItem::Reasoning { .. } | ResponseItem::Other => {}
            }
        }
        if filtered.is_empty() {
            return Ok(());
        }
        self.tx
            .send(RolloutCmd::AddItems(filtered))
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout items: {e}")))
    }

    pub(crate) async fn record_state(&self, state: SessionStateSnapshot) -> std::io::Result<()> {
        self.tx
            .send(RolloutCmd::UpdateState(state))
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout state: {e}")))
    }

    /// Reopen an existing JSONL session file.
    ///
    /// Format expected (see module docs):
    ///   line 0: `SessionMeta`
    ///   lines 1..N-1: `ResponseItem`
    ///   line N: `SessionStateSnapshot`
    pub async fn resume(path: &Path) -> std::io::Result<(Self, SavedSession)> {
        let text = tokio::fs::read_to_string(path).await?;

        // Collect non-empty lines.
        let mut lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            return Err(IoError::other("empty session file"));
        }

        // Session meta (required)
        let meta_line = lines.remove(0);
        let session: SessionMeta = serde_json::from_str(meta_line)
            .map_err(|e| IoError::other(format!("failed to parse session meta: {e}")))?;

        // State is last (required, but may be {})
        let state_line = lines.pop().unwrap_or("{}");
        let state: SessionStateSnapshot = serde_json::from_str(state_line)
            .map_err(|e| IoError::other(format!("failed to parse session state: {e}")))?;

        // Remaining are items
        let mut items = Vec::new();
        for line in lines {
            let item: ResponseItem = serde_json::from_str(line).map_err(|e| {
                IoError::other(format!("failed to parse response item: {e}; line={line:?}"))
            })?;
            items.push(item);
        }

        let saved = SavedSession {
            session,
            items,
            state,
        };

        // Reopen file for appending (rewrites go through write_session)
        let file = std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .open(path)?;
        let saved_clone = saved.clone();
        let (tx, mut rx) = mpsc::channel::<RolloutCmd>(256);
        tokio::task::spawn(async move {
            let mut data = saved_clone;
            let mut file = tokio::fs::File::from_std(file);
            write_session(&mut file, &data).await;
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RolloutCmd::AddItems(items) => data.items.extend(items),
                    RolloutCmd::UpdateState(state) => data.state = state,
                }
                write_session(&mut file, &data).await;
            }
        });

        Ok((Self { tx }, saved))
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

fn create_log_file(config: &Config, session_id: Uuid) -> std::io::Result<LogFileInfo> {
    // Resolve ~/.codex/sessions/YYYY/MM/DD and create it if missing.
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
    dir.push(timestamp.year().to_string());
    dir.push(format!("{:02}", u8::from(timestamp.month())));
    dir.push(format!("{:02}", timestamp.day()));
    fs::create_dir_all(&dir)?;

    // Custom format for YYYY-MM-DDThh-mm-ss. Use `-` instead of `:` for
    // compatibility with filesystems that do not allow colons in filenames.
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

    let filename = format!("rollout-{date_str}-{session_id}.jsonl");

    let path = dir.join(filename);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;

    Ok(LogFileInfo {
        file,
        session_id,
        timestamp,
    })
}
