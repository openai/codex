//! Functionality to persist a Codex conversation *rollout* – a linear list of
//! [`ResponseItem`] objects exchanged during a session – to disk so that
//! sessions can be replayed or inspected later (mirrors the behaviour of the
//! upstream TypeScript implementation).

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
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
/// Rollouts are recorded as JSON and can be inspected with tools such as:
///
/// ```ignore
/// $ jq -C . ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.json
/// $ fx ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.json
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

async fn write_session(file: &mut tokio::fs::File, data: &SavedSession) {
    if file.seek(std::io::SeekFrom::Start(0)).await.is_err() {
        return;
    }
    if file.set_len(0).await.is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_vec_pretty(data) {
        let _ = file.write_all(&json).await;
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

        let recorder = Self { tx };
        Ok(recorder)
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

    pub async fn resume(path: &std::path::Path) -> std::io::Result<(Self, SavedSession)> {
        let bytes = tokio::fs::read(path).await?;
        let saved: SavedSession = serde_json::from_slice(&bytes)
            .map_err(|e| IoError::other(format!("failed to parse session: {e}")))?;

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

    pub async fn load(path: &std::path::Path) -> std::io::Result<SavedSession> {
        let bytes = tokio::fs::read(path).await?;
        let saved: SavedSession = serde_json::from_slice(&bytes)
            .map_err(|e| IoError::other(format!("failed to parse session: {e}")))?;
        Ok(saved)
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
    // Resolve ~/.codex/sessions and create it if missing.
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
    fs::create_dir_all(&dir)?;

    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;

    // Custom format for YYYY-MM-DDThh-mm-ss. Use `-` instead of `:` for
    // compatibility with filesystems that do not allow colons in filenames.
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

    let filename = format!("rollout-{date_str}-{session_id}.json");

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
