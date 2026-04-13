use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use async_trait::async_trait;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadNameUpdatedEvent;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutConfigView;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;
use codex_rollout::SESSIONS_SUBDIR;
use codex_rollout::StateDbHandle;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::append_thread_name;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_meta_by_name_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::rollout_date_parts;
use codex_state::StateRuntime;
use codex_state::ThreadMetadataBuilder;

mod helpers;
mod read;
mod recorder;

use self::helpers::checked_rollout_file_name;
use self::helpers::display_error;
use self::helpers::edge_status_to_state;
use self::helpers::io_error;
use self::helpers::parse_cursor_param;
use self::helpers::rollout_sort_key;
use self::helpers::serialize_cursor;
use self::helpers::source_to_state_string;
use self::recorder::RolloutThreadRecorder;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::FindThreadByNameParams;
use crate::FindThreadSpawnByPathParams;
use crate::ListThreadSpawnEdgesParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::MissingThreadMetadata;
use crate::ReadThreadParams;
use crate::ResolveLegacyPathParams;
use crate::ResumeThreadRecorderParams;
use crate::SetThreadNameParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMetadataPatch;
use crate::ThreadMetadataSeed;
use crate::ThreadPage;
use crate::ThreadRecorder;
use crate::ThreadSpawnEdge;
use crate::ThreadSpawnEdgeStatus;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

/// Local filesystem implementation of [`ThreadStore`] backed by rollout JSONL files.
///
/// This adapter intentionally delegates to the existing rollout and SQLite state helpers so the
/// first concrete implementation preserves today's local storage behavior.
#[derive(Clone)]
pub struct LocalThreadStore {
    pub(crate) config: RolloutConfig,
    state_db: Option<StateDbHandle>,
}

impl LocalThreadStore {
    /// Create a local store using rollout configuration. SQLite state is opened lazily if it
    /// already exists.
    pub fn new(config: RolloutConfig) -> Self {
        Self {
            config,
            state_db: None,
        }
    }

    /// Create a local store from any rollout configuration view.
    pub fn from_config_view(config: &impl RolloutConfigView) -> Self {
        Self::new(RolloutConfig::from_view(config))
    }

    /// Create a local store and initialize the local SQLite state database.
    pub async fn with_state_db(config: RolloutConfig) -> Self {
        let state_db = codex_rollout::state_db::init(&config).await;
        Self { config, state_db }
    }

    /// Create a local store from an existing state runtime.
    pub fn with_state_runtime(config: RolloutConfig, state_db: StateDbHandle) -> Self {
        Self {
            config,
            state_db: Some(state_db),
        }
    }

    pub(crate) async fn state_db(&self) -> Option<StateDbHandle> {
        match &self.state_db {
            Some(state_db) => Some(state_db.clone()),
            None => codex_rollout::state_db::get_state_db(&self.config).await,
        }
    }

    fn request_config(
        &self,
        cwd: PathBuf,
        model_provider: String,
        memory_mode: Option<&str>,
    ) -> RolloutConfig {
        RolloutConfig {
            codex_home: self.config.codex_home.clone(),
            sqlite_home: self.config.sqlite_home.clone(),
            cwd,
            model_provider_id: model_provider,
            generate_memories: memory_mode != Some("disabled"),
        }
    }

    pub(crate) async fn find_path(
        &self,
        thread_id: ThreadId,
        include_archived: bool,
    ) -> ThreadStoreResult<(PathBuf, bool)> {
        let state_db = self.state_db().await;
        let archived_only = (!include_archived).then_some(false);
        if let Some(path) = codex_rollout::state_db::find_rollout_path_by_id(
            state_db.as_deref(),
            thread_id,
            archived_only,
            "local_thread_store_find_path",
        )
        .await
            && path.exists()
        {
            let archived = path.starts_with(self.archived_root());
            return Ok((path, archived));
        }

        match find_thread_path_by_id_str(&self.config.codex_home, &thread_id.to_string()).await {
            Ok(Some(path)) => return Ok((path, false)),
            Ok(None) => {}
            Err(err) => return Err(io_error(err)),
        }

        if include_archived {
            match find_archived_thread_path_by_id_str(
                &self.config.codex_home,
                &thread_id.to_string(),
            )
            .await
            {
                Ok(Some(path)) => return Ok((path, true)),
                Ok(None) => {}
                Err(err) => return Err(io_error(err)),
            }
        }

        Err(ThreadStoreError::ThreadNotFound { thread_id })
    }

    pub(crate) fn sessions_root(&self) -> PathBuf {
        self.config.codex_home.join(SESSIONS_SUBDIR)
    }

    pub(crate) fn archived_root(&self) -> PathBuf {
        self.config.codex_home.join(ARCHIVED_SESSIONS_SUBDIR)
    }

    async fn ensure_metadata_for_update(
        &self,
        thread_id: ThreadId,
        missing: MissingThreadMetadata,
        state_db: &StateRuntime,
    ) -> ThreadStoreResult<()> {
        if state_db
            .get_thread(thread_id)
            .await
            .map_err(display_error)?
            .is_some()
        {
            return Ok(());
        }

        let MissingThreadMetadata::Create(seed) = missing else {
            return Err(ThreadStoreError::ThreadNotFound { thread_id });
        };
        let ThreadMetadataSeed {
            rollout_path,
            created_at,
            source,
            model_provider,
            cwd,
            cli_version,
            sandbox_policy,
            approval_mode,
        } = seed;

        let mut builder = ThreadMetadataBuilder::new(thread_id, rollout_path, created_at, source);
        builder.model_provider = Some(model_provider.clone());
        builder.cwd = cwd;
        builder.cli_version = Some(cli_version);
        builder.sandbox_policy = sandbox_policy;
        builder.approval_mode = approval_mode;
        let metadata = builder.build(model_provider.as_str());
        state_db
            .insert_thread_if_absent(&metadata)
            .await
            .map_err(display_error)?;
        Ok(())
    }
}

#[async_trait]
impl ThreadStore for LocalThreadStore {
    async fn create_thread(
        &self,
        params: CreateThreadParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        let CreateThreadParams {
            thread_id,
            forked_from_id,
            source,
            cwd,
            model_provider,
            base_instructions,
            dynamic_tools,
            memory_mode,
            event_persistence_mode,
            ..
        } = params;
        let config = self.request_config(cwd, model_provider, memory_mode.as_deref());
        let recorder = RolloutRecorder::new(
            &config,
            RolloutRecorderParams::new(
                thread_id,
                forked_from_id,
                source,
                base_instructions,
                dynamic_tools,
                event_persistence_mode.into(),
            ),
            self.state_db().await,
            /*state_builder*/ None,
        )
        .await
        .map_err(io_error)?;
        Ok(Box::new(RolloutThreadRecorder {
            thread_id,
            inner: recorder,
        }))
    }

    async fn resume_thread_recorder(
        &self,
        params: ResumeThreadRecorderParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        let (path, _) = self
            .find_path(params.thread_id, params.include_archived)
            .await?;
        let recorder = RolloutRecorder::new(
            &self.config,
            RolloutRecorderParams::resume(path, params.event_persistence_mode.into()),
            self.state_db().await,
            /*state_builder*/ None,
        )
        .await
        .map_err(io_error)?;
        Ok(Box::new(RolloutThreadRecorder {
            thread_id: params.thread_id,
            inner: recorder,
        }))
    }

    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        let (path, archived) = self
            .find_path(params.thread_id, /*include_archived*/ true)
            .await?;
        for item in &params.items {
            append_rollout_item_to_path(path.as_path(), item)
                .await
                .map_err(io_error)?;
        }
        let state_db = self.state_db().await;
        codex_rollout::state_db::apply_rollout_items(
            state_db.as_deref(),
            path.as_path(),
            self.config.model_provider_id.as_str(),
            /*builder*/ None,
            params.items.as_slice(),
            "local_thread_store_append_items",
            params.new_thread_memory_mode.as_deref(),
            params.updated_at,
        )
        .await;
        codex_rollout::state_db::read_repair_rollout_path(
            state_db.as_deref(),
            Some(params.thread_id),
            Some(archived),
            path.as_path(),
        )
        .await;
        Ok(())
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let (path, _) = self
            .find_path(params.thread_id, params.include_archived)
            .await?;
        let (items, thread_id, _) = RolloutRecorder::load_rollout_items(path.as_path())
            .await
            .map_err(io_error)?;
        let thread_id = thread_id.unwrap_or(params.thread_id);
        Ok(StoredThreadHistory { thread_id, items })
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        match self
            .find_path(params.thread_id, params.include_archived)
            .await
        {
            Ok((path, archived)) => {
                self.stored_thread_from_path(path.as_path(), archived, params.include_history)
                    .await
            }
            Err(ThreadStoreError::ThreadNotFound { .. }) if !params.include_history => {
                let state_db = self.state_db().await;
                let Some(ctx) = state_db.as_deref() else {
                    return Err(ThreadStoreError::ThreadNotFound {
                        thread_id: params.thread_id,
                    });
                };
                let Some(metadata) = ctx
                    .get_thread(params.thread_id)
                    .await
                    .map_err(display_error)?
                else {
                    return Err(ThreadStoreError::ThreadNotFound {
                        thread_id: params.thread_id,
                    });
                };
                self.stored_thread_from_state_metadata(metadata, /*include_history*/ false)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        let mut cursor = parse_cursor_param(params.cursor.as_deref())?;
        let mut last_cursor = cursor.clone();
        let requested_page_size = params.page_size.max(1);
        let sort_key = rollout_sort_key(params.sort_key);
        let allowed_sources = params.allowed_sources;
        let model_providers = params
            .model_providers
            .filter(|providers| !providers.is_empty());
        let mut items = Vec::with_capacity(requested_page_size);
        let mut scanned = 0usize;
        let mut next_cursor = None;

        while items.len() < requested_page_size {
            let page_size = requested_page_size.saturating_sub(items.len());
            let page = if params.archived {
                RolloutRecorder::list_archived_threads(
                    &self.config,
                    page_size,
                    cursor.as_ref(),
                    sort_key,
                    allowed_sources.as_slice(),
                    model_providers.as_deref(),
                    self.config.model_provider_id.as_str(),
                    params.search_term.as_deref(),
                )
                .await
            } else {
                RolloutRecorder::list_threads(
                    &self.config,
                    page_size,
                    cursor.as_ref(),
                    sort_key,
                    allowed_sources.as_slice(),
                    model_providers.as_deref(),
                    self.config.model_provider_id.as_str(),
                    params.search_term.as_deref(),
                )
                .await
            }
            .map_err(io_error)?;

            scanned = scanned.saturating_add(page.num_scanned_files);
            for item in page.items {
                if params
                    .cwd
                    .as_ref()
                    .is_none_or(|expected_cwd| item.cwd.as_ref() == Some(expected_cwd))
                {
                    items.push(
                        self.stored_thread_from_thread_item(item, params.archived)
                            .await?,
                    );
                }
                if items.len() >= requested_page_size {
                    break;
                }
            }

            next_cursor = serialize_cursor(page.next_cursor.as_ref())?;
            if items.len() >= requested_page_size {
                break;
            }

            match page.next_cursor {
                Some(cursor_val) => {
                    if last_cursor.as_ref() == Some(&cursor_val) {
                        next_cursor = None;
                        break;
                    }
                    last_cursor = Some(cursor_val.clone());
                    cursor = Some(cursor_val);
                }
                None => break,
            }
        }

        Ok(ThreadPage {
            items,
            next_cursor,
            scanned: Some(scanned),
        })
    }

    async fn find_thread_by_name(
        &self,
        params: FindThreadByNameParams,
    ) -> ThreadStoreResult<Option<StoredThread>> {
        let state_db = self.state_db().await;
        if let Some(ctx) = state_db.as_deref() {
            let allowed_sources = params
                .allowed_sources
                .iter()
                .map(source_to_state_string)
                .collect::<Vec<_>>();
            for archived_only in
                [false, true]
                    .into_iter()
                    .take(if params.include_archived { 2 } else { 1 })
            {
                let metadata = ctx
                    .find_thread_by_exact_title(
                        params.name.as_str(),
                        allowed_sources.as_slice(),
                        params.model_providers.as_deref(),
                        archived_only,
                        params.cwd.as_deref(),
                    )
                    .await
                    .map_err(display_error)?;
                if let Some(metadata) = metadata {
                    return self
                        .stored_thread_from_state_metadata(metadata, archived_only)
                        .await
                        .map(Some);
                }
            }
        }

        let Some((path, meta)) =
            find_thread_meta_by_name_str(&self.config.codex_home, params.name.as_str())
                .await
                .map_err(io_error)?
        else {
            return Ok(None);
        };
        if params
            .cwd
            .as_ref()
            .is_some_and(|expected_cwd| &meta.meta.cwd != expected_cwd)
        {
            return Ok(None);
        }
        if !params.allowed_sources.is_empty() && !params.allowed_sources.contains(&meta.meta.source)
        {
            return Ok(None);
        }
        if let Some(providers) = params.model_providers.as_ref()
            && !providers.is_empty()
            && !meta
                .meta
                .model_provider
                .as_ref()
                .is_some_and(|provider| providers.contains(provider))
        {
            return Ok(None);
        }
        self.stored_thread_from_path(
            path.as_path(),
            /*archived*/ false,
            /*include_history*/ false,
        )
        .await
        .map(Some)
    }

    async fn set_thread_name(&self, params: SetThreadNameParams) -> ThreadStoreResult<()> {
        let (path, _) = self
            .find_path(params.thread_id, /*include_archived*/ false)
            .await?;
        let item = RolloutItem::EventMsg(EventMsg::ThreadNameUpdated(ThreadNameUpdatedEvent {
            thread_id: params.thread_id,
            thread_name: Some(params.name.clone()),
        }));
        append_rollout_item_to_path(path.as_path(), &item)
            .await
            .map_err(io_error)?;
        append_thread_name(
            &self.config.codex_home,
            params.thread_id,
            params.name.as_str(),
        )
        .await
        .map_err(io_error)?;
        let state_db = self.state_db().await;
        codex_rollout::state_db::reconcile_rollout(
            state_db.as_deref(),
            path.as_path(),
            self.config.model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        Ok(())
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        let missing = params.missing.clone();
        let ThreadMetadataPatch { name, git_info } = params.patch;
        if let Some(name) = name
            && let Some(name) = name
        {
            self.set_thread_name(SetThreadNameParams {
                thread_id: params.thread_id,
                owner: params.owner.clone(),
                name,
            })
            .await?;
        }

        if let Some(git_info) = git_info {
            let state_db = self.state_db().await;
            let Some(ctx) = state_db.as_deref() else {
                return Err(ThreadStoreError::Unavailable {
                    message: "sqlite state db unavailable for git metadata update".to_string(),
                });
            };
            match self
                .find_path(params.thread_id, /*include_archived*/ true)
                .await
            {
                Ok((path, archived)) => {
                    codex_rollout::state_db::reconcile_rollout(
                        Some(ctx),
                        path.as_path(),
                        self.config.model_provider_id.as_str(),
                        /*builder*/ None,
                        &[],
                        Some(archived),
                        /*new_thread_memory_mode*/ None,
                    )
                    .await;
                }
                Err(ThreadStoreError::ThreadNotFound { .. }) => {
                    self.ensure_metadata_for_update(params.thread_id, missing.clone(), ctx)
                        .await?;
                }
                Err(err) => return Err(err),
            }
            self.ensure_metadata_for_update(params.thread_id, missing, ctx)
                .await?;
            ctx.update_thread_git_info(
                params.thread_id,
                git_info.sha.as_ref().map(|value| value.as_deref()),
                git_info.branch.as_ref().map(|value| value.as_deref()),
                git_info.origin_url.as_ref().map(|value| value.as_deref()),
            )
            .await
            .map_err(display_error)?;
        }

        self.read_after_update(params.thread_id).await
    }

    async fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        let (path, _) = self
            .find_path(params.thread_id, /*include_archived*/ false)
            .await?;
        let canonical_sessions_dir = tokio::fs::canonicalize(self.sessions_root())
            .await
            .map_err(io_error)?;
        let canonical_path = tokio::fs::canonicalize(path.as_path())
            .await
            .map_err(io_error)?;
        if !canonical_path.starts_with(&canonical_sessions_dir) {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!(
                    "rollout path `{}` must be in sessions directory",
                    path.display()
                ),
            });
        }
        let file_name = checked_rollout_file_name(&canonical_path, params.thread_id)?;
        let archive_folder = self.archived_root();
        tokio::fs::create_dir_all(&archive_folder)
            .await
            .map_err(io_error)?;
        let archived_path = archive_folder.join(&file_name);
        tokio::fs::rename(&canonical_path, &archived_path)
            .await
            .map_err(io_error)?;
        if let Some(ctx) = self.state_db().await {
            ctx.mark_archived(params.thread_id, archived_path.as_path(), Utc::now())
                .await
                .map_err(display_error)?;
        }
        Ok(())
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        let (path, _) = self
            .find_path(params.thread_id, /*include_archived*/ true)
            .await?;
        let canonical_archived_dir = tokio::fs::canonicalize(self.archived_root())
            .await
            .map_err(io_error)?;
        let canonical_path = tokio::fs::canonicalize(path.as_path())
            .await
            .map_err(io_error)?;
        if !canonical_path.starts_with(&canonical_archived_dir) {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!(
                    "rollout path `{}` must be in archived directory",
                    path.display()
                ),
            });
        }
        let file_name = checked_rollout_file_name(&canonical_path, params.thread_id)?;
        let Some((year, month, day)) = rollout_date_parts(&file_name) else {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!(
                    "rollout path `{}` missing filename timestamp",
                    path.display()
                ),
            });
        };
        let dest_dir = self.sessions_root().join(year).join(month).join(day);
        let restored_path = dest_dir.join(&file_name);
        tokio::fs::create_dir_all(&dest_dir)
            .await
            .map_err(io_error)?;
        tokio::fs::rename(&canonical_path, &restored_path)
            .await
            .map_err(io_error)?;
        tokio::task::spawn_blocking({
            let restored_path = restored_path.clone();
            move || -> std::io::Result<()> {
                let times = std::fs::FileTimes::new().set_modified(SystemTime::now());
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&restored_path)?
                    .set_times(times)?;
                Ok(())
            }
        })
        .await
        .map_err(display_error)?
        .map_err(io_error)?;
        if let Some(ctx) = self.state_db().await {
            ctx.mark_unarchived(params.thread_id, restored_path.as_path())
                .await
                .map_err(display_error)?;
        }
        self.stored_thread_from_path(
            restored_path.as_path(),
            /*archived*/ false,
            /*include_history*/ false,
        )
        .await
    }

    async fn resolve_legacy_path(
        &self,
        params: ResolveLegacyPathParams,
    ) -> ThreadStoreResult<Option<ThreadId>> {
        if !self.supports_legacy_path(params.path.as_path()) {
            return Ok(None);
        }
        let (_, thread_id, _) = RolloutRecorder::load_rollout_items(params.path.as_path())
            .await
            .map_err(io_error)?;
        Ok(thread_id)
    }

    async fn upsert_thread_spawn_edge(&self, edge: ThreadSpawnEdge) -> ThreadStoreResult<()> {
        let state_db = self.state_db().await;
        let Some(ctx) = state_db.as_deref() else {
            return Err(ThreadStoreError::Unavailable {
                message: "sqlite state db unavailable for thread-spawn edge update".to_string(),
            });
        };
        ctx.upsert_thread_spawn_edge(
            edge.parent_thread_id,
            edge.child_thread_id,
            edge_status_to_state(edge.status),
        )
        .await
        .map_err(display_error)?;
        Ok(())
    }

    async fn list_thread_spawn_edges(
        &self,
        params: ListThreadSpawnEdgesParams,
    ) -> ThreadStoreResult<Vec<ThreadSpawnEdge>> {
        let state_db = self.state_db().await;
        let Some(ctx) = state_db.as_deref() else {
            return Err(ThreadStoreError::Unavailable {
                message: "sqlite state db unavailable for thread-spawn edge lookup".to_string(),
            });
        };
        let statuses = match params.status {
            Some(status) => vec![status],
            None => vec![ThreadSpawnEdgeStatus::Open, ThreadSpawnEdgeStatus::Closed],
        };
        let mut edges = Vec::new();
        let mut parents = vec![params.thread_id];
        while let Some(parent_thread_id) = parents.pop() {
            for status in &statuses {
                let children = ctx
                    .list_thread_spawn_children_with_status(
                        parent_thread_id,
                        edge_status_to_state(*status),
                    )
                    .await
                    .map_err(display_error)?;
                for child_thread_id in children {
                    if params.recursive {
                        parents.push(child_thread_id);
                    }
                    edges.push(ThreadSpawnEdge {
                        parent_thread_id,
                        child_thread_id,
                        status: *status,
                    });
                }
            }
            if !params.recursive {
                break;
            }
        }
        Ok(edges)
    }

    async fn find_thread_spawn_by_path(
        &self,
        params: FindThreadSpawnByPathParams,
    ) -> ThreadStoreResult<Option<ThreadId>> {
        let state_db = self.state_db().await;
        let Some(ctx) = state_db.as_deref() else {
            return Err(ThreadStoreError::Unavailable {
                message: "sqlite state db unavailable for thread-spawn path lookup".to_string(),
            });
        };
        let result = if params.recursive {
            ctx.find_thread_spawn_descendant_by_path(params.thread_id, params.agent_path.as_str())
                .await
        } else {
            ctx.find_thread_spawn_child_by_path(params.thread_id, params.agent_path.as_str())
                .await
        }
        .map_err(display_error)?;
        Ok(result)
    }

    fn supports_legacy_path(&self, path: &Path) -> bool {
        path.starts_with(self.sessions_root()) || path.starts_with(self.archived_root())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::TimeZone;
    use chrono::Utc;
    use codex_git_utils::GitSha;
    use codex_protocol::ThreadId;
    use codex_protocol::models::BaseInstructions;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::UserMessageEvent;
    use codex_rollout::RolloutConfig;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use crate::CreateThreadParams;
    use crate::GitInfoPatch;
    use crate::MissingThreadMetadata;
    use crate::ReadThreadParams;
    use crate::ThreadEventPersistenceMode;
    use crate::ThreadMetadataPatch;
    use crate::ThreadMetadataSeed;
    use crate::ThreadOwner;
    use crate::ThreadStore;
    use crate::ThreadStoreError;
    use crate::UpdateThreadMetadataParams;

    use super::LocalThreadStore;

    fn test_config(codex_home: &Path) -> RolloutConfig {
        RolloutConfig {
            codex_home: codex_home.to_path_buf(),
            sqlite_home: codex_home.to_path_buf(),
            cwd: codex_home.to_path_buf(),
            model_provider_id: "test-provider".to_string(),
            generate_memories: true,
        }
    }

    fn metadata_seed(thread_id: ThreadId, codex_home: &Path, cwd: &Path) -> ThreadMetadataSeed {
        ThreadMetadataSeed {
            rollout_path: codex_home.join(format!("sessions/seeded-{thread_id}.jsonl")),
            created_at: Utc
                .with_ymd_and_hms(2025, 1, 3, 4, 5, 6)
                .single()
                .expect("valid timestamp"),
            source: SessionSource::Exec,
            model_provider: "seed-provider".to_string(),
            cwd: cwd.to_path_buf(),
            cli_version: "seed-cli-version".to_string(),
            sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
            approval_mode: AskForApproval::Never,
        }
    }

    fn git_patch(
        sha: impl Into<String>,
        branch: impl Into<String>,
        origin_url: impl Into<String>,
    ) -> ThreadMetadataPatch {
        ThreadMetadataPatch {
            name: None,
            git_info: Some(GitInfoPatch {
                sha: Some(Some(sha.into())),
                branch: Some(Some(branch.into())),
                origin_url: Some(Some(origin_url.into())),
            }),
        }
    }

    async fn create_materialized_thread(store: &LocalThreadStore, cwd: &Path) -> ThreadId {
        let thread_id = ThreadId::new();
        let recorder = store
            .create_thread(CreateThreadParams {
                thread_id,
                owner: ThreadOwner::default(),
                forked_from_id: None,
                source: SessionSource::Exec,
                cwd: cwd.to_path_buf(),
                originator: "test-originator".to_string(),
                cli_version: "test-cli-version".to_string(),
                model_provider: "test-provider".to_string(),
                model: None,
                service_tier: None,
                reasoning_effort: None,
                approval_mode: AskForApproval::OnRequest,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                base_instructions: BaseInstructions::default(),
                dynamic_tools: Vec::new(),
                memory_mode: None,
                git_info: None,
                event_persistence_mode: ThreadEventPersistenceMode::Limited,
            })
            .await
            .expect("create thread recorder");
        recorder
            .record_items(&[
                RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                    message: "hello from the local store".to_string(),
                    images: None,
                    local_images: Vec::new(),
                    text_elements: Vec::new(),
                })),
                RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
                    message: "hello back".to_string(),
                    phase: None,
                    memory_citation: None,
                })),
            ])
            .await
            .expect("record rollout items");
        recorder.flush().await.expect("flush rollout");
        thread_id
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_thread_metadata_create_backfills_missing_sqlite_row() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::with_state_db(test_config(home.path())).await;
        let thread_id = ThreadId::new();
        let seed = metadata_seed(thread_id, home.path(), home.path());

        let stored = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                owner: ThreadOwner::default(),
                patch: git_patch("abc123", "main", "https://example.com/repo.git"),
                missing: MissingThreadMetadata::Create(seed.clone()),
            })
            .await
            .expect("metadata update succeeds");

        assert_eq!(stored.thread_id, thread_id);
        assert_eq!(stored.legacy_path, Some(seed.rollout_path));
        assert_eq!(stored.model_provider, "seed-provider");
        assert_eq!(stored.cwd, home.path());
        assert_eq!(stored.cli_version, "seed-cli-version");
        assert_eq!(stored.source, SessionSource::Exec);
        assert_eq!(
            stored.sandbox_policy,
            SandboxPolicy::new_workspace_write_policy()
        );
        assert_eq!(stored.approval_mode, AskForApproval::Never);
        let git_info = stored.git_info.expect("git info");
        assert_eq!(git_info.commit_hash, Some(GitSha::new("abc123")));
        assert_eq!(git_info.branch, Some("main".to_string()));
        assert_eq!(
            git_info.repository_url,
            Some("https://example.com/repo.git".to_string())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_thread_metadata_error_does_not_backfill_missing_sqlite_row() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::with_state_db(test_config(home.path())).await;
        let thread_id = ThreadId::new();

        let err = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                owner: ThreadOwner::default(),
                patch: git_patch("abc123", "main", "https://example.com/repo.git"),
                missing: MissingThreadMetadata::Error,
            })
            .await
            .expect_err("metadata update should fail");

        assert!(
            matches!(err, ThreadStoreError::ThreadNotFound { thread_id: id } if id == thread_id)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn read_thread_overlays_sqlite_metadata_on_rollout_summary() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::with_state_db(test_config(home.path())).await;
        let thread_id = create_materialized_thread(&store, home.path()).await;

        let stored = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                owner: ThreadOwner::default(),
                patch: git_patch("def456", "feature/thread-store", "ssh://example/repo"),
                missing: MissingThreadMetadata::Error,
            })
            .await
            .expect("metadata update succeeds");

        let git_info = stored.git_info.expect("git info");
        assert_eq!(git_info.commit_hash, Some(GitSha::new("def456")));
        assert_eq!(git_info.branch, Some("feature/thread-store".to_string()));
        assert_eq!(
            git_info.repository_url,
            Some("ssh://example/repo".to_string())
        );

        let reread = store
            .read_thread(ReadThreadParams {
                thread_id,
                owner: ThreadOwner::default(),
                include_archived: false,
                include_history: true,
            })
            .await
            .expect("read thread");

        let git_info = reread.git_info.expect("git info");
        assert_eq!(git_info.commit_hash, Some(GitSha::new("def456")));
        assert_eq!(git_info.branch, Some("feature/thread-store".to_string()));
        assert_eq!(
            git_info.repository_url,
            Some("ssh://example/repo".to_string())
        );
        assert_eq!(reread.preview, "hello from the local store");
        assert_eq!(
            reread.history.expect("history").items.len(),
            3,
            "session metadata plus two recorded events should be replayable"
        );
    }
}
