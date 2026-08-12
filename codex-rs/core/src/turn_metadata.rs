use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use serde_json::Value;
use tokio::task::JoinHandle;

use crate::responses_metadata::CodexResponsesMetadata;
use crate::responses_metadata::CodexResponsesRequestKind;
use crate::responses_metadata::PARENT_TURN_ID_KEY;
use crate::responses_metadata::ROOT_TURN_ID_KEY;
use crate::responses_metadata::TurnMetadataWorkspace;
use crate::responses_metadata::TurnToolNamespacesInfo;
use crate::responses_metadata::filter_extra_metadata;
use crate::responses_metadata::subagent_header_value;
use crate::responses_metadata::subagent_metadata_kind;
use crate::sandbox_tags::permission_profile_policy_tag;
use crate::sandbox_tags::permission_profile_sandbox_tag;
use codex_git_utils::get_git_remote_urls_assume_git_repo;
use codex_git_utils::get_git_repo_root;
use codex_git_utils::get_has_changes_in_repo;
use codex_git_utils::get_head_commit_hash;
use codex_protocol::ThreadId;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;
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
    permission_profile: &PermissionProfile,
    sandbox: Option<&str>,
) -> CodexResponsesMetadata {
    CodexResponsesMetadata {
        request_kind: Some(CodexResponsesRequestKind::Memory),
        subagent_header: subagent_header_value(session_source),
        sandbox: sandbox.map(ToString::to_string),
        sandbox_mode: Some(
            permission_profile_policy_tag(permission_profile, cwd.as_path()).to_string(),
        ),
        workspaces: memory_workspaces(cwd).await,
        ..CodexResponsesMetadata::new(installation_id, session_id, thread_id, window_id)
    }
}

#[derive(Debug)]
pub(crate) struct TurnMetadataState {
    cwd: AbsolutePathBuf,
    repo_root: Option<PathBuf>,
    session_id: String,
    thread_id: String,
    forked_from_thread_id: Option<ThreadId>,
    parent_thread_id: Option<ThreadId>,
    parent_turn_id: OnceLock<String>,
    root_turn_id: OnceLock<String>,
    subagent_header: Option<String>,
    subagent_kind: Option<String>,
    thread_source: Option<ThreadSource>,
    turn_id: String,
    sandbox: Option<String>,
    sandbox_mode: Option<String>,
    auto_review_enabled: bool,
    node_repl_auto_review_required: bool,
    node_repl_disabled: bool,
    enriched_workspaces: RwLock<Option<BTreeMap<String, TurnMetadataWorkspace>>>,
    tool_namespaces_info: RwLock<Option<TurnToolNamespacesInfo>>,
    turn_started_at_unix_ms: RwLock<Option<i64>>,
    responses_api_metadata: RwLock<BTreeMap<String, String>>,
    responsesapi_client_metadata: RwLock<BTreeMap<String, String>>,
    root_turn_ambiguous: AtomicBool,
    user_input_requested_during_turn: AtomicBool,
    enrichment_task: Mutex<Option<JoinHandle<()>>>,
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
        auto_review_enabled: bool,
        model_info: &ModelInfo,
    ) -> Self {
        let repo_root = get_git_repo_root(&cwd);
        let sandbox = Some(
            permission_profile_sandbox_tag(
                permission_profile,
                windows_sandbox_level,
                enforce_managed_network,
            )
            .to_string(),
        );
        let sandbox_mode =
            Some(permission_profile_policy_tag(permission_profile, cwd.as_path()).to_string());
        Self {
            cwd,
            repo_root,
            session_id,
            thread_id,
            forked_from_thread_id,
            parent_thread_id,
            parent_turn_id: OnceLock::new(),
            root_turn_id: OnceLock::new(),
            subagent_header: subagent_header_value(session_source),
            subagent_kind: subagent_metadata_kind(session_source),
            thread_source,
            turn_id,
            sandbox,
            sandbox_mode,
            auto_review_enabled,
            node_repl_auto_review_required: model_info.node_repl_auto_review_required,
            node_repl_disabled: model_info.node_repl_disabled,
            enriched_workspaces: RwLock::new(None),
            tool_namespaces_info: RwLock::new(None),
            turn_started_at_unix_ms: RwLock::new(None),
            responses_api_metadata: RwLock::new(BTreeMap::new()),
            responsesapi_client_metadata: RwLock::new(BTreeMap::new()),
            root_turn_ambiguous: AtomicBool::new(false),
            user_input_requested_during_turn: AtomicBool::new(false),
            enrichment_task: Mutex::new(None),
        }
    }

    pub(crate) fn current_meta_value_for_mcp_request(
        &self,
        context: McpTurnMetadataContext<'_>,
    ) -> Option<serde_json::Value> {
        let mut responses_metadata = self.mcp_metadata_template();
        // Never serialize harness-owned tool inventory for external MCP servers.
        responses_metadata.tool_namespaces_info = None;
        let Value::Object(mut metadata) = responses_metadata.turn_metadata_value()? else {
            return None;
        };
        metadata.remove(PARENT_TURN_ID_KEY);
        metadata.remove(ROOT_TURN_ID_KEY);
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

    pub(crate) fn set_tool_namespaces_info(&self, tool_namespaces_info: TurnToolNamespacesInfo) {
        *self
            .tool_namespaces_info
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            (!tool_namespaces_info.is_empty()).then_some(tool_namespaces_info);
    }

    pub(crate) fn set_parent_turn_id(&self, parent_turn_id: String) {
        if parent_turn_id.trim().is_empty() {
            return;
        }
        let _ = self.parent_turn_id.set(parent_turn_id);
    }

    pub(crate) fn set_root_turn_id(&self, root_turn_id: String) {
        if root_turn_id.trim().is_empty() {
            return;
        }
        let _ = self.root_turn_id.set(root_turn_id);
    }

    pub(crate) fn root_turn_id(&self) -> Option<String> {
        self.root_turn_id
            .get()
            .filter(|_| !self.root_turn_ambiguous.load(Ordering::Relaxed))
            .cloned()
    }

    pub(crate) fn mark_root_turn_ambiguous(&self) {
        self.root_turn_ambiguous.store(true, Ordering::Relaxed);
    }

    pub(crate) fn can_start_root_turn(&self, session_source: &SessionSource) -> bool {
        if session_source.is_non_root_agent() {
            return false;
        }
        match &self.thread_source {
            // Desktop create/fork/send lacks trusted app-server provenance; fail closed.
            Some(ThreadSource::Subagent | ThreadSource::MemoryConsolidation) => false,
            Some(ThreadSource::Feature(feature)) => {
                !matches!(feature.as_str(), "system" | "title") && !feature.starts_with("ambient")
            }
            Some(ThreadSource::User) | None => true,
        }
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

    pub(crate) fn set_responses_api_metadata(
        &self,
        responses_api_metadata: BTreeMap<String, String>,
    ) {
        *self
            .responses_api_metadata
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = responses_api_metadata;
    }

    pub(crate) fn workspace_kind(&self) -> Option<String> {
        self.responsesapi_client_metadata
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(WORKSPACE_KIND_KEY)
            .cloned()
    }

    fn responses_metadata_template(&self) -> CodexResponsesMetadata {
        let mut metadata = self.mcp_metadata_template();
        metadata.extra.extend(
            self.responses_api_metadata
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
        metadata
    }

    fn mcp_metadata_template(&self) -> CodexResponsesMetadata {
        let mut extra = self
            .responsesapi_client_metadata
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for key in self
            .responses_api_metadata
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
        {
            extra.remove(key);
        }
        CodexResponsesMetadata {
            turn_id: Some(self.turn_id.clone()),
            forked_from_thread_id: self.forked_from_thread_id,
            parent_thread_id: self.parent_thread_id,
            parent_turn_id: self.parent_turn_id.get().cloned(),
            root_turn_id: self.root_turn_id(),
            subagent_header: self.subagent_header.clone(),
            subagent_kind: self.subagent_kind.clone(),
            thread_source: self.thread_source.clone(),
            sandbox: self.sandbox.clone(),
            sandbox_mode: self.sandbox_mode.clone(),
            auto_review_enabled: Some(self.auto_review_enabled),
            node_repl_auto_review_required: Some(self.node_repl_auto_review_required),
            node_repl_disabled: Some(self.node_repl_disabled),
            workspaces: self.current_workspaces(),
            tool_namespaces_info: self
                .tool_namespaces_info
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            turn_started_at_unix_ms: self.current_turn_started_at_unix_ms(),
            extra,
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

    pub(crate) fn spawn_git_enrichment_task(self: &Arc<Self>) {
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

        let state = Arc::clone(self);
        *task_guard = Some(tokio::spawn(async move {
            let Some(repo_root) = state.repo_root.clone() else {
                return;
            };
            let workspace_git_metadata = state.fetch_workspace_git_metadata(&repo_root).await;

            if workspace_git_metadata.is_empty() {
                return;
            }

            let mut workspaces = BTreeMap::new();
            workspaces.insert(
                repo_root.to_string_lossy().into_owned(),
                workspace_git_metadata.into(),
            );
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

    async fn fetch_workspace_git_metadata(&self, repo_root: &Path) -> WorkspaceGitMetadata {
        let (head_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
            get_head_commit_hash(&self.cwd),
            get_git_remote_urls_assume_git_repo(&self.cwd),
            get_has_changes_in_repo(&self.cwd, repo_root),
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
    let Some(repo_root) = get_git_repo_root(cwd) else {
        return BTreeMap::new();
    };
    let (head_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd),
        get_has_changes_in_repo(cwd, &repo_root),
    );
    let workspace_git_metadata = WorkspaceGitMetadata {
        associated_remote_urls,
        latest_git_commit_hash: head_commit_hash.map(|sha| sha.0),
        has_changes,
    };
    let mut workspaces = BTreeMap::new();
    if !workspace_git_metadata.is_empty() {
        workspaces.insert(
            repo_root.to_string_lossy().into_owned(),
            workspace_git_metadata.into(),
        );
    }
    workspaces
}

#[cfg(test)]
#[path = "turn_metadata_tests.rs"]
mod tests;
