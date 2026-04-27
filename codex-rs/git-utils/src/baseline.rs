use anyhow::Context;
use gix::hash::ObjectId;
use gix::objs::Tree;
use gix::objs::tree::Entry;
use gix::objs::tree::EntryKind;
use gix::objs::tree::EntryMode;
use similar::TextDiff;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tokio::task;

use crate::operations::run_git_for_status;

const BASELINE_COMMIT_MESSAGE: &str =
    "Initialize Codex git baseline\n\nCo-authored-by: Codex <noreply@openai.com>";

/// File-level change status between a git baseline and the current directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitBaselineChangeStatus {
    Added,
    Modified,
    Deleted,
}

impl GitBaselineChangeStatus {
    /// Returns the short git-style status label for this change.
    pub fn label(self) -> &'static str {
        match self {
            GitBaselineChangeStatus::Added => "A",
            GitBaselineChangeStatus::Modified => "M",
            GitBaselineChangeStatus::Deleted => "D",
        }
    }
}

/// One changed file between a git baseline and the current directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBaselineChange {
    pub status: GitBaselineChangeStatus,
    pub path: String,
}

/// Structured diff from the latest git baseline reset to the current directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBaselineDiff {
    pub changes: Vec<GitBaselineChange>,
    pub unified_diff: String,
}

impl GitBaselineDiff {
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GitBaselineFileEntry {
    oid: ObjectId,
    mode: EntryMode,
}

/// Replaces any existing `.git` metadata in `root` with a fresh one-commit baseline.
///
/// This is intentionally destructive for `root/.git`. It is meant for internal directories where
/// git is used only as a baseline/diff implementation detail, not for user repositories.
pub async fn reset_git_repository(root: &Path) -> anyhow::Result<()> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || reset_git_repository_sync(&root)).await?
}

/// Ensures `root` has a usable git baseline repository.
///
/// Existing usable `.git/` metadata is preserved. Missing or unusable metadata is replaced with a
/// fresh one-commit baseline.
pub async fn ensure_git_baseline_repository(root: &Path) -> anyhow::Result<()> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || {
        fs::create_dir_all(&root)
            .with_context(|| format!("create git baseline root {}", root.display()))?;
        if root.join(".git").is_dir()
            && let Ok(repo) = gix::open(&root)
            && head_file_entries(&repo).is_ok()
        {
            return Ok(());
        }
        reset_git_repository_sync(&root)
    })
    .await?
}

fn reset_git_repository_sync(root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root)
        .with_context(|| format!("create git baseline root {}", root.display()))?;
    remove_git_metadata(root)?;
    let repo = gix::init(root).with_context(|| format!("init git repo {}", root.display()))?;
    commit_current_tree(&repo, BASELINE_COMMIT_MESSAGE)?;
    write_index_from_head(root)?;
    Ok(())
}

/// Returns the diff between the latest baseline reset and the current directory contents.
pub async fn diff_since_latest_init(root: &Path) -> anyhow::Result<GitBaselineDiff> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || {
        let repo = gix::open(&root).with_context(|| format!("open git repo {}", root.display()))?;
        let head_entries = head_file_entries(&repo)?;
        let current_entries = current_file_entries(&repo, &root)?;
        let changes = diff_entries(&head_entries, &current_entries);
        let unified_diff =
            render_unified_diff(&repo, &root, &head_entries, &current_entries, &changes)?;
        Ok(GitBaselineDiff {
            changes,
            unified_diff,
        })
    })
    .await?
}

fn remove_git_metadata(root: &Path) -> anyhow::Result<()> {
    let git_path = root.join(".git");
    let metadata = match fs::symlink_metadata(&git_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("stat {}", git_path.display())),
    };

    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(&git_path).with_context(|| format!("remove {}", git_path.display()))
    } else {
        fs::remove_file(&git_path).with_context(|| format!("remove {}", git_path.display()))
    }
}

fn commit_current_tree(repo: &gix::Repository, message: &str) -> anyhow::Result<()> {
    let root = repo
        .workdir()
        .context("git baseline repo must have a worktree")?;
    let tree_id = write_tree(repo, root)?;
    let signature = codex_signature();
    let mut time = gix::date::parse::TimeBuf::default();
    let signature_ref = signature.to_ref(&mut time);
    repo.commit_as(
        signature_ref,
        signature_ref,
        "HEAD",
        message,
        tree_id,
        Vec::<ObjectId>::new(),
    )
    .context("commit git baseline repo")?;
    Ok(())
}

fn write_index_from_head(root: &Path) -> anyhow::Result<()> {
    run_git_for_status(root, ["read-tree", "--reset", "HEAD"], /*env*/ None)
        .context("write git baseline index from HEAD")
}

fn codex_signature() -> gix::actor::Signature {
    gix::actor::Signature {
        name: "Codex".into(),
        email: "noreply@openai.com".into(),
        time: gix::date::Time {
            seconds: chrono::Utc::now().timestamp(),
            offset: 0,
        },
    }
}

fn write_tree(repo: &gix::Repository, dir: &Path) -> anyhow::Result<ObjectId> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name == OsStr::new(".git") {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let oid = write_tree(repo, &path)?;
            let tree = repo
                .find_tree(oid)
                .with_context(|| format!("load tree {}", path.display()))?;
            if tree.decode()?.entries.is_empty() {
                continue;
            }
            entries.push(Entry {
                mode: EntryKind::Tree.into(),
                filename: os_str_to_bstring(&file_name),
                oid,
            });
        } else if file_type.is_file() {
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let oid = repo
                .write_blob(bytes)
                .with_context(|| format!("write blob {}", path.display()))?
                .detach();
            entries.push(Entry {
                mode: file_mode(&path, EntryKind::Blob)?,
                filename: os_str_to_bstring(&file_name),
                oid,
            });
        } else if file_type.is_symlink() {
            let target =
                fs::read_link(&path).with_context(|| format!("read symlink {}", path.display()))?;
            let oid = repo
                .write_blob(path_to_bytes(&target))
                .with_context(|| format!("write symlink blob {}", path.display()))?
                .detach();
            entries.push(Entry {
                mode: EntryKind::Link.into(),
                filename: os_str_to_bstring(&file_name),
                oid,
            });
        }
    }

    entries.sort();
    repo.write_object(&Tree { entries })
        .context("write tree object")
        .map(gix::Id::detach)
}

fn head_file_entries(
    repo: &gix::Repository,
) -> anyhow::Result<BTreeMap<String, GitBaselineFileEntry>> {
    let tree_id = repo.head_tree_id().context("load HEAD tree id")?;
    let tree = repo.find_tree(tree_id.detach()).context("load HEAD tree")?;
    let mut entries = BTreeMap::new();
    collect_tree_entries(repo, tree, PathBuf::new(), &mut entries)?;
    Ok(entries)
}

fn collect_tree_entries(
    repo: &gix::Repository,
    tree: gix::Tree<'_>,
    prefix: PathBuf,
    entries: &mut BTreeMap<String, GitBaselineFileEntry>,
) -> anyhow::Result<()> {
    for entry in tree.iter() {
        let entry = entry?;
        let file_name = bstr_to_path(entry.inner.filename);
        let path = prefix.join(file_name);
        if entry.inner.mode.is_tree() {
            let tree = repo
                .find_tree(entry.inner.oid.to_owned())
                .context("load child tree")?;
            collect_tree_entries(repo, tree, path, entries)?;
        } else {
            entries.insert(
                path_to_slash_string(&path),
                GitBaselineFileEntry {
                    oid: entry.inner.oid.to_owned(),
                    mode: entry.inner.mode,
                },
            );
        }
    }
    Ok(())
}

fn current_file_entries(
    repo: &gix::Repository,
    root: &Path,
) -> anyhow::Result<BTreeMap<String, GitBaselineFileEntry>> {
    let mut entries = BTreeMap::new();
    collect_current_entries(repo, root, root, &mut entries)?;
    Ok(entries)
}

fn collect_current_entries(
    repo: &gix::Repository,
    root: &Path,
    dir: &Path,
    entries: &mut BTreeMap<String, GitBaselineFileEntry>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name() == Some(OsStr::new(".git")) {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_current_entries(repo, root, &path, entries)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            entries.insert(
                relative_slash_path(root, &path)?,
                GitBaselineFileEntry {
                    oid: blob_oid(repo, &bytes)?,
                    mode: file_mode(&path, EntryKind::Blob)?,
                },
            );
        } else if file_type.is_symlink() {
            let target =
                fs::read_link(&path).with_context(|| format!("read symlink {}", path.display()))?;
            entries.insert(
                relative_slash_path(root, &path)?,
                GitBaselineFileEntry {
                    oid: blob_oid(repo, &path_to_bytes(&target))?,
                    mode: EntryKind::Link.into(),
                },
            );
        }
    }
    Ok(())
}

fn blob_oid(repo: &gix::Repository, bytes: &[u8]) -> anyhow::Result<ObjectId> {
    gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, bytes)
        .context("compute git baseline blob oid")
}

fn diff_entries(
    head: &BTreeMap<String, GitBaselineFileEntry>,
    current: &BTreeMap<String, GitBaselineFileEntry>,
) -> Vec<GitBaselineChange> {
    let mut entries = Vec::new();
    for (path, entry) in current {
        match head.get(path) {
            None => entries.push(GitBaselineChange {
                status: GitBaselineChangeStatus::Added,
                path: path.clone(),
            }),
            Some(head_entry) if head_entry != entry => entries.push(GitBaselineChange {
                status: GitBaselineChangeStatus::Modified,
                path: path.clone(),
            }),
            Some(_) => {}
        }
    }
    for path in head.keys() {
        if !current.contains_key(path) {
            entries.push(GitBaselineChange {
                status: GitBaselineChangeStatus::Deleted,
                path: path.clone(),
            });
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}

fn render_unified_diff(
    repo: &gix::Repository,
    root: &Path,
    head_entries: &BTreeMap<String, GitBaselineFileEntry>,
    current_entries: &BTreeMap<String, GitBaselineFileEntry>,
    changes: &[GitBaselineChange],
) -> anyhow::Result<String> {
    let mut rendered = String::new();
    for change in changes {
        rendered.push_str(&render_change_diff(
            repo,
            root,
            head_entries,
            current_entries,
            change,
        )?);
    }
    Ok(rendered)
}

fn render_change_diff(
    repo: &gix::Repository,
    root: &Path,
    head_entries: &BTreeMap<String, GitBaselineFileEntry>,
    current_entries: &BTreeMap<String, GitBaselineFileEntry>,
    change: &GitBaselineChange,
) -> anyhow::Result<String> {
    let old_entry = head_entries.get(&change.path);
    let new_entry = current_entries.get(&change.path);
    let old_bytes = old_entry
        .map(|entry| read_head_blob(repo, entry))
        .transpose()
        .with_context(|| format!("read HEAD content for {}", change.path))?;
    let new_bytes = new_entry
        .map(|_| read_current_file_bytes(root, &change.path))
        .transpose()
        .with_context(|| format!("read current content for {}", change.path))?;

    let old_text = String::from_utf8_lossy(old_bytes.as_deref().unwrap_or_default());
    let new_text = String::from_utf8_lossy(new_bytes.as_deref().unwrap_or_default());
    let old_header = if old_bytes.is_some() {
        format!("a/{}", change.path)
    } else {
        "/dev/null".to_string()
    };
    let new_header = if new_bytes.is_some() {
        format!("b/{}", change.path)
    } else {
        "/dev/null".to_string()
    };

    let mut section = format!("diff --git a/{0} b/{0}\n", change.path);
    match (old_entry, new_entry) {
        (None, Some(entry)) => {
            section.push_str(&format!("new file mode {}\n", mode_label(entry.mode)));
        }
        (Some(entry), None) => {
            section.push_str(&format!("deleted file mode {}\n", mode_label(entry.mode)));
        }
        (Some(old), Some(new)) if old.mode != new.mode => {
            section.push_str(&format!(
                "old mode {}\nnew mode {}\n",
                mode_label(old.mode),
                mode_label(new.mode)
            ));
        }
        (Some(_), Some(_)) => {}
        (None, None) => return Ok(String::new()),
    }

    let diff = TextDiff::from_lines(&old_text, &new_text)
        .unified_diff()
        .context_radius(3)
        .header(&old_header, &new_header)
        .to_string();
    section.push_str(&diff);
    if !section.ends_with('\n') {
        section.push('\n');
    }
    Ok(section)
}

fn read_head_blob(repo: &gix::Repository, entry: &GitBaselineFileEntry) -> anyhow::Result<Vec<u8>> {
    let mut blob = repo.find_blob(entry.oid)?;
    Ok(blob.take_data())
}

fn read_current_file_bytes(root: &Path, relative_path: &str) -> anyhow::Result<Vec<u8>> {
    let path = root.join(relative_path);
    let metadata =
        fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        let target =
            fs::read_link(&path).with_context(|| format!("read symlink {}", path.display()))?;
        Ok(path_to_bytes(&target))
    } else {
        fs::read(&path).with_context(|| format!("read {}", path.display()))
    }
}

fn mode_label(mode: EntryMode) -> &'static str {
    match mode.kind() {
        EntryKind::Blob => "100644",
        EntryKind::BlobExecutable => "100755",
        EntryKind::Link => "120000",
        EntryKind::Tree => "040000",
        EntryKind::Commit => "160000",
    }
}

#[cfg(unix)]
fn file_mode(path: &Path, default: EntryKind) -> anyhow::Result<EntryMode> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)?.permissions().mode();
    Ok(if mode & 0o111 == 0 {
        default.into()
    } else {
        EntryKind::BlobExecutable.into()
    })
}

#[cfg(not(unix))]
fn file_mode(_path: &Path, default: EntryKind) -> anyhow::Result<EntryMode> {
    Ok(default.into())
}

#[cfg(unix)]
fn os_str_to_bstring(value: &OsStr) -> gix::bstr::BString {
    use std::os::unix::ffi::OsStrExt;

    value.as_bytes().into()
}

#[cfg(not(unix))]
fn os_str_to_bstring(value: &OsStr) -> gix::bstr::BString {
    value.to_string_lossy().as_bytes().into()
}

#[cfg(unix)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

fn bstr_to_path(value: &gix::bstr::BStr) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        PathBuf::from(OsStr::from_bytes(value))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(value.to_string())
    }
}

fn relative_slash_path(root: &Path, path: &Path) -> anyhow::Result<String> {
    path.strip_prefix(root)
        .with_context(|| format!("strip {} from {}", root.display(), path.display()))
        .map(path_to_slash_string)
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn git_stdout(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run git command");
        assert!(
            output.status.success(),
            "git command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    #[tokio::test]
    async fn reset_creates_fresh_baseline() {
        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("MEMORY.md"), "baseline").expect("write memory");

        reset_git_repository(&root).await.expect("reset repo");

        assert!(root.join(".git").is_dir());
        assert!(root.join(".git/index").is_file());
        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert!(!diff.has_changes());
        assert_eq!(diff.unified_diff, "");
        assert_eq!(git_stdout(&root, &["status", "--porcelain"]), "");
        assert_eq!(git_stdout(&root, &["ls-files"]), "MEMORY.md\n");
    }

    #[tokio::test]
    async fn ensure_recovers_from_unborn_repository() {
        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("MEMORY.md"), "memory").expect("write memory");
        gix::init(&root).expect("init git repo without baseline commit");

        ensure_git_baseline_repository(&root)
            .await
            .expect("ensure repo");

        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert!(!diff.has_changes());
        assert_eq!(git_stdout(&root, &["status", "--porcelain"]), "");
        assert_eq!(git_stdout(&root, &["ls-files"]), "MEMORY.md\n");
    }

    #[tokio::test]
    async fn diff_reports_added_modified_and_deleted_files() {
        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        fs::create_dir_all(root.join("rollout_summaries")).expect("create rollout summaries");
        fs::write(root.join("MEMORY.md"), "old").expect("write memory");
        fs::write(
            root.join("rollout_summaries/deleted.md"),
            "thread_id: 00000000-0000-4000-8000-000000000001\nimportant stale evidence\n",
        )
        .expect("write rollout summary");
        reset_git_repository(&root).await.expect("reset repo");

        fs::write(root.join("MEMORY.md"), "new").expect("update memory");
        fs::write(root.join("memory_summary.md"), "summary").expect("write summary");
        fs::remove_file(root.join("rollout_summaries/deleted.md")).expect("delete summary");

        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert_eq!(
            diff.changes,
            vec![
                GitBaselineChange {
                    status: GitBaselineChangeStatus::Modified,
                    path: "MEMORY.md".to_string(),
                },
                GitBaselineChange {
                    status: GitBaselineChangeStatus::Added,
                    path: "memory_summary.md".to_string(),
                },
                GitBaselineChange {
                    status: GitBaselineChangeStatus::Deleted,
                    path: "rollout_summaries/deleted.md".to_string(),
                },
            ]
        );
        assert!(
            diff.unified_diff
                .contains("diff --git a/MEMORY.md b/MEMORY.md")
        );
        assert!(diff.unified_diff.contains("-old"));
        assert!(diff.unified_diff.contains("+new"));
        assert!(
            diff.unified_diff
                .contains("diff --git a/memory_summary.md b/memory_summary.md")
        );
        assert!(diff.unified_diff.contains("+summary"));
        assert!(
            diff.unified_diff.contains(
                "diff --git a/rollout_summaries/deleted.md b/rollout_summaries/deleted.md"
            )
        );
        assert!(diff.unified_diff.contains("deleted file mode 100644"));
        assert!(
            diff.unified_diff
                .contains("-thread_id: 00000000-0000-4000-8000-000000000001")
        );
        assert!(diff.unified_diff.contains("-important stale evidence"));
    }

    #[tokio::test]
    async fn reset_drops_previous_history() {
        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("MEMORY.md"), "old").expect("write memory");
        reset_git_repository(&root).await.expect("reset repo");

        fs::write(root.join("MEMORY.md"), "new").expect("update memory");
        reset_git_repository(&root).await.expect("reset repo again");

        let repo = gix::open(&root).expect("open repo");
        let head = repo.head_id().expect("head").detach();
        let commit = repo.find_commit(head).expect("find head commit");
        assert_eq!(commit.parent_ids().count(), 0);
        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert!(!diff.has_changes());
    }

    #[tokio::test]
    async fn status_scan_does_not_write_added_file_blobs() {
        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        fs::create_dir_all(&root).expect("create root");
        reset_git_repository(&root).await.expect("reset repo");
        let added_content = b"new uncommitted memory";
        fs::write(root.join("MEMORY.md"), added_content).expect("write memory");

        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert!(diff.has_changes());

        let repo = gix::open(&root).expect("open repo");
        let added_oid = blob_oid(&repo, added_content).expect("compute added oid");
        assert!(
            repo.find_blob(added_oid).is_err(),
            "status scans should hash current files without writing loose git objects"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reports_executable_bit_changes_as_modified() {
        use std::os::unix::fs::PermissionsExt;

        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        fs::create_dir_all(&root).expect("create root");
        let path = root.join("MEMORY.md");
        fs::write(&path, "same content").expect("write memory");
        reset_git_repository(&root).await.expect("reset repo");
        let mut permissions = fs::metadata(&path).expect("stat memory").permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(&path, permissions).expect("chmod memory");

        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert_eq!(
            diff.changes,
            vec![GitBaselineChange {
                status: GitBaselineChangeStatus::Modified,
                path: "MEMORY.md".to_string(),
            }]
        );
        assert!(diff.unified_diff.contains("old mode 100644"));
        assert!(diff.unified_diff.contains("new mode 100755"));
    }
}
