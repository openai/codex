use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::WorktreeInfo;
use crate::WorktreeLocation;
use crate::WorktreeSource;
use crate::git;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeThreadMetadata {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMetadata {
    pub version: u32,
    pub manager: String,
    pub backend: String,
    #[serde(default = "default_source")]
    pub source: WorktreeSource,
    #[serde(default = "default_location")]
    pub location: WorktreeLocation,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub branch: Option<String>,
    pub repo_id: String,
    pub repo_name: String,
    pub source_repo_root: PathBuf,
    pub original_relative_cwd: PathBuf,
    pub worktree_git_root: PathBuf,
    pub workspace_cwd: PathBuf,
    pub created_at: i64,
    pub updated_at: i64,
    pub owner_thread_id: Option<String>,
    pub tmux_session: Option<String>,
}

impl WorktreeMetadata {
    pub fn from_info(info: &WorktreeInfo, source_repo_root: PathBuf) -> Self {
        let now = unix_seconds();
        Self {
            version: 1,
            manager: "codex-cli".to_string(),
            backend: "git".to_string(),
            source: info.source,
            location: info.location,
            id: info.id.clone(),
            name: info.name.clone(),
            slug: info.slug.clone(),
            branch: info.branch.clone(),
            repo_id: info.id.clone(),
            repo_name: info.repo_name.clone(),
            source_repo_root,
            original_relative_cwd: info.original_relative_cwd.clone(),
            worktree_git_root: info.worktree_git_root.clone(),
            workspace_cwd: info.workspace_cwd.clone(),
            created_at: now,
            updated_at: now,
            owner_thread_id: info.owner_thread_id.clone(),
            tmux_session: None,
        }
    }
}

fn default_source() -> WorktreeSource {
    WorktreeSource::Legacy
}

fn default_location() -> WorktreeLocation {
    WorktreeLocation::CodexHome
}

pub fn read_worktree_metadata(worktree_path: &Path) -> Result<Option<WorktreeMetadata>> {
    let path = metadata_path(worktree_path, "codex-worktree.json")?;
    read_json_if_exists(&path)
}

pub fn write_worktree_metadata(worktree_path: &Path, metadata: &WorktreeMetadata) -> Result<()> {
    let path = metadata_path(worktree_path, "codex-worktree.json")?;
    write_json(&path, metadata)
}

pub fn bind_thread(workspace_cwd: &Path, thread_id: &str) -> Result<()> {
    let git_root = git::stdout(workspace_cwd, &["rev-parse", "--show-toplevel"])?;
    let git_root = PathBuf::from(git_root);
    let owner = WorktreeThreadMetadata {
        version: 1,
        owner_thread_id: Some(thread_id.to_string()),
    };
    let owner_path = metadata_path(&git_root, "codex-thread.json")?;
    write_json(&owner_path, &owner)?;

    if let Some(mut metadata) = read_worktree_metadata(&git_root)? {
        metadata.owner_thread_id = Some(thread_id.to_string());
        metadata.updated_at = unix_seconds();
        write_worktree_metadata(&git_root, &metadata)?;
    }
    Ok(())
}

pub fn write_pending_owner_metadata(worktree_path: &Path) -> Result<()> {
    let metadata = WorktreeThreadMetadata {
        version: 1,
        owner_thread_id: None,
    };
    let path = metadata_path(worktree_path, "codex-thread.json")?;
    write_json(&path, &metadata)
}

fn read_json_if_exists<T>(path: &Path) -> Result<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&contents)?))
}

fn write_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: serde::Serialize,
{
    let contents = serde_json::to_string_pretty(value)?;
    fs::write(path, contents)?;
    Ok(())
}

fn metadata_path(worktree_path: &Path, name: &str) -> Result<PathBuf> {
    let path = git::stdout(worktree_path, &["rev-parse", "--git-path", name])?;
    let path = PathBuf::from(path);
    Ok(if path.is_absolute() {
        path
    } else {
        worktree_path.join(path)
    })
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
