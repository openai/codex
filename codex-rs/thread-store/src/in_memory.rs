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
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadByRolloutPathParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMetadataPatch;
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
    use super::*;
    use crate::ListItemsParams;
    use crate::ListTurnsParams;
    use crate::SortDirection;
    use crate::StoredTurnItemsView;

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
    metadata_updates: HashMap<ThreadId, ThreadMetadataPatch>,
    names: HashMap<ThreadId, Option<String>>,
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
        if let Some(name) = params.patch.name.clone() {
            state.names.insert(params.thread_id, name);
        }
        merge_metadata_patch(
            state.metadata_updates.entry(params.thread_id).or_default(),
            params.patch,
        );
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
    let metadata = state.metadata_updates.get(&thread_id);
    let rollout_path = state
        .rollout_paths
        .iter()
        .find_map(|(path, mapped_thread_id)| {
            (*mapped_thread_id == thread_id).then(|| path.clone())
        });

    Ok(StoredThread {
        thread_id,
        rollout_path: metadata
            .and_then(|metadata| metadata.rollout_path.clone())
            .or(rollout_path),
        forked_from_id: created.forked_from_id,
        preview: metadata
            .and_then(|metadata| metadata.preview.clone())
            .unwrap_or_default(),
        name,
        model_provider: metadata
            .and_then(|metadata| metadata.model_provider.clone())
            .unwrap_or_else(|| "test".to_string()),
        model: metadata.and_then(|metadata| metadata.model.clone()),
        reasoning_effort: metadata.and_then(|metadata| metadata.reasoning_effort),
        created_at: metadata
            .and_then(|metadata| metadata.created_at)
            .unwrap_or_else(Utc::now),
        updated_at: metadata
            .and_then(|metadata| metadata.updated_at)
            .unwrap_or_else(Utc::now),
        archived_at: None,
        cwd: metadata
            .and_then(|metadata| metadata.cwd.clone())
            .unwrap_or_default(),
        cli_version: metadata
            .and_then(|metadata| metadata.cli_version.clone())
            .unwrap_or_else(|| "test".to_string()),
        source: metadata
            .and_then(|metadata| metadata.source.clone())
            .unwrap_or_else(|| created.source.clone()),
        thread_source: metadata
            .and_then(|metadata| metadata.thread_source)
            .unwrap_or(created.thread_source),
        agent_nickname: metadata.and_then(|metadata| metadata.agent_nickname.clone().flatten()),
        agent_role: metadata.and_then(|metadata| metadata.agent_role.clone().flatten()),
        agent_path: metadata.and_then(|metadata| metadata.agent_path.clone().flatten()),
        git_info: metadata.and_then(git_info_from_patch),
        approval_mode: metadata
            .and_then(|metadata| metadata.approval_mode)
            .unwrap_or(AskForApproval::Never),
        sandbox_policy: metadata
            .and_then(|metadata| metadata.sandbox_policy.clone())
            .unwrap_or_else(SandboxPolicy::new_read_only_policy),
        token_usage: metadata.and_then(|metadata| metadata.token_usage.clone()),
        first_user_message: metadata.and_then(|metadata| metadata.first_user_message.clone()),
        history,
    })
}

fn merge_metadata_patch(current: &mut ThreadMetadataPatch, next: ThreadMetadataPatch) {
    if next.name.is_some() {
        current.name = next.name;
    }
    if next.rollout_path.is_some() {
        current.rollout_path = next.rollout_path;
    }
    if next.preview.is_some() {
        current.preview = next.preview;
    }
    if next.title.is_some() {
        current.title = next.title;
    }
    if next.model_provider.is_some() {
        current.model_provider = next.model_provider;
    }
    if next.model.is_some() {
        current.model = next.model;
    }
    if next.reasoning_effort.is_some() {
        current.reasoning_effort = next.reasoning_effort;
    }
    if next.created_at.is_some() {
        current.created_at = next.created_at;
    }
    if next.updated_at.is_some() {
        current.updated_at = next.updated_at;
    }
    if next.source.is_some() {
        current.source = next.source;
    }
    if next.thread_source.is_some() {
        current.thread_source = next.thread_source;
    }
    if next.agent_nickname.is_some() {
        current.agent_nickname = next.agent_nickname;
    }
    if next.agent_role.is_some() {
        current.agent_role = next.agent_role;
    }
    if next.agent_path.is_some() {
        current.agent_path = next.agent_path;
    }
    if next.cwd.is_some() {
        current.cwd = next.cwd;
    }
    if next.cli_version.is_some() {
        current.cli_version = next.cli_version;
    }
    if next.approval_mode.is_some() {
        current.approval_mode = next.approval_mode;
    }
    if next.sandbox_policy.is_some() {
        current.sandbox_policy = next.sandbox_policy;
    }
    if next.token_usage.is_some() {
        current.token_usage = next.token_usage;
    }
    if next.first_user_message.is_some() {
        current.first_user_message = next.first_user_message;
    }
    if let Some(git_info) = next.git_info {
        let existing = current.git_info.take();
        current.git_info = Some(merge_git_info_patch(existing, git_info));
    }
    if next.memory_mode.is_some() {
        current.memory_mode = next.memory_mode;
    }
    if next.dynamic_tools.is_some() {
        current.dynamic_tools = next.dynamic_tools;
    }
}

fn merge_git_info_patch(
    current: Option<crate::GitInfoPatch>,
    next: crate::GitInfoPatch,
) -> crate::GitInfoPatch {
    let mut current = current.unwrap_or_default();
    if next.sha.is_some() {
        current.sha = next.sha;
    }
    if next.branch.is_some() {
        current.branch = next.branch;
    }
    if next.origin_url.is_some() {
        current.origin_url = next.origin_url;
    }
    current
}

fn git_info_from_patch(patch: &ThreadMetadataPatch) -> Option<codex_protocol::protocol::GitInfo> {
    let git_info = patch.git_info.as_ref()?;
    let sha = git_info.sha.clone().flatten();
    let branch = git_info.branch.clone().flatten();
    let origin_url = git_info.origin_url.clone().flatten();
    if sha.is_none() && branch.is_none() && origin_url.is_none() {
        return None;
    }
    Some(codex_protocol::protocol::GitInfo {
        commit_hash: sha.as_deref().map(codex_git_utils::GitSha::new),
        branch,
        repository_url: origin_url,
    })
}
