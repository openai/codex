use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;

use async_trait::async_trait;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;

use crate::AppendThreadItemsParams;
use crate::ApplyThreadMetadataParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadByRolloutPathParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMetadataUpdate;
use crate::ThreadPage;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

static IN_MEMORY_THREAD_STORES: OnceLock<Mutex<HashMap<String, Arc<InMemoryThreadStore>>>> =
    OnceLock::new();

fn stores() -> &'static Mutex<HashMap<String, Arc<InMemoryThreadStore>>> {
    IN_MEMORY_THREAD_STORES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use codex_protocol::models::BaseInstructions;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadMemoryMode;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ListItemsParams;
    use crate::ListTurnsParams;
    use crate::SortDirection;
    use crate::StoredTurnItemsView;
    use crate::ThreadEventPersistenceMode;
    use crate::ThreadPersistenceMetadata;

    #[tokio::test]
    async fn default_turn_pagination_methods_return_unsupported() {
        let store = InMemoryThreadStore::default();
        let thread_id = ThreadId::default();

        let turns_err = store
            .list_turns(ListTurnsParams {
                thread_id,
                include_archived: true,
                cursor: None,
                page_size: 10,
                sort_direction: SortDirection::Asc,
                items_view: StoredTurnItemsView::Summary,
            })
            .await
            .expect_err("default list_turns should be unsupported");
        assert!(matches!(
            turns_err,
            ThreadStoreError::Unsupported {
                operation: "list_turns"
            }
        ));

        let items_err = store
            .list_items(ListItemsParams {
                thread_id,
                turn_id: "turn_1".to_string(),
                include_archived: true,
                cursor: None,
                page_size: 10,
                sort_direction: SortDirection::Asc,
            })
            .await
            .expect_err("default list_items should be unsupported");
        assert!(matches!(
            items_err,
            ThreadStoreError::Unsupported {
                operation: "list_items"
            }
        ));
    }

    #[tokio::test]
    async fn apply_thread_metadata_updates_readable_metadata() {
        let store = InMemoryThreadStore::default();
        let thread_id = ThreadId::new();
        store
            .create_thread(create_thread_params(thread_id))
            .await
            .expect("create thread");
        let token_usage = TokenUsage {
            total_tokens: 42,
            ..Default::default()
        };
        let git_info = GitInfo {
            commit_hash: None,
            branch: Some("main".to_string()),
            repository_url: Some("https://example.test/repo.git".to_string()),
        };
        let created_at = Utc::now();
        let updated_at = Utc::now();

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    preview: Some("preview".to_string()),
                    name: Some(Some("name".to_string())),
                    model_provider: Some("provider".to_string()),
                    model: Some(Some("model".to_string())),
                    created_at: Some(created_at),
                    updated_at: Some(updated_at),
                    cwd: Some(PathBuf::from("/tmp/project")),
                    cli_version: Some("cli-version".to_string()),
                    token_usage: Some(Some(token_usage.clone())),
                    first_user_message: Some(Some("first".to_string())),
                    git_info: Some(Some(git_info.clone())),
                    ..Default::default()
                },
            })
            .await
            .expect("apply metadata");

        let stored = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
            .expect("read thread");

        assert_eq!(stored.preview, "preview");
        assert_eq!(stored.name.as_deref(), Some("name"));
        assert_eq!(stored.model_provider, "provider");
        assert_eq!(stored.model.as_deref(), Some("model"));
        assert_eq!(stored.created_at, created_at);
        assert_eq!(stored.updated_at, updated_at);
        assert_eq!(stored.cwd, PathBuf::from("/tmp/project"));
        assert_eq!(stored.cli_version, "cli-version");
        assert_eq!(stored.token_usage, Some(token_usage));
        assert_eq!(stored.first_user_message.as_deref(), Some("first"));
        let stored_git_info = stored.git_info.expect("git info");
        assert_eq!(stored_git_info.branch, git_info.branch);
        assert_eq!(stored_git_info.repository_url, git_info.repository_url);

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    name: Some(None),
                    token_usage: Some(None),
                    first_user_message: Some(None),
                    git_info: Some(None),
                    ..Default::default()
                },
            })
            .await
            .expect("clear metadata");

        let stored = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
            .expect("read thread");

        assert_eq!(stored.name, None);
        assert_eq!(stored.token_usage, None);
        assert_eq!(stored.first_user_message, None);
        assert!(stored.git_info.is_none());
    }

    fn create_thread_params(thread_id: ThreadId) -> CreateThreadParams {
        CreateThreadParams {
            thread_id,
            forked_from_id: None,
            source: SessionSource::Exec,
            originator: "test".to_string(),
            thread_source: None,
            base_instructions: BaseInstructions::default(),
            dynamic_tools: Vec::new(),
            metadata: ThreadPersistenceMetadata {
                cwd: Some(PathBuf::from("/tmp/project")),
                model_provider: "test-provider".to_string(),
                memory_mode: ThreadMemoryMode::Enabled,
            },
            event_persistence_mode: ThreadEventPersistenceMode::Limited,
        }
    }
}

fn stores_guard() -> MutexGuard<'static, HashMap<String, Arc<InMemoryThreadStore>>> {
    match stores().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Recorded call counts for [`InMemoryThreadStore`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InMemoryThreadStoreCalls {
    pub create_thread: usize,
    pub resume_thread: usize,
    pub append_items: usize,
    pub persist_thread: usize,
    pub flush_thread: usize,
    pub shutdown_thread: usize,
    pub discard_thread: usize,
    pub load_history: usize,
    pub read_thread: usize,
    pub read_thread_by_rollout_path: usize,
    pub list_threads: usize,
    pub apply_thread_metadata: usize,
    pub update_thread_metadata: usize,
    pub archive_thread: usize,
    pub unarchive_thread: usize,
}

/// In-memory [`ThreadStore`] implementation for tests and debug configs.
///
/// Test and debug configs can select this store by id, letting tests exercise
/// config-driven non-local persistence without requiring the real remote gRPC
/// service.
#[derive(Default)]
pub struct InMemoryThreadStore {
    state: tokio::sync::Mutex<InMemoryThreadStoreState>,
}

#[derive(Default)]
struct InMemoryThreadStoreState {
    calls: InMemoryThreadStoreCalls,
    created_threads: HashMap<ThreadId, CreateThreadParams>,
    histories: HashMap<ThreadId, Vec<RolloutItem>>,
    metadata_updates: HashMap<ThreadId, ThreadMetadataUpdate>,
    names: HashMap<ThreadId, Option<String>>,
    git_infos: HashMap<ThreadId, Option<GitInfo>>,
    rollout_paths: HashMap<PathBuf, ThreadId>,
}

impl InMemoryThreadStore {
    /// Returns the store associated with `id`, creating it if needed.
    pub fn for_id(id: impl Into<String>) -> Arc<Self> {
        let id = id.into();
        let mut stores = stores_guard();
        stores
            .entry(id)
            .or_insert_with(|| Arc::new(Self::default()))
            .clone()
    }

    /// Removes a shared in-memory store for `id`.
    pub fn remove_id(id: &str) -> Option<Arc<Self>> {
        stores_guard().remove(id)
    }

    /// Returns the calls observed by this store.
    pub async fn calls(&self) -> InMemoryThreadStoreCalls {
        self.state.lock().await.calls.clone()
    }
}

#[async_trait]
impl ThreadStore for InMemoryThreadStore {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.create_thread += 1;
        state.histories.entry(params.thread_id).or_default();
        state.created_threads.insert(params.thread_id, params);
        Ok(())
    }

    async fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.resume_thread += 1;
        state.histories.entry(params.thread_id).or_default();
        if let Some(rollout_path) = params.rollout_path {
            state.rollout_paths.insert(rollout_path, params.thread_id);
        }
        Ok(())
    }

    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.append_items += 1;
        state
            .histories
            .entry(params.thread_id)
            .or_default()
            .extend(params.items);
        Ok(())
    }

    async fn apply_thread_metadata(
        &self,
        params: ApplyThreadMetadataParams,
    ) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.apply_thread_metadata += 1;
        if !state.histories.contains_key(&params.thread_id) {
            return Err(ThreadStoreError::ThreadNotFound {
                thread_id: params.thread_id,
            });
        }
        let update = params.update;
        if let Some(rollout_path) = update.rollout_path.as_ref() {
            state
                .rollout_paths
                .insert(rollout_path.clone(), params.thread_id);
        }
        if let Some(name) = update.name.clone() {
            state.names.insert(params.thread_id, name);
        }
        if let Some(git_info) = update.git_info.clone() {
            state.git_infos.insert(params.thread_id, git_info);
        }
        merge_metadata_update(
            state.metadata_updates.entry(params.thread_id).or_default(),
            update,
        );
        Ok(())
    }

    async fn persist_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.persist_thread += 1;
        Ok(())
    }

    async fn flush_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.flush_thread += 1;
        Ok(())
    }

    async fn shutdown_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.shutdown_thread += 1;
        Ok(())
    }

    async fn discard_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.discard_thread += 1;
        Ok(())
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let mut state = self.state.lock().await;
        state.calls.load_history += 1;
        let items = state.histories.get(&params.thread_id).cloned().ok_or(
            ThreadStoreError::ThreadNotFound {
                thread_id: params.thread_id,
            },
        )?;
        Ok(StoredThreadHistory {
            thread_id: params.thread_id,
            items,
        })
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.read_thread += 1;
        stored_thread_from_state(&state, params.thread_id, params.include_history)
    }

    async fn read_thread_by_rollout_path(
        &self,
        params: ReadThreadByRolloutPathParams,
    ) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.read_thread_by_rollout_path += 1;
        let Some(thread_id) = state.rollout_paths.get(&params.rollout_path).copied() else {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!(
                    "in-memory thread store does not know rollout path {}",
                    params.rollout_path.display()
                ),
            });
        };
        stored_thread_from_state(&state, thread_id, params.include_history)
    }

    async fn list_threads(&self, _params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        let mut state = self.state.lock().await;
        state.calls.list_threads += 1;
        let mut items = state
            .created_threads
            .keys()
            .map(|thread_id| {
                stored_thread_from_state(&state, *thread_id, /*include_history*/ false)
            })
            .collect::<ThreadStoreResult<Vec<_>>>()?;
        items.sort_by_key(|item| item.thread_id.to_string());
        Ok(ThreadPage {
            items,
            next_cursor: None,
        })
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.update_thread_metadata += 1;
        if let Some(name) = params.patch.name {
            let update = ThreadMetadataUpdate {
                name: Some(Some(name.clone())),
                updated_at: Some(Utc::now()),
                ..Default::default()
            };
            merge_metadata_update(
                state.metadata_updates.entry(params.thread_id).or_default(),
                update,
            );
            state.names.insert(params.thread_id, Some(name));
        }
        stored_thread_from_state(&state, params.thread_id, /*include_history*/ false)
    }

    async fn archive_thread(&self, _params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.archive_thread += 1;
        Ok(())
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.unarchive_thread += 1;
        stored_thread_from_state(&state, params.thread_id, /*include_history*/ false)
    }
}

fn stored_thread_from_state(
    state: &InMemoryThreadStoreState,
    thread_id: ThreadId,
    include_history: bool,
) -> ThreadStoreResult<StoredThread> {
    let created = state
        .created_threads
        .get(&thread_id)
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?;
    let history_items = state.histories.get(&thread_id).cloned().unwrap_or_default();
    let history = include_history.then(|| StoredThreadHistory {
        thread_id,
        items: history_items.clone(),
    });
    let name = state.names.get(&thread_id).cloned().flatten();
    let git_info = state.git_infos.get(&thread_id).cloned().flatten();
    let update = state.metadata_updates.get(&thread_id);
    let name = update
        .and_then(|update| update.name.clone())
        .unwrap_or(name);
    let git_info = update
        .and_then(|update| update.git_info.clone())
        .unwrap_or(git_info);
    let rollout_path = update
        .and_then(|update| update.rollout_path.clone())
        .or_else(|| {
            state
                .rollout_paths
                .iter()
                .find_map(|(path, mapped_thread_id)| {
                    (*mapped_thread_id == thread_id).then(|| path.clone())
                })
        });

    Ok(StoredThread {
        thread_id,
        rollout_path,
        forked_from_id: created.forked_from_id,
        preview: update
            .and_then(|update| update.preview.clone())
            .unwrap_or_default(),
        name,
        model_provider: update
            .and_then(|update| update.model_provider.clone())
            .unwrap_or_else(|| "test".to_string()),
        model: update
            .and_then(|update| update.model.clone())
            .unwrap_or_default(),
        reasoning_effort: update
            .and_then(|update| update.reasoning_effort)
            .unwrap_or_default(),
        created_at: update
            .and_then(|update| update.created_at)
            .unwrap_or_else(Utc::now),
        updated_at: update
            .and_then(|update| update.updated_at)
            .unwrap_or_else(Utc::now),
        archived_at: None,
        cwd: update
            .and_then(|update| update.cwd.clone())
            .unwrap_or_default(),
        cli_version: update
            .and_then(|update| update.cli_version.clone())
            .unwrap_or_else(|| "test".to_string()),
        source: update
            .and_then(|update| update.source.clone())
            .unwrap_or_else(|| created.source.clone()),
        thread_source: update
            .and_then(|update| update.thread_source)
            .unwrap_or(created.thread_source),
        agent_nickname: update
            .and_then(|update| update.agent_nickname.clone())
            .unwrap_or_default(),
        agent_role: update
            .and_then(|update| update.agent_role.clone())
            .unwrap_or_default(),
        agent_path: update
            .and_then(|update| update.agent_path.clone())
            .unwrap_or_default(),
        git_info,
        approval_mode: update
            .and_then(|update| update.approval_mode)
            .unwrap_or(AskForApproval::Never),
        sandbox_policy: update
            .and_then(|update| update.sandbox_policy.clone())
            .unwrap_or_else(SandboxPolicy::new_read_only_policy),
        token_usage: update
            .and_then(|update| update.token_usage.clone())
            .unwrap_or_default(),
        first_user_message: update
            .and_then(|update| update.first_user_message.clone())
            .unwrap_or_default(),
        history,
    })
}

fn merge_metadata_update(target: &mut ThreadMetadataUpdate, update: ThreadMetadataUpdate) {
    let ThreadMetadataUpdate {
        rollout_path,
        preview,
        name,
        model_provider,
        model,
        reasoning_effort,
        created_at,
        updated_at,
        source,
        thread_source,
        agent_nickname,
        agent_role,
        agent_path,
        cwd,
        cli_version,
        approval_mode,
        sandbox_policy,
        token_usage,
        first_user_message,
        git_info,
        memory_mode,
        dynamic_tools,
    } = update;
    if rollout_path.is_some() {
        target.rollout_path = rollout_path;
    }
    if preview.is_some() {
        target.preview = preview;
    }
    if name.is_some() {
        target.name = name;
    }
    if model_provider.is_some() {
        target.model_provider = model_provider;
    }
    if model.is_some() {
        target.model = model;
    }
    if reasoning_effort.is_some() {
        target.reasoning_effort = reasoning_effort;
    }
    if created_at.is_some() {
        target.created_at = created_at;
    }
    if updated_at.is_some() {
        target.updated_at = updated_at;
    }
    if source.is_some() {
        target.source = source;
    }
    if thread_source.is_some() {
        target.thread_source = thread_source;
    }
    if agent_nickname.is_some() {
        target.agent_nickname = agent_nickname;
    }
    if agent_role.is_some() {
        target.agent_role = agent_role;
    }
    if agent_path.is_some() {
        target.agent_path = agent_path;
    }
    if cwd.is_some() {
        target.cwd = cwd;
    }
    if cli_version.is_some() {
        target.cli_version = cli_version;
    }
    if approval_mode.is_some() {
        target.approval_mode = approval_mode;
    }
    if sandbox_policy.is_some() {
        target.sandbox_policy = sandbox_policy;
    }
    if token_usage.is_some() {
        target.token_usage = token_usage;
    }
    if first_user_message.is_some() {
        target.first_user_message = first_user_message;
    }
    if git_info.is_some() {
        target.git_info = git_info;
    }
    if memory_mode.is_some() {
        target.memory_mode = memory_mode;
    }
    if dynamic_tools.is_some() {
        target.dynamic_tools = dynamic_tools;
    }
}
