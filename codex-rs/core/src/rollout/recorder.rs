//! Persist Codex session rollouts (.jsonl) so sessions can be replayed or inspected later.

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;

use chrono::SecondsFormat;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::BaseInstructions;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{self};
use tokio::sync::oneshot;
use tracing::info;
use tracing::trace;
use tracing::warn;

use super::ARCHIVED_SESSIONS_SUBDIR;
use super::SESSIONS_SUBDIR;
use super::list::Cursor;
use super::list::ThreadItem;
use super::list::ThreadListConfig;
use super::list::ThreadListLayout;
use super::list::ThreadSortKey;
use super::list::ThreadsPage;
use super::list::get_threads;
use super::list::get_threads_in_root;
use super::list::parse_cursor;
use super::list::parse_timestamp_uuid_from_filename;
use super::metadata;
use super::policy::is_persisted_response_item;
use crate::config::Config;
use crate::default_client::originator;
use crate::git_info::collect_git_info;
use crate::path_utils;
use crate::state_db;
use crate::state_db::StateDbHandle;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_state::StateRuntime;
use codex_state::ThreadMetadataBuilder;

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
    state_db: Option<StateDbHandle>,
}

#[derive(Clone)]
pub enum RolloutRecorderParams {
    Create {
        conversation_id: ThreadId,
        forked_from_id: Option<ThreadId>,
        source: SessionSource,
        base_instructions: BaseInstructions,
        dynamic_tools: Vec<DynamicToolSpec>,
    },
    Resume {
        path: PathBuf,
    },
}

enum RolloutCmd {
    AddItems(Vec<RolloutItem>),
    Persist {
        ack: oneshot::Sender<()>,
    },
    /// Ensure all prior writes are processed; respond when flushed.
    Flush {
        ack: oneshot::Sender<()>,
    },
    Shutdown {
        ack: oneshot::Sender<()>,
    },
}

impl RolloutRecorderParams {
    pub fn new(
        conversation_id: ThreadId,
        forked_from_id: Option<ThreadId>,
        source: SessionSource,
        base_instructions: BaseInstructions,
        dynamic_tools: Vec<DynamicToolSpec>,
    ) -> Self {
        Self::Create {
            conversation_id,
            forked_from_id,
            source,
            base_instructions,
            dynamic_tools,
        }
    }

    pub fn resume(path: PathBuf) -> Self {
        Self::Resume { path }
    }
}

impl RolloutRecorder {
    /// List threads (rollout files) under the provided Codex home directory.
    pub async fn list_threads(
        config: &Config,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
    ) -> std::io::Result<ThreadsPage> {
        Self::list_threads_with_db_fallback(
            config,
            page_size,
            cursor,
            sort_key,
            allowed_sources,
            model_providers,
            default_provider,
            false,
        )
        .await
    }

    /// List archived threads (rollout files) under the archived sessions directory.
    pub async fn list_archived_threads(
        config: &Config,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
    ) -> std::io::Result<ThreadsPage> {
        Self::list_threads_with_db_fallback(
            config,
            page_size,
            cursor,
            sort_key,
            allowed_sources,
            model_providers,
            default_provider,
            true,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_threads_with_db_fallback(
        config: &Config,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
        archived: bool,
    ) -> std::io::Result<ThreadsPage> {
        let codex_home = config.codex_home.as_path();
        // Filesystem-first listing intentionally overfetches so we can repair stale/missing
        // SQLite rollout paths before the final DB-backed page is returned.
        let fs_page_size = page_size.saturating_mul(2).max(page_size);
        let fs_page = if archived {
            let root = codex_home.join(ARCHIVED_SESSIONS_SUBDIR);
            get_threads_in_root(
                root,
                fs_page_size,
                cursor,
                sort_key,
                ThreadListConfig {
                    allowed_sources,
                    model_providers,
                    default_provider,
                    layout: ThreadListLayout::Flat,
                },
            )
            .await?
        } else {
            get_threads(
                codex_home,
                fs_page_size,
                cursor,
                sort_key,
                allowed_sources,
                model_providers,
                default_provider,
            )
            .await?
        };

        let state_db_ctx = state_db::get_state_db(config, None).await;
        if state_db_ctx.is_none() {
            // Keep legacy behavior when SQLite is unavailable: return filesystem results
            // at the requested page size.
            return Ok(truncate_fs_page(fs_page, page_size, sort_key));
        }

        // Warm the DB by repairing every filesystem hit before querying SQLite.
        for item in &fs_page.items {
            state_db::read_repair_rollout_path(
                state_db_ctx.as_deref(),
                item.thread_id,
                Some(archived),
                item.path.as_path(),
            )
            .await;
        }

        if let Some(db_page) = state_db::list_threads_db(
            state_db_ctx.as_deref(),
            codex_home,
            page_size,
            cursor,
            sort_key,
            allowed_sources,
            model_providers,
            archived,
        )
        .await
        {
            return Ok(db_page.into());
        }
        // If SQLite listing still fails, return the filesystem page rather than failing the list.
        tracing::error!("Falling back on rollout system");
        state_db::record_discrepancy("list_threads_with_db_fallback", "falling_back");
        Ok(truncate_fs_page(fs_page, page_size, sort_key))
    }

    /// Find the newest recorded thread path, optionally filtering to a matching cwd.
    #[allow(clippy::too_many_arguments)]
    pub async fn find_latest_thread_path(
        config: &Config,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
        filter_cwd: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        let codex_home = config.codex_home.as_path();
        let state_db_ctx = state_db::get_state_db(config, None).await;
        if state_db_ctx.is_some() {
            let mut db_cursor = cursor.cloned();
            loop {
                let Some(db_page) = state_db::list_threads_db(
                    state_db_ctx.as_deref(),
                    codex_home,
                    page_size,
                    db_cursor.as_ref(),
                    sort_key,
                    allowed_sources,
                    model_providers,
                    false,
                )
                .await
                else {
                    break;
                };
                if let Some(path) = select_resume_path_from_db_page(&db_page, filter_cwd) {
                    return Ok(Some(path));
                }
                db_cursor = db_page.next_anchor.map(Into::into);
                if db_cursor.is_none() {
                    break;
                }
            }
        }

        let mut cursor = cursor.cloned();
        loop {
            let page = get_threads(
                codex_home,
                page_size,
                cursor.as_ref(),
                sort_key,
                allowed_sources,
                model_providers,
                default_provider,
            )
            .await?;
            if let Some(path) = select_resume_path(&page, filter_cwd) {
                return Ok(Some(path));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(None);
            }
        }
    }

    /// Attempt to create a new [`RolloutRecorder`].
    ///
    /// For newly created sessions, this precomputes path/metadata and defers
    /// file creation/open until an explicit `persist()` call.
    ///
    /// For resumed sessions, this immediately opens the existing rollout file.
    pub async fn new(
        config: &Config,
        params: RolloutRecorderParams,
        state_db_ctx: Option<StateDbHandle>,
        state_builder: Option<ThreadMetadataBuilder>,
    ) -> std::io::Result<Self> {
        let (file, deferred_log_file_info, rollout_path, meta) = match params {
            RolloutRecorderParams::Create {
                conversation_id,
                forked_from_id,
                source,
                base_instructions,
                dynamic_tools,
            } => {
                let log_file_info = precompute_log_file_info(config, conversation_id)?;
                let path = log_file_info.path.clone();
                let session_id = log_file_info.conversation_id;
                let started_at = log_file_info.timestamp;

                let timestamp_format: &[FormatItem] = format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
                );
                let timestamp = started_at
                    .to_offset(time::UtcOffset::UTC)
                    .format(timestamp_format)
                    .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

                let session_meta = SessionMeta {
                    id: session_id,
                    forked_from_id,
                    timestamp,
                    cwd: config.cwd.clone(),
                    originator: originator().value,
                    cli_version: env!("CARGO_PKG_VERSION").to_string(),
                    source,
                    model_provider: Some(config.model_provider_id.clone()),
                    base_instructions: Some(base_instructions),
                    dynamic_tools: if dynamic_tools.is_empty() {
                        None
                    } else {
                        Some(dynamic_tools)
                    },
                };

                (None, Some(log_file_info), path, Some(session_meta))
            }
            RolloutRecorderParams::Resume { path } => (
                Some(
                    tokio::fs::OpenOptions::new()
                        .append(true)
                        .open(&path)
                        .await?,
                ),
                None,
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
        tokio::task::spawn(rollout_writer(
            file,
            deferred_log_file_info,
            rx,
            meta,
            cwd,
            rollout_path.clone(),
            state_db_ctx.clone(),
            state_builder,
            config.model_provider_id.clone(),
        ));

        Ok(Self {
            tx,
            rollout_path,
            state_db: state_db_ctx,
        })
    }

    pub fn rollout_path(&self) -> &Path {
        self.rollout_path.as_path()
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.state_db.clone()
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

    /// Materialize the rollout file and persist all buffered items.
    ///
    /// This is idempotent; after first materialization, repeated calls are no-ops.
    pub async fn persist(&self) -> std::io::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Persist { ack: tx })
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout persist: {e}")))?;
        rx.await
            .map_err(|e| IoError::other(format!("failed waiting for rollout persist: {e}")))
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

    pub(crate) async fn load_rollout_items(
        path: &Path,
    ) -> std::io::Result<(Vec<RolloutItem>, Option<ThreadId>, usize)> {
        trace!("Resuming rollout from {path:?}");
        let text = tokio::fs::read_to_string(path).await?;
        if text.trim().is_empty() {
            return Err(IoError::other("empty session file"));
        }

        let mut items: Vec<RolloutItem> = Vec::new();
        let mut thread_id: Option<ThreadId> = None;
        let mut parse_errors = 0usize;
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    warn!("failed to parse line as JSON: {line:?}, error: {e}");
                    parse_errors = parse_errors.saturating_add(1);
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
                    trace!("failed to parse rollout line: {e}");
                    parse_errors = parse_errors.saturating_add(1);
                }
            }
        }

        tracing::debug!(
            "Resumed rollout with {} items, thread ID: {:?}, parse errors: {}",
            items.len(),
            thread_id,
            parse_errors,
        );
        Ok((items, thread_id, parse_errors))
    }

    pub async fn get_rollout_history(path: &Path) -> std::io::Result<InitialHistory> {
        let (items, thread_id, _parse_errors) = Self::load_rollout_items(path).await?;
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

fn truncate_fs_page(
    mut page: ThreadsPage,
    page_size: usize,
    sort_key: ThreadSortKey,
) -> ThreadsPage {
    if page.items.len() <= page_size {
        return page;
    }
    page.items.truncate(page_size);
    page.next_cursor = page.items.last().and_then(|item| {
        let file_name = item.path.file_name()?.to_str()?;
        let (created_at, id) = parse_timestamp_uuid_from_filename(file_name)?;
        let cursor_token = match sort_key {
            ThreadSortKey::CreatedAt => format!("{}|{id}", created_at.format(&Rfc3339).ok()?),
            ThreadSortKey::UpdatedAt => format!("{}|{id}", item.updated_at.as_deref()?),
        };
        parse_cursor(cursor_token.as_str())
    });
    page
}

struct LogFileInfo {
    /// Full path to the rollout file.
    path: PathBuf,

    /// Session ID (also embedded in filename).
    conversation_id: ThreadId,

    /// Timestamp for the start of the session.
    timestamp: OffsetDateTime,
}

fn precompute_log_file_info(
    config: &Config,
    conversation_id: ThreadId,
) -> std::io::Result<LogFileInfo> {
    // Resolve ~/.codex/sessions/YYYY/MM/DD path.
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
    dir.push(timestamp.year().to_string());
    dir.push(format!("{:02}", u8::from(timestamp.month())));
    dir.push(format!("{:02}", timestamp.day()));

    // Custom format for YYYY-MM-DDThh-mm-ss. Use `-` instead of `:` for
    // compatibility with filesystems that do not allow colons in filenames.
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

    let filename = format!("rollout-{date_str}-{conversation_id}.jsonl");

    let path = dir.join(filename);

    Ok(LogFileInfo {
        path,
        conversation_id,
        timestamp,
    })
}

fn open_log_file(path: &Path) -> std::io::Result<File> {
    let Some(parent) = path.parent() else {
        return Err(IoError::other(format!(
            "rollout path has no parent: {}",
            path.display()
        )));
    };
    fs::create_dir_all(parent)?;
    std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
}

#[allow(clippy::too_many_arguments)]
async fn rollout_writer(
    file: Option<tokio::fs::File>,
    mut deferred_log_file_info: Option<LogFileInfo>,
    mut rx: mpsc::Receiver<RolloutCmd>,
    mut meta: Option<SessionMeta>,
    cwd: std::path::PathBuf,
    rollout_path: PathBuf,
    state_db_ctx: Option<StateDbHandle>,
    mut state_builder: Option<ThreadMetadataBuilder>,
    default_provider: String,
) -> std::io::Result<()> {
    let mut writer = file.map(|file| JsonlWriter { file });
    let mut buffered_items = Vec::<RolloutItem>::new();
    if let Some(builder) = state_builder.as_mut() {
        builder.rollout_path = rollout_path.clone();
    }

    // Resumed sessions already have a file handle open, so session metadata can
    // be written immediately if present.
    if writer.is_some()
        && let Some(session_meta) = meta.take()
    {
        write_session_meta(
            writer.as_mut(),
            session_meta,
            &cwd,
            &rollout_path,
            state_db_ctx.as_deref(),
            &mut state_builder,
            default_provider.as_str(),
        )
        .await?;
    }

    // Process rollout commands
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(items) => {
                let mut persisted_items = Vec::new();
                for item in items {
                    if is_persisted_response_item(&item) {
                        persisted_items.push(item);
                    }
                }
                if persisted_items.is_empty() {
                    continue;
                }

                if writer.is_none() {
                    buffered_items.extend(persisted_items);
                    continue;
                }

                write_and_reconcile_items(
                    writer.as_mut(),
                    persisted_items.as_slice(),
                    &rollout_path,
                    state_db_ctx.as_deref(),
                    &mut state_builder,
                    default_provider.as_str(),
                )
                .await?;
            }
            RolloutCmd::Persist { ack } => {
                if writer.is_none() {
                    let result = async {
                        let Some(log_file_info) = deferred_log_file_info.take() else {
                            return Err(IoError::other(
                                "deferred rollout recorder missing log file metadata",
                            ));
                        };
                        let file = open_log_file(log_file_info.path.as_path())?;
                        writer = Some(JsonlWriter {
                            file: tokio::fs::File::from_std(file),
                        });

                        if let Some(session_meta) = meta.take() {
                            write_session_meta(
                                writer.as_mut(),
                                session_meta,
                                &cwd,
                                &rollout_path,
                                state_db_ctx.as_deref(),
                                &mut state_builder,
                                default_provider.as_str(),
                            )
                            .await?;
                        }

                        if !buffered_items.is_empty() {
                            write_and_reconcile_items(
                                writer.as_mut(),
                                buffered_items.as_slice(),
                                &rollout_path,
                                state_db_ctx.as_deref(),
                                &mut state_builder,
                                default_provider.as_str(),
                            )
                            .await?;
                            buffered_items.clear();
                        }

                        Ok(())
                    }
                    .await;

                    if let Err(err) = result {
                        let _ = ack.send(());
                        return Err(err);
                    }
                }
                let _ = ack.send(());
            }
            RolloutCmd::Flush { ack } => {
                // Deferred fresh threads may not have an initialized file yet.
                if let Some(writer) = writer.as_mut()
                    && let Err(e) = writer.file.flush().await
                {
                    let _ = ack.send(());
                    return Err(e);
                }
                let _ = ack.send(());
            }
            RolloutCmd::Shutdown { ack } => {
                let _ = ack.send(());
            }
        }
    }

    Ok(())
}

async fn write_session_meta(
    mut writer: Option<&mut JsonlWriter>,
    session_meta: SessionMeta,
    cwd: &Path,
    rollout_path: &Path,
    state_db_ctx: Option<&StateRuntime>,
    state_builder: &mut Option<ThreadMetadataBuilder>,
    default_provider: &str,
) -> std::io::Result<()> {
    let git_info = collect_git_info(cwd).await;
    let session_meta_line = SessionMetaLine {
        meta: session_meta,
        git: git_info,
    };
    if state_db_ctx.is_some() {
        *state_builder = metadata::builder_from_session_meta(&session_meta_line, rollout_path);
    }

    let rollout_item = RolloutItem::SessionMeta(session_meta_line);
    if let Some(writer) = writer.as_mut() {
        writer.write_rollout_item(&rollout_item).await?;
    }
    state_db::reconcile_rollout(
        state_db_ctx,
        rollout_path,
        default_provider,
        state_builder.as_ref(),
        std::slice::from_ref(&rollout_item),
        None,
    )
    .await;
    Ok(())
}

async fn write_and_reconcile_items(
    mut writer: Option<&mut JsonlWriter>,
    items: &[RolloutItem],
    rollout_path: &Path,
    state_db_ctx: Option<&StateRuntime>,
    state_builder: &mut Option<ThreadMetadataBuilder>,
    default_provider: &str,
) -> std::io::Result<()> {
    if let Some(writer) = writer.as_mut() {
        for item in items {
            writer.write_rollout_item(item).await?;
        }
    }
    if let Some(builder) = state_builder.as_mut() {
        builder.rollout_path = rollout_path.to_path_buf();
    }
    state_db::apply_rollout_items(
        state_db_ctx,
        rollout_path,
        default_provider,
        state_builder.as_ref(),
        items,
        "rollout_writer",
    )
    .await;
    Ok(())
}

struct JsonlWriter {
    file: tokio::fs::File,
}

#[derive(serde::Serialize)]
struct RolloutLineRef<'a> {
    timestamp: String,
    #[serde(flatten)]
    item: &'a RolloutItem,
}

impl JsonlWriter {
    async fn write_rollout_item(&mut self, rollout_item: &RolloutItem) -> std::io::Result<()> {
        let timestamp_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        let timestamp = OffsetDateTime::now_utc()
            .format(timestamp_format)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

        let line = RolloutLineRef {
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

impl From<codex_state::ThreadsPage> for ThreadsPage {
    fn from(db_page: codex_state::ThreadsPage) -> Self {
        let items = db_page
            .items
            .into_iter()
            .map(|item| ThreadItem {
                path: item.rollout_path,
                thread_id: Some(item.id),
                first_user_message: item.first_user_message,
                cwd: Some(item.cwd),
                git_branch: item.git_branch,
                git_sha: item.git_sha,
                git_origin_url: item.git_origin_url,
                source: Some(
                    serde_json::from_value(Value::String(item.source))
                        .unwrap_or(SessionSource::Unknown),
                ),
                model_provider: Some(item.model_provider),
                cli_version: Some(item.cli_version),
                created_at: Some(item.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)),
                updated_at: Some(item.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)),
            })
            .collect();
        Self {
            items,
            next_cursor: db_page.next_anchor.map(Into::into),
            num_scanned_files: db_page.num_scanned_rows,
            reached_scan_cap: false,
        }
    }
}

fn select_resume_path(page: &ThreadsPage, filter_cwd: Option<&Path>) -> Option<PathBuf> {
    match filter_cwd {
        Some(cwd) => page.items.iter().find_map(|item| {
            if item
                .cwd
                .as_ref()
                .is_some_and(|session_cwd| cwd_matches(session_cwd, cwd))
            {
                Some(item.path.clone())
            } else {
                None
            }
        }),
        None => page.items.first().map(|item| item.path.clone()),
    }
}

fn select_resume_path_from_db_page(
    page: &codex_state::ThreadsPage,
    filter_cwd: Option<&Path>,
) -> Option<PathBuf> {
    match filter_cwd {
        Some(cwd) => page.items.iter().find_map(|item| {
            if cwd_matches(item.cwd.as_path(), cwd) {
                Some(item.rollout_path.clone())
            } else {
                None
            }
        }),
        None => page.items.first().map(|item| item.rollout_path.clone()),
    }
}

fn cwd_matches(session_cwd: &Path, cwd: &Path) -> bool {
    if let (Ok(ca), Ok(cb)) = (
        path_utils::normalize_for_path_comparison(session_cwd),
        path_utils::normalize_for_path_comparison(cwd),
    ) {
        return ca == cb;
    }
    session_cwd == cwd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use crate::features::Feature;
    use chrono::TimeZone;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::fs::{self};
    use std::io::Write;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn write_session_file(root: &Path, ts: &str, uuid: Uuid) -> std::io::Result<PathBuf> {
        let day_dir = root.join("sessions/2025/01/03");
        fs::create_dir_all(&day_dir)?;
        let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
        let mut file = File::create(&path)?;
        let meta = serde_json::json!({
            "timestamp": ts,
            "type": "session_meta",
            "payload": {
                "id": uuid,
                "timestamp": ts,
                "cwd": ".",
                "originator": "test_originator",
                "cli_version": "test_version",
                "source": "cli",
                "model_provider": "test-provider",
            },
        });
        writeln!(file, "{meta}")?;
        let user_event = serde_json::json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": "Hello from user",
                "kind": "plain",
            },
        });
        writeln!(file, "{user_event}")?;
        Ok(path)
    }

    #[tokio::test]
    async fn recorder_materializes_only_after_explicit_persist() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let thread_id = ThreadId::new();
        let recorder = RolloutRecorder::new(
            &config,
            RolloutRecorderParams::new(
                thread_id,
                None,
                SessionSource::Exec,
                BaseInstructions::default(),
                Vec::new(),
            ),
            None,
            None,
        )
        .await?;

        let rollout_path = recorder.rollout_path().to_path_buf();
        assert!(
            !rollout_path.exists(),
            "rollout file should not exist before first user message"
        );

        recorder
            .record_items(&[RolloutItem::EventMsg(EventMsg::AgentMessage(
                AgentMessageEvent {
                    message: "buffered-event".to_string(),
                },
            ))])
            .await?;
        recorder.flush().await?;
        assert!(
            !rollout_path.exists(),
            "rollout file should remain deferred before first user message"
        );

        recorder
            .record_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(
                UserMessageEvent {
                    message: "first-user-message".to_string(),
                    images: None,
                    local_images: Vec::new(),
                    text_elements: Vec::new(),
                },
            ))])
            .await?;
        recorder.flush().await?;
        assert!(
            !rollout_path.exists(),
            "user-message-like items should not materialize without explicit persist"
        );

        recorder.persist().await?;
        // Second call verifies `persist()` is idempotent after materialization.
        recorder.persist().await?;
        assert!(rollout_path.exists(), "rollout file should be materialized");

        let text = std::fs::read_to_string(&rollout_path)?;
        assert!(
            text.contains("\"type\":\"session_meta\""),
            "expected session metadata in rollout"
        );
        let buffered_idx = text
            .find("buffered-event")
            .expect("buffered event in rollout");
        let user_idx = text
            .find("first-user-message")
            .expect("first user message in rollout");
        assert!(
            buffered_idx < user_idx,
            "buffered items should preserve ordering"
        );
        let text_after_second_persist = std::fs::read_to_string(&rollout_path)?;
        assert_eq!(text_after_second_persist, text);

        recorder.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn list_threads_db_disabled_does_not_skip_paginated_items() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        config.features.disable(Feature::Sqlite);

        let newest = write_session_file(home.path(), "2025-01-03T12-00-00", Uuid::from_u128(9001))?;
        let middle = write_session_file(home.path(), "2025-01-02T12-00-00", Uuid::from_u128(9002))?;
        let _oldest =
            write_session_file(home.path(), "2025-01-01T12-00-00", Uuid::from_u128(9003))?;

        let default_provider = config.model_provider_id.clone();
        let page1 = RolloutRecorder::list_threads(
            &config,
            1,
            None,
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
        )
        .await?;
        assert_eq!(page1.items.len(), 1);
        assert_eq!(page1.items[0].path, newest);
        let cursor = page1.next_cursor.clone().expect("cursor should be present");

        let page2 = RolloutRecorder::list_threads(
            &config,
            1,
            Some(&cursor),
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
        )
        .await?;
        assert_eq!(page2.items.len(), 1);
        assert_eq!(page2.items[0].path, middle);
        Ok(())
    }

    #[tokio::test]
    async fn list_threads_db_enabled_drops_missing_rollout_paths() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        config.features.enable(Feature::Sqlite);

        let uuid = Uuid::from_u128(9010);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let stale_path = home.path().join(format!(
            "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
        ));

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.model_provider_id.clone(),
            None,
        )
        .await
        .expect("state db should initialize");
        runtime
            .mark_backfill_complete(None)
            .await
            .expect("backfill should be complete");
        let created_at = chrono::Utc
            .with_ymd_and_hms(2025, 1, 3, 13, 0, 0)
            .single()
            .expect("valid datetime");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            stale_path,
            created_at,
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.first_user_message = Some("Hello from user".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let default_provider = config.model_provider_id.clone();
        let page = RolloutRecorder::list_threads(
            &config,
            10,
            None,
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
        )
        .await?;
        assert_eq!(page.items.len(), 0);
        let stored_path = runtime
            .find_rollout_path_by_id(thread_id, Some(false))
            .await
            .expect("state db lookup should succeed");
        assert_eq!(stored_path, None);
        Ok(())
    }

    #[tokio::test]
    async fn list_threads_db_enabled_repairs_stale_rollout_paths() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        config.features.enable(Feature::Sqlite);

        let uuid = Uuid::from_u128(9011);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let real_path = write_session_file(home.path(), "2025-01-03T13-00-00", uuid)?;
        let stale_path = home.path().join(format!(
            "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
        ));

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.model_provider_id.clone(),
            None,
        )
        .await
        .expect("state db should initialize");
        runtime
            .mark_backfill_complete(None)
            .await
            .expect("backfill should be complete");
        let created_at = chrono::Utc
            .with_ymd_and_hms(2025, 1, 3, 13, 0, 0)
            .single()
            .expect("valid datetime");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            stale_path,
            created_at,
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.first_user_message = Some("Hello from user".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let default_provider = config.model_provider_id.clone();
        let page = RolloutRecorder::list_threads(
            &config,
            1,
            None,
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
        )
        .await?;
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].path, real_path);

        let repaired_path = runtime
            .find_rollout_path_by_id(thread_id, Some(false))
            .await
            .expect("state db lookup should succeed");
        assert_eq!(repaired_path, Some(real_path));
        Ok(())
    }
}
