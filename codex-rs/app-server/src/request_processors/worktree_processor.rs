use super::*;

use std::path::PathBuf;

use codex_worktree::DirtyPolicy;
use codex_worktree::DirtyState;
use codex_worktree::WorktreeInfo;
use codex_worktree::WorktreeListQuery;
use codex_worktree::WorktreeLocation;
use codex_worktree::WorktreeRemoveRequest;
use codex_worktree::WorktreeRequest;
use codex_worktree::WorktreeSource;
use codex_worktree::WorktreeWarning;

#[derive(Clone)]
pub(crate) struct WorktreeRequestProcessor {
    config_manager: ConfigManager,
}

impl WorktreeRequestProcessor {
    pub(crate) fn new(config_manager: ConfigManager) -> Self {
        Self { config_manager }
    }

    pub(crate) async fn list(
        &self,
        params: WorktreeListParams,
    ) -> Result<WorktreeListResponse, JSONRPCErrorError> {
        let source_cwd = if params.all {
            None
        } else {
            Some(self.resolve_cwd(params.cwd).await?)
        };
        let data = codex_worktree::list_worktrees(WorktreeListQuery {
            codex_home: self.config_manager.codex_home().to_path_buf(),
            source_cwd,
            include_all_repos: params.all,
        })
        .map_err(map_worktree_error)?
        .into_iter()
        .map(api_worktree_info)
        .collect();
        Ok(WorktreeListResponse { data })
    }

    pub(crate) async fn inspect_source(
        &self,
        params: WorktreeInspectSourceParams,
    ) -> Result<WorktreeInspectSourceResponse, JSONRPCErrorError> {
        let cwd = self.resolve_cwd(params.cwd).await?;
        let dirty = api_dirty_state(codex_worktree::dirty_state(&cwd).map_err(map_worktree_error)?);
        Ok(WorktreeInspectSourceResponse { dirty })
    }

    pub(crate) async fn create(
        &self,
        params: WorktreeCreateParams,
    ) -> Result<WorktreeCreateResponse, JSONRPCErrorError> {
        let cwd = self.resolve_cwd(params.cwd).await?;
        let resolution = codex_worktree::ensure_worktree(WorktreeRequest {
            codex_home: self.config_manager.codex_home().to_path_buf(),
            source_cwd: cwd,
            branch: params.branch,
            base_ref: params.base_ref,
            dirty_policy: dirty_policy_from_api(params.dirty_policy),
        })
        .map_err(map_worktree_error)?;
        Ok(WorktreeCreateResponse {
            reused: resolution.reused,
            info: api_worktree_info(resolution.info),
            warnings: resolution
                .warnings
                .into_iter()
                .map(api_worktree_warning)
                .collect(),
        })
    }

    pub(crate) async fn remove(
        &self,
        params: WorktreeRemoveParams,
    ) -> Result<WorktreeRemoveResponse, JSONRPCErrorError> {
        let cwd = self.resolve_cwd(params.cwd).await?;
        let result = codex_worktree::remove_worktree(WorktreeRemoveRequest {
            codex_home: self.config_manager.codex_home().to_path_buf(),
            source_cwd: Some(cwd),
            name_or_path: params.name_or_path,
            force: params.force,
            delete_branch: params.delete_branch,
        })
        .map_err(map_worktree_error)?;
        Ok(WorktreeRemoveResponse {
            removed_path: result.removed_path.to_string_lossy().to_string(),
            deleted_branch: result.deleted_branch,
        })
    }

    pub(crate) async fn prune(
        &self,
        params: WorktreePruneParams,
    ) -> Result<WorktreePruneResponse, JSONRPCErrorError> {
        let stale_paths = codex_worktree::prune_stale_managed_worktree_dirs(
            self.config_manager.codex_home(),
            params.dry_run,
        )
        .map_err(map_worktree_error)?;
        Ok(WorktreePruneResponse {
            paths: stale_paths
                .into_iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
        })
    }

    async fn resolve_cwd(&self, cwd: Option<String>) -> Result<PathBuf, JSONRPCErrorError> {
        match cwd {
            Some(cwd) => Ok(PathBuf::from(cwd)),
            None => self
                .config_manager
                .load_latest_config(/*fallback_cwd*/ None)
                .await
                .map(|config| config.cwd.to_path_buf())
                .map_err(|err| internal_error(format!("failed to load worktree cwd: {err}"))),
        }
    }
}

fn map_worktree_error(err: anyhow::Error) -> JSONRPCErrorError {
    invalid_request(err.to_string())
}

fn dirty_policy_from_api(value: ApiWorktreeDirtyPolicy) -> DirtyPolicy {
    match value {
        ApiWorktreeDirtyPolicy::Fail => DirtyPolicy::Fail,
        ApiWorktreeDirtyPolicy::Ignore => DirtyPolicy::Ignore,
        ApiWorktreeDirtyPolicy::CopyTracked => DirtyPolicy::CopyTracked,
        ApiWorktreeDirtyPolicy::CopyAll => DirtyPolicy::CopyAll,
        ApiWorktreeDirtyPolicy::MoveTracked => DirtyPolicy::MoveTracked,
        ApiWorktreeDirtyPolicy::MoveAll => DirtyPolicy::MoveAll,
    }
}

fn api_dirty_state(value: DirtyState) -> ApiWorktreeDirtyState {
    ApiWorktreeDirtyState {
        has_staged_changes: value.has_staged_changes,
        has_unstaged_changes: value.has_unstaged_changes,
        has_untracked_files: value.has_untracked_files,
    }
}

fn api_worktree_info(value: WorktreeInfo) -> ApiWorktreeInfo {
    ApiWorktreeInfo {
        id: value.id,
        name: value.name,
        slug: value.slug,
        source: api_worktree_source(value.source),
        location: api_worktree_location(value.location),
        repo_name: value.repo_name,
        repo_root: value.repo_root.to_string_lossy().to_string(),
        common_git_dir: value.common_git_dir.to_string_lossy().to_string(),
        worktree_git_root: value.worktree_git_root.to_string_lossy().to_string(),
        workspace_cwd: value.workspace_cwd.to_string_lossy().to_string(),
        original_relative_cwd: value.original_relative_cwd.to_string_lossy().to_string(),
        branch: value.branch,
        head: value.head,
        owner_thread_id: value.owner_thread_id,
        metadata_path: value.metadata_path.to_string_lossy().to_string(),
        dirty: api_dirty_state(value.dirty),
    }
}

fn api_worktree_source(value: WorktreeSource) -> ApiWorktreeSource {
    match value {
        WorktreeSource::Cli => ApiWorktreeSource::Cli,
        WorktreeSource::App => ApiWorktreeSource::App,
        WorktreeSource::Legacy => ApiWorktreeSource::Legacy,
        WorktreeSource::Git => ApiWorktreeSource::Git,
    }
}

fn api_worktree_location(value: WorktreeLocation) -> ApiWorktreeLocation {
    match value {
        WorktreeLocation::Sibling => ApiWorktreeLocation::Sibling,
        WorktreeLocation::CodexHome => ApiWorktreeLocation::CodexHome,
        WorktreeLocation::External => ApiWorktreeLocation::External,
    }
}

fn api_worktree_warning(value: WorktreeWarning) -> ApiWorktreeWarning {
    ApiWorktreeWarning {
        message: value.message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn dirty_policy_conversion_preserves_every_variant() {
        assert_eq!(
            [
                ApiWorktreeDirtyPolicy::Fail,
                ApiWorktreeDirtyPolicy::Ignore,
                ApiWorktreeDirtyPolicy::CopyTracked,
                ApiWorktreeDirtyPolicy::CopyAll,
                ApiWorktreeDirtyPolicy::MoveTracked,
                ApiWorktreeDirtyPolicy::MoveAll,
            ]
            .map(dirty_policy_from_api),
            [
                DirtyPolicy::Fail,
                DirtyPolicy::Ignore,
                DirtyPolicy::CopyTracked,
                DirtyPolicy::CopyAll,
                DirtyPolicy::MoveTracked,
                DirtyPolicy::MoveAll,
            ]
        );
    }
}
