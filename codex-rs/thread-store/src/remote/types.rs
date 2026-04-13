use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use serde::Deserialize;
use serde::Serialize;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::DynamicToolsParams;
use crate::FindThreadByNameParams;
use crate::FindThreadSpawnByPathParams;
use crate::ListThreadSpawnEdgesParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResumeThreadRecorderParams;
use crate::SetThreadMemoryModeParams;
use crate::SetThreadNameParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadEventPersistenceMode;
use crate::ThreadMetadataPatch;
use crate::ThreadOwner;
use crate::ThreadSortKey;
use crate::ThreadSpawnEdge;
use crate::ThreadSpawnEdgeStatus;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateThreadRequest {
    pub thread: RemoteThreadMetadata,
    pub initial_payload_codec: String,
    pub event_persistence_mode: String,
}

impl CreateThreadRequest {
    pub(crate) fn from_params(params: CreateThreadParams) -> Self {
        Self {
            thread: RemoteThreadMetadata::from_create_params(&params),
            initial_payload_codec: super::ROLLOUT_ITEMS_CODEC.to_string(),
            event_persistence_mode: event_persistence_mode(params.event_persistence_mode),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResumeThreadRecorderRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub include_archived: bool,
    pub event_persistence_mode: String,
}

impl ResumeThreadRecorderRequest {
    pub(crate) fn from_params(params: ResumeThreadRecorderParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            include_archived: params.include_archived,
            event_persistence_mode: event_persistence_mode(params.event_persistence_mode),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendThreadRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub idempotency_key: Option<String>,
    pub updated_at: Option<i64>,
    pub new_thread_memory_mode: Option<String>,
    pub event_persistence_mode: Option<String>,
    pub index_patch: ThreadIndexPatch,
    pub payload_codec: String,
}

impl AppendThreadRequest {
    pub(crate) fn from_params(
        params: AppendThreadItemsParams,
        event_persistence_mode: Option<String>,
    ) -> Self {
        let index_patch = ThreadIndexPatch::from_items(
            params.items.as_slice(),
            params.updated_at.unwrap_or_else(Utc::now),
            params.new_thread_memory_mode.clone(),
        );
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            idempotency_key: params.idempotency_key,
            updated_at: params.updated_at.map(|updated_at| updated_at.timestamp()),
            new_thread_memory_mode: params.new_thread_memory_mode,
            event_persistence_mode,
            index_patch,
            payload_codec: super::ROLLOUT_ITEMS_CODEC.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadIndexPatch {
    pub updated_at: i64,
    pub first_user_message: Option<String>,
    pub preview: Option<String>,
    pub name: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub memory_mode: Option<String>,
}

impl ThreadIndexPatch {
    fn from_items(
        items: &[RolloutItem],
        updated_at: DateTime<Utc>,
        memory_mode: Option<String>,
    ) -> Self {
        let mut patch = Self {
            updated_at: updated_at.timestamp(),
            memory_mode,
            ..Self::default()
        };
        for item in items {
            match item {
                RolloutItem::EventMsg(codex_protocol::protocol::EventMsg::UserMessage(event)) => {
                    if patch.first_user_message.is_none() {
                        patch.first_user_message = Some(event.message.clone());
                    }
                    if patch.preview.is_none() {
                        patch.preview = Some(event.message.clone());
                    }
                }
                RolloutItem::EventMsg(codex_protocol::protocol::EventMsg::ThreadNameUpdated(
                    event,
                )) => {
                    patch.name.clone_from(&event.thread_name);
                }
                RolloutItem::EventMsg(codex_protocol::protocol::EventMsg::TokenCount(event)) => {
                    if let Some(info) = event.info.as_ref() {
                        patch.token_usage = Some(info.total_token_usage.clone());
                    }
                }
                RolloutItem::SessionMeta(meta) => {
                    if patch.memory_mode.is_none() {
                        patch.memory_mode.clone_from(&meta.meta.memory_mode);
                    }
                }
                RolloutItem::ResponseItem(_)
                | RolloutItem::Compacted(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::EventMsg(_) => {}
            }
        }
        patch
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadThreadHistoryRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub include_archived: bool,
}

impl LoadThreadHistoryRequest {
    pub(crate) fn from_params(params: LoadThreadHistoryParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            include_archived: params.include_archived,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadThreadHistoryResponse {
    pub thread_id: String,
    pub payload_codec: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredThreadPayload {
    pub thread_id: String,
    pub items: Vec<RolloutItem>,
}

impl StoredThreadPayload {
    pub(crate) fn into_stored_thread_history(self) -> ThreadStoreResult<StoredThreadHistory> {
        Ok(StoredThreadHistory {
            thread_id: parse_thread_id(self.thread_id)?,
            items: self.items,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadThreadRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub include_archived: bool,
    pub include_history: bool,
}

impl ReadThreadRequest {
    pub(crate) fn from_params(params: ReadThreadParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            include_archived: params.include_archived,
            include_history: params.include_history,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadThreadResponse {
    pub thread: RemoteThreadMetadata,
    pub history_payload_codec: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListThreadsRequest {
    pub owner: ThreadOwner,
    pub page_size: usize,
    pub cursor: Option<String>,
    pub sort_key: String,
    pub allowed_sources: Vec<SessionSource>,
    pub model_providers: Option<Vec<String>>,
    pub archived: bool,
    pub cwd: Option<PathBuf>,
    pub search_term: Option<String>,
}

impl ListThreadsRequest {
    pub(crate) fn from_params(params: ListThreadsParams) -> Self {
        Self {
            owner: params.owner,
            page_size: params.page_size,
            cursor: params.cursor,
            sort_key: sort_key(params.sort_key),
            allowed_sources: params.allowed_sources,
            model_providers: params.model_providers,
            archived: params.archived,
            cwd: params.cwd,
            search_term: params.search_term,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListThreadsResponse {
    pub items: Vec<RemoteThreadMetadata>,
    pub next_cursor: Option<String>,
    pub scanned: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindThreadByNameRequest {
    pub owner: ThreadOwner,
    pub name: String,
    pub include_archived: bool,
    pub cwd: Option<PathBuf>,
    pub allowed_sources: Vec<SessionSource>,
    pub model_providers: Option<Vec<String>>,
}

impl FindThreadByNameRequest {
    pub(crate) fn from_params(params: FindThreadByNameParams) -> Self {
        Self {
            owner: params.owner,
            name: params.name,
            include_archived: params.include_archived,
            cwd: params.cwd,
            allowed_sources: params.allowed_sources,
            model_providers: params.model_providers,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindThreadByNameResponse {
    pub thread: Option<RemoteThreadMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetThreadNameRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub name: String,
}

impl SetThreadNameRequest {
    pub(crate) fn from_params(params: SetThreadNameParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            name: params.name,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateThreadMetadataRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub patch: ThreadMetadataPatch,
}

impl UpdateThreadMetadataRequest {
    pub(crate) fn from_params(params: UpdateThreadMetadataParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            patch: params.patch,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveThreadRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
}

impl ArchiveThreadRequest {
    pub(crate) fn from_params(params: ArchiveThreadParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DynamicToolsRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
}

impl DynamicToolsRequest {
    pub(crate) fn from_params(params: DynamicToolsParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DynamicToolsResponse {
    pub dynamic_tools: Option<Vec<DynamicToolSpec>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryModeRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
}

impl MemoryModeRequest {
    pub(crate) fn from_params(params: crate::ThreadMemoryModeParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryModeResponse {
    pub memory_mode: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetMemoryModeRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub memory_mode: String,
}

impl SetMemoryModeRequest {
    pub(crate) fn from_params(params: SetThreadMemoryModeParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            memory_mode: params.memory_mode,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadSpawnEdgeRecord {
    pub parent_thread_id: String,
    pub child_thread_id: String,
    pub status: String,
}

impl ThreadSpawnEdgeRecord {
    pub(crate) fn from_thread_spawn_edge(edge: ThreadSpawnEdge) -> Self {
        Self {
            parent_thread_id: edge.parent_thread_id.to_string(),
            child_thread_id: edge.child_thread_id.to_string(),
            status: spawn_status(edge.status),
        }
    }

    pub(crate) fn into_thread_spawn_edge(self) -> ThreadStoreResult<ThreadSpawnEdge> {
        Ok(ThreadSpawnEdge {
            parent_thread_id: parse_thread_id(self.parent_thread_id)?,
            child_thread_id: parse_thread_id(self.child_thread_id)?,
            status: parse_spawn_status(self.status.as_str())?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListThreadSpawnEdgesRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub recursive: bool,
    pub status: Option<String>,
}

impl ListThreadSpawnEdgesRequest {
    pub(crate) fn from_params(params: ListThreadSpawnEdgesParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            recursive: params.recursive,
            status: params.status.map(spawn_status),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListThreadSpawnEdgesResponse {
    pub edges: Vec<ThreadSpawnEdgeRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindThreadSpawnByPathRequest {
    pub thread_id: String,
    pub owner: ThreadOwner,
    pub recursive: bool,
    pub agent_path: String,
}

impl FindThreadSpawnByPathRequest {
    pub(crate) fn from_params(params: FindThreadSpawnByPathParams) -> Self {
        Self {
            thread_id: params.thread_id.to_string(),
            owner: params.owner,
            recursive: params.recursive,
            agent_path: params.agent_path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindThreadSpawnByPathResponse {
    pub thread_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteThreadMetadata {
    pub thread_id: String,
    pub forked_from_id: Option<String>,
    pub owner: ThreadOwner,
    pub preview: String,
    pub name: Option<String>,
    pub model_provider: String,
    pub model: Option<String>,
    pub service_tier: Option<ServiceTier>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub cwd: PathBuf,
    pub cli_version: String,
    pub source: SessionSource,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub agent_path: Option<String>,
    pub git_info: Option<GitInfo>,
    pub approval_mode: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub token_usage: Option<TokenUsage>,
    pub first_user_message: Option<String>,
    pub memory_mode: Option<String>,
}

impl RemoteThreadMetadata {
    fn from_create_params(params: &CreateThreadParams) -> Self {
        let now = Utc::now().timestamp();
        Self {
            thread_id: params.thread_id.to_string(),
            forked_from_id: params.forked_from_id.map(|id| id.to_string()),
            owner: params.owner.clone(),
            preview: String::new(),
            name: None,
            model_provider: params.model_provider.clone(),
            model: params.model.clone(),
            service_tier: params.service_tier,
            reasoning_effort: params.reasoning_effort,
            created_at: now,
            updated_at: now,
            archived_at: None,
            cwd: params.cwd.clone(),
            cli_version: params.cli_version.clone(),
            source: params.source.clone(),
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            git_info: params.git_info.clone(),
            approval_mode: params.approval_mode,
            sandbox_policy: params.sandbox_policy.clone(),
            token_usage: None,
            first_user_message: None,
            memory_mode: params.memory_mode.clone(),
        }
    }

    pub(crate) fn into_stored_thread(
        self,
        history: Option<StoredThreadHistory>,
    ) -> ThreadStoreResult<StoredThread> {
        Ok(StoredThread {
            thread_id: parse_thread_id(self.thread_id)?,
            forked_from_id: self.forked_from_id.map(parse_thread_id).transpose()?,
            legacy_path: None,
            owner: self.owner,
            preview: self.preview,
            name: self.name,
            model_provider: self.model_provider,
            model: self.model,
            service_tier: self.service_tier,
            reasoning_effort: self.reasoning_effort,
            created_at: datetime_from_unix_seconds(self.created_at, "createdAt")?,
            updated_at: datetime_from_unix_seconds(self.updated_at, "updatedAt")?,
            archived_at: self
                .archived_at
                .map(|archived_at| datetime_from_unix_seconds(archived_at, "archivedAt"))
                .transpose()?,
            cwd: self.cwd,
            cli_version: self.cli_version,
            source: self.source,
            agent_nickname: self.agent_nickname,
            agent_role: self.agent_role,
            agent_path: self.agent_path,
            git_info: self.git_info,
            approval_mode: self.approval_mode,
            sandbox_policy: self.sandbox_policy,
            token_usage: self.token_usage,
            first_user_message: self.first_user_message,
            memory_mode: self.memory_mode,
            history,
        })
    }
}

pub(crate) fn event_persistence_mode(mode: ThreadEventPersistenceMode) -> String {
    match mode {
        ThreadEventPersistenceMode::Limited => "limited",
        ThreadEventPersistenceMode::Extended => "extended",
    }
    .to_string()
}

fn sort_key(sort_key: ThreadSortKey) -> String {
    match sort_key {
        ThreadSortKey::CreatedAt => "created_at",
        ThreadSortKey::UpdatedAt => "updated_at",
    }
    .to_string()
}

fn spawn_status(status: ThreadSpawnEdgeStatus) -> String {
    match status {
        ThreadSpawnEdgeStatus::Open => "open",
        ThreadSpawnEdgeStatus::Closed => "closed",
    }
    .to_string()
}

fn parse_spawn_status(status: &str) -> ThreadStoreResult<ThreadSpawnEdgeStatus> {
    match status {
        "open" => Ok(ThreadSpawnEdgeStatus::Open),
        "closed" => Ok(ThreadSpawnEdgeStatus::Closed),
        _ => Err(ThreadStoreError::InvalidRequest {
            message: format!("invalid remote thread spawn edge status `{status}`"),
        }),
    }
}

fn parse_thread_id(thread_id: String) -> ThreadStoreResult<ThreadId> {
    ThreadId::from_string(thread_id.as_str()).map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("invalid remote thread id `{thread_id}`: {err}"),
    })
}

fn datetime_from_unix_seconds(value: i64, field: &str) -> ThreadStoreResult<DateTime<Utc>> {
    DateTime::from_timestamp(value, 0).ok_or_else(|| ThreadStoreError::InvalidRequest {
        message: format!("invalid remote `{field}` timestamp `{value}`"),
    })
}
