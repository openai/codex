//! Persist Codex session rollouts (.jsonl) so sessions can be replayed or inspected later.

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{self};
use tokio::sync::oneshot;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::config::Config;
use crate::git_info::GitInfo;
use crate::git_info::collect_git_info;
use crate::models::ResponseItem;

const SESSIONS_SUBDIR: &str = "sessions";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SessionMeta {
    pub id: Uuid,
    pub timestamp: String,
    pub instructions: Option<String>,
}

#[derive(Serialize)]
struct SessionMetaWithGit {
    #[serde(flatten)]
    meta: SessionMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitInfo>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SessionStateSnapshot {}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SavedSession {
    pub session: SessionMeta,
    #[serde(default)]
    pub items: Vec<ResponseItem>,
    #[serde(default)]
    pub state: SessionStateSnapshot,
    pub session_id: Uuid,
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
    // Fan out commands to multiple background writers (JSONL and JSON snapshot).
    txs: Vec<Sender<RolloutCmd>>,
}

enum RolloutCmd {
    AddItems(Vec<ResponseItem>),
    UpdateState(SessionStateSnapshot),
    Shutdown { ack: oneshot::Sender<()> },
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
            timestamp: ts_local,
        } = create_log_file(config, uuid)?;

        let timestamp_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        let timestamp = ts_local
            .format(timestamp_format)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

        // Clone the cwd for the spawned task to collect git info asynchronously
        let cwd = config.cwd.clone();

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller's thread.
        let (tx_jsonl, rx_jsonl) = mpsc::channel::<RolloutCmd>(256);

        // Spawn a Tokio task that owns the JSONL file handle and performs async
        // writes. Using `tokio::fs::File` keeps everything on the async I/O
        // driver instead of blocking the runtime.
        tokio::task::spawn(rollout_writer(
            tokio::fs::File::from_std(file),
            rx_jsonl,
            Some(SessionMeta {
                timestamp,
                id: session_id,
                instructions,
            }),
            cwd.clone(),
        ));

        // Spawn a second background task that maintains a pretty-printed JSON
        // snapshot under ~/.codex/sessions/rollout-YYYY-MM-DD-<uuid>.json.
        let snapshot_path = create_snapshot_filepath(config, session_id, ts_local)?;
        let (tx_snapshot, rx_snapshot) = mpsc::channel::<RolloutCmd>(256);
        tokio::task::spawn(snapshot_writer(
            snapshot_path,
            rx_snapshot,
            SnapshotSessionMeta {
                timestamp: ts_local,
                id: session_id,
                // Start empty; will be set to the first user message when available.
                instructions: String::new(),
            },
            Vec::new(),
        ));

        Ok(Self {
            txs: vec![tx_jsonl, tx_snapshot],
        })
    }

    pub(crate) async fn record_items(&self, items: &[ResponseItem]) -> std::io::Result<()> {
        let mut filtered = Vec::new();
        for item in items {
            match item {
                // Note that function calls may look a bit strange if they are
                // "fully qualified MCP tool calls," so we could consider
                // reformatting them in that case.
                ResponseItem::Message { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::FunctionCallOutput { .. }
                | ResponseItem::Reasoning { .. } => filtered.push(item.clone()),
                ResponseItem::Other => {
                    // These should never be serialized.
                    continue;
                }
            }
        }
        if filtered.is_empty() {
            return Ok(());
        }
        // Send to all writers; if any fails, return error.
        let mut last_err: Option<std::io::Error> = None;
        for tx in &self.txs {
            if let Err(e) = tx.send(RolloutCmd::AddItems(filtered.clone())).await {
                last_err = Some(IoError::other(format!(
                    "failed to queue rollout items: {e}"
                )));
            }
        }
        if let Some(e) = last_err {
            Err(e)
        } else {
            Ok(())
        }
    }

    pub(crate) async fn record_state(&self, state: SessionStateSnapshot) -> std::io::Result<()> {
        let mut last_err: Option<std::io::Error> = None;
        for tx in &self.txs {
            if let Err(e) = tx.send(RolloutCmd::UpdateState(state.clone())).await {
                last_err = Some(IoError::other(format!(
                    "failed to queue rollout state: {e}"
                )));
            }
        }
        if let Some(e) = last_err {
            Err(e)
        } else {
            Ok(())
        }
    }

    pub async fn resume(
        path: &Path,
        cwd: std::path::PathBuf,
    ) -> std::io::Result<(Self, SavedSession)> {
        info!("Resuming rollout from {path:?}");
        let text = tokio::fs::read_to_string(path).await?;
        let mut lines = text.lines();
        let meta_line = lines
            .next()
            .ok_or_else(|| IoError::other("empty session file"))?;
        let session: SessionMeta = serde_json::from_str(meta_line)
            .map_err(|e| IoError::other(format!("failed to parse session meta: {e}")))?;
        let mut items = Vec::new();
        let mut state = SessionStateSnapshot::default();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v.get("record_type")
                .and_then(|rt| rt.as_str())
                .map(|s| s == "state")
                .unwrap_or(false)
            {
                if let Ok(s) = serde_json::from_value::<SessionStateSnapshot>(v.clone()) {
                    state = s
                }
                continue;
            }
            match serde_json::from_value::<ResponseItem>(v.clone()) {
                Ok(item) => match item {
                    ResponseItem::Message { .. }
                    | ResponseItem::LocalShellCall { .. }
                    | ResponseItem::FunctionCall { .. }
                    | ResponseItem::FunctionCallOutput { .. }
                    | ResponseItem::Reasoning { .. } => items.push(item),
                    ResponseItem::Other => {}
                },
                Err(e) => {
                    warn!("failed to parse item: {v:?}, error: {e}");
                }
            }
        }

        let saved = SavedSession {
            session: session.clone(),
            items: items.clone(),
            state: state.clone(),
            session_id: session.id,
        };

        let file = std::fs::OpenOptions::new()
            .append(true)
            .read(true)
            .open(path)?;

        let (tx_jsonl, rx_jsonl) = mpsc::channel::<RolloutCmd>(256);
        tokio::task::spawn(rollout_writer(
            tokio::fs::File::from_std(file),
            rx_jsonl,
            None,
            cwd.clone(),
        ));

        // Also start a snapshot writer that continues writing to the JSON snapshot file for this
        // session id. Derive the date from the saved session timestamp.
        let ts_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        // Parse the stored timestamp, but don't fail resume if parsing fails – fall back to
        // current local time so we can still continue appending to the JSONL file and keep a
        // snapshot up to date. This avoids breaking resume due to minor formatting mismatches.
        let ts = match OffsetDateTime::parse(&session.timestamp, ts_format) {
            Ok(ts_utc) => {
                let local_offset = OffsetDateTime::now_local()
                    .map_err(|e| IoError::other(format!("failed to get local time offset: {e}")))?
                    .offset();
                ts_utc.to_offset(local_offset)
            }
            Err(e) => {
                warn!(
                    "failed to parse session timestamp '{ts}': {e}; using current local time for snapshot path",
                    ts = session.timestamp
                );
                OffsetDateTime::now_local().map_err(|e| {
                    IoError::other(format!(
                        "failed to get local time for snapshot fallback: {e}"
                    ))
                })?
            }
        };
        // Determine the sessions directory by scanning ancestors for the
        // top-level `sessions` folder to support both legacy nested layouts
        // (sessions/YYYY/MM/DD/file) and the new flat layout (sessions/file).
        let sessions_dir = path
            .ancestors()
            .find(|p| p.file_name().map(|n| n == SESSIONS_SUBDIR).unwrap_or(false))
            .ok_or_else(|| IoError::other("invalid rollout path; missing sessions directory"))?;
        let snapshot_path = snapshot_filepath_in_dir(sessions_dir, session.id, ts)?;

        // Seed instructions from the first user message in the restored items, if any.
        let initial_instructions = first_user_message_text(&items).unwrap_or_default();

        let (tx_snapshot, rx_snapshot) = mpsc::channel::<RolloutCmd>(256);
        tokio::task::spawn(snapshot_writer(
            snapshot_path,
            rx_snapshot,
            SnapshotSessionMeta {
                timestamp: ts,
                id: session.id,
                instructions: initial_instructions,
            },
            items.clone(),
        ));

        info!("Resumed rollout successfully from {path:?}");
        Ok((
            Self {
                txs: vec![tx_jsonl, tx_snapshot],
            },
            saved,
        ))
    }

    pub async fn shutdown(&self) -> std::io::Result<()> {
        // Send shutdown to all writers and wait for their acks.
        let mut acks = Vec::new();
        for tx in &self.txs {
            let (tx_done, rx_done) = oneshot::channel();
            match tx.send(RolloutCmd::Shutdown { ack: tx_done }).await {
                Ok(_) => acks.push(rx_done),
                Err(e) => {
                    warn!("failed to send rollout shutdown command: {e}");
                }
            }
        }
        // Wait for all acks.
        for rx in acks {
            if let Err(e) = rx.await {
                warn!("failed waiting for rollout shutdown: {e}");
            }
        }
        Ok(())
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
    // Resolve ~/.codex/sessions and create it if missing (no date subdirectories).
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
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
        .append(true)
        .create(true)
        .open(&path)?;

    Ok(LogFileInfo {
        file,
        session_id,
        timestamp,
    })
}

async fn rollout_writer(
    file: tokio::fs::File,
    mut rx: mpsc::Receiver<RolloutCmd>,
    mut meta: Option<SessionMeta>,
    cwd: std::path::PathBuf,
) -> std::io::Result<()> {
    let mut writer = JsonlWriter { file };

    // If we have a meta, collect git info asynchronously and write meta first
    if let Some(session_meta) = meta.take() {
        let git_info = collect_git_info(&cwd).await;
        let session_meta_with_git = SessionMetaWithGit {
            meta: session_meta,
            git: git_info,
        };

        // Write the SessionMeta as the first item in the file
        writer.write_line(&session_meta_with_git).await?;
    }

    // Process rollout commands
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(items) => {
                for item in items {
                    match item {
                        ResponseItem::Message { .. }
                        | ResponseItem::LocalShellCall { .. }
                        | ResponseItem::FunctionCall { .. }
                        | ResponseItem::FunctionCallOutput { .. }
                        | ResponseItem::Reasoning { .. } => {
                            writer.write_line(&item).await?;
                        }
                        ResponseItem::Other => {}
                    }
                }
            }
            RolloutCmd::UpdateState(state) => {
                #[derive(Serialize)]
                struct StateLine<'a> {
                    record_type: &'static str,
                    #[serde(flatten)]
                    state: &'a SessionStateSnapshot,
                }
                writer
                    .write_line(&StateLine {
                        record_type: "state",
                        state: &state,
                    })
                    .await?;
            }
            RolloutCmd::Shutdown { ack } => {
                let _ = ack.send(());
            }
        }
    }

    Ok(())
}

struct JsonlWriter {
    file: tokio::fs::File,
}

impl JsonlWriter {
    async fn write_line(&mut self, item: &impl serde::Serialize) -> std::io::Result<()> {
        let mut json = serde_json::to_string(item)?;
        json.push('\n');
        self.file.write_all(json.as_bytes()).await?;
        self.file.flush().await?;
        Ok(())
    }
}

// ---- Pretty JSON snapshot writer ------------------------------------------------------------

#[derive(Clone)]
struct SnapshotSessionMeta {
    id: Uuid,
    timestamp: OffsetDateTime,
    instructions: String,
}

#[derive(Serialize)]
struct SnapshotSessionMetaJson<'a> {
    timestamp: &'a str,
    id: Uuid,
    instructions: &'a str,
}

#[derive(Serialize)]
struct SnapshotRoot<'a> {
    session: SnapshotSessionMetaJson<'a>,
    items: &'a [ResponseItem],
}

fn create_snapshot_filepath(
    config: &Config,
    session_id: Uuid,
    timestamp: OffsetDateTime,
) -> std::io::Result<std::path::PathBuf> {
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
    snapshot_filepath_in_dir(&dir, session_id, timestamp)
}

fn snapshot_filepath_in_dir(
    sessions_dir: &std::path::Path,
    session_id: Uuid,
    timestamp: OffsetDateTime,
) -> std::io::Result<std::path::PathBuf> {
    fs::create_dir_all(sessions_dir)?;

    // YYYY-MM-DD (local time)
    let format: &[FormatItem] = format_description!("[year]-[month]-[day]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::other(format!("failed to format snapshot date: {e}")))?;

    let filename = format!("rollout-{date_str}-{session_id}.json");
    Ok(sessions_dir.join(filename))
}

async fn snapshot_writer(
    path: std::path::PathBuf,
    mut rx: mpsc::Receiver<RolloutCmd>,
    mut meta: SnapshotSessionMeta,
    mut items: Vec<ResponseItem>,
) -> std::io::Result<()> {
    // Write the initial JSON file.
    write_snapshot(&path, &meta, &items).await?;

    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(new_items) => {
                // Update instructions once: set to the first user message text if currently empty.
                if meta.instructions.is_empty() {
                    if let Some(instr) = first_user_message_text(&new_items) {
                        meta.instructions = instr;
                    }
                }

                items.extend(new_items.into_iter().filter(|item| match item {
                    ResponseItem::Other => false,
                    _ => true,
                }));
                write_snapshot(&path, &meta, &items).await?;
            }
            RolloutCmd::UpdateState(_) => {
                // State is not included in the pretty snapshot per requirements.
            }
            RolloutCmd::Shutdown { ack } => {
                let _ = ack.send(());
            }
        }
    }

    Ok(())
}

fn first_user_message_text(items: &[ResponseItem]) -> Option<String> {
    for item in items {
        if let ResponseItem::Message { role, content, .. } = item {
            if role == "user" {
                // Concatenate InputText entries separated by newlines
                let mut parts: Vec<String> = Vec::new();
                for c in content {
                    match c {
                        crate::models::ContentItem::InputText { text } => parts.push(text.clone()),
                        _ => {}
                    }
                }
                return Some(parts.join("\n"));
            }
        }
    }
    None
}

async fn write_snapshot(
    path: &std::path::Path,
    meta: &SnapshotSessionMeta,
    items: &[ResponseItem],
) -> std::io::Result<()> {
    // Format timestamp as RFC3339 with Z (mirror existing meta string).
    let ts_format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    let ts_str = meta
        .timestamp
        .format(ts_format)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

    let root = SnapshotRoot {
        session: SnapshotSessionMetaJson {
            timestamp: &ts_str,
            id: meta.id,
            instructions: &meta.instructions,
        },
        items,
    };

    // Write to a temp file then atomically replace the destination.
    let tmp_path = path.with_extension("json.tmp");

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Create tmp file and set restrictive permissions where supported.
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(&tmp_path).await?;

    let json_pretty = serde_json::to_string_pretty(&root)
        .map_err(|e| IoError::other(format!("failed to serialize snapshot: {e}")))?;
    file.write_all(json_pretty.as_bytes()).await?;
    file.flush().await?;

    // Replace destination. On Windows, rename fails if the destination exists; remove first.
    #[cfg(windows)]
    {
        let _ = tokio::fs::remove_file(path).await;
    }
    tokio::fs::rename(&tmp_path, path).await?;

    Ok(())
}
