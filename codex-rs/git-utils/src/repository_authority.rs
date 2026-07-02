use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use same_file::Handle;

use helpers::RegisteredWorktree;
use helpers::RegisteredWorktreeReadError;
use helpers::registry_error;

mod authority;
mod helpers;
mod plain_config;
pub(crate) use authority::RepositoryAuthority;
pub(crate) use plain_config::CommonConfigAuthority;
pub(crate) use plain_config::inspect_plain_common_config_authority;

#[derive(Debug)]
struct RepositoryAuthorityRefusal {
    message: String,
}

impl std::fmt::Display for RepositoryAuthorityRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RepositoryAuthorityRefusal {}

/// Preserve the provenance of an explicit repository-authority policy
/// refusal while retaining `io::Error` at the Git runner boundary.
pub(crate) fn authority_refusal(message: impl Into<String>) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        RepositoryAuthorityRefusal {
            message: message.into(),
        },
    )
}

pub(crate) fn is_authority_refusal(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(<dyn std::error::Error + Send + Sync>::is::<RepositoryAuthorityRefusal>)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryMetadataKind {
    StandardPrimary,
    SeparatePrimary,
    Linked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedRepositoryMetadata {
    pub(crate) marker: PathBuf,
    pub(crate) git_dir: PathBuf,
    pub(crate) common_dir: PathBuf,
    pub(crate) kind: RepositoryMetadataKind,
    pub(crate) routes: Vec<RepositoryMetadataRoute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryMetadataRoute {
    pub(crate) spelling: PathBuf,
    pub(crate) target: PathBuf,
    pub(crate) kind: RepositoryMetadataRouteKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryMetadataRouteKind {
    Traversal,
    StandardDirectory,
}

pub(crate) fn read_bounded_marker(path: &Path) -> io::Result<Vec<u8>> {
    #[cfg(test)]
    BOUNDED_MARKER_READ_COUNT.with(|count| count.set(count.get() + 1));
    read_bounded_file(path, 64 * 1024, "Git metadata marker is too large")
}

#[cfg(test)]
thread_local! {
    static BOUNDED_MARKER_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_bounded_marker_read_count() {
    BOUNDED_MARKER_READ_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn bounded_marker_read_count() -> usize {
    BOUNDED_MARKER_READ_COUNT.with(std::cell::Cell::get)
}

pub(crate) fn parse_marker_path(contents: &[u8], prefix: &[u8]) -> io::Result<PathBuf> {
    let contents = contents
        .strip_prefix(prefix)
        .ok_or_else(|| invalid_data("malformed Git metadata marker"))?;
    let contents = trim_trailing_ascii_whitespace(contents);
    if contents.is_empty() || contents.contains(&0) {
        return Err(invalid_data("empty Git metadata marker path"));
    }
    bytes_to_path(contents)
}

pub(crate) fn resolve_repository_metadata(
    dot_git: &Path,
) -> io::Result<ResolvedRepositoryMetadata> {
    let metadata = std::fs::symlink_metadata(dot_git)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_data("symlinked Git metadata marker"));
    }
    let marker_is_directory = metadata.is_dir();
    let (git_dir, git_dir_route, default_kind) = if marker_is_directory {
        (
            std::fs::canonicalize(dot_git)?,
            Some((
                dot_git.to_path_buf(),
                RepositoryMetadataRouteKind::StandardDirectory,
            )),
            RepositoryMetadataKind::StandardPrimary,
        )
    } else if metadata.is_file() {
        let contents = read_bounded_marker(dot_git)?;
        let route = raw_path_against(
            parse_marker_path(&contents, b"gitdir: ")?,
            dot_git
                .parent()
                .ok_or_else(|| invalid_data("Git metadata marker has no parent"))?,
        );
        let git_dir = std::fs::canonicalize(&route)?;
        (
            git_dir,
            Some((route, RepositoryMetadataRouteKind::Traversal)),
            RepositoryMetadataKind::SeparatePrimary,
        )
    } else {
        return Err(invalid_data("unsupported Git metadata marker"));
    };
    let commondir = git_dir.join("commondir");
    match std::fs::symlink_metadata(&commondir) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(invalid_data("unsupported Git common-dir marker"));
            }
            let contents = read_bounded_marker(&commondir)?;
            let route = raw_path_against(parse_marker_path(&contents, b"")?, &git_dir);
            let common_dir = std::fs::canonicalize(&route)?;
            let mut routes = git_dir_route
                .map(|(spelling, kind)| RepositoryMetadataRoute {
                    spelling,
                    target: git_dir.clone(),
                    kind,
                })
                .into_iter()
                .collect::<Vec<_>>();
            routes.push(RepositoryMetadataRoute {
                spelling: route,
                target: common_dir.clone(),
                kind: RepositoryMetadataRouteKind::Traversal,
            });
            return Ok(ResolvedRepositoryMetadata {
                marker: dot_git.to_path_buf(),
                git_dir,
                common_dir,
                kind: RepositoryMetadataKind::Linked,
                routes,
            });
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) => {}
        Err(error) => return Err(error),
    }

    if git_dir
        .parent()
        .is_some_and(|parent| parent.file_name() == Some(std::ffi::OsStr::new("worktrees")))
    {
        let common_dir = git_dir
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| invalid_data("linked Git admin dir has no common parent"))?;
        let common_dir = std::fs::canonicalize(common_dir)?;
        let routes = git_dir_route
            .map(|(spelling, kind)| RepositoryMetadataRoute {
                spelling,
                target: git_dir.clone(),
                kind,
            })
            .into_iter()
            .collect();
        return Ok(ResolvedRepositoryMetadata {
            marker: dot_git.to_path_buf(),
            git_dir,
            common_dir,
            kind: RepositoryMetadataKind::Linked,
            routes,
        });
    }

    Ok(ResolvedRepositoryMetadata {
        marker: dot_git.to_path_buf(),
        common_dir: git_dir.clone(),
        git_dir: git_dir.clone(),
        kind: default_kind,
        routes: git_dir_route
            .map(|(spelling, kind)| RepositoryMetadataRoute {
                spelling,
                target: git_dir,
                kind,
            })
            .into_iter()
            .collect(),
    })
}

pub(crate) fn repository_common_dir_for_candidate_root(root: &Path) -> io::Result<Option<PathBuf>> {
    let marker = root.join(".git");
    match std::fs::symlink_metadata(&marker) {
        Ok(_) => resolve_repository_metadata(&marker).map(|metadata| Some(metadata.common_dir)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn linked_common_dir_for_root(root: &Path) -> io::Result<Option<PathBuf>> {
    let marker = root.join(".git");
    let metadata = match std::fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Err(invalid_data("symlinked Git metadata marker"));
    }
    if !metadata.is_file() && !metadata.is_dir() {
        return Ok(None);
    }
    let resolved = resolve_repository_metadata(&marker)?;
    Ok((resolved.kind == RepositoryMetadataKind::Linked).then_some(resolved.common_dir))
}

fn registered_worktrees(
    common_dir: &Path,
) -> Result<Vec<RegisteredWorktree>, RegisteredWorktreeReadError> {
    let worktrees = common_dir.join("worktrees");
    match std::fs::symlink_metadata(&worktrees) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(registry_error(
                &worktrees,
                invalid_data("unsupported Git worktree registry"),
            ));
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(registry_error(&worktrees, error)),
    }

    let mut registrations = Vec::new();
    let entries =
        std::fs::read_dir(&worktrees).map_err(|error| registry_error(&worktrees, error))?;
    for entry in entries {
        let admin_dir = entry
            .map_err(|error| registry_error(&worktrees, error))?
            .path();
        let metadata = std::fs::symlink_metadata(&admin_dir)
            .map_err(|error| registry_error(&admin_dir, error))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(registry_error(
                &admin_dir,
                invalid_data("unsupported Git worktree registry entry"),
            ));
        }
        let gitdir_marker = admin_dir.join("gitdir");
        let metadata = std::fs::symlink_metadata(&gitdir_marker)
            .map_err(|error| registry_error(&gitdir_marker, error))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(registry_error(
                &gitdir_marker,
                invalid_data("unsupported Git worktree gitdir marker"),
            ));
        }
        let contents = read_bounded_marker(&gitdir_marker)
            .map_err(|error| registry_error(&gitdir_marker, error))?;
        let marker_route = raw_path_against(
            parse_marker_path(&contents, b"")
                .map_err(|error| registry_error(&gitdir_marker, error))?,
            &admin_dir,
        );
        #[cfg(windows)]
        if marker_route
            .to_str()
            .is_none_or(crate::path_authority::windows_path_is_ambiguous)
        {
            return Err(registry_error(
                &gitdir_marker,
                invalid_data("ambiguous Windows Git worktree path"),
            ));
        }
        // Git writes this protected registry marker from real paths (either
        // absolute or relative). Normalize it only after retaining the raw
        // route long enough to apply platform path grammar.
        let marker = resolve_path_against(marker_route.clone(), &admin_dir);
        if marker.file_name() != Some(std::ffi::OsStr::new(".git")) {
            return Err(registry_error(
                &gitdir_marker,
                invalid_data("malformed Git worktree gitdir marker"),
            ));
        }
        let root = marker
            .parent()
            .ok_or_else(|| {
                registry_error(
                    &gitdir_marker,
                    invalid_data("Git worktree marker has no parent"),
                )
            })?
            .to_path_buf();
        registrations.push(RegisteredWorktree {
            root,
            admin_dir,
            marker_route,
        });
    }
    Ok(registrations)
}

pub(crate) fn primary_authority_is_proven(
    current_root: &Path,
    common_dir: &Path,
    roots: &[PathBuf],
) -> io::Result<bool> {
    for root in roots {
        if directories_refer_to_same_location(root, current_root).unwrap_or(false) {
            continue;
        }
        let marker = root.join(".git");
        match std::fs::symlink_metadata(&marker) {
            Ok(_) => {
                let resolved = resolve_repository_metadata(&marker)?;
                if resolved.kind != RepositoryMetadataKind::Linked
                    && directories_refer_to_same_location(&resolved.common_dir, common_dir)?
                {
                    return Ok(true);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

pub(crate) fn directories_refer_to_same_location(left: &Path, right: &Path) -> io::Result<bool> {
    let left_metadata = match std::fs::metadata(left) {
        Ok(metadata) => metadata,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let right_metadata = match std::fs::metadata(right) {
        Ok(metadata) => metadata,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    if !left_metadata.is_dir() || !right_metadata.is_dir() {
        return Ok(false);
    }
    Ok(Handle::from_path(left)? == Handle::from_path(right)?)
}

pub(crate) fn path_has_untrusted_root_identity_ancestor(path: &Path, roots: &[PathBuf]) -> bool {
    let mut root_identities = Vec::new();
    for root in roots {
        match std::fs::metadata(root) {
            Ok(metadata) if metadata.is_dir() => match Handle::from_path(root) {
                Ok(identity) => root_identities.push(identity),
                Err(_) => return true,
            },
            Ok(_) => return true,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(_) => return true,
        }
    }
    if root_identities.is_empty() {
        return false;
    }

    let start = match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => path,
        Ok(_) => path.parent().unwrap_or(path),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            path.parent().unwrap_or(path)
        }
        Err(_) => return true,
    };
    for ancestor in start.ancestors() {
        match std::fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => match Handle::from_path(ancestor) {
                Ok(identity) if root_identities.contains(&identity) => return true,
                Ok(_) => {}
                Err(_) => return true,
            },
            Ok(_) => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(_) => return true,
        }
    }
    false
}

fn read_bounded_file(path: &Path, max_bytes: u64, too_large: &str) -> io::Result<Vec<u8>> {
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut contents = Vec::new();
    file.take(max_bytes + 1).read_to_end(&mut contents)?;
    if contents.len() as u64 > max_bytes {
        return Err(invalid_data(too_large));
    }
    Ok(contents)
}

fn resolve_path_against(path: PathBuf, base: &Path) -> PathBuf {
    AbsolutePathBuf::resolve_path_against_base(path, base).into_path_buf()
}

fn raw_path_against(path: PathBuf, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn bytes_to_path(bytes: &[u8]) -> io::Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())))
    }
    #[cfg(not(unix))]
    {
        Ok(PathBuf::from(std::str::from_utf8(bytes).map_err(|_| {
            invalid_data("non-UTF-8 Git filesystem path")
        })?))
    }
}

fn trim_trailing_ascii_whitespace(mut value: &[u8]) -> &[u8] {
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "repository_authority_tests.rs"]
mod tests;
