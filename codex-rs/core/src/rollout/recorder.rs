//! Persist Codex session rollouts (.jsonl) so sessions can be replayed or inspected later.

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{self};
use tokio::sync::oneshot;
use tracing::info;
use tracing::warn;

use super::SESSIONS_SUBDIR;
use super::list::ConversationsPage;
use super::list::Cursor;
use super::list::get_conversations;
use super::policy::is_persisted_response_item;
use crate::config::Config;
use crate::conversation_manager::InitialHistory;
use crate::conversation_manager::ResumedHistory;
use crate::git_info::GitInfo;
use crate::git_info::collect_git_info;
use crate::rollout::policy::is_persisted_event;
use codex_protocol::models::ResponseItem;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct SessionMeta {
    pub id: ConversationId,
    pub timestamp: String,
    pub cwd: String,
    pub originator: String,
    pub cli_version: String,
    pub instructions: Option<String>,
}

// SessionMetaWithGit is used in writes and reads; ensure it implements Debug.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionMetaWithGit {
    #[serde(flatten)]
    meta: SessionMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitInfo>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SessionStateSnapshot {}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SavedSession {
    pub session: SessionMeta,
    #[serde(default)]
    pub items: Vec<ResponseItem>,
    #[serde(default)]
    pub state: SessionStateSnapshot,
    pub session_id: ConversationId,
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
pub struct RolloutRecorder {
    tx: Sender<RolloutCmd>,
    path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "record_type", rename_all = "snake_case")]
enum TaggedLine {
    Response {
        #[serde(flatten)]
        item: ResponseItem,
    },
    Event {
        #[serde(flatten)]
        event: Event,
    },
    SessionMeta {
        #[serde(flatten)]
        meta: SessionMetaWithGit,
    },
    PrevSessionMeta {
        #[serde(flatten)]
        meta: SessionMetaWithGit,
    },
    State {
        #[serde(flatten)]
        state: SessionStateSnapshot,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TimestampedLine {
    timestamp: String,
    #[serde(flatten)]
    record: TaggedLine,
}

#[derive(Debug, Clone)]
pub enum RolloutItem {
    ResponseItem(ResponseItem),
    Event(Event),
    SessionMeta(SessionMetaWithGit),
}

impl From<ResponseItem> for RolloutItem {
    fn from(item: ResponseItem) -> Self {
        RolloutItem::ResponseItem(item)
    }
}

impl From<Event> for RolloutItem {
    fn from(event: Event) -> Self {
        RolloutItem::Event(event)
    }
}

/// Convenience helpers to extract typed items from a list of rollout items.
pub trait RolloutItemSliceExt {
    fn get_response_items(&self) -> Vec<ResponseItem>;
    fn get_events(&self) -> Vec<EventMsg>;
}

impl RolloutItemSliceExt for [RolloutItem] {
    fn get_response_items(&self) -> Vec<ResponseItem> {
        self.iter()
            .filter_map(|it| match it {
                RolloutItem::ResponseItem(ri) => Some(ri.clone()),
                _ => None,
            })
            .collect()
    }

    fn get_events(&self) -> Vec<EventMsg> {
        self.iter()
            .filter_map(|it| match it {
                RolloutItem::Event(ev) => Some(ev.msg.clone()),
                _ => None,
            })
            .collect()
    }
}

enum RolloutCmd {
    AddResponseItems(Vec<ResponseItem>),
    AddEvents(Vec<Event>),
    AddSessionMeta(SessionMetaWithGit),
    Flush { ack: oneshot::Sender<()> },
    Shutdown { ack: oneshot::Sender<()> },
}

impl RolloutRecorder {
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[allow(dead_code)]
    /// List conversations (rollout files) under the provided Codex home directory.
    pub async fn list_conversations(
        codex_home: &Path,
        page_size: usize,
        cursor: Option<&Cursor>,
    ) -> std::io::Result<ConversationsPage> {
        get_conversations(codex_home, page_size, cursor).await
    }

    /// Attempt to create a new [`RolloutRecorder`]. If the sessions directory
    /// cannot be created or the rollout file cannot be opened we return the
    /// error so the caller can decide whether to disable persistence.
    pub async fn new(
        config: &Config,
        conversation_id: ConversationId,
        instructions: Option<String>,
    ) -> std::io::Result<Self> {
        let LogFileInfo {
            file,
            conversation_id: session_id,
            timestamp,
            path,
        } = create_log_file(config, conversation_id)?;

        let timestamp_format: &[FormatItem] =
            format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
        let timestamp = timestamp
            .to_offset(time::UtcOffset::UTC)
            .format(timestamp_format)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

        let cwd = config.cwd.to_path_buf();

        let (tx, rx) = mpsc::channel(100);

        tokio::task::spawn(rollout_writer(
            tokio::fs::File::from_std(file),
            rx,
            Some(SessionMeta {
                timestamp,
                id: session_id,
                cwd: config.cwd.to_string_lossy().to_string(),
                originator: config.responses_originator_header.clone(),
                cli_version: env!("CARGO_PKG_VERSION").to_string(),
                instructions,
            }),
            cwd,
        ));

        Ok(Self { tx, path })
    }

    pub(crate) async fn record_items(&self, item: RolloutItem) -> std::io::Result<()> {
        match item {
            RolloutItem::ResponseItem(item) => self.record_response_item(&item).await,
            RolloutItem::Event(event) => self.record_event(&event).await,
            RolloutItem::SessionMeta(meta) => self.record_session_meta(&meta).await,
        }
    }

    /// Ensure all writes up to this point have been processed by the writer task.
    ///
    /// This is a sequencing barrier for readers that plan to open and read the
    /// rollout file immediately after calling this method. The background writer
    /// processes the channel serially; when it dequeues `Flush`, all prior
    /// `AddResponseItems`/`AddEvents`/`AddSessionMeta` have already been written
    /// via `write_line`, which calls `file.flush()` (OS‐buffer flush).
    pub async fn flush(&self) -> std::io::Result<()> {
        let (tx_done, rx_done) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Flush { ack: tx_done })
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout flush: {e}")))?;
        rx_done
            .await
            .map_err(|e| IoError::other(format!("failed waiting for rollout flush: {e}")))
    }

    async fn record_response_item(&self, item: &ResponseItem) -> std::io::Result<()> {
        // Note that function calls may look a bit strange if they are
        // "fully qualified MCP tool calls," so we could consider
        // reformatting them in that case.
        if !is_persisted_response_item(item) {
            return Ok(());
        }
        self.tx
            .send(RolloutCmd::AddResponseItems(vec![item.clone()]))
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout items: {e}")))
    }

    async fn record_event(&self, event: &Event) -> std::io::Result<()> {
        if !is_persisted_event(event) {
            return Ok(());
        }
        self.tx
            .send(RolloutCmd::AddEvents(vec![event.clone()]))
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout event: {e}")))
    }

    async fn record_session_meta(&self, meta: &SessionMetaWithGit) -> std::io::Result<()> {
        self.tx
            .send(RolloutCmd::AddSessionMeta(meta.clone()))
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout session meta: {e}")))
    }

    pub async fn get_rollout_history(path: &Path) -> std::io::Result<InitialHistory> {
        info!("Resuming rollout from {path:?}");
        tracing::error!("Resuming rollout from {path:?}");
        let text = tokio::fs::read_to_string(path).await?;
        let mut lines = text.lines();
        let first_line = lines
            .next()
            .ok_or_else(|| IoError::other("empty session file"))?;
        let conversation_id = if let Ok(TimestampedLine {
            record: TaggedLine::SessionMeta { meta },
            ..
        }) = serde_json::from_str::<TimestampedLine>(first_line)
        {
            Some(meta.meta.id)
        } else if let Ok(meta) = serde_json::from_str::<SessionMetaWithGit>(first_line) {
            Some(meta.meta.id)
        } else if let Ok(meta) = serde_json::from_str::<SessionMeta>(first_line) {
            Some(meta.id)
        } else {
            return Err(IoError::other(
                "failed to parse first line of rollout file as SessionMeta",
            ));
        };

        let mut items: Vec<RolloutItem> = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<TimestampedLine>(line) {
                Ok(TimestampedLine {
                    record: TaggedLine::State { .. },
                    ..
                }) => {}
                Ok(TimestampedLine {
                    record: TaggedLine::Event { event },
                    ..
                }) => items.push(RolloutItem::Event(event)),
                Ok(TimestampedLine {
                    record: TaggedLine::SessionMeta { meta },
                    ..
                })
                | Ok(TimestampedLine {
                    record: TaggedLine::PrevSessionMeta { meta },
                    ..
                }) => items.push(RolloutItem::SessionMeta(meta)),
                Ok(TimestampedLine {
                    record: TaggedLine::Response { item },
                    ..
                }) => {
                    if is_persisted_response_item(&item) {
                        items.push(RolloutItem::ResponseItem(item));
                    }
                }
                Err(_) => warn!("failed to parse rollout line: {line}"),
            }
        }

        tracing::error!(
            "Resumed rollout with {} items, conversation ID: {:?}",
            items.len(),
            conversation_id
        );
        let conversation_id = conversation_id
            .ok_or_else(|| IoError::other("failed to parse conversation ID from rollout file"))?;

        if items.is_empty() {
            return Ok(InitialHistory::New);
        }

        info!("Resumed rollout successfully from {path:?}");
        Ok(InitialHistory::Resumed(ResumedHistory {
            conversation_id,
            history: items,
            rollout_path: path.to_path_buf(),
        }))
    }

    pub async fn shutdown(&self) -> std::io::Result<()> {
        let (tx_done, rx_done) = oneshot::channel();
        match self.tx.send(RolloutCmd::Shutdown { ack: tx_done }).await {
            Ok(_) => rx_done
                .await
                .map_err(|e| IoError::other(format!("failed waiting for rollout shutdown: {e}"))),
            Err(e) => {
                warn!("failed to send rollout shutdown command: {e}");
                Err(IoError::other(format!(
                    "failed to send rollout shutdown command: {e}"
                )))
            }
        }
    }
}

struct LogFileInfo {
    /// Opened file handle to the rollout file.
    file: File,

    /// Session ID (also embedded in filename).
    conversation_id: ConversationId,

    /// Timestamp for the start of the session.
    timestamp: OffsetDateTime,

    /// Full filesystem path to the rollout file.
    path: PathBuf,
}

fn create_log_file(
    config: &Config,
    conversation_id: ConversationId,
) -> std::io::Result<LogFileInfo> {
    // Resolve ~/.codex/sessions/YYYY/MM/DD and create it if missing.
    let timestamp = OffsetDateTime::now_utc();
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

    let filename = format!("rollout-{date_str}-{conversation_id}.jsonl");

    let path = dir.join(filename);
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;

    Ok(LogFileInfo {
        file,
        conversation_id,
        timestamp,
        path,
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
        writer
            .write_tagged(TaggedLine::SessionMeta {
                meta: session_meta_with_git,
            })
            .await?;
    }

    // Process rollout commands
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddResponseItems(items) => {
                for item in items {
                    if is_persisted_response_item(&item) {
                        writer.write_tagged(TaggedLine::Response { item }).await?;
                    }
                }
            }
            RolloutCmd::AddEvents(events) => {
                for event in events {
                    writer.write_tagged(TaggedLine::Event { event }).await?;
                }
            }
            // Sequencing barrier: by the time we handle `Flush`, all previously
            // queued writes have been applied and flushed to OS buffers.
            RolloutCmd::Flush { ack } => {
                let _ = ack.send(());
            }
            RolloutCmd::AddSessionMeta(meta) => {
                writer
                    .write_tagged(TaggedLine::PrevSessionMeta { meta })
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

    async fn write_tagged(&mut self, record: TaggedLine) -> std::io::Result<()> {
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;
        let line = TimestampedLine { timestamp, record };
        self.write_line(&line).await
    }
}
