//! Persist Codex session rollouts (.jsonl) so sessions can be replayed or inspected later.

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::BaseInstructions;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
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
use super::policy::EventPersistenceMode;
use super::policy::is_persisted_response_item;
use crate::config::Config;
use crate::default_client::originator;
use crate::git_info::collect_git_info;
use crate::path_utils;
use crate::state_db;
use crate::state_db::StateDbHandle;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_state::StateRuntime;
use codex_state::ThreadMetadataBuilder;

/// Logical position in the rollout stream.
///
/// This typed wrapper keeps rollout positions distinct from raw slice offsets.
///
/// `-1` is the newest rollout row that already existed when this source was created. Older
/// persisted rows are more negative, and any rows appended after startup are `0`, `1`, `2`, and
/// so on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RolloutIndex(i64);

impl RolloutIndex {
    /// Return the next newer logical rollout position.
    pub(crate) fn next_newer(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Parsed rollout source plus file-level metadata gathered while loading by path.
pub(crate) struct LoadedRolloutSource {
    pub(crate) source: InMemoryRolloutSource,
    pub(crate) thread_id: Option<ThreadId>,
    pub(crate) parse_errors: usize,
}

/// In-memory `RolloutSource` for the current startup/replay paths that still materialize the
/// entire rollout before reconstructing history or extracting metadata.
///
/// The long-term plan is to make this source lazy over the rollout file so startup and replay can
/// support unbounded conversation history without loading the entire rollout into memory up front.
/// When that happens, the equivalent source should keep the same "load older items on demand"
/// contract, but page older rollout rows from disk instead of cloning them out of a fully loaded
/// `Vec<RolloutItem>`.
#[derive(Clone, Debug)]
pub struct InMemoryRolloutSource {
    rollout_items: Vec<RolloutItem>,
    startup_rollout_len: i64,
}

impl InMemoryRolloutSource {
    /// Build an in-memory `RolloutSource` from a fully loaded rollout item stream.
    pub(crate) fn new(rollout_items: Vec<RolloutItem>) -> Self {
        let startup_rollout_len = i64::try_from(rollout_items.len()).unwrap_or(i64::MAX);
        Self {
            rollout_items,
            startup_rollout_len,
        }
    }

    /// Load a persisted rollout file into the eager in-memory `RolloutSource` used today.
    pub(crate) async fn load_from_path(path: &Path) -> std::io::Result<LoadedRolloutSource> {
        let (items, thread_id, parse_errors) = RolloutStore::load_rollout_items(path).await?;
        Ok(LoadedRolloutSource {
            source: Self::new(items),
            thread_id,
            parse_errors,
        })
    }

    /// Append newly recorded rollout items after startup.
    pub(crate) fn append_items(&mut self, items: Vec<RolloutItem>) {
        self.rollout_items.extend(items);
    }

    /// Consume the source back into owned rollout items.
    pub(crate) fn into_items(self) -> Vec<RolloutItem> {
        self.rollout_items
    }

    /// Return the inclusive index of the start of the persisted rollout file.
    pub(crate) fn inclusive_start_of_rollout_index(&self) -> RolloutIndex {
        RolloutIndex(-self.startup_rollout_len)
    }

    /// Return the exclusive end index of the persisted rollout file plus any items
    /// appended in memory after startup.
    pub(crate) fn exclusive_end_of_rollout_index(&self) -> RolloutIndex {
        let rollout_len = i64::try_from(self.rollout_items.len()).unwrap_or(i64::MAX);
        RolloutIndex(rollout_len.saturating_sub(self.startup_rollout_len))
    }

    /// Iterate forward from the inclusive `start` position.
    pub(crate) fn iter_forward_from(
        &self,
        start: RolloutIndex,
    ) -> impl Iterator<Item = (RolloutIndex, &RolloutItem)> + '_ {
        let start = self.loaded_offset_from_rollout_index(start);
        self.rollout_items[start..]
            .iter()
            .enumerate()
            .map(move |(offset, item)| {
                let offset = i64::try_from(offset).unwrap_or(i64::MAX);
                (
                    RolloutIndex(start as i64 + offset - self.startup_rollout_len),
                    item,
                )
            })
    }

    /// Iterate backward over rows strictly older than the exclusive `end` position.
    pub(crate) fn iter_reverse_from(
        &self,
        end: RolloutIndex,
    ) -> impl Iterator<Item = (RolloutIndex, &RolloutItem)> + '_ {
        let end = self.loaded_offset_from_rollout_index(end);
        self.rollout_items[..end]
            .iter()
            .enumerate()
            .rev()
            .map(move |(actual_index, item)| {
                let actual_index = i64::try_from(actual_index).unwrap_or(i64::MAX);
                (RolloutIndex(actual_index - self.startup_rollout_len), item)
            })
    }

    fn loaded_offset_from_rollout_index(&self, rollout_index: RolloutIndex) -> usize {
        let actual_index = rollout_index.0.saturating_add(self.startup_rollout_len);
        if actual_index <= 0 {
            return 0;
        }

        match usize::try_from(actual_index) {
            Ok(actual_index) => actual_index.min(self.rollout_items.len()),
            Err(_) => self.rollout_items.len(),
        }
    }
}

/// Manages the canonical rollout state for the current process and persists it to disk.
///
/// Rollouts are recorded as JSONL and can be inspected with tools such as:
///
/// ```ignore
/// $ jq -C . ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// $ fx ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// ```
#[derive(Clone)]
pub struct RolloutStore {
    tx: Sender<RolloutCmd>,
    pub(crate) rollout_path: PathBuf,
    state_db: Option<StateDbHandle>,
    event_persistence_mode: EventPersistenceMode,
    // Canonical in-memory `RolloutSource` for this process. Startup loading and runtime writes
    // both update this source so replay sees the same rollout items and session metadata that the
    // writer is responsible for persisting.
    pub(crate) source: Arc<Mutex<InMemoryRolloutSource>>,
}

/// Parameters for creating or resuming a rollout store.
#[derive(Clone)]
pub enum RolloutStoreParams {
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
        source: Option<InMemoryRolloutSource>,
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

impl RolloutStoreParams {
    /// Create rollout parameters for a brand-new session.
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

    /// Create rollout parameters for resuming an existing persisted rollout.
    pub fn resume(path: PathBuf, event_persistence_mode: EventPersistenceMode) -> Self {
        Self::Resume {
            path,
            event_persistence_mode,
            source: None,
        }
    }

    /// Create rollout parameters for resume when startup already has a preloaded
    /// in-memory rollout source and should avoid reading the rollout file again.
    pub fn resume_with_source(
        path: PathBuf,
        event_persistence_mode: EventPersistenceMode,
        source: InMemoryRolloutSource,
    ) -> Self {
        Self::Resume {
            path,
            event_persistence_mode,
            source: Some(source),
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
            event.aggregated_output = truncate_text(
                &event.aggregated_output,
                TruncationPolicy::Bytes(PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES),
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

impl RolloutStore {
    /// Load a persisted rollout file into the eager in-memory `RolloutSource` used today.
    pub(crate) async fn load_source(path: &Path) -> std::io::Result<LoadedRolloutSource> {
        InMemoryRolloutSource::load_from_path(path).await
    }

    /// List threads (rollout files) under the provided Codex home directory.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_threads(
        config: &Config,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
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
            allowed_sources,
            model_providers,
            default_provider,
            false,
            search_term,
        )
        .await
    }

    /// List archived threads (rollout files) under the archived sessions directory.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_archived_threads(
        config: &Config,
        page_size: usize,
        cursor: Option<&Cursor>,
        sort_key: ThreadSortKey,
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
            allowed_sources,
            model_providers,
            default_provider,
            true,
            search_term,
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
        search_term: Option<&str>,
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

        let state_db_ctx = state_db::get_state_db(config).await;
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
            search_term,
        )
        .await
        {
            return Ok(db_page.into());
        }
        // If SQLite listing still fails, return the filesystem page rather than failing the list.
        tracing::error!("Falling back on rollout system");
        tracing::warn!("state db discrepancy during list_threads_with_db_fallback: falling_back");
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
                    allowed_sources,
                    model_providers,
                    false,
                    None,
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

    /// Attempt to create a new [`RolloutStore`].
    ///
    /// For both new and resumed sessions, this opens the rollout file immediately so the path is
    /// readable as soon as startup returns.
    pub async fn new(
        config: &Config,
        params: RolloutStoreParams,
        state_db_ctx: Option<StateDbHandle>,
        state_builder: Option<ThreadMetadataBuilder>,
    ) -> std::io::Result<Self> {
        let (file, rollout_path, source, meta, event_persistence_mode) = match params {
            RolloutStoreParams::Create {
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
                    cwd: config.cwd.clone(),
                    originator: originator().value,
                    cli_version: env!("CARGO_PKG_VERSION").to_string(),
                    agent_nickname: source.get_nickname(),
                    agent_role: source.get_agent_role(),
                    source,
                    model_provider: Some(config.model_provider_id.clone()),
                    base_instructions: Some(base_instructions),
                    dynamic_tools: if dynamic_tools.is_empty() {
                        None
                    } else {
                        Some(dynamic_tools)
                    },
                    memory_mode: (!config.memories.generate_memories)
                        .then_some("disabled".to_string()),
                };
                let session_meta_line = SessionMetaLine {
                    meta: session_meta,
                    git: None,
                };

                (
                    tokio::fs::File::from_std(open_log_file(path.as_path())?),
                    path,
                    InMemoryRolloutSource::new(vec![RolloutItem::SessionMeta(
                        session_meta_line.clone(),
                    )]),
                    Some(session_meta_line),
                    event_persistence_mode,
                )
            }
            RolloutStoreParams::Resume {
                path,
                event_persistence_mode,
                source,
            } => {
                let source = match source {
                    Some(source) => source,
                    None => Self::load_source(path.as_path()).await?.source,
                };
                (
                    tokio::fs::OpenOptions::new()
                        .append(true)
                        .open(&path)
                        .await?,
                    path,
                    source,
                    None,
                    event_persistence_mode,
                )
            }
        };

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller's thread.
        let (tx, rx) = mpsc::channel::<RolloutCmd>(256);

        let source = Arc::new(Mutex::new(source));
        let mut writer = JsonlWriter { file };
        let mut state_builder = state_builder;
        if let Some(session_meta_line) = meta {
            write_session_meta(
                &mut writer,
                session_meta_line,
                &rollout_path,
                &source,
                state_db_ctx.as_deref(),
                &mut state_builder,
                config.model_provider_id.as_str(),
            )
            .await?;
        }
        // Spawn a Tokio task that owns the file handle and performs async
        // writes. Using `tokio::fs::File` keeps everything on the async I/O
        // driver instead of blocking the runtime.
        tokio::task::spawn(rollout_writer(
            writer.file,
            rx,
            rollout_path.clone(),
            state_db_ctx.clone(),
            state_builder,
            config.model_provider_id.clone(),
        ));

        Ok(Self {
            tx,
            rollout_path,
            state_db: state_db_ctx,
            event_persistence_mode,
            source,
        })
    }

    /// Return the path for this rollout file.
    pub fn rollout_path(&self) -> &Path {
        self.rollout_path.as_path()
    }

    /// Return the state DB handle used for rollout reconciliation, when enabled.
    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.state_db.clone()
    }

    pub(crate) async fn record_items(&self, items: &[RolloutItem]) -> std::io::Result<()> {
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
        let mut source = self.source.lock().await;
        self.tx
            .send(RolloutCmd::AddItems(filtered.clone()))
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout items: {e}")))?;
        source.append_items(filtered);
        Ok(())
    }

    /// Ensure all queued rollout writes have been persisted to disk.
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

    // TODO(ccunningham): move this parser under the source implementation
    // (not done in this PR to reduce diff)
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

            match serde_json::from_value::<RolloutLine>(v.clone()) {
                Ok(rollout_line) => match rollout_line.item {
                    RolloutItem::SessionMeta(session_meta_line) => {
                        if thread_id.is_none() {
                            thread_id = Some(session_meta_line.meta.id);
                        }
                        items.push(RolloutItem::SessionMeta(session_meta_line));
                    }
                    RolloutItem::ResponseItem(item) => items.push(RolloutItem::ResponseItem(item)),
                    RolloutItem::Compacted(item) => items.push(RolloutItem::Compacted(item)),
                    RolloutItem::TurnContext(item) => items.push(RolloutItem::TurnContext(item)),
                    RolloutItem::EventMsg(event) => items.push(RolloutItem::EventMsg(event)),
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

    /// Load a persisted rollout file into the eager startup history shape used today.
    pub async fn get_rollout_history(path: &Path) -> std::io::Result<InitialHistory> {
        let LoadedRolloutSource {
            source,
            thread_id,
            parse_errors: _,
        } = Self::load_source(path).await?;
        let conversation_id = thread_id
            .ok_or_else(|| IoError::other("failed to parse thread ID from rollout file"))?;
        // `InitialHistory::Resumed` still carries an owned `Vec<RolloutItem>`, so this is the
        // boundary where the store's canonical `RolloutSource` snapshot is flattened back into
        // the protocol-owned rollout payload for resume/fork startup.
        // TODO(ccunningham): when resume startup becomes lazy, replace this eager
        // `InitialHistory::Resumed` materialization with a `RolloutSource`-backed history input.
        let items = source.into_items();

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

    /// Shut down the background writer after draining all previously queued work.
    pub async fn shutdown(&self) -> std::io::Result<()> {
        let (tx_done, rx_done) = oneshot::channel();
        match self.tx.send(RolloutCmd::Shutdown { ack: tx_done }).await {
            Ok(_) => rx_done
                .await
                .map_err(|e| IoError::other(format!("failed waiting for rollout shutdown: {e}")))?,
            Err(e) => {
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
    file: tokio::fs::File,
    mut rx: mpsc::Receiver<RolloutCmd>,
    rollout_path: PathBuf,
    state_db_ctx: Option<StateDbHandle>,
    mut state_builder: Option<ThreadMetadataBuilder>,
    default_provider: String,
) -> std::io::Result<()> {
    let mut writer = JsonlWriter { file };
    if let Some(builder) = state_builder.as_mut() {
        builder.rollout_path = rollout_path.clone();
    }

    // Process rollout commands
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(items) => {
                if items.is_empty() {
                    continue;
                }

                write_and_reconcile_items(
                    &mut writer,
                    items.as_slice(),
                    &rollout_path,
                    state_db_ctx.as_deref(),
                    state_builder.as_ref(),
                    default_provider.as_str(),
                )
                .await?;
            }
            RolloutCmd::Persist { ack } => {
                if let Err(e) = writer.file.flush().await {
                    let _ = ack.send(());
                    return Err(e);
                }
                let _ = ack.send(());
            }
            RolloutCmd::Flush { ack } => {
                if let Err(e) = writer.file.flush().await {
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

#[allow(clippy::too_many_arguments)]
async fn write_session_meta(
    writer: &mut JsonlWriter,
    mut session_meta_line: SessionMetaLine,
    rollout_path: &Path,
    source: &Arc<Mutex<InMemoryRolloutSource>>,
    state_db_ctx: Option<&StateRuntime>,
    state_builder: &mut Option<ThreadMetadataBuilder>,
    default_provider: &str,
) -> std::io::Result<()> {
    if session_meta_line.git.is_none() {
        session_meta_line.git = collect_git_info(&session_meta_line.meta.cwd).await;
    }
    let memory_mode = session_meta_line.meta.memory_mode.clone();
    if state_db_ctx.is_some() {
        *state_builder = metadata::builder_from_session_meta(&session_meta_line, rollout_path);
    }

    let rollout_item = RolloutItem::SessionMeta(session_meta_line.clone());
    {
        let mut source = source.lock().await;
        if let Some(RolloutItem::SessionMeta(existing_session_meta)) =
            source.rollout_items.first_mut()
        {
            *existing_session_meta = session_meta_line;
        }
    }
    writer.write_rollout_item(&rollout_item).await?;
    state_db::reconcile_rollout(
        state_db_ctx,
        rollout_path,
        default_provider,
        state_builder.as_ref(),
        std::slice::from_ref(&rollout_item),
        None,
        memory_mode.as_deref(),
    )
    .await;
    Ok(())
}

async fn write_and_reconcile_items(
    writer: &mut JsonlWriter,
    items: &[RolloutItem],
    rollout_path: &Path,
    state_db_ctx: Option<&StateRuntime>,
    state_builder: Option<&ThreadMetadataBuilder>,
    default_provider: &str,
) -> std::io::Result<()> {
    for item in items {
        writer.write_rollout_item(item).await?;
    }
    sync_thread_state_after_write(
        state_db_ctx,
        rollout_path,
        state_builder,
        items,
        default_provider,
        None,
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

    if let Ok(LoadedRolloutSource {
        source,
        thread_id: _,
        parse_errors: _,
    }) = RolloutStore::load_source(rollout_path).await
        && let Some(latest_turn_context_cwd) = source
            .iter_reverse_from(source.exclusive_end_of_rollout_index())
            .find_map(|(_, item)| match item {
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
    use crate::git_info::collect_git_info;
    use chrono::TimeZone;
    use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::TurnContextItem;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::fs::{self};
    use std::io::Write;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::Duration;
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
    async fn store_creates_rollout_immediately() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let thread_id = ThreadId::new();
        let store = RolloutStore::new(
            &config,
            RolloutStoreParams::new(
                thread_id,
                None,
                SessionSource::Exec,
                BaseInstructions::default(),
                Vec::new(),
                EventPersistenceMode::Limited,
            ),
            None,
            None,
        )
        .await?;

        let rollout_path = store.rollout_path().to_path_buf();
        assert!(
            rollout_path.exists(),
            "rollout file should exist at startup"
        );

        let source_session_meta = {
            let source = store.source.lock().await;
            source
                .iter_forward_from(source.inclusive_start_of_rollout_index())
                .find_map(|(_, item)| match item {
                    RolloutItem::SessionMeta(session_meta_line) => Some(session_meta_line.clone()),
                    RolloutItem::ResponseItem(_)
                    | RolloutItem::Compacted(_)
                    | RolloutItem::TurnContext(_)
                    | RolloutItem::EventMsg(_) => None,
                })
                .expect("session meta in source")
        };
        let loaded_rollout = RolloutStore::load_source(&rollout_path).await?;
        let persisted_session_meta = loaded_rollout
            .source
            .iter_forward_from(loaded_rollout.source.inclusive_start_of_rollout_index())
            .find_map(|(_, item)| match item {
                RolloutItem::SessionMeta(session_meta_line) => Some(session_meta_line.clone()),
                RolloutItem::ResponseItem(_)
                | RolloutItem::Compacted(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::EventMsg(_) => None,
            })
            .expect("session meta in persisted rollout");
        assert_eq!(
            serde_json::to_value(persisted_session_meta)?,
            serde_json::to_value(source_session_meta.clone())?,
        );
        assert_eq!(
            serde_json::to_value(source_session_meta.git)?,
            serde_json::to_value(collect_git_info(&config.cwd).await)?,
        );

        store
            .record_items(&[RolloutItem::EventMsg(EventMsg::AgentMessage(
                AgentMessageEvent {
                    message: "buffered-event".to_string(),
                    phase: None,
                },
            ))])
            .await?;
        store.flush().await?;

        store
            .record_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(
                UserMessageEvent {
                    message: "first-user-message".to_string(),
                    images: None,
                    local_images: Vec::new(),
                    text_elements: Vec::new(),
                },
            ))])
            .await?;
        store.flush().await?;

        store.persist().await?;
        // Second call verifies `persist()` is idempotent after startup materialization.
        store.persist().await?;
        assert!(rollout_path.exists(), "rollout file should exist");

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

        store.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn store_keeps_source_order_consistent_with_queue_order() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let store = Arc::new(
            RolloutStore::new(
                &config,
                RolloutStoreParams::new(
                    ThreadId::new(),
                    None,
                    SessionSource::Exec,
                    BaseInstructions::default(),
                    Vec::new(),
                    EventPersistenceMode::Limited,
                ),
                None,
                None,
            )
            .await?,
        );

        let user_message_a = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "A".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }));
        let user_message_b = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "B".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }));

        let source_guard = store.source.lock().await;

        let store_a = Arc::clone(&store);
        let user_message_a_for_task = user_message_a.clone();
        let task_a =
            tokio::spawn(async move { store_a.record_items(&[user_message_a_for_task]).await });
        tokio::task::yield_now().await;

        let store_b = Arc::clone(&store);
        let user_message_b_for_task = user_message_b.clone();
        let task_b =
            tokio::spawn(async move { store_b.record_items(&[user_message_b_for_task]).await });
        tokio::task::yield_now().await;

        drop(source_guard);
        task_a.await.expect("join task A")?;
        task_b.await.expect("join task B")?;

        let actual_items = store.source.lock().await.clone().into_items();
        assert!(matches!(
            actual_items.first(),
            Some(RolloutItem::SessionMeta(_))
        ));
        let actual_tail = serde_json::to_value(&actual_items[1..])?;
        let expected_tail = serde_json::to_value(vec![user_message_a, user_message_b])?;
        assert_eq!(actual_tail, expected_tail);

        store.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn metadata_irrelevant_events_touch_state_db_updated_at() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow sqlite");

        let state_db =
            StateRuntime::init(home.path().to_path_buf(), config.model_provider_id.clone())
                .await
                .expect("state db should initialize");
        state_db
            .mark_backfill_complete(None)
            .await
            .expect("backfill should be complete");

        let thread_id = ThreadId::new();
        let recorder = RolloutStore::new(
            &config,
            RolloutStoreParams::new(
                thread_id,
                None,
                SessionSource::Cli,
                BaseInstructions::default(),
                Vec::new(),
                EventPersistenceMode::Limited,
            ),
            Some(state_db.clone()),
            None,
        )
        .await?;

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
        recorder.persist().await?;
        recorder.flush().await?;
        let initial_thread = state_db
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        let initial_updated_at = initial_thread.updated_at;
        let initial_title = initial_thread.title.clone();
        let initial_first_user_message = initial_thread.first_user_message.clone();

        tokio::time::sleep(Duration::from_secs(1)).await;

        recorder
            .record_items(&[RolloutItem::EventMsg(EventMsg::AgentMessage(
                AgentMessageEvent {
                    message: "assistant text".to_string(),
                    phase: None,
                },
            ))])
            .await?;
        recorder.flush().await?;

        let updated_thread = state_db
            .get_thread(thread_id)
            .await
            .expect("thread should load after agent message")
            .expect("thread should still exist");

        assert!(updated_thread.updated_at > initial_updated_at);
        assert_eq!(updated_thread.title, initial_title);
        assert_eq!(
            updated_thread.first_user_message,
            initial_first_user_message
        );

        recorder.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn metadata_irrelevant_events_fall_back_to_upsert_when_thread_missing()
    -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow sqlite");

        let state_db =
            StateRuntime::init(home.path().to_path_buf(), config.model_provider_id.clone())
                .await
                .expect("state db should initialize");
        let thread_id = ThreadId::new();
        let rollout_path = home.path().join("rollout.jsonl");
        let builder = ThreadMetadataBuilder::new(
            thread_id,
            rollout_path.clone(),
            Utc::now(),
            SessionSource::Cli,
        );
        let items = vec![RolloutItem::EventMsg(EventMsg::AgentMessage(
            AgentMessageEvent {
                message: "assistant text".to_string(),
                phase: None,
            },
        ))];

        sync_thread_state_after_write(
            Some(state_db.as_ref()),
            rollout_path.as_path(),
            Some(&builder),
            items.as_slice(),
            config.model_provider_id.as_str(),
            None,
        )
        .await;

        let thread = state_db
            .get_thread(thread_id)
            .await
            .expect("thread should load after fallback")
            .expect("thread should be inserted after fallback");
        assert_eq!(thread.id, thread_id);

        Ok(())
    }

    #[tokio::test]
    async fn list_threads_db_disabled_does_not_skip_paginated_items() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        config
            .features
            .disable(Feature::Sqlite)
            .expect("test config should allow sqlite to be disabled");

        let newest = write_session_file(home.path(), "2025-01-03T12-00-00", Uuid::from_u128(9001))?;
        let middle = write_session_file(home.path(), "2025-01-02T12-00-00", Uuid::from_u128(9002))?;
        let _oldest =
            write_session_file(home.path(), "2025-01-01T12-00-00", Uuid::from_u128(9003))?;

        let default_provider = config.model_provider_id.clone();
        let page1 = RolloutStore::list_threads(
            &config,
            1,
            None,
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
            None,
        )
        .await?;
        assert_eq!(page1.items.len(), 1);
        assert_eq!(page1.items[0].path, newest);
        let cursor = page1.next_cursor.clone().expect("cursor should be present");

        let page2 = RolloutStore::list_threads(
            &config,
            1,
            Some(&cursor),
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
            None,
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
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow sqlite");

        let uuid = Uuid::from_u128(9010);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let stale_path = home.path().join(format!(
            "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
        ));

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.model_provider_id.clone(),
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
        let page = RolloutStore::list_threads(
            &config,
            10,
            None,
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
            None,
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
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow sqlite");

        let uuid = Uuid::from_u128(9011);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let real_path = write_session_file(home.path(), "2025-01-03T13-00-00", uuid)?;
        let stale_path = home.path().join(format!(
            "sessions/2099/01/01/rollout-2099-01-01T00-00-00-{uuid}.jsonl"
        ));

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.model_provider_id.clone(),
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
        let page = RolloutStore::list_threads(
            &config,
            1,
            None,
            ThreadSortKey::CreatedAt,
            &[],
            None,
            default_provider.as_str(),
            None,
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

    #[tokio::test]
    async fn resume_candidate_matches_cwd_reads_latest_turn_context() -> std::io::Result<()> {
        let home = TempDir::new().expect("temp dir");
        let stale_cwd = home.path().join("stale");
        let latest_cwd = home.path().join("latest");
        fs::create_dir_all(&stale_cwd)?;
        fs::create_dir_all(&latest_cwd)?;

        let path = write_session_file(home.path(), "2025-01-03T13-00-00", Uuid::from_u128(9012))?;
        let mut file = std::fs::OpenOptions::new().append(true).open(&path)?;
        let turn_context = RolloutLine {
            timestamp: "2025-01-03T13:00:01Z".to_string(),
            item: RolloutItem::TurnContext(TurnContextItem {
                turn_id: Some("turn-1".to_string()),
                trace_id: None,
                cwd: latest_cwd.clone(),
                current_date: None,
                timezone: None,
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                network: None,
                model: "test-model".to_string(),
                personality: None,
                collaboration_mode: None,
                realtime_active: None,
                effort: None,
                summary: ReasoningSummaryConfig::Auto,
                user_instructions: None,
                developer_instructions: None,
                final_output_json_schema: None,
                truncation_policy: None,
            }),
        };
        writeln!(file, "{}", serde_json::to_string(&turn_context)?)?;

        assert!(
            resume_candidate_matches_cwd(
                path.as_path(),
                Some(stale_cwd.as_path()),
                latest_cwd.as_path(),
                "test-provider",
            )
            .await
        );
        Ok(())
    }
}
