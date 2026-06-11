use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use serde_json::Value;
use tokio::task::JoinHandle;

use crate::responses_metadata::CodexResponsesMetadata;
use crate::responses_metadata::CodexResponsesRequestKind;
use crate::responses_metadata::TurnMetadataWorkspace;
use crate::responses_metadata::filter_extra_metadata;
use crate::responses_metadata::subagent_header_value;
use crate::responses_metadata::subagent_metadata_kind;
use crate::sandbox_tags::permission_profile_sandbox_tag;
use codex_git_utils::get_git_remote_urls_assume_git_repo;
use codex_git_utils::get_git_repo_root;
use codex_git_utils::get_has_changes;
use codex_git_utils::get_head_commit_hash;
use codex_protocol::ThreadId;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use codex_utils_absolute_path::AbsolutePathBuf;

const MODEL_KEY: &str = "model";
const REASONING_EFFORT_KEY: &str = "reasoning_effort";
const USER_INPUT_REQUESTED_DURING_TURN_KEY: &str = "user_input_requested_during_turn";
const WORKSPACE_KIND_KEY: &str = "workspace_kind";

pub(crate) struct McpTurnMetadataContext<'a> {
    pub(crate) model: &'a str,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
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

impl From<WorkspaceGitMetadata> for TurnMetadataWorkspace {
    fn from(value: WorkspaceGitMetadata) -> Self {
        Self {
            associated_remote_urls: value.associated_remote_urls,
            latest_git_commit_hash: value.latest_git_commit_hash,
            has_changes: value.has_changes,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn detached_memory_responses_metadata(
    installation_id: String,
    session_id: String,
    thread_id: String,
    window_id: String,
    session_source: &SessionSource,
    cwd: &AbsolutePathBuf,
    sandbox: Option<&str>,
) -> CodexResponsesMetadata {
    CodexResponsesMetadata {
        request_kind: Some(CodexResponsesRequestKind::Memory),
        subagent_header: subagent_header_value(session_source),
        sandbox: sandbox.map(ToString::to_string),
        workspaces: memory_workspaces(cwd).await,
        ..CodexResponsesMetadata::new(installation_id, session_id, thread_id, window_id)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TurnMetadataState {
    cwd: AbsolutePathBuf,
    repo_root: Option<String>,
    session_id: String,
    thread_id: String,
    forked_from_thread_id: Option<ThreadId>,
    parent_thread_id: Option<ThreadId>,
    subagent_header: Option<String>,
    subagent_kind: Option<String>,
    turn_id: String,
    sandbox: Option<String>,
    enriched_workspaces: Arc<RwLock<Option<BTreeMap<String, TurnMetadataWorkspace>>>>,
    turn_started_at_unix_ms: Arc<RwLock<Option<i64>>>,
    responsesapi_client_metadata: Arc<RwLock<BTreeMap<String, String>>>,
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
        Self {
            cwd,
            repo_root,
            session_id,
            thread_id,
            forked_from_thread_id,
            parent_thread_id,
            subagent_header: subagent_header_value(session_source),
            subagent_kind: subagent_metadata_kind(session_source),
            turn_id,
            sandbox,
            enriched_workspaces: Arc::new(RwLock::new(None)),
            turn_started_at_unix_ms: Arc::new(RwLock::new(None)),
            responsesapi_client_metadata: Arc::new(RwLock::new(BTreeMap::new())),
            user_input_requested_during_turn: Arc::new(AtomicBool::new(false)),
            enrichment_task: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn current_meta_value_for_mcp_request(
        &self,
        context: McpTurnMetadataContext<'_>,
    ) -> Option<serde_json::Value> {
        let Value::Object(mut metadata) =
            self.responses_metadata_template().turn_metadata_value()?
        else {
            return None;
        };
        metadata.insert(
            MODEL_KEY.to_string(),
            Value::String(context.model.to_string()),
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

    pub(crate) fn to_responses_metadata(
        &self,
        installation_id: String,
        window_id: String,
        request_kind: CodexResponsesRequestKind,
    ) -> CodexResponsesMetadata {
        CodexResponsesMetadata {
            installation_id,
            window_id,
            request_kind: Some(request_kind),
            ..self.responses_metadata_template()
        }
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
            filter_extra_metadata(responsesapi_client_metadata);
    }

    pub(crate) fn workspace_kind(&self) -> Option<String> {
        self.responsesapi_client_metadata
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(WORKSPACE_KIND_KEY)
            .cloned()
    }

    fn responses_metadata_template(&self) -> CodexResponsesMetadata {
        CodexResponsesMetadata {
            turn_id: Some(self.turn_id.clone()),
            forked_from_thread_id: self.forked_from_thread_id,
            parent_thread_id: self.parent_thread_id,
            subagent_header: self.subagent_header.clone(),
            subagent_kind: self.subagent_kind.clone(),
            sandbox: self.sandbox.clone(),
            workspaces: self.current_workspaces(),
            turn_started_at_unix_ms: self.current_turn_started_at_unix_ms(),
            extra: self
                .responsesapi_client_metadata
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            ..CodexResponsesMetadata::new(
                String::new(),
                self.session_id.clone(),
                self.thread_id.clone(),
                String::new(),
            )
        }
    }

    fn current_workspaces(&self) -> BTreeMap<String, TurnMetadataWorkspace> {
        self.enriched_workspaces
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_default()
    }

    fn current_turn_started_at_unix_ms(&self) -> Option<i64> {
        *self
            .turn_started_at_unix_ms
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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

            if workspace_git_metadata.is_empty() {
                return;
            }

            let mut workspaces = BTreeMap::new();
            workspaces.insert(repo_root, workspace_git_metadata.into());
            *state
                .enriched_workspaces
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(workspaces);
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

async fn memory_workspaces(cwd: &AbsolutePathBuf) -> BTreeMap<String, TurnMetadataWorkspace> {
    let repo_root = get_git_repo_root(cwd).map(|root| root.to_string_lossy().into_owned());
    let (head_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd),
        get_has_changes(cwd),
    );
    let workspace_git_metadata = WorkspaceGitMetadata {
        associated_remote_urls,
        latest_git_commit_hash: head_commit_hash.map(|sha| sha.0),
        has_changes,
    };
    let mut workspaces = BTreeMap::new();
    if let Some(repo_root) = repo_root
        && !workspace_git_metadata.is_empty()
    {
        workspaces.insert(repo_root, workspace_git_metadata.into());
    }
    workspaces
}

#[cfg(test)]
#[path = "turn_metadata_tests.rs"]
mod tests;
