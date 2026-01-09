//! Persist Codex session rollouts (.jsonl) so sessions can be replayed or inspected later.

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
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

use super::SESSIONS_SUBDIR;
use super::list::Cursor;
use super::list::ThreadsPage;
use super::list::get_threads;
use super::policy::is_persisted_response_item;
use crate::config::Config;
use crate::default_client::originator;
use crate::git_info::collect_git_info;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;

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
    pub(crate) rollout_path: PathBuf,
}

#[derive(Clone)]
pub enum RolloutRecorderParams {
    Create {
        conversation_id: ThreadId,
        instructions: Option<String>,
        source: SessionSource,
    },
    Resume {
        path: PathBuf,
    },
}

enum RolloutCmd {
    AddItems(Vec<RolloutItem>),
    /// Ensure all prior writes are processed; respond when flushed.
    Flush {
        ack: oneshot::Sender<()>,
    },
    /// Rewrite the first SessionMeta line in the rollout file to include a name.
    SetSessionName {
        name: String,
        ack: oneshot::Sender<std::io::Result<()>>,
    },
    Shutdown {
        ack: oneshot::Sender<()>,
    },
}

impl RolloutRecorderParams {
    pub fn new(
        conversation_id: ThreadId,
        instructions: Option<String>,
        source: SessionSource,
    ) -> Self {
        Self::Create {
            conversation_id,
            instructions,
            source,
        }
    }

    pub fn resume(path: PathBuf) -> Self {
        Self::Resume { path }
    }
}

impl RolloutRecorder {
    /// List threads (rollout files) under the provided Codex home directory.
    pub async fn list_threads(
        codex_home: &Path,
        page_size: usize,
        cursor: Option<&Cursor>,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
    ) -> std::io::Result<ThreadsPage> {
        get_threads(
            codex_home,
            page_size,
            cursor,
            allowed_sources,
            model_providers,
            default_provider,
        )
        .await
    }

    /// Attempt to create a new [`RolloutRecorder`]. If the sessions directory
    /// cannot be created or the rollout file cannot be opened we return the
    /// error so the caller can decide whether to disable persistence.
    pub async fn new(config: &Config, params: RolloutRecorderParams) -> std::io::Result<Self> {
        let (file, rollout_path, meta) = match params {
            RolloutRecorderParams::Create {
                conversation_id,
                instructions,
                source,
            } => {
                let LogFileInfo {
                    file,
                    path,
                    conversation_id: session_id,
                    timestamp,
                } = create_log_file(config, conversation_id)?;

                let timestamp_format: &[FormatItem] = format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
                );
                let timestamp = timestamp
                    .to_offset(time::UtcOffset::UTC)
                    .format(timestamp_format)
                    .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

                (
                    tokio::fs::File::from_std(file),
                    path,
                    Some(SessionMeta {
                        id: session_id,
                        timestamp,
                        cwd: config.cwd.clone(),
                        name: None,
                        originator: originator().value.clone(),
                        cli_version: env!("CARGO_PKG_VERSION").to_string(),
                        instructions,
                        source,
                        model_provider: Some(config.model_provider_id.clone()),
                    }),
                )
            }
            RolloutRecorderParams::Resume { path } => (
                tokio::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .await?,
                path,
                None,
            ),
        };

        // Clone the cwd for the spawned task to collect git info asynchronously
        let cwd = config.cwd.clone();

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller's thread.
        let (tx, rx) = mpsc::channel::<RolloutCmd>(256);

        // Spawn a Tokio task that owns the file handle and performs async
        // writes. Using `tokio::fs::File` keeps everything on the async I/O
        // driver instead of blocking the runtime.
        tokio::task::spawn(rollout_writer(file, rx, meta, cwd));

        Ok(Self { tx, rollout_path })
    }

    pub(crate) async fn record_items(&self, items: &[RolloutItem]) -> std::io::Result<()> {
        let mut filtered = Vec::new();
        for item in items {
            // Note that function calls may look a bit strange if they are
            // "fully qualified MCP tool calls," so we could consider
            // reformatting them in that case.
            if is_persisted_response_item(item) {
                filtered.push(item.clone());
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

    /// Flush all queued writes and wait until they are committed by the writer task.
    pub async fn flush(&self) -> std::io::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Flush { ack: tx })
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout flush: {e}")))?;
        rx.await
            .map_err(|e| IoError::other(format!("failed waiting for rollout flush: {e}")))
    }

    pub async fn set_session_name(&self, name: String) -> std::io::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::SetSessionName { name, ack: tx })
            .await
            .map_err(|e| IoError::other(format!("failed to queue session name update: {e}")))?;
        rx.await
            .map_err(|e| IoError::other(format!("failed waiting for session name update: {e}")))?
    }

    pub async fn get_rollout_history(path: &Path) -> std::io::Result<InitialHistory> {
        info!("Resuming rollout from {path:?}");
        let text = tokio::fs::read_to_string(path).await?;
        if text.trim().is_empty() {
            return Err(IoError::other("empty session file"));
        }

        let mut items: Vec<RolloutItem> = Vec::new();
        let mut thread_id: Option<ThreadId> = None;
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    warn!("failed to parse line as JSON: {line:?}, error: {e}");
                    continue;
                }
            };

            // Parse the rollout line structure
            match serde_json::from_value::<RolloutLine>(v.clone()) {
                Ok(rollout_line) => match rollout_line.item {
                    RolloutItem::SessionMeta(session_meta_line) => {
                        // Use the FIRST SessionMeta encountered in the file as the canonical
                        // thread id and main session information. Keep all items intact.
                        if thread_id.is_none() {
                            thread_id = Some(session_meta_line.meta.id);
                        }
                        items.push(RolloutItem::SessionMeta(session_meta_line));
                    }
                    RolloutItem::ResponseItem(item) => {
                        items.push(RolloutItem::ResponseItem(item));
                    }
                    RolloutItem::Compacted(item) => {
                        items.push(RolloutItem::Compacted(item));
                    }
                    RolloutItem::TurnContext(item) => {
                        items.push(RolloutItem::TurnContext(item));
                    }
                    RolloutItem::EventMsg(_ev) => {
                        items.push(RolloutItem::EventMsg(_ev));
                    }
                },
                Err(e) => {
                    warn!("failed to parse rollout line: {v:?}, error: {e}");
                }
            }
        }

        info!(
            "Resumed rollout with {} items, thread ID: {:?}",
            items.len(),
            thread_id
        );
        let conversation_id = thread_id
            .ok_or_else(|| IoError::other("failed to parse thread ID from rollout file"))?;

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

    /// Full path to the rollout file.
    path: PathBuf,

    /// Session ID (also embedded in filename).
    conversation_id: ThreadId,

    /// Timestamp for the start of the session.
    timestamp: OffsetDateTime,
}

fn create_log_file(config: &Config, conversation_id: ThreadId) -> std::io::Result<LogFileInfo> {
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

    let filename = format!("rollout-{date_str}-{conversation_id}.jsonl");

    let path = dir.join(filename);
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;

    Ok(LogFileInfo {
        file,
        path,
        conversation_id,
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
        let session_meta_line = SessionMetaLine {
            meta: session_meta,
            git: git_info,
        };

        // Write the SessionMeta as the first item in the file, wrapped in a rollout line
        writer
            .write_rollout_item(RolloutItem::SessionMeta(session_meta_line))
            .await?;
    }

    // Process rollout commands
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(items) => {
                for item in items {
                    if is_persisted_response_item(&item) {
                        writer.write_rollout_item(item).await?;
                    }
                }
            }
            RolloutCmd::Flush { ack } => {
                // Ensure underlying file is flushed and then ack.
                if let Err(e) = writer.file.flush().await {
                    let _ = ack.send(());
                    return Err(e);
                }
                let _ = ack.send(());
            }
            RolloutCmd::SetSessionName { name, ack } => {
                let result = rewrite_session_name(&mut writer, &rollout_path, &name).await;
                let _ = ack.send(result);
            }
            RolloutCmd::Shutdown { ack } => {
                let _ = ack.send(());
            }
        }
    }

    Ok(())
}

async fn rewrite_session_name(
    writer: &mut JsonlWriter,
    rollout_path: &Path,
    name: &str,
) -> std::io::Result<()> {
    // Flush and close the writer's file handle before swapping the on-disk file,
    // otherwise subsequent appends would keep writing to the old inode/handle.
    writer.file.flush().await?;

    // Compute the rewritten contents first so any read/parse/legacy-format errors
    // don't disturb the active writer handle.
    let rewritten_contents = rewrite_first_session_meta_line_name(rollout_path, name).await?;

    // Close the active handle using a portable placeholder.
    let placeholder = tokio::fs::File::from_std(tempfile::tempfile()?);
    let old_file = std::mem::replace(&mut writer.file, placeholder);
    drop(old_file);

    if let Err(e) = replace_rollout_file(rollout_path, rewritten_contents).await {
        // Best-effort: ensure the writer keeps pointing at the rollout file, not the placeholder.
        let reopened = tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(rollout_path)
            .await;
        if let Ok(reopened) = reopened {
            let placeholder = std::mem::replace(&mut writer.file, reopened);
            drop(placeholder);
        }
        return Err(e);
    }

    // Re-open the rollout for appends and drop the placeholder handle.
    let reopened = tokio::fs::OpenOptions::new()
        .append(true)
        .open(rollout_path)
        .await?;
    let placeholder = std::mem::replace(&mut writer.file, reopened);
    drop(placeholder);

    Ok(())
}

async fn rewrite_first_session_meta_line_name(
    rollout_path: &Path,
    name: &str,
) -> std::io::Result<String> {
    let text = tokio::fs::read_to_string(rollout_path).await?;
    let mut rewritten = false;

    // Rewrite the first non-empty line only. Since 43809a454 ("Introduce rollout items",
    // 2025-09-09), rollouts we write always start with a RolloutLine wrapping
    // RolloutItem::SessionMeta(_).
    let mut out = String::with_capacity(text.len() + 32);
    for line in text.lines() {
        if !rewritten && !line.trim().is_empty() {
            out.push_str(&rewrite_session_meta_line_name(line, name)?);
            rewritten = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }

    if !rewritten {
        return Err(IoError::other(
            "failed to set session name: rollout has no SessionMeta line",
        ));
    }

    Ok(out)
}

fn rewrite_session_meta_line_name(line: &str, name: &str) -> std::io::Result<String> {
    let mut rollout_line = serde_json::from_str::<RolloutLine>(line).map_err(IoError::other)?;
    let RolloutItem::SessionMeta(meta_line) = &mut rollout_line.item else {
        return Err(IoError::other(
            "failed to set session name: rollout has no SessionMeta line",
        ));
    };

    meta_line.meta.name = Some(name.to_string());
    serde_json::to_string(&rollout_line).map_err(IoError::other)
}

async fn replace_rollout_file(path: &Path, contents: String) -> std::io::Result<()> {
    let Some(dir) = path.parent() else {
        return Err(IoError::other("rollout path has no parent directory"));
    };

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    use std::io::Write as _;
    tmp.write_all(contents.as_bytes())?;
    tmp.flush()?;

    let (_file, tmp_path) = tmp.keep()?;
    drop(_file);

    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path)?;
    }

    #[cfg(not(windows))]
    {
        std::fs::rename(&tmp_path, path)?;
    }

    Ok(())
}

struct JsonlWriter {
    file: tokio::fs::File,
}

impl JsonlWriter {
    async fn write_rollout_item(&mut self, rollout_item: RolloutItem) -> std::io::Result<()> {
        let timestamp_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        let timestamp = OffsetDateTime::now_utc()
            .format(timestamp_format)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

        let line = RolloutLine {
            timestamp,
            item: rollout_item,
        };
        self.write_line(&line).await
    }
    async fn write_line(&mut self, item: &impl serde::Serialize) -> std::io::Result<()> {
        let mut json = serde_json::to_string(item)?;
        json.push('\n');
        self.file.write_all(json.as_bytes()).await?;
        self.file.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn set_session_name_rewrites_first_session_meta_line() -> std::io::Result<()> {
        let config = crate::config::test_config();

        let conversation_id = ThreadId::new();
        let recorder = RolloutRecorder::new(
            &config,
            RolloutRecorderParams::new(conversation_id, None, SessionSource::Cli),
        )
        .await?;

        recorder
            .set_session_name("My Session Name".to_string())
            .await?;

        let text = tokio::fs::read_to_string(&recorder.rollout_path).await?;
        let first_line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        let rollout_line: RolloutLine = serde_json::from_str(first_line)?;
        let RolloutItem::SessionMeta(meta_line) = rollout_line.item else {
            panic!("expected SessionMeta as first rollout line");
        };
        assert_eq!(meta_line.meta.name.as_deref(), Some("My Session Name"));
        Ok(())
    }

    #[tokio::test]
    async fn set_session_name_failure_does_not_redirect_future_writes() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let rollout_path = dir.path().join("rollout.jsonl");

        // Invalid JSON as the first non-empty line triggers a parse error in the rewrite step.
        tokio::fs::write(&rollout_path, "{\n").await?;

        let file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&rollout_path)
            .await?;
        let mut writer = JsonlWriter { file };

        assert!(
            rewrite_session_name(&mut writer, &rollout_path, "name")
                .await
                .is_err()
        );

        writer.file.write_all(b"AFTER\n").await?;
        writer.file.flush().await?;

        let text = tokio::fs::read_to_string(&rollout_path).await?;
        assert!(text.trim_end().ends_with("AFTER"));
        Ok(())
    }
}
