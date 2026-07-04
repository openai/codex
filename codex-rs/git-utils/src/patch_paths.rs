//! Effective patch-path discovery and safe staging guards.

use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::apply::write_temp_patch;
use crate::git_command::GitRunner;
use crate::git_config::path_is_within;
use crate::guarded_config::GuardedGitConfig;

/// Extract effective patch paths through a bound operation configuration.
pub(crate) fn extract_effective_paths_from_patch_guarded(
    config: &GuardedGitConfig<'_>,
    patch_path: &Path,
    revert: bool,
) -> io::Result<Vec<String>> {
    let forward_paths = git_apply_numstat_paths_guarded(config, patch_path, revert)?;
    let reverse_paths = git_apply_numstat_paths_guarded(config, patch_path, !revert)?;
    normalize_effective_patch_paths(forward_paths, reverse_paths)
}

fn normalize_effective_patch_paths(
    forward_paths: Vec<String>,
    reverse_paths: Vec<String>,
) -> io::Result<Vec<String>> {
    if forward_paths.len() != reverse_paths.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "forward and reverse patch parsing returned different path counts",
        ));
    }
    let effective_paths: std::collections::BTreeSet<String> =
        forward_paths.into_iter().chain(reverse_paths).collect();
    if effective_paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "patch does not identify any paths",
        ));
    }
    effective_paths
        .into_iter()
        .map(validate_patch_path)
        .collect()
}

fn git_apply_numstat_paths_guarded(
    config: &GuardedGitConfig<'_>,
    patch_path: &Path,
    revert: bool,
) -> io::Result<Vec<String>> {
    let mut command = config.apply_command()?;
    command.args(["--numstat", "-z"]);
    if revert {
        command.arg("-R");
    }
    command.arg("--").arg(patch_path);
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "failed to parse patch paths: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    parse_numstat_paths(&output.stdout)
}

/// Best-effort extraction of the paths Git would apply.
///
/// Security-sensitive callers must use the fallible internal extractor so an
/// invalid or ambiguous patch is rejected instead of becoming an empty list.
pub fn extract_paths_from_patch(diff_text: &str) -> Vec<String> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    extract_paths_from_patch_from_cwd(diff_text, &cwd)
}

fn extract_paths_from_patch_from_cwd(diff_text: &str, cwd: &Path) -> Vec<String> {
    let Ok((tmpdir, patch_path)) = write_temp_patch(diff_text) else {
        return Vec::new();
    };
    let paths = (|| -> io::Result<Vec<String>> {
        let git = GitRunner::for_cwd_io(cwd)?;
        let git_root = crate::get_git_repo_root(cwd)
            .ok_or_else(|| io::Error::other("not a Git repository"))?;
        let git_root = std::fs::canonicalize(git_root)?;
        let config = GuardedGitConfig::authorize(&git, &git_root, Vec::new())?;
        extract_effective_paths_from_patch_guarded(&config, &patch_path, /*revert*/ false)
    })()
    .unwrap_or_default();
    drop(tmpdir);
    paths
}

fn parse_numstat_paths(output: &[u8]) -> io::Result<Vec<String>> {
    if !output.is_empty() && !output.ends_with(&[0]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "git apply returned an unterminated numstat path record",
        ));
    }
    let mut paths = Vec::new();
    let mut records = output.split(|byte| *byte == 0).peekable();
    while let Some(record) = records.next() {
        if record.is_empty() && records.peek().is_none() {
            break;
        }
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let _added = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "git apply returned an ambiguous numstat path record",
            )
        })?;
        let _deleted = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "git apply returned an ambiguous numstat path record",
            )
        })?;
        let path = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "git apply returned an ambiguous numstat path record",
            )
        })?;
        if path.is_empty() {
            let old = records
                .next()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "git apply returned an incomplete rename path record",
                    )
                })?;
            let new = records
                .next()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "git apply returned an incomplete rename path record",
                    )
                })?;
            insert_numstat_path(&mut paths, old)?;
            insert_numstat_path(&mut paths, new)?;
        } else {
            insert_numstat_path(&mut paths, path)?;
        }
    }
    Ok(paths)
}

fn insert_numstat_path(paths: &mut Vec<String>, path: &[u8]) -> io::Result<()> {
    let path = std::str::from_utf8(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "git apply returned a non-UTF-8 patch path",
        )
    })?;
    paths.push(path.to_string());
    Ok(())
}

fn validate_patch_path(path: String) -> io::Result<String> {
    if path.starts_with('/')
        || path.ends_with('/')
        || invalid_platform_patch_path(&path)
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "patch path is not a normalized repository-relative path",
        ));
    }
    Ok(path)
}

#[cfg(windows)]
fn invalid_platform_patch_path(path: &str) -> bool {
    invalid_windows_patch_path(path)
}

#[cfg(not(windows))]
fn invalid_platform_patch_path(_path: &str) -> bool {
    false
}

#[cfg(any(windows, test))]
fn invalid_windows_patch_path(path: &str) -> bool {
    path.split('/').any(invalid_windows_patch_component)
}

#[cfg(any(windows, test))]
fn invalid_windows_patch_component(component: &str) -> bool {
    if component.bytes().any(|byte| {
        byte <= 0x1f || matches!(byte, b'\\' | b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*')
    }) || matches!(component.as_bytes().last(), Some(b'.' | b' '))
    {
        return true;
    }

    let reserved_suffix = |suffix: &str| {
        let suffix = suffix.trim_start_matches(' ');
        suffix.is_empty() || matches!(suffix.as_bytes().first(), Some(b'.' | b':'))
    };

    if ["AUX", "CON", "CONIN$", "CONOUT$", "NUL", "PRN"]
        .iter()
        .any(|reserved| {
            component
                .get(..reserved.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(reserved))
                && component.get(reserved.len()..).is_some_and(reserved_suffix)
        })
    {
        return true;
    }

    [b"COM", b"LPT"].iter().any(|reserved| {
        let Some(rest) = component.get(3..) else {
            return false;
        };
        let mut chars = rest.chars();
        component.as_bytes()[..3].eq_ignore_ascii_case(*reserved)
            && matches!(chars.next(), Some('1'..='9' | '¹' | '²' | '³'))
            && reserved_suffix(chars.as_str())
    })
}

/// Stage only the files that actually exist on disk for the given diff.
pub fn stage_paths(git_root: &Path, diff: &str) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(git_root)?;
    let requested_cwd = std::fs::canonicalize(git_root)?;
    let canonical_root = crate::get_git_repo_root(&requested_cwd)
        .ok_or_else(|| io::Error::other("not a Git repository"))
        .and_then(std::fs::canonicalize)?;
    let config = GuardedGitConfig::authorize(&git, &canonical_root, Vec::new())?;
    let (tmpdir, patch_path) = write_temp_patch(diff)?;
    let paths =
        extract_effective_paths_from_patch_guarded(&config, &patch_path, /*revert*/ true)?;
    let _guard = tmpdir;
    stage_effective_paths_guarded(&config, &paths)
}

/// Preserve the existing reverse staging behavior while constructing every
/// Git child from the operation-owned configuration capability.
pub(crate) fn stage_effective_paths_guarded(
    config: &GuardedGitConfig<'_>,
    paths: &[String],
) -> io::Result<()> {
    let confined = confine_patch_paths_guarded(config, paths)?;
    let mut existing = Vec::new();
    for path in confined.into_exact_leaves()? {
        let joined = config.canonical_root().join(&path);
        if let Ok(metadata) = std::fs::symlink_metadata(&joined) {
            if leaf_is_traversable_directory(metadata.file_type()) {
                return Err(containment_error(
                    "refusing to recursively stage a directory patch path",
                ));
            }
            existing.push(path);
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    let mut args = vec!["--".to_string()];
    args.extend(existing);
    let mut command = config.literal_add_command()?;
    command.args(&args);
    let _output = command.output()?;
    // Preserve the existing best-effort staging contract on non-zero status.
    Ok(())
}

#[cfg(not(windows))]
fn leaf_is_traversable_directory(file_type: std::fs::FileType) -> bool {
    file_type.is_dir()
}

#[cfg(windows)]
fn leaf_is_traversable_directory(file_type: std::fs::FileType) -> bool {
    use std::os::windows::fs::FileTypeExt;

    // Git traverses junctions and container-mapped directory symlinks. Refuse
    // all directory-valued reparse leaves; true file symlinks remain allowed.
    file_type.is_dir() || file_type.is_symlink_dir()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfinedPathRole {
    StrictAncestor,
    Leaf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfinedPathOrigin {
    Raw,
    Canonical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfinedPathCandidate {
    pub(crate) path: String,
    pub(crate) origin: ConfinedPathOrigin,
    pub(crate) role: ConfinedPathRole,
    pub(crate) depth: usize,
}

impl ConfinedPathCandidate {
    fn new(path: String, origin: ConfinedPathOrigin, role: ConfinedPathRole) -> Self {
        let depth = path.split('/').count();
        Self {
            path,
            origin,
            role,
            depth,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConfinedPatchPath {
    pub(crate) exact_leaf: String,
    pub(crate) candidates: Vec<ConfinedPathCandidate>,
}

#[derive(Debug)]
pub(crate) struct ConfinedPatchPaths {
    pub(crate) entries: Vec<ConfinedPatchPath>,
}

impl ConfinedPatchPaths {
    fn into_exact_leaves(self) -> io::Result<Vec<String>> {
        self.entries
            .into_iter()
            .map(|entry| {
                let expected_depth = entry.exact_leaf.split('/').count();
                entry
                    .candidates
                    .into_iter()
                    .find(|candidate| {
                        candidate.origin == ConfinedPathOrigin::Raw
                            && candidate.role == ConfinedPathRole::Leaf
                            && candidate.depth == expected_depth
                            && candidate.path == entry.exact_leaf
                    })
                    .map(|candidate| candidate.path)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "confined patch path is missing its exact raw leaf",
                        )
                    })
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) fn confine_patch_paths(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
) -> io::Result<ConfinedPatchPaths> {
    let config = GuardedGitConfig::authorize(git, git_root, Vec::new())?;
    confine_patch_paths_guarded(&config, paths)
}

fn confine_patch_paths_guarded(
    config: &GuardedGitConfig<'_>,
    paths: &[String],
) -> io::Result<ConfinedPatchPaths> {
    let canonical_root = std::fs::canonicalize(config.canonical_root())?;
    let metadata_dirs = canonical_git_metadata_dirs_guarded(config)?;
    confine_patch_paths_with_metadata(&canonical_root, paths, &metadata_dirs)
}

fn confine_patch_paths_with_metadata(
    canonical_root: &Path,
    paths: &[String],
    metadata_dirs: &[PathBuf],
) -> io::Result<ConfinedPatchPaths> {
    if paths.is_empty() {
        return Ok(ConfinedPatchPaths {
            entries: Vec::new(),
        });
    }
    if paths.iter().all(|path| !path.contains('/')) {
        return Ok(ConfinedPatchPaths {
            entries: paths
                .iter()
                .map(|leaf| ConfinedPatchPath {
                    exact_leaf: leaf.clone(),
                    candidates: [ConfinedPathOrigin::Raw, ConfinedPathOrigin::Canonical]
                        .map(|origin| {
                            ConfinedPathCandidate::new(leaf.clone(), origin, ConfinedPathRole::Leaf)
                        })
                        .into(),
                })
                .collect(),
        });
    }

    let mut entries = Vec::with_capacity(paths.len());
    let mut prefix_cache = std::collections::BTreeMap::new();

    for leaf in paths {
        let components = leaf.split('/').collect::<Vec<_>>();
        let mut candidates = Vec::new();
        insert_candidate_prefixes(
            components.iter().copied(),
            ConfinedPathOrigin::Raw,
            &mut candidates,
        );

        let (existing_len, mut projected) = longest_existing_strict_prefix(
            canonical_root,
            &components,
            metadata_dirs,
            &mut prefix_cache,
        )?;
        projected.extend(
            components[existing_len..]
                .iter()
                .map(|component| (*component).to_string()),
        );
        insert_candidate_prefixes(
            projected.iter().map(String::as_str),
            ConfinedPathOrigin::Canonical,
            &mut candidates,
        );
        entries.push(ConfinedPatchPath {
            exact_leaf: leaf.clone(),
            candidates,
        });
    }

    Ok(ConfinedPatchPaths { entries })
}

fn longest_existing_strict_prefix(
    canonical_root: &Path,
    components: &[&str],
    metadata_dirs: &[PathBuf],
    prefix_cache: &mut std::collections::BTreeMap<String, Option<Vec<String>>>,
) -> io::Result<(usize, Vec<String>)> {
    let mut longest = (0, Vec::new());
    for existing_len in 1..components.len() {
        let prefix = components[..existing_len].join("/");
        if let Some(cached) = prefix_cache.get(&prefix) {
            if let Some(relative) = cached {
                longest = (existing_len, relative.clone());
            }
            continue;
        }
        match std::fs::canonicalize(canonical_root.join(&prefix)) {
            Ok(resolved) => {
                let relative =
                    confined_relative_components(&resolved, canonical_root, metadata_dirs)?;
                prefix_cache.insert(prefix, Some(relative.clone()));
                longest = (existing_len, relative);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                prefix_cache.insert(prefix, None);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(longest)
}

fn confined_relative_components(
    resolved: &Path,
    canonical_root: &Path,
    metadata_dirs: &[PathBuf],
) -> io::Result<Vec<String>> {
    if metadata_dirs
        .iter()
        .any(|metadata_dir| path_is_within(resolved, metadata_dir))
    {
        return Err(containment_error(
            "patch path alias resolves into Git repository metadata",
        ));
    }
    let relative = resolved
        .strip_prefix(canonical_root)
        .map_err(|_| containment_error("patch path alias resolves outside the Git worktree"))?;
    if relative.as_os_str().is_empty() {
        return Err(containment_error(
            "patch path alias resolves to the Git worktree root",
        ));
    }
    relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "patch path alias is not valid UTF-8",
                    )
                })
        })
        .collect()
}

fn containment_error(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

fn canonical_git_metadata_dirs_guarded(config: &GuardedGitConfig<'_>) -> io::Result<Vec<PathBuf>> {
    let queries = [
        ["rev-parse", "--absolute-git-dir"],
        ["rev-parse", "--git-common-dir"],
    ];
    let mut metadata_dirs = std::collections::BTreeSet::new();
    for args in queries {
        let mut command = config.rev_parse_command()?;
        command.args(&args[1..]);
        let output = command.output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to resolve Git repository metadata (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let path = String::from_utf8_lossy(&output.stdout);
        let path = path.trim_end_matches(['\r', '\n']);
        if path.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned an empty repository metadata path",
            ));
        }
        let path = PathBuf::from(path);
        let absolute = if path.is_absolute() {
            path
        } else {
            config.canonical_root().join(path)
        };
        metadata_dirs.insert(std::fs::canonicalize(absolute)?);
    }
    Ok(metadata_dirs.into_iter().collect())
}

fn insert_candidate_prefixes<'a>(
    components: impl IntoIterator<Item = &'a str>,
    origin: ConfinedPathOrigin,
    candidates: &mut Vec<ConfinedPathCandidate>,
) {
    let components = components.into_iter().collect::<Vec<_>>();
    let mut path = String::new();
    for (index, component) in components.iter().enumerate() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(component);
        candidates.push(ConfinedPathCandidate::new(
            path.clone(),
            origin,
            if index + 1 == components.len() {
                ConfinedPathRole::Leaf
            } else {
                ConfinedPathRole::StrictAncestor
            },
        ));
    }
}

#[cfg(test)]
#[path = "patch_paths_tests.rs"]
mod tests;
