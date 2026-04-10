//! Persist Codex session rollouts (.jsonl) so sessions can be replayed or inspected later.

use std::fs;
use std::fs::File;
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::BaseInstructions;
use codex_utils_string::truncate_middle_chars;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;

use super::ARCHIVED_SESSIONS_SUBDIR;
use super::SESSIONS_SUBDIR;
use super::list::Cursor;
use super::list::SortDirection;
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
use super::policy::EventPersistenceMode;
use super::policy::is_persisted_response_item;
use crate::config::RolloutConfigView;
use crate::default_client::originator;
use crate::state_db;
use crate::state_db::StateDbHandle;
use codex_git_utils::collect_git_info;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GitInfo as ProtocolGitInfo;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_state::StateRuntime;
use codex_state::ThreadMetadataBuilder;
use codex_utils_path as path_utils;

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
    writer_task: Arc<RolloutWriterTask>,
    pub(crate) rollout_path: PathBuf,
    state_db: Option<StateDbHandle>,
    event_persistence_mode: EventPersistenceMode,
}

#[derive(Clone)]
pub enum RolloutRecorderParams {
    Create {
        conversation_id: ThreadId,
        forked_from_id: Option<ThreadId>,
        source: SessionSource,
        base_instructions: BaseInstructions,
        dynamic_tools: Vec<DynamicToolSpec>,
        event_persistence_mode: EventPersistenceMode,
    },
    Resume {
        path: PathBuf,
        event_persistence_mode: EventPersistenceMode,
    },
}

enum RolloutCmd {
    AddItems(Vec<RolloutItem>),
    Persist {
        ack: oneshot::Sender<std::io::Result<()>>,
    },
    /// Ensure all prior writes are processed; respond when flushed.
    Flush {
        ack: oneshot::Sender<std::io::Result<()>>,
    },
    Shutdown {
        ack: oneshot::Sender<std::io::Result<()>>,
    },
}

/// Observable state for the background rollout writer task.
struct RolloutWriterTask {
    handle: Mutex<Option<JoinHandle<()>>>,
    terminal_failure: Mutex<Option<Arc<IoError>>>,
}

impl RolloutWriterTask {
    /// Create task observability state before spawning the writer.
    fn new() -> Self {
        Self {
            handle: Mutex::new(None),
            terminal_failure: Mutex::new(None),
        }
    }

    /// Store the spawned task handle so it remains owned for the lifetime of recorder clones.
    fn set_handle(&self, handle: JoinHandle<()>) {
        let mut guard = self
            .handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(handle);
    }

    /// Remember a terminal task failure for future recorder API calls.
    fn mark_failed(&self, err: &IoError) {
        let mut guard = self
            .terminal_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(Arc::new(clone_io_error(err)));
    }

    /// Return the terminal writer-task failure, if the task exited with an error.
    fn terminal_failure(&self) -> Option<IoError> {
        let guard = self
            .terminal_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.as_ref().map(|err| clone_io_error(err.as_ref()))
    }
}

fn clone_io_error(err: &IoError) -> IoError {
    IoError::new(err.kind(), err.to_string())
}

impl RolloutRecorderParams {
    pub fn new(
        conversation_id: ThreadId,
        forked_from_id: Option<ThreadId>,
        source: SessionSource,
        base_instructions: BaseInstructions,
        dynamic_tools: Vec<DynamicToolSpec>,
        event_persistence_mode: EventPersistenceMode,
    ) -> Self {
        Self::Create {
            conversation_id,
            forked_from_id,
            source,
            base_instructions,
            dynamic_tools,
            event_persistence_mode,
        }
    }

    pub fn resume(path: PathBuf, event_persistence_mode: EventPersistenceMode) -> Self {
        Self::Resume {
            path,
            event_persistence_mode,
        }
    }
}

const PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES: usize = 10_000;

fn sanitize_rollout_item_for_persistence(
    item: RolloutItem,
    mode: EventPersistenceMode,
) -> RolloutItem {
    if mode != EventPersistenceMode::Extended {
        return item;
    }

    match item {
        RolloutItem::EventMsg(EventMsg::ExecCommandEnd(mut event)) => {
            // Persist only a bounded aggregated summary of command output.
            event.aggregated_output = truncate_middle_chars(
                &event.aggregated_output,
                PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES,
            );
            // Drop unnecessary fields from rollout storage since aggregated_output is all we need.
            event.stdout.clear();
            event.stderr.clear();
            event.formatted_output.clear();
            RolloutItem::EventMsg(EventMsg::ExecCommandEnd(event))
        }
        _ => item,
    }
}

impl RolloutRecorder {
    /// List threads (rollout files) under the provided Codex home directory.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_threads(
        config: &impl RolloutConfigView,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        sort_direction: SortDirection,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
        search_term: Option<&str>,
    ) -> std::io::Result<ThreadsPage> {
        Self::list_threads_with_db_fallback(
            config,
            page_size,
            cursor,
            sort_key,
            sort_direction,
            allowed_sources,
            model_providers,
            default_provider,
            /*archived*/ false,
            search_term,
        )
        .await
    }

    /// List archived threads (rollout files) under the archived sessions directory.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_archived_threads(
        config: &impl RolloutConfigView,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        sort_direction: SortDirection,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
        search_term: Option<&str>,
    ) -> std::io::Result<ThreadsPage> {
        Self::list_threads_with_db_fallback(
            config,
            page_size,
            cursor,
            sort_key,
            sort_direction,
            allowed_sources,
            model_providers,
            default_provider,
            /*archived*/ true,
            search_term,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_threads_with_db_fallback(
        config: &impl RolloutConfigView,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        sort_direction: SortDirection,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
        archived: bool,
        search_term: Option<&str>,
    ) -> std::io::Result<ThreadsPage> {
        let codex_home = config.codex_home();
        let state_db_ctx = state_db::get_state_db(config).await;

        if search_term.is_some()
            && let Some(db_page) = state_db::list_threads_db(
                state_db_ctx.as_deref(),
                codex_home,
                page_size,
                cursor,
                sort_key,
                sort_direction,
                allowed_sources,
                model_providers,
                archived,
                search_term,
            )
            .await
            && (!db_page.items.is_empty() || cursor.is_some())
        {
            return Ok(db_page.into());
        }

        // Filesystem-first listing intentionally overfetches so we can repair stale/missing
        // SQLite rollout paths before the final DB-backed page is returned.
        let fs_page_size = page_size.saturating_mul(2).max(page_size);
        let fs_page = match sort_direction {
            SortDirection::Asc => {
                list_threads_from_files_asc(
                    codex_home,
                    page_size,
                    cursor,
                    sort_key,
                    allowed_sources,
                    model_providers,
                    default_provider,
                    archived,
                    search_term,
                )
                .await?
            }
            SortDirection::Desc => {
                list_threads_from_files_desc(
                    codex_home,
                    fs_page_size,
                    cursor,
                    sort_key,
                    allowed_sources,
                    model_providers,
                    default_provider,
                    archived,
                )
                .await?
            }
        };

        if state_db_ctx.is_none() {
            // Keep legacy behavior when SQLite is unavailable: return filesystem results
            // at the requested page size.
            return Ok(match sort_direction {
                SortDirection::Asc => fs_page,
                SortDirection::Desc => truncate_fs_page(fs_page, page_size, sort_key),
            });
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
            sort_direction,
            allowed_sources,
            model_providers,
            archived,
            search_term,
        )
        .await
        {
            return Ok(db_page.into());
        }
        // If SQLite listing still fails, return the filesystem page rather than failing the list.
        tracing::error!("Falling back on rollout system");
        tracing::warn!("state db discrepancy during list_threads_with_db_fallback: falling_back");
        Ok(match sort_direction {
            SortDirection::Asc => fs_page,
            SortDirection::Desc => truncate_fs_page(fs_page, page_size, sort_key),
        })
    }

    /// Find the newest recorded thread path, optionally filtering to a matching cwd.
    #[allow(clippy::too_many_arguments)]
    pub async fn find_latest_thread_path(
        config: &impl RolloutConfigView,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
        allowed_sources: &[SessionSource],
        model_providers: Option<&[String]>,
        default_provider: &str,
        filter_cwd: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        let codex_home = config.codex_home();
        let state_db_ctx = state_db::get_state_db(config).await;
        if state_db_ctx.is_some() {
            let mut db_cursor = cursor.cloned();
            loop {
                let Some(db_page) = state_db::list_threads_db(
                    state_db_ctx.as_deref(),
                    codex_home,
                    page_size,
                    db_cursor.as_ref(),
                    sort_key,
                    SortDirection::Desc,
                    allowed_sources,
                    model_providers,
                    /*archived*/ false,
                    /*search_term*/ None,
                )
                .await
                else {
                    break;
                };
                if let Some(path) =
                    select_resume_path_from_db_page(&db_page, filter_cwd, default_provider).await
                {
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
            if let Some(path) = select_resume_path(&page, filter_cwd, default_provider).await {
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
        config: &impl RolloutConfigView,
        params: RolloutRecorderParams,
        state_db_ctx: Option<StateDbHandle>,
        state_builder: Option<ThreadMetadataBuilder>,
    ) -> std::io::Result<Self> {
        let (file, deferred_log_file_info, rollout_path, meta, event_persistence_mode) =
            match params {
                RolloutRecorderParams::Create {
                    conversation_id,
                    forked_from_id,
                    source,
                    base_instructions,
                    dynamic_tools,
                    event_persistence_mode,
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
                        cwd: config.cwd().to_path_buf(),
                        originator: originator().value,
                        cli_version: env!("CARGO_PKG_VERSION").to_string(),
                        agent_nickname: source.get_nickname(),
                        agent_role: source.get_agent_role(),
                        agent_path: source.get_agent_path().map(Into::into),
                        source,
                        model_provider: Some(config.model_provider_id().to_string()),
                        base_instructions: Some(base_instructions),
                        dynamic_tools: if dynamic_tools.is_empty() {
                            None
                        } else {
                            Some(dynamic_tools)
                        },
                        memory_mode: (!config.generate_memories())
                            .then_some("disabled".to_string()),
                    };

                    (
                        None,
                        Some(log_file_info),
                        path,
                        Some(session_meta),
                        event_persistence_mode,
                    )
                }
                RolloutRecorderParams::Resume {
                    path,
                    event_persistence_mode,
                } => (
                    Some(
                        tokio::fs::OpenOptions::new()
                            .append(true)
                            .open(&path)
                            .await?,
                    ),
                    None,
                    path,
                    None,
                    event_persistence_mode,
                ),
            };

        // Clone the cwd for the spawned task to collect git info asynchronously
        let cwd = config.cwd().to_path_buf();

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller's thread.
        let (tx, rx) = mpsc::channel::<RolloutCmd>(256);
        // Spawn a Tokio task that owns the file handle and performs async
        // writes. Using `tokio::fs::File` keeps everything on the async I/O
        // driver instead of blocking the runtime.
        let writer_task = Arc::new(RolloutWriterTask::new());
        let writer_task_for_spawn = Arc::clone(&writer_task);
        let rollout_path_for_spawn = rollout_path.clone();
        let default_provider = config.model_provider_id().to_string();
        let generate_memories = config.generate_memories();
        let state_db_ctx_for_spawn = state_db_ctx.clone();
        let handle = tokio::task::spawn(async move {
            let result = rollout_writer(
                file,
                deferred_log_file_info,
                rx,
                meta,
                cwd,
                rollout_path_for_spawn.clone(),
                state_db_ctx_for_spawn,
                state_builder,
                default_provider,
                generate_memories,
            )
            .await;
            if let Err(err) = result {
                // This is the terminal background-task failure path. Normal I/O failures stay inside
                // `rollout_writer`, are reported through command acks, and leave items buffered for retry.
                error!(
                    "rollout writer task failed for {}: {err}",
                    rollout_path_for_spawn.display()
                );
                writer_task_for_spawn.mark_failed(&err);
            }
        });
        writer_task.set_handle(handle);

        Ok(Self {
            tx,
            writer_task,
            rollout_path,
            state_db: state_db_ctx,
            event_persistence_mode,
        })
    }

    pub fn rollout_path(&self) -> &Path {
        self.rollout_path.as_path()
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.state_db.clone()
    }

    pub async fn record_items(&self, items: &[RolloutItem]) -> std::io::Result<()> {
        let mut filtered = Vec::new();
        for item in items {
            // Note that function calls may look a bit strange if they are
            // "fully qualified MCP tool calls," so we could consider
            // reformatting them in that case.
            if is_persisted_response_item(item, self.event_persistence_mode) {
                filtered.push(sanitize_rollout_item_for_persistence(
                    item.clone(),
                    self.event_persistence_mode,
                ));
            }
        }
        if filtered.is_empty() {
            return Ok(());
        }
        self.tx
            .send(RolloutCmd::AddItems(filtered))
            .await
            .map_err(|e| {
                self.writer_task.terminal_failure().unwrap_or_else(|| {
                    IoError::other(format!("failed to queue rollout items: {e}"))
                })
            })
    }

    /// Materialize the rollout file and persist all buffered items.
    ///
    /// This is idempotent. If materialization fails, the recorder keeps all pending items in memory
    /// and a later `persist()` or `flush()` can retry opening and writing the rollout file.
    pub async fn persist(&self) -> std::io::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Persist { ack: tx })
            .await
            .map_err(|e| {
                self.writer_task.terminal_failure().unwrap_or_else(|| {
                    IoError::other(format!("failed to queue rollout persist: {e}"))
                })
            })?;
        rx.await.map_err(|e| {
            self.writer_task.terminal_failure().unwrap_or_else(|| {
                IoError::other(format!("failed waiting for rollout persist: {e}"))
            })
        })?
    }

    /// Flush all queued writes and wait until they are committed by the writer task.
    ///
    /// If the first writer attempt fails, the writer drops and reopens the file handle before
    /// retrying. This returns an error only when that retry also fails or the writer task is gone.
    pub async fn flush(&self) -> std::io::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Flush { ack: tx })
            .await
            .map_err(|e| {
                self.writer_task.terminal_failure().unwrap_or_else(|| {
                    IoError::other(format!("failed to queue rollout flush: {e}"))
                })
            })?;
        rx.await.map_err(|e| {
            self.writer_task
                .terminal_failure()
                .unwrap_or_else(|| IoError::other(format!("failed waiting for rollout flush: {e}")))
        })?
    }

    pub async fn load_rollout_items(
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

    /// Drain pending items before stopping the writer task.
    ///
    /// If draining fails, the writer stays alive so callers can continue retrying flush/shutdown.
    pub async fn shutdown(&self) -> std::io::Result<()> {
        let (tx_done, rx_done) = oneshot::channel();
        match self.tx.send(RolloutCmd::Shutdown { ack: tx_done }).await {
            Ok(_) => rx_done.await.map_err(|e| {
                self.writer_task.terminal_failure().unwrap_or_else(|| {
                    IoError::other(format!("failed waiting for rollout shutdown: {e}"))
                })
            })??,
            Err(e) => {
                if let Some(err) = self.writer_task.terminal_failure() {
                    warn!(
                        "failed to send rollout shutdown command because writer task failed: {err}"
                    );
                    return Err(err);
                }
                warn!("failed to send rollout shutdown command: {e}");
                return Err(IoError::other(format!(
                    "failed to send rollout shutdown command: {e}"
                )));
            }
        };
        Ok(())
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

#[allow(clippy::too_many_arguments)]
async fn list_threads_from_files_desc(
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
    default_provider: &str,
    archived: bool,
) -> std::io::Result<ThreadsPage> {
    if archived {
        let root = codex_home.join(ARCHIVED_SESSIONS_SUBDIR);
        get_threads_in_root(
            root,
            page_size,
            cursor,
            sort_key,
            ThreadListConfig {
                allowed_sources,
                model_providers,
                default_provider,
                layout: ThreadListLayout::Flat,
            },
        )
        .await
    } else {
        get_threads(
            codex_home,
            page_size,
            cursor,
            sort_key,
            allowed_sources,
            model_providers,
            default_provider,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn list_threads_from_files_asc(
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
    default_provider: &str,
    archived: bool,
    search_term: Option<&str>,
) -> std::io::Result<ThreadsPage> {
    let mut all_items = Vec::new();
    let mut scanned_files = 0usize;
    let mut reached_scan_cap = false;
    let mut page_cursor = None;
    loop {
        let page = list_threads_from_files_desc(
            codex_home,
            /*page_size*/ 256,
            page_cursor.as_ref(),
            sort_key,
            allowed_sources,
            model_providers,
            default_provider,
            archived,
        )
        .await?;
        scanned_files = scanned_files.saturating_add(page.num_scanned_files);
        reached_scan_cap |= page.reached_scan_cap;
        all_items.extend(page.items);
        page_cursor = page.next_cursor;
        if page_cursor.is_none() || reached_scan_cap {
            break;
        }
    }

    if let Some(search_term) = search_term {
        all_items.retain(|item| {
            item.first_user_message
                .as_deref()
                .is_some_and(|preview| preview.contains(search_term))
        });
    }

    all_items.sort_by_key(|item| thread_item_sort_key(item, sort_key));
    if let Some(cursor) = cursor {
        let anchor = cursor.parts();
        all_items
            .retain(|item| thread_item_sort_key(item, sort_key).is_some_and(|key| key > anchor));
    }

    let more_matches_available = all_items.len() > page_size;
    all_items.truncate(page_size);
    let next_cursor = if more_matches_available {
        all_items
            .last()
            .and_then(|item| cursor_from_thread_item(item, sort_key))
    } else {
        None
    };

    Ok(ThreadsPage {
        items: all_items,
        next_cursor,
        num_scanned_files: scanned_files,
        reached_scan_cap,
    })
}

fn thread_item_sort_key(
    item: &ThreadItem,
    sort_key: ThreadSortKey,
) -> Option<(OffsetDateTime, uuid::Uuid)> {
    let file_name = item.path.file_name()?.to_str()?;
    let (created_at, id) = parse_timestamp_uuid_from_filename(file_name)?;
    let timestamp = match sort_key {
        ThreadSortKey::CreatedAt => created_at,
        ThreadSortKey::UpdatedAt => {
            let updated_at = item.updated_at.as_deref()?;
            OffsetDateTime::parse(updated_at, &Rfc3339).ok()?
        }
    };
    Some((timestamp, id))
}

fn cursor_from_thread_item(item: &ThreadItem, sort_key: ThreadSortKey) -> Option<Cursor> {
    let (timestamp, id) = thread_item_sort_key(item, sort_key)?;
    let cursor_token = format!("{}|{id}", timestamp.format(&Rfc3339).ok()?);
    parse_cursor(cursor_token.as_str())
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
    config: &impl RolloutConfigView,
    conversation_id: ThreadId,
) -> std::io::Result<LogFileInfo> {
    // Resolve ~/.codex/sessions/YYYY/MM/DD path.
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;
    let mut dir = config.codex_home().to_path_buf();
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

/// Mutable state owned by the background rollout writer.
///
/// Items are first appended to `pending_items`; persist/flush/shutdown remove each item from that
/// queue only after it is written successfully. I/O failures drop the file handle but keep the
/// unwritten suffix so the next barrier can reopen the file and retry.
struct RolloutWriterState {
    writer: Option<JsonlWriter>,
    deferred_log_file_info: Option<LogFileInfo>,
    pending_items: Vec<RolloutItem>,
    meta: Option<SessionMeta>,
    cwd: PathBuf,
    rollout_path: PathBuf,
    state_db_ctx: Option<StateDbHandle>,
    state_builder: Option<ThreadMetadataBuilder>,
    default_provider: String,
    generate_memories: bool,
    last_logged_error: Option<String>,
}

impl RolloutWriterState {
    #[allow(clippy::too_many_arguments)]
    fn new(
        file: Option<tokio::fs::File>,
        deferred_log_file_info: Option<LogFileInfo>,
        meta: Option<SessionMeta>,
        cwd: PathBuf,
        rollout_path: PathBuf,
        state_db_ctx: Option<StateDbHandle>,
        mut state_builder: Option<ThreadMetadataBuilder>,
        default_provider: String,
        generate_memories: bool,
    ) -> Self {
        if let Some(builder) = state_builder.as_mut() {
            builder.rollout_path = rollout_path.clone();
        }
        Self {
            writer: file.map(|file| JsonlWriter { file }),
            deferred_log_file_info,
            pending_items: Vec::new(),
            meta,
            cwd,
            rollout_path,
            state_db_ctx,
            state_builder,
            default_provider,
            generate_memories,
            last_logged_error: None,
        }
    }

    fn add_items(&mut self, items: Vec<RolloutItem>) {
        self.pending_items.extend(items);
    }

    async fn flush_if_materialized(&mut self) {
        if self.is_deferred() {
            return;
        }
        if let Err(err) = self.flush().await {
            self.enter_recovery_mode(&err);
        }
    }

    async fn persist(&mut self) -> std::io::Result<()> {
        self.write_pending_with_recovery("persist").await
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        if self.is_deferred() && self.pending_items.is_empty() {
            return Ok(());
        }
        self.write_pending_with_recovery("flush").await
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        if self.is_deferred() && self.pending_items.is_empty() {
            return Ok(());
        }
        self.write_pending_with_recovery("shutdown").await
    }

    async fn write_pending_with_recovery(&mut self, operation: &str) -> std::io::Result<()> {
        match self.write_pending_once().await {
            Ok(()) => {
                self.last_logged_error = None;
                Ok(())
            }
            Err(first_err) => {
                self.enter_recovery_mode(&first_err);
                warn!("failed to {operation} rollout writer; reopening and retrying: {first_err}");
                match self.write_pending_once().await {
                    Ok(()) => {
                        self.last_logged_error = None;
                        Ok(())
                    }
                    Err(second_err) => {
                        self.enter_recovery_mode(&second_err);
                        warn!(
                            "retrying rollout writer {operation} failed; first error: \
                             {first_err}; final error: {second_err}"
                        );
                        Err(second_err)
                    }
                }
            }
        }
    }

    fn is_deferred(&self) -> bool {
        self.writer.is_none() && self.deferred_log_file_info.is_some()
    }

    fn enter_recovery_mode(&mut self, err: &IoError) {
        let message = err.to_string();
        if self.last_logged_error.as_ref() != Some(&message) {
            error!(
                "rollout writer failed for {}; buffered rollout items will be retried: {err}",
                self.rollout_path.display()
            );
        }
        self.last_logged_error = Some(message);
        self.writer = None;
    }

    async fn ensure_writer_open(&mut self) -> std::io::Result<()> {
        if self.writer.is_some() {
            return Ok(());
        }

        let path = self
            .deferred_log_file_info
            .as_ref()
            .map(|info| info.path.as_path())
            .unwrap_or(self.rollout_path.as_path());
        let file = open_log_file(path)?;
        self.writer = Some(JsonlWriter {
            file: tokio::fs::File::from_std(file),
        });
        self.deferred_log_file_info = None;
        Ok(())
    }

    async fn write_session_meta_if_needed(&mut self) -> std::io::Result<()> {
        let Some(session_meta) = self.meta.as_ref().cloned() else {
            return Ok(());
        };
        write_session_meta(
            self.writer.as_mut(),
            session_meta,
            &self.cwd,
            &self.rollout_path,
            self.state_db_ctx.as_deref(),
            &mut self.state_builder,
            self.default_provider.as_str(),
            self.generate_memories,
        )
        .await?;
        self.meta = None;
        Ok(())
    }

    async fn write_pending_once(&mut self) -> std::io::Result<()> {
        self.ensure_writer_open().await?;
        self.write_session_meta_if_needed().await?;

        self.write_pending_items_once().await?;

        if let Some(writer) = self.writer.as_mut() {
            writer.file.flush().await?;
        }
        Ok(())
    }

    async fn write_pending_items_once(&mut self) -> std::io::Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Err(IoError::other("rollout writer is not open"));
        };

        let mut written_count = 0usize;
        let mut write_result = Ok(());
        for item in &self.pending_items {
            if let Err(err) = writer.write_rollout_item(item).await {
                write_result = Err(err);
                break;
            }
            written_count += 1;
        }

        if written_count > 0 {
            let written_items: Vec<RolloutItem> =
                self.pending_items.drain(..written_count).collect();
            sync_thread_state_after_write(
                self.state_db_ctx.as_deref(),
                &self.rollout_path,
                self.state_builder.as_ref(),
                written_items.as_slice(),
                self.default_provider.as_str(),
                /*new_thread_memory_mode*/ None,
            )
            .await;
        }

        write_result
    }
}

#[allow(clippy::too_many_arguments)]
async fn rollout_writer(
    file: Option<tokio::fs::File>,
    deferred_log_file_info: Option<LogFileInfo>,
    mut rx: mpsc::Receiver<RolloutCmd>,
    meta: Option<SessionMeta>,
    cwd: PathBuf,
    rollout_path: PathBuf,
    state_db_ctx: Option<StateDbHandle>,
    state_builder: Option<ThreadMetadataBuilder>,
    default_provider: String,
    generate_memories: bool,
) -> std::io::Result<()> {
    let mut state = RolloutWriterState::new(
        file,
        deferred_log_file_info,
        meta,
        cwd,
        rollout_path,
        state_db_ctx,
        state_builder,
        default_provider,
        generate_memories,
    );

    // Process rollout commands
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(items) => {
                state.add_items(items);
                state.flush_if_materialized().await;
            }
            RolloutCmd::Persist { ack } => {
                let _ = ack.send(state.persist().await);
            }
            RolloutCmd::Flush { ack } => {
                let _ = ack.send(state.flush().await);
            }
            RolloutCmd::Shutdown { ack } => match state.shutdown().await {
                Ok(()) => {
                    let _ = ack.send(Ok(()));
                    break;
                }
                Err(err) => {
                    let _ = ack.send(Err(err));
                }
            },
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn write_session_meta(
    mut writer: Option<&mut JsonlWriter>,
    session_meta: SessionMeta,
    cwd: &Path,
    rollout_path: &Path,
    state_db_ctx: Option<&StateRuntime>,
    state_builder: &mut Option<ThreadMetadataBuilder>,
    default_provider: &str,
    generate_memories: bool,
) -> std::io::Result<()> {
    let git_info = collect_git_info(cwd).await.map(|info| ProtocolGitInfo {
        commit_hash: info.commit_hash,
        branch: info.branch,
        repository_url: info.repository_url,
    });
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
    sync_thread_state_after_write(
        state_db_ctx,
        rollout_path,
        state_builder.as_ref(),
        std::slice::from_ref(&rollout_item),
        default_provider,
        (!generate_memories).then_some("disabled"),
    )
    .await;
    Ok(())
}

async fn sync_thread_state_after_write(
    state_db_ctx: Option<&StateRuntime>,
    rollout_path: &Path,
    state_builder: Option<&ThreadMetadataBuilder>,
    items: &[RolloutItem],
    default_provider: &str,
    new_thread_memory_mode: Option<&str>,
) {
    let updated_at = Utc::now();
    if new_thread_memory_mode.is_some()
        || items
            .iter()
            .any(codex_state::rollout_item_affects_thread_metadata)
    {
        state_db::apply_rollout_items(
            state_db_ctx,
            rollout_path,
            default_provider,
            state_builder,
            items,
            "rollout_writer",
            new_thread_memory_mode,
            Some(updated_at),
        )
        .await;
        return;
    }

    let thread_id = state_builder
        .map(|builder| builder.id)
        .or_else(|| metadata::builder_from_items(items, rollout_path).map(|builder| builder.id));
    if state_db::touch_thread_updated_at(state_db_ctx, thread_id, updated_at, "rollout_writer")
        .await
    {
        return;
    }
    state_db::apply_rollout_items(
        state_db_ctx,
        rollout_path,
        default_provider,
        state_builder,
        items,
        "rollout_writer",
        new_thread_memory_mode,
        Some(updated_at),
    )
    .await;
}

/// Append one already-filtered rollout item to an existing rollout JSONL file.
///
/// This is for metadata updates to unloaded threads. Live sessions should use
/// `RolloutRecorder::record_items` so rollout and SQLite updates remain ordered
/// with the rest of the session stream.
pub async fn append_rollout_item_to_path(
    rollout_path: &Path,
    item: &RolloutItem,
) -> std::io::Result<()> {
    let file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(rollout_path)
        .await?;
    let mut writer = JsonlWriter { file };
    writer.write_rollout_item(item).await
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
                    serde_json::from_str(item.source.as_str())
                        .or_else(|_| serde_json::from_value(Value::String(item.source)))
                        .unwrap_or(SessionSource::Unknown),
                ),
                agent_nickname: item.agent_nickname,
                agent_role: item.agent_role,
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

async fn select_resume_path(
    page: &ThreadsPage,
    filter_cwd: Option<&Path>,
    default_provider: &str,
) -> Option<PathBuf> {
    match filter_cwd {
        Some(cwd) => {
            for item in &page.items {
                if resume_candidate_matches_cwd(
                    item.path.as_path(),
                    item.cwd.as_deref(),
                    cwd,
                    default_provider,
                )
                .await
                {
                    return Some(item.path.clone());
                }
            }
            None
        }
        None => page.items.first().map(|item| item.path.clone()),
    }
}

async fn resume_candidate_matches_cwd(
    rollout_path: &Path,
    cached_cwd: Option<&Path>,
    cwd: &Path,
    default_provider: &str,
) -> bool {
    if cached_cwd.is_some_and(|session_cwd| cwd_matches(session_cwd, cwd)) {
        return true;
    }

    if let Ok((items, _, _)) = RolloutRecorder::load_rollout_items(rollout_path).await
        && let Some(latest_turn_context_cwd) = items.iter().rev().find_map(|item| match item {
            RolloutItem::TurnContext(turn_context) => Some(turn_context.cwd.as_path()),
            RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::EventMsg(_) => None,
        })
    {
        return cwd_matches(latest_turn_context_cwd, cwd);
    }

    metadata::extract_metadata_from_rollout(rollout_path, default_provider)
        .await
        .is_ok_and(|outcome| cwd_matches(outcome.metadata.cwd.as_path(), cwd))
}

async fn select_resume_path_from_db_page(
    page: &codex_state::ThreadsPage,
    filter_cwd: Option<&Path>,
    default_provider: &str,
) -> Option<PathBuf> {
    match filter_cwd {
        Some(cwd) => {
            for item in &page.items {
                if resume_candidate_matches_cwd(
                    item.rollout_path.as_path(),
                    Some(item.cwd.as_path()),
                    cwd,
                    default_provider,
                )
                .await
                {
                    return Some(item.rollout_path.clone());
                }
            }
            None
        }
        None => page.items.first().map(|item| item.rollout_path.clone()),
    }
}

fn cwd_matches(session_cwd: &Path, cwd: &Path) -> bool {
    path_utils::paths_match_after_normalization(session_cwd, cwd)
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
