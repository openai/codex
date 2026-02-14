use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use serde::Serialize;
use tokio::task::JoinHandle;

use crate::git_info::get_git_remote_urls_assume_git_repo;
use crate::git_info::get_git_repo_root;
use crate::git_info::get_has_changes;
use crate::git_info::get_head_commit_hash;
use crate::sandbox_tags::sandbox_tag;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;

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

fn build_turn_metadata_bag(
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
        turn_id,
        workspaces,
        sandbox,
    }
}

pub async fn build_turn_metadata_header(cwd: &Path, sandbox: Option<&str>) -> Option<String> {
    let repo_root = get_git_repo_root(cwd).map(|root| root.to_string_lossy().into_owned());

    let (latest_git_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd),
        get_has_changes(cwd),
    );
    if latest_git_commit_hash.is_none()
        && associated_remote_urls.is_none()
        && has_changes.is_none()
        && sandbox.is_none()
    {
        return None;
    }

    build_turn_metadata_bag(
        None,
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
    cwd: PathBuf,
    repo_root: Option<String>,
    base_metadata: TurnMetadataBag,
    base_header: String,
    enriched_header: Arc<RwLock<Option<String>>>,
    enrichment_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TurnMetadataState {
    pub(crate) fn new(
        turn_id: String,
        cwd: PathBuf,
        sandbox_policy: &SandboxPolicy,
        windows_sandbox_level: WindowsSandboxLevel,
    ) -> Self {
        let repo_root = get_git_repo_root(&cwd).map(|root| root.to_string_lossy().into_owned());
        let sandbox = Some(sandbox_tag(sandbox_policy, windows_sandbox_level).to_string());
        let base_metadata = build_turn_metadata_bag(Some(turn_id), sandbox, None, None);
        let base_header = base_metadata
            .to_header_value()
            .unwrap_or_else(|| "{}".to_string());

        Self {
            cwd,
            repo_root,
            base_metadata,
            base_header,
            enriched_header: Arc::new(RwLock::new(None)),
            enrichment_task: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn current_header_value(&self) -> Option<String> {
        if let Some(header) = self
            .enriched_header
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .cloned()
        {
            return Some(header);
        }
        Some(self.base_header.clone())
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
        let (latest_git_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
            get_head_commit_hash(&self.cwd),
            get_git_remote_urls_assume_git_repo(&self.cwd),
            get_has_changes(&self.cwd),
        );

        WorkspaceGitMetadata {
            associated_remote_urls,
            latest_git_commit_hash,
            has_changes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::Value;
    use tempfile::TempDir;
    use tokio::process::Command;

    #[tokio::test]
    async fn build_turn_metadata_header_includes_has_changes_for_clean_repo() {
        let temp_dir = TempDir::new().expect("temp dir");
        let repo_path = temp_dir.path().join("repo");
        std::fs::create_dir_all(&repo_path).expect("create repo");

        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git init");
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git config user.name");
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git config user.email");

        std::fs::write(repo_path.join("README.md"), "hello").expect("write file");
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git commit");

        let header = build_turn_metadata_header(&repo_path, Some("none"))
            .await
            .expect("header");
        let parsed: Value = serde_json::from_str(&header).expect("valid json");
        let workspace = parsed
            .get("workspaces")
            .and_then(Value::as_object)
            .and_then(|workspaces| workspaces.values().next())
            .cloned()
            .expect("workspace");

        assert_eq!(
            workspace.get("has_changes").and_then(Value::as_bool),
            Some(false)
        );
    }
}
