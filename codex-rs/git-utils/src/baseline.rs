use anyhow::Context;
use gix::hash::ObjectId;
use gix::objs::Tree;
use gix::objs::tree::Entry;
use gix::objs::tree::EntryKind;
use gix::objs::tree::EntryMode;
use similar::TextDiff;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
#[cfg(windows)]
use std::os::windows::fs::FileTypeExt;
use std::path::Path;
use std::path::PathBuf;
use tokio::task;

use crate::operations::run_git_for_status;

// Match the default file limit for automatic ghost snapshots. A baseline is derived data.
const MAX_BASELINE_FILE_BYTES: u64 = 10 * 1024 * 1024;
const BASELINE_VERSION_FILE: &str = "codex-baseline-version";
const BASELINE_VERSION: &[u8] = b"1\n";

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
    with_baseline_lock(root, |root| {
        reset_git_repository_sync(root, BaselineContents::Current)
    })
    .await
}

/// Ensures the internal directory has a usable, size-bounded baseline.
///
/// Old or incomplete baselines are discarded without reading their blobs. The replacement is
/// empty so all eligible working files remain visible to the next consolidation as additions.
pub async fn ensure_git_baseline_repository(root: &Path) -> anyhow::Result<()> {
    with_baseline_lock(root, ensure_git_baseline_repository_sync).await
}

fn ensure_git_baseline_repository_sync(root: &Path) -> anyhow::Result<()> {
    let git_dir = root.join(".git");
    let mut version = Vec::new();
    // Never follow a gitfile or directory symlink into another repository.
    if fs::symlink_metadata(&git_dir).is_ok_and(|metadata| metadata.is_dir())
        && fs::File::open(git_dir.join(BASELINE_VERSION_FILE))
            .and_then(|file| {
                file.take(BASELINE_VERSION.len() as u64 + 1)
                    .read_to_end(&mut version)
            })
            .is_ok()
        && version == BASELINE_VERSION
        && let Ok(repo) = gix::open::Options::isolated().open(&git_dir)
    {
        let repo = repo.to_thread_local();
        if repo.common_dir() == repo.git_dir() && head_file_entries(&repo).is_ok() {
            return Ok(());
        }
    }
    reset_git_repository_sync(root, BaselineContents::Empty)
}

// Imports and background consolidation can overlap, including across app-server processes.
// A nested .git keeps the lock outside both the replaceable baseline and memory scans.
async fn with_baseline_lock<T: Send + 'static>(
    root: &Path,
    operation: impl FnOnce(&Path) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let root = root.to_owned();
    task::spawn_blocking(move || {
        fs::create_dir_all(&root)?;
        let root = fs::canonicalize(root)?;
        let control = baseline_control(&root);
        for dir in [root.join(".codex-baseline"), control.clone()] {
            match fs::create_dir(&dir) {
                Ok(()) => {}
                Err(err)
                    if err.kind() == std::io::ErrorKind::AlreadyExists
                        && fs::symlink_metadata(&dir)?.is_dir()
                        && fs::canonicalize(&dir)? == dir => {}
                Err(err) => return Err(err.into()),
            }
        }
        anyhow::ensure!(
            !control.join("HEAD").try_exists()?,
            "baseline control directory contains a repository"
        );
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(control.join("codex-baseline.lock"))?;
        lock.lock().context("lock git baseline")?;
        operation(&root)
    })
    .await?
}

fn baseline_control(root: &Path) -> PathBuf {
    root.join(".codex-baseline").join(".git")
}

fn discard_baseline_path(path: &Path) -> anyhow::Result<()> {
    match path.symlink_metadata() {
        #[cfg(windows)]
        Ok(meta) if meta.file_type().is_symlink_dir() => fs::remove_dir(path)?,
        Ok(meta) if meta.is_dir() => {
            // Invalidate before deletion: interruption must not leave trusted partial metadata.
            match fs::remove_file(path.join(BASELINE_VERSION_FILE)) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
            fs::remove_dir_all(path)?;
        }
        Ok(_) => fs::remove_file(path)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

enum BaselineContents {
    Empty,
    Current,
}

fn reset_git_repository_sync(root: &Path, contents: BaselineContents) -> anyhow::Result<()> {
    let git_dir = root.join(".git");
    // Only metadata is disposable. An interrupted reset leaves no version marker, so the next
    // operation retries with an empty baseline rather than trusting a partial snapshot.
    discard_baseline_path(&git_dir)?;
    let repo = gix::ThreadSafeRepository::init_opts(
        root,
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
        gix::open::Options::isolated(),
    )?
    .to_thread_local();
    let tree_id = match contents {
        BaselineContents::Empty => repo.write_object(Tree::default())?.detach(),
        BaselineContents::Current => write_tree(&repo, root)?,
    };
    let signature = codex_signature();
    let mut time = gix::date::parse::TimeBuf::default();
    let signature = signature.to_ref(&mut time);
    repo.commit_as(
        signature,
        signature,
        "HEAD",
        BASELINE_COMMIT_MESSAGE,
        tree_id,
        Vec::<ObjectId>::new(),
    )?;
    write_index_from_head(root)?;
    fs::write(git_dir.join(BASELINE_VERSION_FILE), BASELINE_VERSION)?;
    Ok(())
}

fn write_index_from_head(root: &Path) -> anyhow::Result<()> {
    // Pin the index write to this worktree. Relative paths also work with Git for Windows
    // when Rust has canonicalized the working directory to a verbatim path.
    run_git_for_status(
        root,
        ["read-tree", "--reset", "HEAD"],
        Some(&[
            ("GIT_DIR".into(), ".git".into()),
            ("GIT_COMMON_DIR".into(), ".git".into()),
            ("GIT_INDEX_FILE".into(), ".git/index".into()),
            ("GIT_OBJECT_DIRECTORY".into(), ".git/objects".into()),
            ("GIT_WORK_TREE".into(), ".".into()),
        ]),
    )?;
    Ok(())
}

/// Returns the diff between the latest baseline and the current directory's bounded files.
pub async fn diff_since_latest_init(root: &Path) -> anyhow::Result<GitBaselineDiff> {
    with_baseline_lock(root, |root| {
        ensure_git_baseline_repository_sync(root)?;
        let mut repo = gix::open::Options::isolated().open(root)?.to_thread_local();
        let mut head_entries = head_file_entries(&repo)?;
        let mut current_entries = BTreeMap::new();
        let mut excluded = BTreeSet::new();
        collect_current_entries(&repo, root, root, &mut current_entries, &mut excluded)?;
        // Inputs may grow during synchronization, after the initial baseline was prepared.
        if excluded.iter().any(|path| head_entries.contains_key(path)) {
            drop(repo);
            reset_git_repository_sync(root, BaselineContents::Empty)?;
            repo = gix::open::Options::isolated().open(root)?.to_thread_local();
            head_entries.clear();
        }
        let changes = diff_entries(&head_entries, &current_entries);
        let unified_diff =
            render_unified_diff(&repo, root, &head_entries, &current_entries, &changes)?;
        Ok(GitBaselineDiff {
            changes,
            unified_diff,
        })
    })
    .await
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
            let Some((bytes, mode)) = read_baseline_file(&path)? else {
                continue;
            };
            let oid = repo
                .write_blob(bytes)
                .with_context(|| format!("write blob {}", path.display()))?
                .detach();
            entries.push(Entry {
                mode,
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

fn collect_current_entries(
    repo: &gix::Repository,
    root: &Path,
    dir: &Path,
    entries: &mut BTreeMap<String, GitBaselineFileEntry>,
    excluded: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name() == Some(OsStr::new(".git")) {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_current_entries(repo, root, &path, entries, excluded)?;
        } else if file_type.is_file() {
            let Some((bytes, mode)) = read_baseline_file(&path)? else {
                excluded.insert(relative_slash_path(root, &path)?);
                continue;
            };
            entries.insert(
                relative_slash_path(root, &path)?,
                GitBaselineFileEntry {
                    oid: blob_oid(repo, &bytes)?,
                    mode,
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

/// Bounds the read even if a file grows between checking its size and reaching EOF.
fn read_baseline_file(path: &Path) -> anyhow::Result<Option<(Vec<u8>, EntryMode)>> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let metadata = file.metadata()?;
    let len = metadata.len();
    if len > MAX_BASELINE_FILE_BYTES {
        return Ok(None);
    }
    // Reserve an EOF probe byte so bounded reads of stable files need no reallocation.
    let mut bytes = Vec::with_capacity(len as usize + 1);
    file.take(MAX_BASELINE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= MAX_BASELINE_FILE_BYTES).then_some((bytes, file_mode(&metadata))))
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
    anyhow::ensure!(
        repo.find_header(entry.oid)?.size() <= MAX_BASELINE_FILE_BYTES,
        "baseline blob exceeds the snapshot file limit"
    );
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
        read_baseline_file(&path)?
            .map(|(bytes, _mode)| bytes)
            .context("file grew beyond the snapshot file limit during diff")
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
fn file_mode(metadata: &fs::Metadata) -> EntryMode {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o111 == 0 {
        EntryKind::Blob.into()
    } else {
        EntryKind::BlobExecutable.into()
    }
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> EntryMode {
    EntryKind::Blob.into()
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
#[path = "baseline_recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    pub(super) fn git_stdout(root: &Path, args: &[&str]) -> String {
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

        let oversized = root.join("large-output");
        fs::File::create(&oversized)
            .expect("create large file")
            .set_len(MAX_BASELINE_FILE_BYTES + 1)
            .expect("size large file");

        reset_git_repository(&root).await.expect("reset repo");

        assert!(root.join(".git").is_dir());
        assert!(root.join(".git/index").is_file());
        let diff = diff_since_latest_init(&root).await.expect("diff");
        assert!(!diff.has_changes());
        assert_eq!(diff.unified_diff, "");
        assert_eq!(
            git_stdout(&root, &["status", "--porcelain"]),
            "?? large-output\n"
        );
        assert_eq!(git_stdout(&root, &["ls-files"]), "MEMORY.md\n");

        // An older baseline could already contain the oversized blob.
        git_stdout(&root, &["add", "large-output"]);
        git_stdout(
            &root,
            &[
                "-c",
                "user.name=Codex",
                "-c",
                "user.email=codex@example.com",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-m",
                "Legacy baseline",
            ],
        );
        fs::remove_file(root.join(".git").join(BASELINE_VERSION_FILE)).expect("remove version");
        fs::write(root.join("MEMORY.md"), "pending edit").expect("edit memory");
        ensure_git_baseline_repository(&root)
            .await
            .expect("migrate legacy baseline");
        assert_eq!(
            git_stdout(&root, &["ls-tree", "-r", "--name-only", "HEAD"]),
            ""
        );
        assert_eq!(git_stdout(&root, &["ls-files"]), "");
        let diff = diff_since_latest_init(&root)
            .await
            .expect("diff after migration");
        assert_eq!(
            diff.changes,
            vec![GitBaselineChange {
                status: GitBaselineChangeStatus::Added,
                path: "MEMORY.md".to_string(),
            }]
        );
        assert!(diff.unified_diff.contains("+pending edit"));
        assert_eq!(
            fs::metadata(oversized).expect("worktree file").len(),
            MAX_BASELINE_FILE_BYTES + 1
        );

        // Phase 2 may grow a tracked input after preparation and before computing its diff.
        fs::write(root.join("memory_summary.md"), "summary").expect("write summary");
        reset_git_repository(&root)
            .await
            .expect("reset bounded baseline");
        fs::File::options()
            .write(true)
            .open(root.join("MEMORY.md"))
            .expect("open tracked")
            .set_len(MAX_BASELINE_FILE_BYTES + 1)
            .expect("grow tracked");
        let diff = diff_since_latest_init(&root)
            .await
            .expect("diff after input synchronization");
        assert_eq!(
            diff.changes,
            vec![GitBaselineChange {
                status: GitBaselineChangeStatus::Added,
                path: "memory_summary.md".to_string(),
            }]
        );
        assert_eq!(git_stdout(&root, &["ls-files"]), "");
        assert_eq!(git_stdout(&root, &["diff", "HEAD", "--stat"]), "");
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
        assert_eq!(
            diff.changes,
            vec![GitBaselineChange {
                status: GitBaselineChangeStatus::Added,
                path: "MEMORY.md".to_string(),
            }]
        );
        assert_eq!(
            git_stdout(&root, &["status", "--porcelain"]),
            "?? MEMORY.md\n"
        );
        assert_eq!(git_stdout(&root, &["ls-files"]), "");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_index_ignores_configured_hooks_path() {
        let home = TempDir::new().expect("tempdir");
        let root = home.path().join("repo");
        let hooks_dir = root.join(".git/hooks-path-test");
        let marker_path = root.join("hook-ran");
        let hook_path = hooks_dir.join("post-index-change");

        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("MEMORY.md"), "baseline").expect("write memory");
        reset_git_repository(&root).await.expect("reset repo");
        fs::create_dir_all(&hooks_dir).expect("create hook dir");
        codex_utils_cargo_bin::write_executable(
            &hook_path,
            &format!(
                "#!/bin/sh\nprintf ran > \"{}\"\n",
                marker_path.to_string_lossy()
            ),
        )
        .expect("write post-index-change hook");
        git_stdout(
            &root,
            &[
                "config",
                "core.hooksPath",
                hooks_dir.to_string_lossy().as_ref(),
            ],
        );

        write_index_from_head(&root).expect("rewrite baseline index");

        assert!(
            !marker_path.exists(),
            "baseline index writes should not invoke configured hook directories"
        );
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
