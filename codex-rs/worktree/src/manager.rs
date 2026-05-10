use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

use crate::WorktreeInfo;
use crate::WorktreeListQuery;
use crate::WorktreeLocation;
use crate::WorktreeRemoveRequest;
use crate::WorktreeRemoveResult;
use crate::WorktreeRequest;
use crate::WorktreeResolution;
use crate::WorktreeSource;
use crate::WorktreeWarning;
use crate::dirty;
use crate::git;
use crate::metadata;
use crate::metadata::WorktreeMetadata;
use crate::paths;

pub fn ensure_worktree(req: WorktreeRequest) -> Result<WorktreeResolution> {
    let repo = SourceRepo::resolve(&req.source_cwd)?;
    let branch = req.branch.clone();
    let slug = paths::slugify_name(&branch)?;
    ensure_safe_branch_name(&repo.root, &branch)?;
    let worktree_git_root = paths::sibling_worktree_git_root(&repo.primary_root, &branch)?;
    let workspace_cwd = worktree_git_root.join(&repo.relative_cwd);

    if worktree_git_root.exists() {
        let Some(metadata) = metadata::read_worktree_metadata(&worktree_git_root)? else {
            anyhow::bail!(
                "managed worktree path {} already exists but is not owned by Codex",
                worktree_git_root.display()
            );
        };
        ensure_existing_worktree_matches_branch(&worktree_git_root, &metadata, &branch)?;
        let info = info_from_existing_worktree(
            &req.codex_home,
            &worktree_git_root,
            Some(branch),
            Some(slug),
        )?;
        return Ok(WorktreeResolution {
            reused: true,
            info,
            warnings: Vec::new(),
        });
    }

    if let Some(path) = branch_checkout_path(&repo.root, &branch)?
        && path != worktree_git_root
    {
        anyhow::bail!(
            "branch {branch} is already checked out at {}; remove that worktree first",
            path.display()
        );
    }

    let warnings = dirty::validate_dirty_policy_before_create(&repo.root, req.dirty_policy)?;
    let branch_exists = branch_exists(&repo.root, &branch);
    let has_head = git::status(&repo.root, &["rev-parse", "--verify", "HEAD"]).is_ok();
    fs::create_dir_all(
        worktree_git_root
            .parent()
            .context("managed worktree path has no parent")?,
    )?;
    if branch_exists {
        git::status(
            &repo.root,
            &[
                "worktree",
                "add",
                &worktree_git_root.to_string_lossy(),
                &branch,
            ],
        )?;
    } else if req.base_ref.is_none() && !has_head {
        git::status(
            &repo.root,
            &[
                "worktree",
                "add",
                "--orphan",
                "-b",
                &branch,
                &worktree_git_root.to_string_lossy(),
            ],
        )?;
    } else {
        let base_ref = req.base_ref.as_deref().unwrap_or("HEAD");
        git::status(
            &repo.root,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                &worktree_git_root.to_string_lossy(),
                base_ref,
            ],
        )?;
    }

    let prepared_transfer = match dirty::prepare_dirty_policy_after_create(
        &repo.root,
        &worktree_git_root,
        req.dirty_policy,
    ) {
        Ok(prepared_transfer) => prepared_transfer,
        Err(err) => {
            return Err(rollback_failed_create(
                &repo.root,
                &worktree_git_root,
                &branch,
                /*delete_branch*/ !branch_exists,
                err,
            ));
        }
    };
    dirty::finalize_dirty_policy_after_create(&repo.root, prepared_transfer)?;
    let dirty = dirty::dirty_state(&worktree_git_root)?;
    let head = git::stdout(&worktree_git_root, &["rev-parse", "HEAD"]).ok();
    let mut info = WorktreeInfo {
        id: repo.id.clone(),
        name: branch.clone(),
        slug,
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
        metadata_path: metadata_path_for_display(&worktree_git_root)?,
        dirty,
    };
    metadata::write_pending_owner_metadata(&worktree_git_root)?;
    let worktree_metadata = WorktreeMetadata::from_info(&info, repo.root);
    metadata::write_worktree_metadata(&worktree_git_root, &worktree_metadata)?;
    info.owner_thread_id = worktree_metadata.owner_thread_id;
    Ok(WorktreeResolution {
        reused: false,
        info,
        warnings: warnings
            .into_iter()
            .map(|message| WorktreeWarning { message })
            .collect(),
    })
}

fn rollback_failed_create(
    repo_root: &Path,
    worktree_git_root: &Path,
    branch: &str,
    delete_branch: bool,
    err: anyhow::Error,
) -> anyhow::Error {
    let mut rollback_errors = Vec::new();
    let worktree_arg = worktree_git_root.to_string_lossy();
    if let Err(rollback_err) =
        git::status(repo_root, &["worktree", "remove", "--force", &worktree_arg])
    {
        rollback_errors.push(rollback_err.to_string());
    }
    if delete_branch && let Err(rollback_err) = git::status(repo_root, &["branch", "-D", branch]) {
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

pub fn resolve_worktree(codex_home: &Path, cwd: &Path) -> Result<Option<WorktreeInfo>> {
    let Ok(root) = git::stdout(cwd, &["rev-parse", "--show-toplevel"]) else {
        return Ok(None);
    };
    let root = PathBuf::from(root);
    if !paths::is_managed_worktree_path(&root, codex_home)
        && metadata::read_worktree_metadata(&root)?.is_none()
    {
        return Ok(None);
    }
    Ok(Some(info_from_existing_worktree(
        codex_home, &root, /*fallback_name*/ None, /*fallback_slug*/ None,
    )?))
}

pub fn list_worktrees(query: WorktreeListQuery) -> Result<Vec<WorktreeInfo>> {
    let repo_filter = if query.include_all_repos {
        None
    } else {
        let source_cwd = query
            .source_cwd
            .as_ref()
            .context("source cwd is required unless include_all_repos is true")?;
        Some(SourceRepo::resolve(source_cwd)?)
    };
    let mut entries = Vec::new();
    if let Some(repo_filter) = repo_filter.as_ref() {
        for worktree in parse_worktree_list(&git::stdout(
            &repo_filter.root,
            &["worktree", "list", "--porcelain"],
        )?) {
            let Ok(info) = info_from_existing_worktree(
                &query.codex_home,
                worktree.path.as_path(),
                worktree.branch.clone(),
                worktree
                    .branch
                    .as_deref()
                    .and_then(|branch| paths::slugify_name(branch).ok()),
            ) else {
                continue;
            };
            if worktree_matches_repo(&info, repo_filter) {
                entries.push(info);
            }
        }
    }

    let root = paths::codex_worktrees_root(&query.codex_home);
    if root.exists() {
        for worktree_root in discover_codex_home_worktree_roots(&root)? {
            let Ok(info) = info_from_existing_worktree(
                &query.codex_home,
                worktree_root.as_path(),
                /*fallback_name*/ None,
                /*fallback_slug*/ None,
            ) else {
                continue;
            };
            if let Some(repo_filter) = repo_filter.as_ref()
                && !worktree_matches_repo(&info, repo_filter)
            {
                continue;
            }
            entries.push(info);
        }
    }
    let mut unique_entries = Vec::new();
    for entry in entries {
        if unique_entries.iter().any(|existing: &WorktreeInfo| {
            paths_match(&existing.worktree_git_root, &entry.worktree_git_root)
        }) {
            continue;
        }
        unique_entries.push(entry);
    }
    let mut entries = unique_entries;
    entries.sort_by(|a, b| {
        display_branch_or_name(a)
            .cmp(display_branch_or_name(b))
            .then_with(|| a.worktree_git_root.cmp(&b.worktree_git_root))
    });
    Ok(entries)
}

fn discover_codex_home_worktree_roots(root: &Path) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for parent in fs::read_dir(root)? {
        let parent = parent?;
        if !parent.file_type()?.is_dir() {
            continue;
        }
        let parent_path = parent.path();
        if is_git_root(&parent_path) {
            roots.push(parent_path);
            continue;
        }
        for child in fs::read_dir(parent_path)? {
            let child = child?;
            if !child.file_type()?.is_dir() {
                continue;
            }
            let child_path = child.path();
            if is_git_root(&child_path) {
                roots.push(child_path);
                continue;
            }
            for grandchild in fs::read_dir(child_path)? {
                let grandchild = grandchild?;
                if !grandchild.file_type()?.is_dir() {
                    continue;
                }
                let grandchild_path = grandchild.path();
                if is_git_root(&grandchild_path) {
                    roots.push(grandchild_path);
                }
            }
        }
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn is_git_root(path: &Path) -> bool {
    path.join(".git").exists()
}

pub fn stale_managed_worktree_dirs(codex_home: &Path) -> Result<Vec<PathBuf>> {
    let root = paths::codex_worktrees_root(codex_home);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut stale = Vec::new();
    for repo_dir in fs::read_dir(&root)? {
        let repo_dir = repo_dir?;
        if !repo_dir.file_type()?.is_dir() {
            continue;
        }
        for slug_dir in fs::read_dir(repo_dir.path())? {
            let slug_dir = slug_dir?;
            if !slug_dir.file_type()?.is_dir() {
                continue;
            }
            let mut has_repo_dir = false;
            for repo_root in fs::read_dir(slug_dir.path())? {
                let repo_root = repo_root?;
                if !repo_root.file_type()?.is_dir() {
                    continue;
                }
                has_repo_dir = true;
                if !repo_root.path().join(".git").exists() || !git_root_is_valid(&repo_root.path())
                {
                    stale.push(repo_root.path());
                }
            }
            if !has_repo_dir {
                stale.push(slug_dir.path());
            }
        }
    }
    stale.sort();
    Ok(stale)
}

pub fn prune_stale_managed_worktree_dirs(codex_home: &Path, dry_run: bool) -> Result<Vec<PathBuf>> {
    let stale = stale_managed_worktree_dirs(codex_home)?;
    if dry_run {
        return Ok(stale);
    }

    for path in &stale {
        fs::remove_dir_all(path).with_context(|| {
            format!(
                "failed to remove stale worktree directory {}",
                path.display()
            )
        })?;
    }
    Ok(stale)
}

fn git_root_is_valid(path: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn worktree_matches_repo(info: &WorktreeInfo, repo: &SourceRepo) -> bool {
    info.id == repo.id || paths_match(&info.common_git_dir, &repo.common_git_dir)
}

fn paths_match(a: &Path, b: &Path) -> bool {
    let a = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let b = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    a == b
}

pub fn remove_worktree(req: WorktreeRemoveRequest) -> Result<WorktreeRemoveResult> {
    let target = target_worktree_path(&req)?;
    let metadata = metadata::read_worktree_metadata(&target)?
        .context("refusing to remove a worktree not managed by Codex")?;
    let dirty = dirty::dirty_state(&target)?;
    if dirty.is_dirty() && !req.force {
        anyhow::bail!(
            "refusing to remove dirty worktree {}; use --force to override",
            target.display()
        );
    }
    let branch = current_branch(&target)?;
    let mut args = vec!["worktree", "remove"];
    if req.force {
        args.push("--force");
    }
    let target_arg = target.to_string_lossy();
    args.push(&target_arg);
    let primary_root = primary_worktree_root(&target)?;
    git::status(&primary_root, &args)?;

    let mut deleted_branch = None;
    if req.delete_branch
        && let Some(branch) = branch
    {
        if req.force {
            git::status(&primary_root, &["branch", "-D", &branch])?;
        } else {
            git::status(&primary_root, &["branch", "-d", &branch])?;
        }
        deleted_branch = Some(branch);
    }

    if metadata.location == WorktreeLocation::CodexHome
        && let Some(parent) = metadata.worktree_git_root.parent()
        && parent.exists()
        && parent.read_dir()?.next().is_none()
    {
        fs::remove_dir(parent)?;
    }

    Ok(WorktreeRemoveResult {
        removed_path: target,
        deleted_branch,
    })
}

fn ensure_existing_worktree_matches_branch(
    worktree_git_root: &Path,
    metadata: &WorktreeMetadata,
    requested_branch: &str,
) -> Result<()> {
    if metadata.branch.as_deref() == Some(requested_branch) || metadata.name == requested_branch {
        return Ok(());
    }
    if current_branch(worktree_git_root)?.as_deref() == Some(requested_branch) {
        return Ok(());
    }
    anyhow::bail!(
        "managed worktree path {} is already used by {}; choose a different branch name",
        worktree_git_root.display(),
        metadata.branch.as_deref().unwrap_or(metadata.name.as_str())
    )
}

fn target_worktree_path(req: &WorktreeRemoveRequest) -> Result<PathBuf> {
    let raw = PathBuf::from(&req.name_or_path);
    if raw.is_absolute() {
        return Ok(raw);
    }
    let entries = list_worktrees(WorktreeListQuery {
        codex_home: req.codex_home.clone(),
        source_cwd: req.source_cwd.clone(),
        include_all_repos: req.source_cwd.is_none(),
    })?;
    let matches = entries
        .into_iter()
        .filter(|entry| {
            entry.branch.as_deref() == Some(req.name_or_path.as_str())
                || entry.name == req.name_or_path
                || entry.slug == req.name_or_path
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [entry] => Ok(entry.worktree_git_root.clone()),
        [] => anyhow::bail!("no managed worktree named {}", req.name_or_path),
        _ => anyhow::bail!(
            "multiple managed worktrees named {}; pass a path instead",
            req.name_or_path
        ),
    }
}

fn info_from_existing_worktree(
    codex_home: &Path,
    worktree_git_root: &Path,
    fallback_name: Option<String>,
    fallback_slug: Option<String>,
) -> Result<WorktreeInfo> {
    let metadata = metadata::read_worktree_metadata(worktree_git_root)?;
    let root = git::stdout(worktree_git_root, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|_| worktree_git_root.to_path_buf());
    let common_git_dir = git::stdout(worktree_git_root, &["rev-parse", "--git-common-dir"])
        .map(|value| absolutize(worktree_git_root, Path::new(&value)))
        .unwrap_or_else(|_| PathBuf::new());
    let branch = current_branch(worktree_git_root)?;
    let head = git::stdout(worktree_git_root, &["rev-parse", "HEAD"]).ok();
    let dirty = dirty::dirty_state(worktree_git_root).unwrap_or_default();
    let (source, location) = classify_worktree(codex_home, worktree_git_root, metadata.as_ref());
    let repo_name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    let id = metadata
        .as_ref()
        .map(|metadata| metadata.repo_id.clone())
        .unwrap_or_else(|| {
            root.strip_prefix(paths::codex_worktrees_root(codex_home))
                .ok()
                .and_then(|path| path.components().next())
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .unwrap_or_default()
        });
    let name = metadata
        .as_ref()
        .map(|metadata| metadata.name.clone())
        .or(fallback_name)
        .or_else(|| branch.clone())
        .unwrap_or_else(|| repo_name.clone());
    let slug = metadata
        .as_ref()
        .map(|metadata| metadata.slug.clone())
        .or(fallback_slug)
        .unwrap_or_else(|| paths::slugify_name(&name).unwrap_or_else(|_| name.clone()));
    let workspace_cwd = metadata
        .as_ref()
        .map(|metadata| metadata.workspace_cwd.clone())
        .unwrap_or_else(|| root.clone());
    let original_relative_cwd = metadata
        .as_ref()
        .map(|metadata| metadata.original_relative_cwd.clone())
        .unwrap_or_default();
    Ok(WorktreeInfo {
        id,
        name,
        slug,
        source,
        location,
        repo_name,
        repo_root: root,
        common_git_dir,
        worktree_git_root: worktree_git_root.to_path_buf(),
        workspace_cwd,
        original_relative_cwd,
        branch,
        head,
        owner_thread_id: metadata.and_then(|metadata| metadata.owner_thread_id),
        metadata_path: metadata_path_for_display(worktree_git_root)?,
        dirty,
    })
}

fn classify_worktree(
    codex_home: &Path,
    worktree_git_root: &Path,
    metadata: Option<&WorktreeMetadata>,
) -> (WorktreeSource, WorktreeLocation) {
    if let Some(metadata) = metadata {
        return (metadata.source, metadata.location);
    }
    if paths::is_managed_worktree_path(worktree_git_root, codex_home) {
        return (WorktreeSource::App, WorktreeLocation::CodexHome);
    }
    (WorktreeSource::Git, WorktreeLocation::External)
}

fn display_branch_or_name(info: &WorktreeInfo) -> &str {
    info.branch.as_deref().unwrap_or(&info.name)
}

struct SourceRepo {
    root: PathBuf,
    primary_root: PathBuf,
    relative_cwd: PathBuf,
    common_git_dir: PathBuf,
    repo_name: String,
    id: String,
}

impl SourceRepo {
    fn resolve(source_cwd: &Path) -> Result<Self> {
        let source_cwd = source_cwd
            .canonicalize()
            .unwrap_or_else(|_| source_cwd.to_path_buf());
        let root = PathBuf::from(git::stdout(&source_cwd, &["rev-parse", "--show-toplevel"])?);
        let root = root.canonicalize().unwrap_or(root);
        let common_git_dir_raw = git::stdout(&source_cwd, &["rev-parse", "--git-common-dir"])?;
        let common_git_dir = absolutize(&source_cwd, Path::new(&common_git_dir_raw))
            .canonicalize()
            .unwrap_or_else(|_| absolutize(&source_cwd, Path::new(&common_git_dir_raw)));
        let primary_root = primary_worktree_root(&root)
            .unwrap_or_else(|_| root.clone())
            .canonicalize()
            .unwrap_or_else(|_| root.clone());
        let origin = git::stdout(&root, &["remote", "get-url", "origin"]).ok();
        let id = paths::repo_fingerprint(&common_git_dir, origin.as_deref());
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

fn branch_checkout_path(root: &Path, branch: &str) -> Result<Option<PathBuf>> {
    let worktrees = parse_worktree_list(&git::stdout(root, &["worktree", "list", "--porcelain"])?);
    Ok(worktrees
        .into_iter()
        .find_map(|entry| (entry.branch.as_deref() == Some(branch)).then_some(entry.path)))
}

fn branch_exists(root: &Path, branch: &str) -> bool {
    git::status(
        root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

fn ensure_safe_branch_name(root: &Path, branch: &str) -> Result<()> {
    if branch.trim().is_empty() {
        anyhow::bail!("branch name must not be empty");
    }
    git::status(root, &["check-ref-format", "--branch", branch]).context("invalid branch name")
}

fn current_branch(root: &Path) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(root)
        .output()?;
    if output.status.success() {
        let branch = String::from_utf8(output.stdout)?.trim().to_string();
        Ok((!branch.is_empty()).then_some(branch))
    } else {
        Ok(None)
    }
}

fn primary_worktree_root(root: &Path) -> Result<PathBuf> {
    let worktrees = parse_worktree_list(&git::stdout(root, &["worktree", "list", "--porcelain"])?);
    worktrees
        .into_iter()
        .next()
        .map(|entry| entry.path)
        .context("git did not report a primary worktree")
}

fn metadata_path_for_display(worktree_path: &Path) -> Result<PathBuf> {
    let path = git::stdout(
        worktree_path,
        &["rev-parse", "--git-path", "codex-worktree.json"],
    )?;
    Ok(absolutize(worktree_path, Path::new(&path)))
}

fn absolutize(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[derive(Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn parse_worktree_list_preserves_branches() {
        let entries = parse_worktree_list(
            "worktree /repo\nHEAD abc\nbranch refs/heads/main\n\nworktree /repo.wt\nHEAD def\nbranch refs/heads/codex/demo\n\n",
        );
        assert_eq!(
            entries,
            vec![
                GitWorktreeEntry {
                    path: PathBuf::from("/repo"),
                    branch: Some("main".to_string())
                },
                GitWorktreeEntry {
                    path: PathBuf::from("/repo.wt"),
                    branch: Some("codex/demo".to_string())
                }
            ]
        );
    }

    #[test]
    fn prune_stale_managed_worktree_dirs_respects_dry_run() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let stale_path = paths::codex_worktrees_root(codex_home.path())
            .join("repo-id")
            .join("demo")
            .join("codex.demo");
        fs::create_dir_all(&stale_path)?;

        assert_eq!(
            prune_stale_managed_worktree_dirs(codex_home.path(), /*dry_run*/ true)?,
            vec![stale_path.clone()]
        );
        assert!(stale_path.exists());

        assert_eq!(
            prune_stale_managed_worktree_dirs(codex_home.path(), /*dry_run*/ false)?,
            vec![stale_path.clone()]
        );
        assert!(!stale_path.exists());
        Ok(())
    }
}
