use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsCopyResponse;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsGetMetadataParams;
use codex_app_server_protocol::FsGetMetadataResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsRemoveResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::RequestId;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_worktree::DirtyPolicy;
use codex_worktree::DirtyState;
use codex_worktree::WorktreeInfo;
use codex_worktree::WorktreeLocation;
use codex_worktree::WorktreeMetadata;
use codex_worktree::WorktreeRemoveResult;
use codex_worktree::WorktreeRequest;
use codex_worktree::WorktreeResolution;
use codex_worktree::WorktreeSource;
use codex_worktree::WorktreeThreadMetadata;
use codex_worktree::WorktreeWarning;
use uuid::Uuid;

use crate::workspace_command::WorkspaceCommand;
use crate::workspace_command::WorkspaceCommandRunner;

pub(crate) async fn list_current_repo_worktrees(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    source_cwd: &Path,
) -> Result<Vec<WorktreeInfo>> {
    let repo = RemoteSourceRepo::resolve(runner, source_cwd).await?;
    let worktrees = parse_worktree_list(
        &git_stdout(runner, &repo.root, &["worktree", "list", "--porcelain"]).await?,
    );
    let mut infos = Vec::new();
    for entry in worktrees {
        infos.push(
            info_from_existing_worktree(runner, request_handle, &entry.path, entry.branch, &repo)
                .await?,
        );
    }
    Ok(infos)
}

pub(crate) async fn source_dirty_state(
    runner: &WorkspaceCommandRunner,
    source_cwd: &Path,
) -> Result<DirtyState> {
    let repo = RemoteSourceRepo::resolve(runner, source_cwd).await?;
    dirty_state(runner, &repo.root).await
}

pub(crate) async fn ensure_worktree(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    req: WorktreeRequest,
) -> Result<WorktreeResolution> {
    let repo = RemoteSourceRepo::resolve(runner, &req.source_cwd).await?;
    let branch = req.branch.clone();
    git_status(
        runner,
        &repo.root,
        &["check-ref-format", "--branch", &branch],
    )
    .await?;
    let worktree_git_root = codex_worktree::sibling_worktree_git_root(&repo.primary_root, &branch)?;
    let workspace_cwd = worktree_git_root.join(&repo.relative_cwd);

    if path_exists(request_handle, &worktree_git_root).await? {
        let metadata = read_metadata(runner, request_handle, &worktree_git_root)
            .await?
            .context("managed worktree path already exists but is not owned by Codex")?;
        if metadata.branch.as_deref() != Some(branch.as_str()) && metadata.name != branch {
            anyhow::bail!(
                "managed worktree path {} is already used by {}; choose a different branch name",
                worktree_git_root.display(),
                metadata.branch.as_deref().unwrap_or(metadata.name.as_str())
            );
        }
        let info = info_from_existing_worktree(
            runner,
            request_handle,
            &worktree_git_root,
            Some(branch),
            &repo,
        )
        .await?;
        return Ok(WorktreeResolution {
            reused: true,
            info,
            warnings: Vec::new(),
        });
    }

    let worktrees = parse_worktree_list(
        &git_stdout(runner, &repo.root, &["worktree", "list", "--porcelain"]).await?,
    );
    if let Some(existing) = worktrees
        .iter()
        .find(|entry| entry.branch.as_deref() == Some(branch.as_str()))
        && existing.path != worktree_git_root
    {
        anyhow::bail!(
            "branch {branch} is already checked out at {}; remove that worktree first",
            existing.path.display()
        );
    }

    let warnings =
        validate_dirty_policy_before_create(runner, &repo.root, req.dirty_policy).await?;
    create_directory(
        request_handle,
        worktree_git_root
            .parent()
            .context("managed worktree path has no parent")?,
    )
    .await?;
    let branch_exists = git_status_result(
        runner,
        &repo.root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .await?
    .success();
    let has_head = git_status_result(runner, &repo.root, &["rev-parse", "--verify", "HEAD"])
        .await?
        .success();
    if branch_exists {
        git_status(
            runner,
            &repo.root,
            &[
                "worktree",
                "add",
                &worktree_git_root.to_string_lossy(),
                &branch,
            ],
        )
        .await?;
    } else if req.base_ref.is_none() && !has_head {
        git_status(
            runner,
            &repo.root,
            &[
                "worktree",
                "add",
                "--orphan",
                "-b",
                &branch,
                &worktree_git_root.to_string_lossy(),
            ],
        )
        .await?;
    } else {
        let base_ref = req.base_ref.as_deref().unwrap_or("HEAD");
        git_status(
            runner,
            &repo.root,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                &worktree_git_root.to_string_lossy(),
                base_ref,
            ],
        )
        .await?;
    }

    let prepared_transfer = match prepare_dirty_policy_after_create(
        runner,
        request_handle,
        &repo.root,
        &worktree_git_root,
        req.dirty_policy,
    )
    .await
    {
        Ok(prepared_transfer) => prepared_transfer,
        Err(err) => {
            return Err(rollback_failed_create(
                runner,
                &repo.root,
                &worktree_git_root,
                &branch,
                /*delete_branch*/ !branch_exists,
                err,
            )
            .await);
        }
    };
    finalize_dirty_policy_after_create(runner, request_handle, &repo.root, prepared_transfer)
        .await?;
    let dirty = dirty_state(runner, &worktree_git_root).await?;
    let head = git_stdout_result(runner, &worktree_git_root, &["rev-parse", "HEAD"])
        .await?
        .success()
        .then_some(async { git_stdout(runner, &worktree_git_root, &["rev-parse", "HEAD"]).await });
    let head = match head {
        Some(future) => Some(future.await?),
        None => None,
    };
    let info_metadata_path =
        metadata_path(runner, &worktree_git_root, "codex-worktree.json").await?;
    let mut info = WorktreeInfo {
        id: repo.id.clone(),
        name: branch.clone(),
        slug: codex_worktree::slugify_name(&branch)?,
        source: WorktreeSource::Cli,
        location: WorktreeLocation::Sibling,
        repo_name: repo.repo_name.clone(),
        repo_root: repo.root.clone(),
        common_git_dir: repo.common_git_dir.clone(),
        worktree_git_root: worktree_git_root.clone(),
        workspace_cwd,
        original_relative_cwd: repo.relative_cwd.clone(),
        branch: Some(branch),
        head,
        owner_thread_id: None,
        metadata_path: info_metadata_path,
        dirty,
    };
    write_json(
        request_handle,
        &metadata_path(runner, &worktree_git_root, "codex-thread.json").await?,
        &WorktreeThreadMetadata {
            version: 1,
            owner_thread_id: None,
        },
    )
    .await?;
    let metadata = WorktreeMetadata::from_info(&info, repo.root);
    write_json(request_handle, &info.metadata_path, &metadata).await?;
    info.owner_thread_id = metadata.owner_thread_id;
    Ok(WorktreeResolution {
        reused: false,
        info,
        warnings: warnings
            .into_iter()
            .map(|message| WorktreeWarning { message })
            .collect(),
    })
}

pub(crate) async fn remove_worktree(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    source_cwd: &Path,
    target: &str,
    force: bool,
    delete_branch: bool,
) -> Result<WorktreeRemoveResult> {
    let entries = list_current_repo_worktrees(runner, request_handle, source_cwd).await?;
    let info = entries
        .into_iter()
        .find(|entry| {
            entry.branch.as_deref() == Some(target) || entry.name == target || entry.slug == target
        })
        .context("no managed worktree matched target")?;
    let metadata = read_metadata(runner, request_handle, &info.worktree_git_root)
        .await?
        .context("refusing to remove a worktree not managed by Codex")?;
    if metadata.source != WorktreeSource::Cli {
        anyhow::bail!("refusing to remove a worktree not managed by Codex CLI");
    }
    if info.dirty.is_dirty() && !force {
        anyhow::bail!(
            "refusing to remove dirty worktree {}; use --force to override",
            info.worktree_git_root.display()
        );
    }
    let repo = RemoteSourceRepo::resolve(runner, source_cwd).await?;
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    let target_path = info.worktree_git_root.to_string_lossy().to_string();
    args.push(&target_path);
    git_status(runner, &repo.primary_root, &args).await?;
    let mut deleted_branch = None;
    if delete_branch && let Some(branch) = info.branch {
        let delete_flag = if force { "-D" } else { "-d" };
        git_status(
            runner,
            &repo.primary_root,
            &["branch", delete_flag, &branch],
        )
        .await?;
        deleted_branch = Some(branch);
    }
    Ok(WorktreeRemoveResult {
        removed_path: info.worktree_git_root,
        deleted_branch,
    })
}

#[derive(Clone)]
struct RemoteSourceRepo {
    root: PathBuf,
    primary_root: PathBuf,
    relative_cwd: PathBuf,
    common_git_dir: PathBuf,
    repo_name: String,
    id: String,
}

impl RemoteSourceRepo {
    async fn resolve(runner: &WorkspaceCommandRunner, source_cwd: &Path) -> Result<Self> {
        let root =
            PathBuf::from(git_stdout(runner, source_cwd, &["rev-parse", "--show-toplevel"]).await?);
        let common_git_dir_raw =
            git_stdout(runner, source_cwd, &["rev-parse", "--git-common-dir"]).await?;
        let common_git_dir = absolutize(source_cwd, Path::new(&common_git_dir_raw));
        let primary_root = parse_worktree_list(
            &git_stdout(runner, &root, &["worktree", "list", "--porcelain"]).await?,
        )
        .into_iter()
        .next()
        .map(|entry| entry.path)
        .context("git did not report a primary worktree")?;
        let origin = git_stdout_result(runner, &root, &["remote", "get-url", "origin"]).await?;
        let origin = origin.success().then_some(origin.stdout.trim().to_string());
        let id = codex_worktree::repo_fingerprint(&common_git_dir, origin.as_deref());
        let repo_name = primary_root
            .file_name()
            .context("repository root has no directory name")?
            .to_string_lossy()
            .to_string();
        let relative_cwd = source_cwd
            .strip_prefix(&root)
            .unwrap_or_else(|_| Path::new(""))
            .to_path_buf();
        Ok(Self {
            root,
            primary_root,
            relative_cwd,
            common_git_dir,
            repo_name,
            id,
        })
    }
}

async fn info_from_existing_worktree(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    worktree_git_root: &Path,
    fallback_branch: Option<String>,
    repo: &RemoteSourceRepo,
) -> Result<WorktreeInfo> {
    let metadata = read_metadata(runner, request_handle, worktree_git_root).await?;
    let branch = git_stdout_result(
        runner,
        worktree_git_root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .await?
    .success()
    .then_some(async {
        git_stdout(
            runner,
            worktree_git_root,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
        )
        .await
    });
    let branch = match branch {
        Some(future) => Some(future.await?),
        None => fallback_branch,
    };
    let head = git_stdout_result(runner, worktree_git_root, &["rev-parse", "HEAD"])
        .await?
        .success()
        .then_some(async { git_stdout(runner, worktree_git_root, &["rev-parse", "HEAD"]).await });
    let head = match head {
        Some(future) => Some(future.await?),
        None => None,
    };
    let dirty = dirty_state(runner, worktree_git_root)
        .await
        .unwrap_or_default();
    let name = metadata
        .as_ref()
        .map(|metadata| metadata.name.clone())
        .or_else(|| branch.clone())
        .unwrap_or_else(|| repo.repo_name.clone());
    let slug = metadata
        .as_ref()
        .map(|metadata| metadata.slug.clone())
        .unwrap_or_else(|| name.replace(['/', '\\'], "-"));
    Ok(WorktreeInfo {
        id: metadata
            .as_ref()
            .map(|metadata| metadata.repo_id.clone())
            .unwrap_or_else(|| repo.id.clone()),
        name,
        slug,
        source: metadata
            .as_ref()
            .map(|metadata| metadata.source)
            .unwrap_or(WorktreeSource::Git),
        location: metadata
            .as_ref()
            .map(|metadata| metadata.location)
            .unwrap_or(WorktreeLocation::External),
        repo_name: repo.repo_name.clone(),
        repo_root: repo.root.clone(),
        common_git_dir: repo.common_git_dir.clone(),
        worktree_git_root: worktree_git_root.to_path_buf(),
        workspace_cwd: metadata
            .as_ref()
            .map(|metadata| metadata.workspace_cwd.clone())
            .unwrap_or_else(|| worktree_git_root.to_path_buf()),
        original_relative_cwd: metadata
            .as_ref()
            .map(|metadata| metadata.original_relative_cwd.clone())
            .unwrap_or_default(),
        branch,
        head,
        owner_thread_id: metadata.and_then(|metadata| metadata.owner_thread_id),
        metadata_path: metadata_path(runner, worktree_git_root, "codex-worktree.json").await?,
        dirty,
    })
}

async fn dirty_state(runner: &WorkspaceCommandRunner, root: &Path) -> Result<DirtyState> {
    Ok(DirtyState {
        has_staged_changes: !git_stdout(runner, root, &["diff", "--cached", "--name-only", "-z"])
            .await?
            .is_empty(),
        has_unstaged_changes: !git_stdout(runner, root, &["diff", "--name-only", "-z"])
            .await?
            .is_empty(),
        has_untracked_files: !git_stdout(
            runner,
            root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )
        .await?
        .is_empty(),
    })
}

async fn validate_dirty_policy_before_create(
    runner: &WorkspaceCommandRunner,
    root: &Path,
    policy: DirtyPolicy,
) -> Result<Vec<String>> {
    let state = dirty_state(runner, root).await?;
    if !state.is_dirty() {
        return Ok(Vec::new());
    }
    match policy {
        DirtyPolicy::Fail => anyhow::bail!(
            "source checkout has uncommitted changes; use --worktree-dirty ignore, copy-tracked, copy-all, move-tracked, or move-all"
        ),
        DirtyPolicy::Ignore => Ok(vec![
            "source checkout has uncommitted changes; the new worktree was created without them"
                .to_string(),
        ]),
        DirtyPolicy::CopyTracked | DirtyPolicy::MoveTracked if state.has_untracked_files => Ok(vec![
            "untracked files were left in the source checkout; use --worktree-dirty copy-all or move-all to carry them"
                .to_string(),
        ]),
        DirtyPolicy::CopyTracked
        | DirtyPolicy::CopyAll
        | DirtyPolicy::MoveTracked
        | DirtyPolicy::MoveAll => Ok(Vec::new()),
    }
}

async fn prepare_dirty_policy_after_create(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    source_root: &Path,
    worktree_root: &Path,
    policy: DirtyPolicy,
) -> Result<PreparedRemoteDirtyTransfer> {
    let state = dirty_state(runner, source_root).await?;
    if !state.is_dirty() {
        return Ok(PreparedRemoteDirtyTransfer { move_plan: None });
    }
    let plan = RemoteTransferPlan::capture(runner, source_root).await?;
    match policy {
        DirtyPolicy::Fail | DirtyPolicy::Ignore => {
            Ok(PreparedRemoteDirtyTransfer { move_plan: None })
        }
        DirtyPolicy::CopyTracked => {
            plan.apply_tracked_diff(runner, request_handle, worktree_root)
                .await?;
            Ok(PreparedRemoteDirtyTransfer { move_plan: None })
        }
        DirtyPolicy::CopyAll => {
            plan.apply_tracked_diff(runner, request_handle, worktree_root)
                .await?;
            plan.copy_untracked(request_handle, source_root, worktree_root)
                .await?;
            Ok(PreparedRemoteDirtyTransfer { move_plan: None })
        }
        DirtyPolicy::MoveTracked => {
            plan.apply_tracked_diff(runner, request_handle, worktree_root)
                .await?;
            Ok(PreparedRemoteDirtyTransfer {
                move_plan: Some(RemoteMovePlan {
                    transfer: plan,
                    move_untracked: false,
                }),
            })
        }
        DirtyPolicy::MoveAll => {
            plan.apply_tracked_diff(runner, request_handle, worktree_root)
                .await?;
            plan.copy_untracked(request_handle, source_root, worktree_root)
                .await?;
            Ok(PreparedRemoteDirtyTransfer {
                move_plan: Some(RemoteMovePlan {
                    transfer: plan,
                    move_untracked: true,
                }),
            })
        }
    }
}

async fn finalize_dirty_policy_after_create(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    source_root: &Path,
    prepared: PreparedRemoteDirtyTransfer,
) -> Result<()> {
    let Some(move_plan) = prepared.move_plan else {
        return Ok(());
    };
    move_plan
        .transfer
        .clean_source_after_move(
            runner,
            request_handle,
            source_root,
            move_plan.move_untracked,
        )
        .await
        .with_context(|| {
            "worktree already contains transferred changes, but failed to clean the source checkout after move"
        })
}

async fn rollback_failed_create(
    runner: &WorkspaceCommandRunner,
    repo_root: &Path,
    worktree_git_root: &Path,
    branch: &str,
    delete_branch: bool,
    err: anyhow::Error,
) -> anyhow::Error {
    let mut rollback_errors = Vec::new();
    let worktree_arg = worktree_git_root.to_string_lossy().to_string();
    if let Err(rollback_err) = git_status(
        runner,
        repo_root,
        &["worktree", "remove", "--force", &worktree_arg],
    )
    .await
    {
        rollback_errors.push(rollback_err.to_string());
    }
    if delete_branch
        && let Err(rollback_err) = git_status(runner, repo_root, &["branch", "-D", branch]).await
    {
        rollback_errors.push(rollback_err.to_string());
    }
    if rollback_errors.is_empty() {
        err
    } else {
        anyhow::anyhow!(
            "{err}; additionally failed to roll back newly-created worktree: {}",
            rollback_errors.join("; ")
        )
    }
}

struct PreparedRemoteDirtyTransfer {
    move_plan: Option<RemoteMovePlan>,
}

struct RemoteMovePlan {
    transfer: RemoteTransferPlan,
    move_untracked: bool,
}

struct RemoteTransferPlan {
    staged_diff: String,
    unstaged_diff: String,
    tracked_paths: Vec<PathBuf>,
    untracked_paths: Vec<PathBuf>,
}

impl RemoteTransferPlan {
    async fn capture(runner: &WorkspaceCommandRunner, source_root: &Path) -> Result<Self> {
        Ok(Self {
            staged_diff: git_stdout_raw_uncapped(
                runner,
                source_root,
                &["diff", "--cached", "--binary"],
            )
            .await?,
            unstaged_diff: git_stdout_raw_uncapped(runner, source_root, &["diff", "--binary"])
                .await?,
            tracked_paths: tracked_paths(runner, source_root).await?,
            untracked_paths: untracked_paths(runner, source_root).await?,
        })
    }

    async fn apply_tracked_diff(
        &self,
        runner: &WorkspaceCommandRunner,
        request_handle: &AppServerRequestHandle,
        worktree_root: &Path,
    ) -> Result<()> {
        if !self.staged_diff.is_empty() {
            apply_patch_file(
                runner,
                request_handle,
                worktree_root,
                "staged",
                &self.staged_diff,
                &["apply", "--index", "--binary"],
            )
            .await?;
        }
        if !self.unstaged_diff.is_empty() {
            apply_patch_file(
                runner,
                request_handle,
                worktree_root,
                "unstaged",
                &self.unstaged_diff,
                &["apply", "--binary"],
            )
            .await?;
        }
        Ok(())
    }

    async fn copy_untracked(
        &self,
        request_handle: &AppServerRequestHandle,
        source_root: &Path,
        worktree_root: &Path,
    ) -> Result<()> {
        for relative_path in &self.untracked_paths {
            if let Some(parent) = worktree_root.join(relative_path).parent() {
                create_directory(request_handle, parent).await?;
            }
            fs_copy(
                request_handle,
                &source_root.join(relative_path),
                &worktree_root.join(relative_path),
            )
            .await?;
        }
        Ok(())
    }

    async fn clean_source_after_move(
        &self,
        runner: &WorkspaceCommandRunner,
        request_handle: &AppServerRequestHandle,
        source_root: &Path,
        move_untracked: bool,
    ) -> Result<()> {
        if git_status_result(runner, source_root, &["rev-parse", "--verify", "HEAD"])
            .await?
            .success()
        {
            git_status(runner, source_root, &["reset", "--hard", "HEAD"]).await?;
        } else {
            git_status(runner, source_root, &["read-tree", "--empty"]).await?;
            for relative_path in &self.tracked_paths {
                fs_remove(request_handle, &source_root.join(relative_path)).await?;
            }
        }
        if move_untracked {
            for relative_path in &self.untracked_paths {
                fs_remove(request_handle, &source_root.join(relative_path)).await?;
            }
        }
        Ok(())
    }
}

async fn apply_patch_file(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    cwd: &Path,
    label: &str,
    contents: &str,
    git_args: &[&str],
) -> Result<()> {
    let patch_path = metadata_path(
        runner,
        cwd,
        &format!("codex-worktree-{label}-{}.patch", Uuid::new_v4()),
    )
    .await?;
    fs_write(request_handle, &patch_path, contents.as_bytes()).await?;
    let mut args = git_args.to_vec();
    let patch_arg = patch_path.to_string_lossy().to_string();
    args.push(&patch_arg);
    let result = git_status(runner, cwd, &args).await;
    let _ = fs_remove(request_handle, &patch_path).await;
    result
}

async fn tracked_paths(runner: &WorkspaceCommandRunner, root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = paths_from_nul_separated(
        &git_stdout(runner, root, &["diff", "--cached", "--name-only", "-z"]).await?,
    );
    paths.extend(paths_from_nul_separated(
        &git_stdout(runner, root, &["diff", "--name-only", "-z"]).await?,
    ));
    paths.sort();
    paths.dedup();
    Ok(paths)
}

async fn untracked_paths(runner: &WorkspaceCommandRunner, root: &Path) -> Result<Vec<PathBuf>> {
    Ok(paths_from_nul_separated(
        &git_stdout(
            runner,
            root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )
        .await?,
    ))
}

fn paths_from_nul_separated(output: &str) -> Vec<PathBuf> {
    output
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

async fn read_metadata(
    runner: &WorkspaceCommandRunner,
    request_handle: &AppServerRequestHandle,
    worktree_root: &Path,
) -> Result<Option<WorktreeMetadata>> {
    let path = metadata_path(runner, worktree_root, "codex-worktree.json").await?;
    if !path_exists(request_handle, &path).await? {
        return Ok(None);
    }
    let contents = fs_read(request_handle, &path).await?;
    Ok(Some(serde_json::from_slice(&contents)?))
}

async fn write_json<T: serde::Serialize>(
    request_handle: &AppServerRequestHandle,
    path: &Path,
    value: &T,
) -> Result<()> {
    fs_write(request_handle, path, &serde_json::to_vec_pretty(value)?).await
}

async fn metadata_path(
    runner: &WorkspaceCommandRunner,
    root: &Path,
    name: &str,
) -> Result<PathBuf> {
    Ok(absolutize(
        root,
        Path::new(&git_stdout(runner, root, &["rev-parse", "--git-path", name]).await?),
    ))
}

fn absolutize(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[derive(Debug)]
struct GitWorktreeEntry {
    path: PathBuf,
    branch: Option<String>,
}

fn parse_worktree_list(output: &str) -> Vec<GitWorktreeEntry> {
    let mut entries = Vec::new();
    let mut path = None;
    let mut branch = None;
    for line in output.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Some(path) = path.take() {
                entries.push(GitWorktreeEntry {
                    path,
                    branch: branch.take(),
                });
            }
            continue;
        }
        if let Some(raw_path) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(raw_path));
        } else if let Some(raw_branch) = line.strip_prefix("branch ") {
            branch = Some(raw_branch.trim_start_matches("refs/heads/").to_string());
        }
    }
    entries
}

async fn git_stdout(runner: &WorkspaceCommandRunner, cwd: &Path, args: &[&str]) -> Result<String> {
    let output = git_stdout_result(runner, cwd, args).await?;
    if !output.success() {
        anyhow::bail!("git {} failed: {}", args.join(" "), output.stderr.trim());
    }
    Ok(output.stdout.trim_end().to_string())
}

async fn git_stdout_raw_uncapped(
    runner: &WorkspaceCommandRunner,
    cwd: &Path,
    args: &[&str],
) -> Result<String> {
    let argv = std::iter::once("git")
        .chain(args.iter().copied())
        .collect::<Vec<_>>();
    let output = runner
        .run(WorkspaceCommand::new(argv).cwd(cwd).disable_output_cap())
        .await?;
    if !output.success() {
        anyhow::bail!("git {} failed: {}", args.join(" "), output.stderr.trim());
    }
    Ok(output.stdout)
}

async fn git_stdout_result(
    runner: &WorkspaceCommandRunner,
    cwd: &Path,
    args: &[&str],
) -> Result<crate::workspace_command::WorkspaceCommandOutput> {
    let argv = std::iter::once("git")
        .chain(args.iter().copied())
        .collect::<Vec<_>>();
    Ok(runner.run(WorkspaceCommand::new(argv).cwd(cwd)).await?)
}

async fn git_status(runner: &WorkspaceCommandRunner, cwd: &Path, args: &[&str]) -> Result<()> {
    let output = git_status_result(runner, cwd, args).await?;
    if !output.success() {
        anyhow::bail!("git {} failed: {}", args.join(" "), output.stderr.trim());
    }
    Ok(())
}

async fn git_status_result(
    runner: &WorkspaceCommandRunner,
    cwd: &Path,
    args: &[&str],
) -> Result<crate::workspace_command::WorkspaceCommandOutput> {
    git_stdout_result(runner, cwd, args).await
}

fn absolute_path(path: &Path) -> Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path(path).map_err(Into::into)
}

async fn path_exists(request_handle: &AppServerRequestHandle, path: &Path) -> Result<bool> {
    let result: Result<FsGetMetadataResponse, _> = request_handle
        .request_typed(ClientRequest::FsGetMetadata {
            request_id: RequestId::String(format!("worktree-fs-meta-{}", Uuid::new_v4())),
            params: FsGetMetadataParams {
                path: absolute_path(path)?,
            },
        })
        .await;
    Ok(result.is_ok())
}

async fn create_directory(request_handle: &AppServerRequestHandle, path: &Path) -> Result<()> {
    let _: FsCreateDirectoryResponse = request_handle
        .request_typed(ClientRequest::FsCreateDirectory {
            request_id: RequestId::String(format!("worktree-fs-mkdir-{}", Uuid::new_v4())),
            params: FsCreateDirectoryParams {
                path: absolute_path(path)?,
                recursive: Some(true),
            },
        })
        .await?;
    Ok(())
}

async fn fs_read(request_handle: &AppServerRequestHandle, path: &Path) -> Result<Vec<u8>> {
    let response: FsReadFileResponse = request_handle
        .request_typed(ClientRequest::FsReadFile {
            request_id: RequestId::String(format!("worktree-fs-read-{}", Uuid::new_v4())),
            params: FsReadFileParams {
                path: absolute_path(path)?,
            },
        })
        .await?;
    Ok(STANDARD.decode(response.data_base64)?)
}

async fn fs_write(
    request_handle: &AppServerRequestHandle,
    path: &Path,
    bytes: &[u8],
) -> Result<()> {
    let _: FsWriteFileResponse = request_handle
        .request_typed(ClientRequest::FsWriteFile {
            request_id: RequestId::String(format!("worktree-fs-write-{}", Uuid::new_v4())),
            params: FsWriteFileParams {
                path: absolute_path(path)?,
                data_base64: STANDARD.encode(bytes),
            },
        })
        .await?;
    Ok(())
}

async fn fs_copy(
    request_handle: &AppServerRequestHandle,
    source_path: &Path,
    destination_path: &Path,
) -> Result<()> {
    let _: FsCopyResponse = request_handle
        .request_typed(ClientRequest::FsCopy {
            request_id: RequestId::String(format!("worktree-fs-copy-{}", Uuid::new_v4())),
            params: FsCopyParams {
                source_path: absolute_path(source_path)?,
                destination_path: absolute_path(destination_path)?,
                recursive: false,
            },
        })
        .await?;
    Ok(())
}

async fn fs_remove(request_handle: &AppServerRequestHandle, path: &Path) -> Result<()> {
    let _: FsRemoveResponse = request_handle
        .request_typed(ClientRequest::FsRemove {
            request_id: RequestId::String(format!("worktree-fs-remove-{}", Uuid::new_v4())),
            params: FsRemoveParams {
                path: absolute_path(path)?,
                recursive: Some(false),
                force: Some(true),
            },
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::Mutex;

    use pretty_assertions::assert_eq;

    use super::git_stdout_raw_uncapped;
    use crate::workspace_command::WorkspaceCommand;
    use crate::workspace_command::WorkspaceCommandError;
    use crate::workspace_command::WorkspaceCommandExecutor;
    use crate::workspace_command::WorkspaceCommandOutput;
    use crate::workspace_command::WorkspaceCommandRunner;

    struct FakeRunner {
        seen: Mutex<Vec<WorkspaceCommand>>,
        output: WorkspaceCommandOutput,
    }

    impl WorkspaceCommandExecutor for FakeRunner {
        fn run(
            &self,
            command: WorkspaceCommand,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<WorkspaceCommandOutput, WorkspaceCommandError>>
                    + Send
                    + '_,
            >,
        > {
            self.seen.lock().expect("seen lock").push(command);
            Box::pin(async move { Ok(self.output.clone()) })
        }
    }

    #[tokio::test]
    async fn raw_git_stdout_preserves_patch_newline_and_disables_output_cap() {
        let fake_runner = Arc::new(FakeRunner {
            seen: Mutex::new(Vec::new()),
            output: WorkspaceCommandOutput {
                exit_code: 0,
                stdout: "diff --git a/file b/file\n".to_string(),
                stderr: String::new(),
            },
        });
        let runner: WorkspaceCommandRunner = fake_runner.clone();

        let output = git_stdout_raw_uncapped(
            &runner,
            &PathBuf::from("/repo"),
            &["diff", "--cached", "--binary"],
        )
        .await
        .expect("raw git stdout");

        assert_eq!(output, "diff --git a/file b/file\n");
        let seen = fake_runner.seen.lock().expect("seen lock");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].argv, vec!["git", "diff", "--cached", "--binary"]);
        assert_eq!(seen[0].cwd, Some(PathBuf::from("/repo")));
        assert!(seen[0].disable_output_cap);
    }
}
