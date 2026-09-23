//! Resolve repository trust roots through local or remote filesystem access.

use codex_file_system::ExecutorFileSystem;
use codex_file_system::FindUpErrorPolicy;
use codex_file_system::find_nearest_ancestor_with_markers;
use codex_file_system::find_nearest_native_ancestor_with_markers;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

const MAX_GIT_METADATA_FILE_BYTES: u64 = 64 * 1024;

/// Resolve the path that should be used for trust checks. Similar to
/// [`crate::get_git_repo_root`], but resolves to the root of the main
/// repository. Handles worktrees via filesystem inspection without invoking
/// the `git` executable.
pub async fn resolve_root_git_project_for_trust(
    fs: &dyn ExecutorFileSystem,
    cwd: &AbsolutePathBuf,
) -> Option<AbsolutePathBuf> {
    match resolve_root(fs, TrustPath::Native(cwd.clone())).await? {
        TrustPath::Native(root) => Some(root),
        TrustPath::Uri(_) => None,
    }
}

/// Resolve the main repository's trust root using the executor's path convention.
///
/// Uses existing filesystem operations so this also works with independently
/// deployed executors. Native callers should use [`resolve_root_git_project_for_trust`]
/// to preserve paths whose URI representation is opaque.
pub async fn resolve_root_git_project_uri_for_trust(
    fs: &dyn ExecutorFileSystem,
    cwd: &PathUri,
) -> Option<PathUri> {
    resolve_root(fs, TrustPath::Uri(cwd.clone()))
        .await
        .map(|root| root.uri())
}

// Keep native ancestry native: an opaque URI cannot supply parents or children.
// Everything after ancestry discovery shares the same repository validation.
#[derive(Clone)]
enum TrustPath {
    Native(AbsolutePathBuf),
    Uri(PathUri),
}

impl TrustPath {
    fn uri(&self) -> PathUri {
        match self {
            Self::Native(path) => PathUri::from_abs_path(path),
            Self::Uri(path) => path.clone(),
        }
    }

    fn parent(&self) -> Option<Self> {
        match self {
            Self::Native(path) => path.parent().map(Self::Native),
            Self::Uri(path) => path.parent().map(Self::Uri),
        }
    }

    fn join(&self, component: &str) -> Option<Self> {
        match self {
            Self::Native(path) => Some(Self::Native(path.join(component))),
            Self::Uri(path) => path.join(component).ok().map(Self::Uri),
        }
    }
}

async fn resolve_root(fs: &dyn ExecutorFileSystem, cwd: TrustPath) -> Option<TrustPath> {
    let cwd_uri = cwd.uri();
    let base = match fs
        .get_metadata(&cwd_uri, Default::default(), /*sandbox*/ None)
        .await
    {
        Ok(metadata) if metadata.is_directory => cwd.clone(),
        _ => cwd.parent()?,
    };
    let mut base = base;
    let repo_root = loop {
        let candidate = match &base {
            TrustPath::Native(path) => find_nearest_native_ancestor_with_markers(
                fs,
                path,
                vec![".git".to_string()],
                FindUpErrorPolicy::Ignore,
                /*sandbox*/ None,
            )
            .await
            .ok()?
            .map(TrustPath::Native)?,
            TrustPath::Uri(path) => TrustPath::Uri(
                find_nearest_ancestor_with_markers(
                    fs,
                    path,
                    vec![".git".to_string()],
                    FindUpErrorPolicy::Ignore,
                    /*sandbox*/ None,
                )
                .await
                .ok()??,
            ),
        };
        let dot_git = candidate.join(".git")?;
        let metadata = fs
            .get_metadata(&dot_git.uri(), Default::default(), /*sandbox*/ None)
            .await
            .ok()?;
        if !metadata.is_directory
            || fs
                .get_metadata(
                    &dot_git.join("HEAD")?.uri(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await
                .is_ok()
        {
            break candidate;
        }
        base = candidate.parent()?;
    };
    let dot_git_uri = repo_root.join(".git")?.uri();
    let dot_git_metadata = fs
        .get_metadata(&dot_git_uri, Default::default(), /*sandbox*/ None)
        .await
        .ok()?;
    if dot_git_metadata.is_directory {
        return Some(repo_root);
    }
    if !dot_git_metadata.is_file
        || dot_git_metadata.is_symlink
        || dot_git_metadata.size > MAX_GIT_METADATA_FILE_BYTES
    {
        return None;
    }

    let git_dir_uri = read_gitdir_file(fs, &dot_git_uri).await?;
    let git_dir_path = match &repo_root {
        TrustPath::Native(_) => TrustPath::Native(git_dir_uri.to_abs_path().ok()?),
        TrustPath::Uri(_) => TrustPath::Uri(git_dir_uri.clone()),
    };
    let git_dir_metadata = fs
        .get_metadata(&git_dir_uri, Default::default(), /*sandbox*/ None)
        .await
        .ok()?;
    if !git_dir_metadata.is_directory || git_dir_metadata.is_symlink {
        return None;
    }

    let canonical_git_dir_uri = fs.canonicalize(&git_dir_uri, /*sandbox*/ None).await.ok()?;
    let worktrees_dir_uri = canonical_git_dir_uri.parent()?;
    if worktrees_dir_uri.basename().as_deref() != Some("worktrees") {
        return None;
    }
    let common_dir_uri = worktrees_dir_uri.parent()?;
    let worktree_gitdir_uri = canonical_git_dir_uri.join("gitdir").ok()?;
    let commondir_uri = canonical_git_dir_uri.join("commondir").ok()?;
    let (worktree_gitdir_bytes, commondir_bytes) = tokio::join!(
        read_metadata_file(fs, &worktree_gitdir_uri),
        read_metadata_file(fs, &commondir_uri),
    );
    let worktree_gitdir_bytes = worktree_gitdir_bytes?;
    let worktree_gitdir = worktree_gitdir_bytes.trim_ascii();
    if worktree_gitdir.is_empty() {
        return None;
    }
    let worktree_dot_git_uri = canonical_git_dir_uri
        .join_native_bytes(worktree_gitdir)
        .ok()?;
    if worktree_dot_git_uri.basename().as_deref() != Some(".git") {
        return None;
    }
    let commondir_bytes = commondir_bytes?;
    let commondir = commondir_bytes.trim_ascii();
    if commondir.is_empty() {
        return None;
    }
    let linked_common_dir_uri = canonical_git_dir_uri.join_native_bytes(commondir).ok()?;
    let registered_checkout_uri = worktree_dot_git_uri.parent()?;
    let checkout_uri = repo_root.uri();
    // Compare checkout directories, not the final .git entries. Following a
    // substituted .git symlink must never turn an unrelated checkout into the
    // registered one, even if it is swapped after the metadata check above.
    let (registered_checkout, checkout, linked_common_dir) = tokio::join!(
        fs.canonicalize(&registered_checkout_uri, /*sandbox*/ None),
        fs.canonicalize(&checkout_uri, /*sandbox*/ None),
        fs.canonicalize(&linked_common_dir_uri, /*sandbox*/ None),
    );
    // PathUri equality folds Windows ASCII case, but even Windows directories
    // can be case-sensitive. Canonical filesystem identities must match exactly.
    if registered_checkout.ok()?.to_url() != checkout.ok()?.to_url()
        || linked_common_dir.ok()?.to_url() != common_dir_uri.to_url()
    {
        return None;
    }

    // Preserve the existing trust key (including path aliases), but prove that
    // its main checkout actually owns the canonical common directory. A main
    // checkout made with --separate-git-dir may have a regular .git pointer.
    let common_dir = git_dir_path.parent()?.parent()?;
    let main_root = common_dir.parent()?;
    let main_dot_git_uri = main_root.join(".git")?.uri();
    let main_metadata = fs
        .get_metadata(&main_dot_git_uri, Default::default(), /*sandbox*/ None)
        .await
        .ok()?;
    let main_git_dir_uri = if main_metadata.is_directory {
        main_dot_git_uri
    } else {
        read_gitdir_file(fs, &main_dot_git_uri).await?
    };
    if fs
        .canonicalize(&main_git_dir_uri, /*sandbox*/ None)
        .await
        .ok()?
        .to_url()
        != common_dir_uri.to_url()
    {
        return None;
    }
    Some(main_root)
}

async fn read_gitdir_file(fs: &dyn ExecutorFileSystem, path: &PathUri) -> Option<PathUri> {
    let contents = read_metadata_file(fs, path).await?;
    let target = contents.trim_ascii().strip_prefix(b"gitdir:")?.trim_ascii();
    if target.is_empty() {
        return None;
    }
    path.parent()?.join_native_bytes(target).ok()
}

async fn read_metadata_file(fs: &dyn ExecutorFileSystem, path: &PathUri) -> Option<Vec<u8>> {
    let metadata = fs
        .get_metadata(path, Default::default(), /*sandbox*/ None)
        .await
        .ok()?;
    if !metadata.is_file || metadata.is_symlink || metadata.size > MAX_GIT_METADATA_FILE_BYTES {
        return None;
    }
    // Keep using fs/readFile for independently deployed older exec-servers.
    // Their streaming fs/open API is not universally available yet.
    let bytes = fs
        .read_file(path, Default::default(), /*sandbox*/ None)
        .await
        .ok()?;
    (bytes.len() as u64 <= MAX_GIT_METADATA_FILE_BYTES).then_some(bytes)
}
