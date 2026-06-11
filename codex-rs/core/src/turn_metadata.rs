use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionStrategy;
use codex_analytics::CompactionTrigger;
use codex_utils_string::to_ascii_json_string;
use serde::Serialize;
use serde_json::Value;
use tokio::task::JoinHandle;

use crate::sandbox_tags::permission_profile_sandbox_tag;
use codex_git_utils::get_git_remote_urls_assume_git_repo;
use codex_git_utils::get_git_repo_root;
use codex_git_utils::get_has_changes;
use codex_git_utils::get_head_commit_hash;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;
use codex_utils_absolute_path::AbsolutePathBuf;

const MODEL_KEY: &str = "model";
const AGENT_PATH_KEY: &str = "agent_path";
const HAS_SPAWNED_SUBAGENT_KEY: &str = "has_spawned_subagent";
const REASONING_EFFORT_KEY: &str = "reasoning_effort";
const TURN_STARTED_AT_UNIX_MS_KEY: &str = "turn_started_at_unix_ms";
const USER_INPUT_REQUESTED_DURING_TURN_KEY: &str = "user_input_requested_during_turn";
const WORKSPACE_KIND_KEY: &str = "workspace_kind";
const REQUEST_KIND_KEY: &str = "request_kind";
const COMPACTION_KEY: &str = "compaction";
const WINDOW_ID_KEY: &str = "window_id";

pub(crate) struct McpTurnMetadataContext<'a> {
    pub(crate) model: &'a str,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) has_spawned_subagent: bool,
}

/// Metadata present only on outbound model requests that perform compaction.
///
/// These fields describe the operation at dispatch time. Post-response outcomes such as status,
/// error, duration, and token deltas remain in compaction analytics events.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct CompactionTurnMetadata {
    trigger: CompactionTrigger,
    reason: CompactionReason,
    implementation: CompactionImplementation,
    phase: CompactionPhase,
    strategy: CompactionStrategy,
}

impl CompactionTurnMetadata {
    pub(crate) fn new(
        trigger: CompactionTrigger,
        reason: CompactionReason,
        implementation: CompactionImplementation,
        phase: CompactionPhase,
    ) -> Self {
        Self {
            trigger,
            reason,
            implementation,
            phase,
            strategy: CompactionStrategy::Memento,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum TurnMetadataRequestKind {
    Turn,
    Prewarm,
    Compaction,
    Memory,
}

#[derive(Clone, Debug, Default)]
struct WorkspaceGitMetadata {
    associated_remote_urls: Option<BTreeMap<String, String>>,
    latest_git_commit_hash: Option<String>,
    has_changes: Option<bool>,
}

impl WorkspaceGitMetadata {
    fn is_empty(&self) -> bool {
        self.associated_remote_urls.is_none()
            && self.latest_git_commit_hash.is_none()
            && self.has_changes.is_none()
    }
}

#[derive(Clone, Debug, Serialize, Default)]
struct TurnMetadataWorkspace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    associated_remote_urls: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_git_commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    has_changes: Option<bool>,
}

impl From<WorkspaceGitMetadata> for TurnMetadataWorkspace {
    fn from(value: WorkspaceGitMetadata) -> Self {
        Self {
            associated_remote_urls: value.associated_remote_urls,
            latest_git_commit_hash: value.latest_git_commit_hash,
            has_changes: value.has_changes,
        }
    }
}

/// Base payload for the outbound model request `x-codex-turn-metadata` header.
///
/// Turn-owned state populates identity fields, including optional fork and subagent lineage. A
/// concrete request kind is added at outbound model dispatch so turns, startup prewarm, and
/// compaction remain distinguishable. Detached memory requests are constructed as `memory`
/// directly.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TurnMetadataBag {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_kind: Option<TurnMetadataRequestKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    forked_from_thread_id: Option<ThreadId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_thread_id: Option<ThreadId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subagent_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_source: Option<ThreadSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    workspaces: BTreeMap<String, TurnMetadataWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sandbox: Option<String>,
}

impl TurnMetadataBag {
    fn with_workspace_git_metadata(
        mut self,
        repo_root: Option<String>,
        workspace_git_metadata: Option<WorkspaceGitMetadata>,
    ) -> Self {
        if let (Some(repo_root), Some(workspace_git_metadata)) = (repo_root, workspace_git_metadata)
            && !workspace_git_metadata.is_empty()
        {
            self.workspaces
                .insert(repo_root, workspace_git_metadata.into());
        }
        self
    }

    fn to_header_value(&self) -> Option<String> {
        to_ascii_json_string(self).ok()
    }
}

fn merge_turn_metadata(
    header: &str,
    turn_started_at_unix_ms: Option<i64>,
    responsesapi_client_metadata: Option<&HashMap<String, String>>,
) -> Option<String> {
    if turn_started_at_unix_ms.is_none() && responsesapi_client_metadata.is_none() {
        return None;
    }

    let mut metadata = serde_json::from_str::<serde_json::Map<String, Value>>(header).ok()?;
    if let Some(turn_started_at_unix_ms) = turn_started_at_unix_ms {
        metadata.insert(
            TURN_STARTED_AT_UNIX_MS_KEY.to_string(),
            Value::Number(turn_started_at_unix_ms.into()),
        );
    }
    if let Some(responsesapi_client_metadata) = responsesapi_client_metadata {
        for (key, value) in responsesapi_client_metadata {
            if matches!(
                key.as_str(),
                "session_id"
                    | "thread_id"
                    | "turn_id"
                    | TURN_STARTED_AT_UNIX_MS_KEY
                    | "forked_from_thread_id"
                    | "parent_thread_id"
                    | "subagent_kind"
                    | AGENT_PATH_KEY
                    | HAS_SPAWNED_SUBAGENT_KEY
                    | REQUEST_KIND_KEY
                    | COMPACTION_KEY
                    | WINDOW_ID_KEY
            ) {
                continue;
            }
            metadata
                .entry(key.clone())
                .or_insert_with(|| Value::String(value.clone()));
        }
    }
    to_ascii_json_string(&metadata).ok()
}

pub async fn build_turn_metadata_header(
    cwd: &AbsolutePathBuf,
    sandbox: Option<&str>,
) -> Option<String> {
    let repo_root = get_git_repo_root(cwd).map(|root| root.to_string_lossy().into_owned());

    let (head_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd),
        get_has_changes(cwd),
    );
    let latest_git_commit_hash = head_commit_hash.map(|sha| sha.0);
    TurnMetadataBag {
        request_kind: Some(TurnMetadataRequestKind::Memory),
        session_id: None,
        thread_id: None,
        forked_from_thread_id: None,
        parent_thread_id: None,
        subagent_kind: None,
        thread_source: None,
        turn_id: None,
        workspaces: BTreeMap::new(),
        sandbox: sandbox.map(ToString::to_string),
    }
    .with_workspace_git_metadata(
        repo_root,
        Some(WorkspaceGitMetadata {
            associated_remote_urls,
            latest_git_commit_hash,
            has_changes,
        }),
    )
    .to_header_value()
}

#[derive(Clone, Debug)]
pub(crate) struct TurnMetadataState {
    cwd: AbsolutePathBuf,
    repo_root: Option<String>,
    base_metadata: TurnMetadataBag,
    base_header: Option<String>,
    agent_path: Option<String>,
    enriched_header: Arc<RwLock<Option<String>>>,
    turn_started_at_unix_ms: Arc<RwLock<Option<i64>>>,
    responsesapi_client_metadata: Arc<RwLock<Option<HashMap<String, String>>>>,
    user_input_requested_during_turn: Arc<AtomicBool>,
    enrichment_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TurnMetadataState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        session_id: String,
        thread_id: String,
        forked_from_thread_id: Option<ThreadId>,
        parent_thread_id: Option<ThreadId>,
        session_source: &SessionSource,
        thread_source: Option<ThreadSource>,
        turn_id: String,
        cwd: AbsolutePathBuf,
        permission_profile: &PermissionProfile,
        windows_sandbox_level: WindowsSandboxLevel,
        enforce_managed_network: bool,
    ) -> Self {
        let repo_root = get_git_repo_root(&cwd).map(|root| root.to_string_lossy().into_owned());
        let sandbox = Some(
            permission_profile_sandbox_tag(
                permission_profile,
                windows_sandbox_level,
                enforce_managed_network,
            )
            .to_string(),
        );
        let subagent_kind = match session_source {
            SessionSource::SubAgent(subagent_source) => Some(subagent_source.kind().to_string()),
            SessionSource::Cli
            | SessionSource::VSCode
            | SessionSource::Exec
            | SessionSource::Mcp
            | SessionSource::Custom(_)
            | SessionSource::Internal(_)
            | SessionSource::Unknown => None,
        };
        let agent_path = session_source
            .get_agent_path()
            .map(String::from)
            .or_else(|| {
                (!matches!(session_source, SessionSource::SubAgent(_)))
                    .then(|| AgentPath::ROOT.to_string())
            });
        let base_metadata = TurnMetadataBag {
            request_kind: None,
            session_id: Some(session_id),
            thread_id: Some(thread_id),
            forked_from_thread_id,
            parent_thread_id,
            subagent_kind,
            thread_source,
            turn_id: Some(turn_id),
            workspaces: BTreeMap::new(),
            sandbox,
        };
        let base_header = base_metadata.to_header_value();

        Self {
            cwd,
            repo_root,
            base_metadata,
            base_header,
            agent_path,
            enriched_header: Arc::new(RwLock::new(None)),
            turn_started_at_unix_ms: Arc::new(RwLock::new(None)),
            responsesapi_client_metadata: Arc::new(RwLock::new(None)),
            user_input_requested_during_turn: Arc::new(AtomicBool::new(false)),
            enrichment_task: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn current_header_value(&self) -> Option<String> {
        let header = if let Some(header) = self
            .enriched_header
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .cloned()
        {
            header
        } else {
            self.base_header.clone()?
        };
        let turn_started_at_unix_ms = *self
            .turn_started_at_unix_ms
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let responsesapi_client_metadata = self
            .responsesapi_client_metadata
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        merge_turn_metadata(
            &header,
            turn_started_at_unix_ms,
            responsesapi_client_metadata.as_ref(),
        )
        .or(Some(header))
    }

    pub(crate) fn current_meta_value_for_mcp_request(
        &self,
        context: McpTurnMetadataContext<'_>,
    ) -> Option<serde_json::Value> {
        let header = self.current_header_value()?;
        let mut metadata = serde_json::from_str::<serde_json::Map<String, Value>>(&header).ok()?;
        metadata.remove(REQUEST_KIND_KEY);
        if let Some(agent_path) = self.agent_path.as_ref() {
            metadata.insert(
                AGENT_PATH_KEY.to_string(),
                Value::String(agent_path.clone()),
            );
        } else {
            metadata.remove(AGENT_PATH_KEY);
        }
        metadata.insert(
            MODEL_KEY.to_string(),
            Value::String(context.model.to_string()),
        );
        metadata.insert(
            HAS_SPAWNED_SUBAGENT_KEY.to_string(),
            Value::Bool(context.has_spawned_subagent),
        );
        match context.reasoning_effort {
            Some(reasoning_effort) => {
                metadata.insert(
                    REASONING_EFFORT_KEY.to_string(),
                    Value::String(reasoning_effort.to_string()),
                );
            }
            None => {
                metadata.remove(REASONING_EFFORT_KEY);
            }
        }
        if self
            .user_input_requested_during_turn
            .load(Ordering::Relaxed)
        {
            metadata.insert(
                USER_INPUT_REQUESTED_DURING_TURN_KEY.to_string(),
                Value::Bool(true),
            );
        } else {
            metadata.remove(USER_INPUT_REQUESTED_DURING_TURN_KEY);
        }
        Some(Value::Object(metadata))
    }

    fn current_header_value_for_model_request_kind(
        &self,
        window_id: &str,
        request_kind: TurnMetadataRequestKind,
    ) -> Option<String> {
        let header = self.current_header_value()?;
        let mut metadata = serde_json::from_str::<serde_json::Map<String, Value>>(&header).ok()?;
        metadata.insert(
            REQUEST_KIND_KEY.to_string(),
            serde_json::to_value(request_kind).ok()?,
        );
        metadata.insert(
            WINDOW_ID_KEY.to_string(),
            Value::String(window_id.to_string()),
        );
        to_ascii_json_string(&metadata).ok()
    }

    pub(crate) fn current_header_value_for_model_request(&self, window_id: &str) -> Option<String> {
        self.current_header_value_for_model_request_kind(window_id, TurnMetadataRequestKind::Turn)
    }

    pub(crate) fn current_header_value_for_prewarm(&self, window_id: &str) -> Option<String> {
        self.current_header_value_for_model_request_kind(
            window_id,
            TurnMetadataRequestKind::Prewarm,
        )
    }

    pub(crate) fn current_header_value_for_compaction(
        &self,
        window_id: &str,
        compaction: CompactionTurnMetadata,
    ) -> Option<String> {
        let header = self.current_header_value_for_model_request_kind(
            window_id,
            TurnMetadataRequestKind::Compaction,
        )?;
        let mut metadata = serde_json::from_str::<serde_json::Map<String, Value>>(&header).ok()?;
        metadata.insert(
            COMPACTION_KEY.to_string(),
            serde_json::to_value(compaction).ok()?,
        );
        to_ascii_json_string(&metadata).ok()
    }

    pub(crate) fn mark_user_input_requested_during_turn(&self) {
        self.user_input_requested_during_turn
            .store(true, Ordering::Relaxed);
    }

    pub(crate) fn set_responsesapi_client_metadata(
        &self,
        responsesapi_client_metadata: HashMap<String, String>,
    ) {
        *self
            .responsesapi_client_metadata
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(responsesapi_client_metadata);
    }

    pub(crate) fn workspace_kind(&self) -> Option<String> {
        self.responsesapi_client_metadata
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(|metadata| metadata.get(WORKSPACE_KIND_KEY).cloned())
    }

    pub(crate) fn set_turn_started_at_unix_ms(&self, turn_started_at_unix_ms: i64) {
        *self
            .turn_started_at_unix_ms
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(turn_started_at_unix_ms);
    }

    pub(crate) fn spawn_git_enrichment_task(&self) {
        if self.repo_root.is_none() {
            return;
        }

        let mut task_guard = self
            .enrichment_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if task_guard.is_some() {
            return;
        }

        let state = self.clone();
        *task_guard = Some(tokio::spawn(async move {
            let workspace_git_metadata = state.fetch_workspace_git_metadata().await;
            let Some(repo_root) = state.repo_root.clone() else {
                return;
            };

            let enriched_metadata = state
                .base_metadata
                .clone()
                .with_workspace_git_metadata(Some(repo_root), Some(workspace_git_metadata));
            if enriched_metadata.workspaces.is_empty() {
                return;
            }

            if let Some(header_value) = enriched_metadata.to_header_value() {
                *state
                    .enriched_header
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(header_value);
            }
        }));
    }

    pub(crate) fn cancel_git_enrichment_task(&self) {
        let mut task_guard = self
            .enrichment_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(task) = task_guard.take() {
            task.abort();
        }
    }

    async fn fetch_workspace_git_metadata(&self) -> WorkspaceGitMetadata {
        let (head_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
            get_head_commit_hash(&self.cwd),
            get_git_remote_urls_assume_git_repo(&self.cwd),
            get_has_changes(&self.cwd),
        );
        let latest_git_commit_hash = head_commit_hash.map(|sha| sha.0);

        WorkspaceGitMetadata {
            associated_remote_urls,
            latest_git_commit_hash,
            has_changes,
        }
    }
}

#[cfg(test)]
#[path = "turn_metadata_tests.rs"]
mod tests;
