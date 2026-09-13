mod git;
mod metadata;
mod paths;
mod settings;

use crate::git::GitOperation;
use crate::git::git_output;
use crate::git::git_path;
use crate::git::git_path_from_bytes;
use crate::git::git_stdout;
use crate::paths::allocate_worktree_root;
use crate::paths::remove_empty_bucket;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Serialize;
use std::ffi::OsStr;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

pub use git::default_worktree_base;
pub use settings::DEFAULT_WORKTREE_KEEP_COUNT;
pub use settings::WorktreeSettings;

/// A Desktop-compatible checkout and the cwd that should be used to start its thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedWorktree {
    pub root: PathBuf,
    pub cwd: PathBuf,
    pub source_root: PathBuf,
    pub source_cwd: PathBuf,
    pub head_sha: String,
    /// A UTF-8 branch label, when available; absence does not imply detached HEAD.
    pub branch: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateWorktree {
    pub source_cwd: PathBuf,
    pub base: Option<String>,
}

/// Creates and identifies worktrees using the existing Codex Desktop contract.
#[derive(Clone, Debug)]
pub struct WorktreeManager {
    settings: WorktreeSettings,
}

impl WorktreeManager {
    pub fn new(mut settings: WorktreeSettings) -> Self {
        settings.root = dunce::simplified(&settings.root).to_path_buf();
        Self { settings }
    }

    pub fn settings(&self) -> &WorktreeSettings {
        &self.settings
    }

    pub fn create(&self, request: &CreateWorktree) -> Result<ManagedWorktree> {
        if !self.settings.root.is_absolute() {
            bail!("managed worktree root must be an absolute path");
        }

        let source_cwd = dunce::canonicalize(&request.source_cwd)
            .with_context(|| format!("cannot resolve {}", request.source_cwd.display()))?;
        let source_root = repository_root(&source_cwd)?;
        let relative_cwd = source_cwd
            .strip_prefix(&source_root)
            .context("working directory is outside the repository root")?;
        let repository_name = source_root
            .file_name()
            .context("repository root has no directory name")?;
        let revision = format!("{}^{{commit}}", request.base.as_deref().unwrap_or("HEAD"));
        let head_sha = git_stdout(
            &source_root,
            [
                "rev-parse",
                "--verify",
                "--end-of-options",
                revision.as_str(),
            ],
        )?;
        let root = allocate_worktree_root(&self.settings.root, repository_name)?;

        let result = git_output(
            &source_root,
            GitOperation::WorkingTree,
            [
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("--detach"),
                OsStr::new("--no-checkout"),
                root.as_os_str(),
                OsStr::new(&head_sha),
            ],
        );
        if let Err(error) = result {
            remove_empty_bucket(&root);
            return Err(error).context("cannot create managed worktree");
        }

        // Write only the destination's config, without changing shared settings or
        // enabling worktreeConfig for the source repository.
        let result = git_path(
            &root,
            [
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "config.worktree",
            ],
        )
        .and_then(|config| {
            git_output(
                &root,
                GitOperation::Metadata,
                [
                    OsStr::new("config"),
                    OsStr::new("--file"),
                    config.as_os_str(),
                    OsStr::new("core.worktree"),
                    root.as_os_str(),
                ],
            )?;
            // Discover destination-only filters before materializing files, and pin
            // the working tree even when per-worktree configuration is disabled.
            git_output(
                &root,
                GitOperation::WorkingTree,
                [
                    "--work-tree=.",
                    "reset",
                    "--hard",
                    "--no-recurse-submodules",
                    head_sha.as_str(),
                ],
            )
        });
        if let Err(error) = result {
            remove_worktree(&source_root, &root)
                .context("cannot roll back an incomplete managed worktree")?;
            return Err(error).context("cannot populate managed worktree");
        }

        let cwd = root.join(relative_cwd);
        if !is_safe_worktree_cwd(&root, &cwd) {
            remove_worktree(&source_root, &root)
                .context("cannot roll back a worktree missing its working directory")?;
            bail!(
                "requested base does not contain a safe working directory {}",
                relative_cwd.display()
            );
        }

        Ok(ManagedWorktree {
            cwd,
            root,
            source_root,
            source_cwd,
            head_sha,
            branch: None,
        })
    }

    pub fn list(&self, source_cwd: &Path) -> Result<Vec<ManagedWorktree>> {
        let source_cwd = dunce::canonicalize(source_cwd)
            .with_context(|| format!("cannot resolve {}", source_cwd.display()))?;
        let source_root = repository_root(&source_cwd)?;
        let relative_cwd = source_cwd
            .strip_prefix(&source_root)
            .context("working directory is outside the repository root")?;
        let managed_root =
            dunce::canonicalize(&self.settings.root).unwrap_or_else(|_| self.settings.root.clone());
        let source_common_dir = resolve_git_path(&source_root, "--git-common-dir")?;
        let output = git_output(
            &source_cwd,
            GitOperation::Metadata,
            ["worktree", "list", "--porcelain", "-z"],
        )?;
        let fields = output
            .stdout
            .split(|byte| *byte == b'\0')
            .collect::<Vec<_>>();
        let mut worktrees = Vec::new();

        for entry in fields.split(|field| field.is_empty()) {
            let mut root = None;
            let mut head_sha = None;
            let mut branch = None;

            for field in entry {
                if let Some(path) = field.strip_prefix(b"worktree ") {
                    root = Some(git_path_from_bytes(path)?);
                } else if let Some(head) = field.strip_prefix(b"HEAD ") {
                    head_sha = Some(
                        std::str::from_utf8(head)
                            .context("worktree HEAD is not valid UTF-8")?
                            .to_owned(),
                    );
                } else if let Some(name) = field.strip_prefix(b"branch ") {
                    branch = Some(name.strip_prefix(b"refs/heads/").unwrap_or(name));
                }
            }

            let Some(root) = root else {
                continue;
            };
            let Some(bucket) = root.parent() else {
                continue;
            };
            // Keep aliases from changing which registration supplies the metadata.
            if ![root.as_path(), bucket].into_iter().all(|path| {
                fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
            }) {
                continue;
            }
            let canonical_root = dunce::canonicalize(&root).unwrap_or_else(|_| root.clone());
            let linked_git_file = fs::symlink_metadata(canonical_root.join(".git"))
                .is_ok_and(|metadata| metadata.file_type().is_file());
            if !has_managed_layout(&managed_root, &canonical_root) || !linked_git_file {
                continue;
            }
            if linked_worktree_common_dir(&canonical_root).ok().as_ref() != Some(&source_common_dir)
            {
                continue;
            }
            // A different linked checkout can occupy a stale registration's path.
            // Require its administration directory to point back to this checkout.
            let Ok(git_dir) = resolve_git_path(&canonical_root, "--git-dir") else {
                continue;
            };
            let Ok(backlink) = fs::read(git_dir.join("gitdir")) else {
                continue;
            };
            let backlink = backlink.strip_suffix(b"\n").unwrap_or(&backlink);
            #[cfg(windows)]
            let backlink = backlink.strip_suffix(b"\r").unwrap_or(backlink);
            let Ok(backlink) = git_path_from_bytes(backlink) else {
                continue;
            };
            let Ok(backlink) = dunce::canonicalize(git_dir.join(backlink)) else {
                continue;
            };
            let Ok(git_file) = dunce::canonicalize(canonical_root.join(".git")) else {
                continue;
            };
            if backlink != git_file {
                continue;
            }
            let cwd = root.join(relative_cwd);
            if !is_safe_worktree_cwd(&canonical_root, &cwd) {
                continue;
            }

            let head_sha = head_sha
                .with_context(|| format!("managed worktree {} has no HEAD", root.display()))?;
            worktrees.push(ManagedWorktree {
                cwd,
                root,
                source_root: source_root.clone(),
                source_cwd: source_cwd.clone(),
                head_sha,
                branch: branch.and_then(|name| std::str::from_utf8(name).ok().map(str::to_owned)),
            });
        }

        worktrees.sort_by(|left, right| left.root.cmp(&right.root));
        Ok(worktrees)
    }

    pub fn bind_thread(&self, checkout: &Path, thread_id: &str) -> Result<()> {
        let checkout = self.managed_checkout(checkout)?;
        metadata::bind_thread(&checkout, thread_id)
    }

    pub fn owner(&self, checkout: &Path) -> Result<Option<String>> {
        let checkout = self.managed_checkout(checkout)?;
        metadata::owner(&checkout)
    }

    /// Remove a currently registered checkout from this repository, refusing local changes.
    pub fn remove(&self, source_cwd: &Path, root: &Path) -> Result<()> {
        let checkout = self
            .list(source_cwd)?
            .into_iter()
            .find(|checkout| checkout.root == root)
            .with_context(|| {
                format!(
                    "{} is not a managed worktree in this repository",
                    root.display()
                )
            })?;
        let source_cwd = dunce::canonicalize(source_cwd).with_context(|| {
            format!("cannot resolve current directory {}", source_cwd.display())
        })?;
        let checkout_root = dunce::canonicalize(&checkout.root)
            .with_context(|| format!("cannot resolve worktree {}", checkout.root.display()))?;
        if source_cwd.starts_with(&checkout_root) {
            bail!("switch to another checkout before deleting the current worktree");
        }
        let ignored = git_output(
            &checkout.root,
            GitOperation::WorkingTree,
            [
                OsStr::new("ls-files"),
                OsStr::new("--others"),
                OsStr::new("--ignored"),
                OsStr::new("--exclude-standard"),
                OsStr::new("-z"),
            ],
        )?;
        if !ignored.stdout.is_empty() {
            bail!("worktree contains ignored local files; remove them before deleting it");
        }
        git_output(
            &checkout.source_root,
            GitOperation::WorkingTree,
            [
                OsStr::new("worktree"),
                OsStr::new("remove"),
                root.as_os_str(),
            ],
        )?;
        remove_empty_bucket(root);
        Ok(())
    }

    fn managed_checkout(&self, checkout: &Path) -> Result<PathBuf> {
        let managed_root = dunce::canonicalize(&self.settings.root).with_context(|| {
            format!(
                "cannot resolve managed worktree root {}",
                self.settings.root.display()
            )
        })?;
        let checkout = dunce::canonicalize(checkout)
            .with_context(|| format!("cannot resolve worktree {}", checkout.display()))?;
        if !has_managed_layout(&managed_root, &checkout) {
            bail!("{} is not a managed worktree", checkout.display());
        }

        linked_worktree_common_dir(&checkout)?;
        Ok(checkout)
    }
}

fn remove_worktree(source_root: &Path, root: &Path) -> Result<()> {
    let result = git_output(
        source_root,
        GitOperation::WorkingTree,
        [
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
            root.as_os_str(),
        ],
    );
    remove_empty_bucket(root);
    result.map(|_| ())
}

fn is_safe_worktree_cwd(root: &Path, cwd: &Path) -> bool {
    let Ok(root) = dunce::canonicalize(root) else {
        return false;
    };
    dunce::canonicalize(cwd).is_ok_and(|resolved| resolved.is_dir() && resolved.starts_with(&root))
}

fn linked_worktree_common_dir(checkout: &Path) -> Result<PathBuf> {
    if checkout != repository_root(checkout)? {
        bail!("{} is not a worktree root", checkout.display());
    }
    let git_dir = resolve_git_path(checkout, "--git-dir")?;
    let common_dir = resolve_git_path(checkout, "--git-common-dir")?;
    if git_dir == common_dir {
        bail!("{} is not a linked worktree", checkout.display());
    }
    Ok(common_dir)
}

fn has_managed_layout(root: &Path, checkout: &Path) -> bool {
    let Ok(relative) = checkout.strip_prefix(root) else {
        return false;
    };
    let mut components = relative.components();
    let Some(Component::Normal(bucket)) = components.next() else {
        return false;
    };
    let Some(bucket) = bucket.to_str() else {
        return false;
    };
    bucket.len() == 4
        && bucket.bytes().all(|byte| byte.is_ascii_hexdigit())
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
}

fn resolve_git_path(checkout: &Path, argument: &str) -> Result<PathBuf> {
    let path = checkout.join(git_path(checkout, ["rev-parse", argument])?);
    dunce::canonicalize(&path)
        .with_context(|| format!("cannot resolve Git path {}", path.display()))
}

fn repository_root(cwd: &Path) -> Result<PathBuf> {
    let path = git_path(cwd, ["rev-parse", "--show-toplevel"])?;
    dunce::canonicalize(&path)
        .with_context(|| format!("cannot resolve repository root {}", path.display()))
}
