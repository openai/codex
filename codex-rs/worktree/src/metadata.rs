//! Codex metadata read/write helpers for managed worktrees.
//!
//! Metadata is stored through git rev-parse --git-path so it lands beside the worktree's Git
//! metadata whether the checkout uses a directory .git or a file that points into a common Git dir.
//! That placement keeps ownership data attached to the worktree without requiring callers to know
//! the repository's worktree internals.
//!
//! Two files are intentionally maintained. codex-worktree.json describes the managed worktree for
//! inventory and deletion safety, while codex-thread.json records the Codex thread currently bound
//! to that checkout.

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

/// Thread ownership metadata stored beside a worktree's Git metadata.
///
/// A pending worktree is written with no owner before the session starts, then updated after the
/// TUI successfully attaches a thread. Consumers should treat a missing owner as unbound rather
/// than corrupt metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeThreadMetadata {
    /// Metadata schema version.
    pub version: u32,
    /// Thread id currently associated with this worktree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_thread_id: Option<String>,
}

/// Persistent description of a Codex-managed worktree.
///
/// The metadata duplicates a subset of WorktreeInfo so a later process can list or remove a
/// worktree without reconstructing all creation context. The source and location defaults are for
/// older metadata files that predate those fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMetadata {
    /// Metadata schema version.
    pub version: u32,
    /// Manager that created this metadata.
    pub manager: String,
    /// Backing implementation for this worktree.
    pub backend: String,
    /// Source surface that created or exposed this worktree.
    #[serde(default = "default_source")]
    pub source: WorktreeSource,
    /// Filesystem placement category for this worktree.
    #[serde(default = "default_location")]
    pub location: WorktreeLocation,
    /// Stable repository fingerprint used for grouping.
    pub id: String,
    /// Human-readable worktree name.
    pub name: String,
    /// Filesystem/search-friendly worktree slug.
    pub slug: String,
    /// Checked-out branch, if known.
    pub branch: Option<String>,
    /// Stable repository fingerprint retained for compatibility with app metadata.
    pub repo_id: String,
    /// Display name of the source repository.
    pub repo_name: String,
    /// Root of the repository that created this worktree.
    pub source_repo_root: PathBuf,
    /// Relative cwd from the source repository root.
    pub original_relative_cwd: PathBuf,
    /// Root directory of the Git worktree.
    pub worktree_git_root: PathBuf,
    /// Directory Codex should use when starting a session in this worktree.
    pub workspace_cwd: PathBuf,
    /// Unix timestamp for metadata creation.
    pub created_at: i64,
    /// Unix timestamp for the most recent metadata update.
    pub updated_at: i64,
    /// Codex thread currently associated with this worktree.
    pub owner_thread_id: Option<String>,
    /// Optional tmux session name retained for compatibility with older metadata.
    pub tmux_session: Option<String>,
}

impl WorktreeMetadata {
    /// Builds fresh metadata from the inventory record produced during creation.
    ///
    /// This constructor stamps created_at and updated_at together because it is only used for new
    /// metadata. Callers that update an existing worktree should mutate that record and preserve its
    /// original creation time.
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

/// Reads Codex worktree metadata from the Git metadata area for a worktree.
///
/// A missing metadata file returns Ok(None). A parse error means the path looked managed but the
/// stored ownership data is not usable, so callers should surface that as an error rather than
/// silently treating it as unmanaged.
pub fn read_worktree_metadata(worktree_path: &Path) -> Result<Option<WorktreeMetadata>> {
    let path = metadata_path(worktree_path, "codex-worktree.json")?;
    read_json_if_exists(&path)
}

/// Writes Codex worktree metadata to the Git metadata area for a worktree.
///
/// The caller is responsible for ensuring the worktree path is the root of the Git worktree. Writing
/// metadata for a nested cwd would bind the wrong checkout and make later removal checks fail.
pub fn write_worktree_metadata(worktree_path: &Path, metadata: &WorktreeMetadata) -> Result<()> {
    let path = metadata_path(worktree_path, "codex-worktree.json")?;
    write_json(&path, metadata)
}

/// Associates a worktree with a Codex thread id.
///
/// This updates both the lightweight thread metadata and the full worktree metadata when present.
/// It should be called after the session is successfully started or forked; calling it earlier can
/// leave metadata pointing at a thread that was never attached.
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

/// Writes unbound thread metadata for a newly-created worktree.
///
/// The pending owner file lets other Codex surfaces recognize the worktree before a thread is
/// attached. It is intentionally separate from WorktreeMetadata so session binding can be updated
/// independently of creation metadata.
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
