use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use serde::Serialize;
use serde_json::Value;
use tokio::task::JoinHandle;

use crate::sandbox_tags::permission_profile_sandbox_tag;
use codex_git_utils::get_git_remote_urls_assume_git_repo;
use codex_git_utils::get_git_repo_root;
use codex_git_utils::get_has_changes;
use codex_git_utils::get_head_commit_hash;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SessionSource;
use codex_utils_absolute_path::AbsolutePathBuf;

const TURN_STARTED_AT_UNIX_MS_KEY: &str = "turn_started_at_unix_ms";

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

#[derive(Clone, Debug, Serialize, Default)]
pub(crate) struct TurnMetadataBag {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_source: Option<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    workspaces: BTreeMap<String, TurnMetadataWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sandbox: Option<String>,
}

impl TurnMetadataBag {
    fn to_header_value(&self) -> Option<String> {
        serde_json::to_string(self).ok()
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
            if key == TURN_STARTED_AT_UNIX_MS_KEY {
                continue;
            }
            metadata
                .entry(key.clone())
                .or_insert_with(|| Value::String(value.clone()));
        }
    }
    serde_json::to_string(&metadata).ok()
}

fn build_turn_metadata_bag(
    session_id: Option<String>,
    thread_source: Option<&'static str>,
    turn_id: Option<String>,
    sandbox: Option<String>,
    repo_root: Option<String>,
    workspace_git_metadata: Option<WorkspaceGitMetadata>,
) -> TurnMetadataBag {
    let mut workspaces = BTreeMap::new();
    if let (Some(repo_root), Some(workspace_git_metadata)) = (repo_root, workspace_git_metadata)
        && !workspace_git_metadata.is_empty()
    {
        workspaces.insert(repo_root, workspace_git_metadata.into());
    }

    TurnMetadataBag {
        session_id,
        thread_source,
        turn_id,
        workspaces,
        sandbox,
    }
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
    if latest_git_commit_hash.is_none()
        && associated_remote_urls.is_none()
        && has_changes.is_none()
        && sandbox.is_none()
    {
        return None;
    }

    build_turn_metadata_bag(
        /*session_id*/ None,
        /*thread_source*/ None,
        /*turn_id*/ None,
        sandbox.map(ToString::to_string),
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
    base_header: String,
    enriched_header: Arc<RwLock<Option<String>>>,
    turn_started_at_unix_ms: Arc<RwLock<Option<i64>>>,
    responsesapi_client_metadata: Arc<RwLock<Option<HashMap<String, String>>>>,
    enrichment_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TurnMetadataState {
    pub(crate) fn new(
        session_id: String,
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
        let base_metadata = build_turn_metadata_bag(
            Some(session_id),
            session_source.thread_source_name(),
            Some(turn_id),
            sandbox,
            /*repo_root*/ None,
            /*workspace_git_metadata*/ None,
        );
        let base_header = base_metadata
            .to_header_value()
            .unwrap_or_else(|| "{}".to_string());

        Self {
            cwd,
            repo_root,
            base_metadata,
            base_header,
            enriched_header: Arc::new(RwLock::new(None)),
            turn_started_at_unix_ms: Arc::new(RwLock::new(None)),
            responsesapi_client_metadata: Arc::new(RwLock::new(None)),
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
            self.base_header.clone()
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

    pub(crate) fn current_meta_value(&self) -> Option<serde_json::Value> {
        self.current_header_value()
            .and_then(|header| serde_json::from_str(&header).ok())
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

            let enriched_metadata = build_turn_metadata_bag(
                state.base_metadata.session_id.clone(),
                state.base_metadata.thread_source,
                state.base_metadata.turn_id.clone(),
                state.base_metadata.sandbox.clone(),
                Some(repo_root),
                Some(workspace_git_metadata),
            );
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
