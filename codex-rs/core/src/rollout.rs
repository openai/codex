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
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{self};
use uuid::Uuid;

use crate::config::Config;
use crate::models::ResponseItem;

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

#[derive(Clone)]
pub(crate) struct RolloutRecorder {
    tx: Sender<RolloutCmd>,
}

#[derive(Clone)]
enum RolloutCmd {
    AddItems(Vec<ResponseItem>),
    UpdateState(SessionStateSnapshot),
}

impl RolloutRecorder {
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

        let (tx, mut rx) = mpsc::channel::<RolloutCmd>(256);

        tokio::task::spawn(async move {
            let mut file = tokio::fs::File::from_std(file);
            if let Ok(json) = serde_json::to_string(&meta) {
                let _ = file.write_all(json.as_bytes()).await;
                let _ = file.write_all(b"\n").await;
                let _ = file.flush().await;
            }

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RolloutCmd::AddItems(items) => {
                        for item in items {
                            match item {
                                ResponseItem::Message { .. }
                                | ResponseItem::LocalShellCall { .. }
                                | ResponseItem::FunctionCall { .. }
                                | ResponseItem::FunctionCallOutput { .. } => {
                                    if let Ok(json) = serde_json::to_string(&item) {
                                        let _ = file.write_all(json.as_bytes()).await;
                                        let _ = file.write_all(b"\n").await;
                                    }
                                }
                                ResponseItem::Reasoning { .. } | ResponseItem::Other => {}
                            }
                        }
                        let _ = file.flush().await;
                    }
                    RolloutCmd::UpdateState(state) => {
                        #[derive(Serialize)]
                        struct StateLine<'a> {
                            record_type: &'static str,
                            #[serde(flatten)]
                            state: &'a SessionStateSnapshot,
                        }
                        if let Ok(json) = serde_json::to_string(&StateLine {
                            record_type: "state",
                            state: &state,
                        }) {
                            let _ = file.write_all(json.as_bytes()).await;
                            let _ = file.write_all(b"\n").await;
                            let _ = file.flush().await;
                        }
                    }
                }
            }
        });

        Ok(Self { tx })
    }

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

    pub async fn resume(path: &Path) -> std::io::Result<(Self, SavedSession)> {
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
                match serde_json::from_value::<SessionStateSnapshot>(v.clone()) {
                    Ok(s) => state = s,
                    Err(_) => {}
                }
                continue;
            }
            if let Ok(item) = serde_json::from_value::<ResponseItem>(v.clone()) {
                match item {
                    ResponseItem::Message { .. }
                    | ResponseItem::LocalShellCall { .. }
                    | ResponseItem::FunctionCall { .. }
                    | ResponseItem::FunctionCallOutput { .. } => items.push(item),
                    ResponseItem::Reasoning { .. } | ResponseItem::Other => {}
                }
            }
        }

        let saved = SavedSession {
            session: session.clone(),
            items: items.clone(),
            state: state.clone(),
        };

        let file = std::fs::OpenOptions::new()
            .append(true)
            .read(true)
            .open(path)?;

        let (tx, mut rx) = mpsc::channel::<RolloutCmd>(256);
        tokio::task::spawn(async move {
            let mut file = tokio::fs::File::from_std(file);
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RolloutCmd::AddItems(items) => {
                        for item in items {
                            match item {
                                ResponseItem::Message { .. }
                                | ResponseItem::LocalShellCall { .. }
                                | ResponseItem::FunctionCall { .. }
                                | ResponseItem::FunctionCallOutput { .. } => {
                                    if let Ok(json) = serde_json::to_string(&item) {
                                        let _ = file.write_all(json.as_bytes()).await;
                                        let _ = file.write_all(b"\n").await;
                                    }
                                }
                                ResponseItem::Reasoning { .. } | ResponseItem::Other => {}
                            }
                        }
                        let _ = file.flush().await;
                    }
                    RolloutCmd::UpdateState(state) => {
                        #[derive(Serialize)]
                        struct StateLine<'a> {
                            record_type: &'static str,
                            #[serde(flatten)]
                            state: &'a SessionStateSnapshot,
                        }
                        if let Ok(json) = serde_json::to_string(&StateLine {
                            record_type: "state",
                            state: &state,
                        }) {
                            let _ = file.write_all(json.as_bytes()).await;
                            let _ = file.write_all(b"\n").await;
                            let _ = file.flush().await;
                        }
                    }
                }
            }
        });

        Ok((Self { tx }, saved))
    }
}

struct LogFileInfo {
    file: File,
    session_id: Uuid,
    timestamp: OffsetDateTime,
}

fn create_log_file(config: &Config, session_id: Uuid) -> std::io::Result<LogFileInfo> {
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
    dir.push(timestamp.year().to_string());
    dir.push(format!("{:02}", u8::from(timestamp.month())));
    dir.push(format!("{:02}", timestamp.day()));
    fs::create_dir_all(&dir)?;

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
