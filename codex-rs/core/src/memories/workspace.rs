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

/// Generated diff file the Phase 2 consolidation agent reads before editing memories.
pub(super) const WORKSPACE_DIFF_FILENAME: &str = "phase2_workspace_diff.md";

const GITIGNORE_FILENAME: &str = ".gitignore";
const INITIAL_COMMIT_MESSAGE: &str =
    "Initialize Codex memories git state\n\nCo-authored-by: Codex <noreply@openai.com>";
const GITIGNORE_COMMIT_MESSAGE: &str =
    "Ignore generated Codex memories diff\n\nCo-authored-by: Codex <noreply@openai.com>";
const CONSOLIDATION_COMMIT_MESSAGE: &str =
    "Consolidate Codex memories\n\nCo-authored-by: Codex <noreply@openai.com>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoryWorkspaceFileEntry {
    oid: ObjectId,
    mode: EntryMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceChangeStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceChange {
    status: WorkspaceChangeStatus,
    path: String,
}

/// Creates the memory root, initializes its git repository, and keeps the generated diff file in
/// `.gitignore`.
///
/// This commits an initial baseline when the repository has no `HEAD` yet. For existing clean
/// repositories that predate the generated diff file, it commits the `.gitignore` update by itself
/// so that internal housekeeping does not wake the consolidation agent.
pub(super) async fn prepare_git_repo(root: &Path) -> anyhow::Result<()> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || {
        fs::create_dir_all(&root)
            .with_context(|| format!("create memories root {}", root.display()))?;
        let repo = open_or_init(&root)?;
        let gitignore_changed = ensure_gitignore_ignores_workspace_diff(&root)?;
        if repo.head_id().is_err() && has_workspace_files(&root)? {
            commit_current_tree(&repo, INITIAL_COMMIT_MESSAGE)?;
        } else if gitignore_changed && only_gitignore_changed(&repo, &root)? {
            commit_current_tree(&repo, GITIGNORE_COMMIT_MESSAGE)?;
        }
        anyhow::Ok(())
    })
    .await?
}

/// Returns true when the memory root differs from the current git `HEAD` tree.
pub(super) async fn has_changes(root: &Path) -> anyhow::Result<bool> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || {
        let repo = open_or_init(&root)?;
        has_changes_blocking(&repo, &root)
    })
    .await?
}

/// Writes `phase2_workspace_diff.md` with a git-style diff from `HEAD` to the current worktree.
pub(super) async fn write_workspace_diff(root: &Path) -> anyhow::Result<()> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || {
        let repo = open_or_init(&root)?;
        let head_entries = head_file_entries(&repo)?;
        let current_entries = current_file_entries(&repo, &root)?;
        let changes = diff_entries(&head_entries, &current_entries);
        let content =
            render_workspace_diff_file(&repo, &root, &head_entries, &current_entries, &changes)?;
        let path = root.join(WORKSPACE_DIFF_FILENAME);
        fs::write(&path, content)
            .with_context(|| format!("write memory workspace diff file {}", path.display()))?;
        anyhow::Ok(())
    })
    .await?
}

/// Commits the current memory root as the next normal git commit when it differs from `HEAD`.
pub(super) async fn commit_all(root: &Path) -> anyhow::Result<()> {
    let root = root.to_path_buf();
    task::spawn_blocking(move || {
        let repo = open_or_init(&root)?;
        commit_current_tree(&repo, CONSOLIDATION_COMMIT_MESSAGE)?;
        anyhow::Ok(())
    })
    .await?
}

/// Removes the generated workspace diff file when no consolidation agent needs it.
pub(super) async fn remove_workspace_diff(root: &Path) -> anyhow::Result<()> {
    let path = root.join(WORKSPACE_DIFF_FILENAME);
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err)
            .with_context(|| format!("remove memory workspace diff file {}", path.display())),
    }
}

fn open_or_init(root: &Path) -> anyhow::Result<gix::Repository> {
    if root.join(".git").is_dir() {
        gix::open(root).with_context(|| format!("open memories git repo {}", root.display()))
    } else {
        gix::init(root).with_context(|| format!("init memories git repo {}", root.display()))
    }
}

fn ensure_gitignore_ignores_workspace_diff(root: &Path) -> anyhow::Result<bool> {
    let path = root.join(GITIGNORE_FILENAME);
    let existing = match fs::read_to_string(&path) {
        Ok(existing) => existing,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    if existing
        .lines()
        .any(|line| line.trim() == WORKSPACE_DIFF_FILENAME)
    {
        return Ok(false);
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(WORKSPACE_DIFF_FILENAME);
    updated.push('\n');
    fs::write(&path, updated).with_context(|| format!("write {}", path.display()))?;
    Ok(true)
}

fn has_workspace_files(root: &Path) -> anyhow::Result<bool> {
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if entry.file_name() != OsStr::new(".git")
            && !should_ignore_workspace_path(root, &entry.path())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_changes_blocking(repo: &gix::Repository, root: &Path) -> anyhow::Result<bool> {
    let head_entries = head_file_entries(repo)?;
    let current_entries = current_file_entries(repo, root)?;
    Ok(head_entries != current_entries)
}

fn only_gitignore_changed(repo: &gix::Repository, root: &Path) -> anyhow::Result<bool> {
    let head_entries = head_file_entries(repo)?;
    let current_entries = current_file_entries(repo, root)?;
    let changes = diff_entries(&head_entries, &current_entries);
    Ok(!changes.is_empty()
        && changes
            .iter()
            .all(|change| change.path == GITIGNORE_FILENAME))
}

fn commit_current_tree(repo: &gix::Repository, message: &str) -> anyhow::Result<bool> {
    let root = repo
        .workdir()
        .context("memories git repo must have a worktree")?;
    let tree_id = write_tree(repo, root, root)?;
    let parent = repo.head_id().ok().map(gix::Id::detach);
    if let Some(parent) = parent {
        let parent_tree = repo
            .find_commit(parent)
            .context("find memories HEAD commit")?
            .tree_id()
            .context("load memories HEAD tree id")?
            .detach();
        if parent_tree == tree_id {
            return Ok(false);
        }
    }

    let signature = codex_signature();
    let mut time = gix::date::parse::TimeBuf::default();
    let signature_ref = signature.to_ref(&mut time);
    let parents = parent.into_iter().collect::<Vec<_>>();
    repo.commit_as(
        signature_ref,
        signature_ref,
        "HEAD",
        message,
        tree_id,
        parents,
    )
    .context("commit memories git repo")?;
    Ok(true)
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

fn write_tree(repo: &gix::Repository, root: &Path, dir: &Path) -> anyhow::Result<ObjectId> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name == OsStr::new(".git") || should_ignore_workspace_path(root, &path) {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let oid = write_tree(repo, root, &path)?;
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
) -> anyhow::Result<BTreeMap<String, MemoryWorkspaceFileEntry>> {
    let Ok(tree_id) = repo.head_tree_id() else {
        return Ok(BTreeMap::new());
    };
    let tree = repo.find_tree(tree_id.detach()).context("load HEAD tree")?;
    let mut entries = BTreeMap::new();
    collect_tree_entries(repo, tree, PathBuf::new(), &mut entries)?;
    Ok(entries)
}

fn collect_tree_entries(
    repo: &gix::Repository,
    tree: gix::Tree<'_>,
    prefix: PathBuf,
    entries: &mut BTreeMap<String, MemoryWorkspaceFileEntry>,
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
                MemoryWorkspaceFileEntry {
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
) -> anyhow::Result<BTreeMap<String, MemoryWorkspaceFileEntry>> {
    let mut entries = BTreeMap::new();
    collect_current_entries(repo, root, root, &mut entries)?;
    Ok(entries)
}

fn collect_current_entries(
    repo: &gix::Repository,
    root: &Path,
    dir: &Path,
    entries: &mut BTreeMap<String, MemoryWorkspaceFileEntry>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name() == Some(OsStr::new(".git")) || should_ignore_workspace_path(root, &path)
        {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_current_entries(repo, root, &path, entries)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            entries.insert(
                relative_slash_path(root, &path)?,
                MemoryWorkspaceFileEntry {
                    oid: blob_oid(repo, &bytes)?,
                    mode: file_mode(&path, EntryKind::Blob)?,
                },
            );
        } else if file_type.is_symlink() {
            let target =
                fs::read_link(&path).with_context(|| format!("read symlink {}", path.display()))?;
            entries.insert(
                relative_slash_path(root, &path)?,
                MemoryWorkspaceFileEntry {
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
        .context("compute memory workspace blob oid")
}

fn diff_entries(
    head: &BTreeMap<String, MemoryWorkspaceFileEntry>,
    current: &BTreeMap<String, MemoryWorkspaceFileEntry>,
) -> Vec<WorkspaceChange> {
    let mut entries = Vec::new();
    for (path, entry) in current {
        match head.get(path) {
            None => entries.push(WorkspaceChange {
                status: WorkspaceChangeStatus::Added,
                path: path.clone(),
            }),
            Some(head_entry) if head_entry != entry => entries.push(WorkspaceChange {
                status: WorkspaceChangeStatus::Modified,
                path: path.clone(),
            }),
            Some(_) => {}
        }
    }
    for path in head.keys() {
        if !current.contains_key(path) {
            entries.push(WorkspaceChange {
                status: WorkspaceChangeStatus::Deleted,
                path: path.clone(),
            });
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}

fn render_workspace_diff_file(
    repo: &gix::Repository,
    root: &Path,
    head_entries: &BTreeMap<String, MemoryWorkspaceFileEntry>,
    current_entries: &BTreeMap<String, MemoryWorkspaceFileEntry>,
    changes: &[WorkspaceChange],
) -> anyhow::Result<String> {
    let mut rendered = String::from(
        "# Memory Workspace Diff\n\n\
         Generated by Codex before Phase 2 memory consolidation. Read this file first and do not edit it.\n\n\
         ## Status\n",
    );

    if changes.is_empty() {
        rendered.push_str("- none\n");
        return Ok(rendered);
    }

    for change in changes {
        let status = workspace_change_status_label(change.status);
        rendered.push_str(&format!("- {status} {}\n", change.path));
    }
    rendered.push_str("\n## Diff\n\n```diff\n");
    for change in changes {
        rendered.push_str(&render_workspace_change_diff(
            repo,
            root,
            head_entries,
            current_entries,
            change,
        )?);
    }
    rendered.push_str("```\n");
    Ok(rendered)
}

fn render_workspace_change_diff(
    repo: &gix::Repository,
    root: &Path,
    head_entries: &BTreeMap<String, MemoryWorkspaceFileEntry>,
    current_entries: &BTreeMap<String, MemoryWorkspaceFileEntry>,
    change: &WorkspaceChange,
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

fn read_head_blob(
    repo: &gix::Repository,
    entry: &MemoryWorkspaceFileEntry,
) -> anyhow::Result<Vec<u8>> {
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

fn workspace_change_status_label(status: WorkspaceChangeStatus) -> &'static str {
    match status {
        WorkspaceChangeStatus::Added => "A",
        WorkspaceChangeStatus::Modified => "M",
        WorkspaceChangeStatus::Deleted => "D",
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

fn should_ignore_workspace_path(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|relative| relative == Path::new(WORKSPACE_DIFF_FILENAME))
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
#[path = "workspace_tests.rs"]
mod tests;
