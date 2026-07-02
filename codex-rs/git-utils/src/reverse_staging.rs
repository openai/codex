//! Safe preparatory index staging for direct reverse patch application.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use crate::exact_staging::update_index_exact_paths;
use crate::git_command::GitRunner;
use crate::patch_paths::confine_patch_paths;
use crate::patch_paths::leaf_is_traversable_directory;
use crate::patch_paths::leaf_may_run_git_content_filter;

#[cfg(unix)]
const WORKTREE_FILEMODE_CONFIG_ARGS: &[&str] = &["-c", "core.filemode=true"];
#[cfg(not(unix))]
const WORKTREE_FILEMODE_CONFIG_ARGS: &[&str] = &[];

pub(crate) fn stage_effective_paths_for_reverse(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<()> {
    let confined = confine_patch_paths(git, git_root, paths)?;
    let mut worktree_paths = BTreeMap::new();
    for path in confined.into_exact_leaves()? {
        let joined = git_root.join(&path);
        let state = match std::fs::symlink_metadata(&joined) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if leaf_is_traversable_directory(file_type) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "refusing to recursively stage a directory patch path",
                    ));
                }
                ReverseWorktreePath {
                    exists: true,
                    may_run_content_filter: leaf_may_run_git_content_filter(file_type),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => ReverseWorktreePath {
                exists: false,
                may_run_content_filter: false,
            },
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "failed to inspect reverse patch path {}: {error}",
                        joined.display()
                    ),
                ));
            }
        };
        worktree_paths.insert(path, state);
    }

    let exact_paths = worktree_paths.keys().cloned().collect::<Vec<_>>();
    let index_entries = read_reverse_index_entries(git, git_root, &exact_paths, git_config_args)?;
    for path in &exact_paths {
        let entries = index_entries.get(path).map(Vec::as_slice).unwrap_or(&[]);
        if !matches!(
            entries,
            [] | [ReverseIndexEntry {
                tag: b'H',
                stage: 0
            }]
        ) {
            return Err(reverse_staging_error(format!(
                "refusing to stage reverse patch path {path:?} because its index entry is conflicted or carries assume-unchanged/skip-worktree state"
            )));
        }
    }

    let cached_invisible_status = read_name_status(
        git,
        git_root,
        &exact_paths,
        git_config_args,
        &[
            "--literal-pathspecs",
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "--no-renames",
            "--ita-invisible-in-index",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ],
        "inspect reverse paths with intent-to-add entries hidden",
    )?;
    let cached_visible_status = read_name_status(
        git,
        git_root,
        &exact_paths,
        git_config_args,
        &[
            "--literal-pathspecs",
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "--no-renames",
            "--ita-visible-in-index",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ],
        "inspect reverse paths with intent-to-add entries visible",
    )?;
    if cached_invisible_status != cached_visible_status {
        let path = exact_paths
            .iter()
            .find(|path| cached_invisible_status.get(*path) != cached_visible_status.get(*path))
            .ok_or_else(|| invalid_reverse_index_output("missing intent-to-add path"))?;
        return Err(reverse_staging_error(format!(
            "refusing to stage reverse patch path {path:?} because it has intent-to-add index state"
        )));
    }
    let worktree_changed_paths = read_changed_paths(
        git,
        git_root,
        &exact_paths,
        git_config_args,
        WORKTREE_FILEMODE_CONFIG_ARGS,
        &[
            "--literal-pathspecs",
            "diff-files",
            "--name-only",
            "-z",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ],
        "compare reverse path index entries with the worktree",
    )?;

    let mut staging_candidates = Vec::new();
    for path in &exact_paths {
        let worktree = worktree_paths
            .get(path)
            .ok_or_else(|| invalid_reverse_index_output("missing worktree path state"))?;
        let entries = index_entries.get(path).map(Vec::as_slice).unwrap_or(&[]);
        match entries {
            [] if !worktree.exists => continue,
            [] => staging_candidates.push(path.clone()),
            [_] if worktree_changed_paths.contains(path) => {
                staging_candidates.push(path.clone());
            }
            [_] => continue,
            _ => unreachable!("index entries were validated above"),
        }
    }
    if staging_candidates.is_empty() {
        return Ok(());
    }

    if let Some(path) = staging_candidates
        .iter()
        .find(|path| cached_visible_status.contains_key(*path))
    {
        return Err(reverse_staging_error(format!(
            "refusing to prepare a reverse patch because staging {path:?} would replace existing staged data"
        )));
    }

    let content_filter_paths = staging_candidates
        .iter()
        .filter(|path| {
            worktree_paths
                .get(*path)
                .is_some_and(|state| state.may_run_content_filter)
        })
        .cloned()
        .collect::<Vec<_>>();
    let result = update_index_exact_paths(
        git,
        git_root,
        &staging_candidates,
        &content_filter_paths,
        git_config_args,
    )?;
    if result.exit_code == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "failed to stage reverse patch paths (exit {}): {}",
            result.exit_code,
            result.stderr.trim()
        )))
    }
}

#[derive(Clone, Copy)]
struct ReverseWorktreePath {
    exists: bool,
    may_run_content_filter: bool,
}

#[derive(Clone, Copy)]
struct ReverseIndexEntry {
    tag: u8,
    stage: u8,
}

fn read_reverse_index_entries(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<BTreeMap<String, Vec<ReverseIndexEntry>>> {
    let mut command = git.command();
    command
        .args(git_config_args)
        .args([
            "--literal-pathspecs",
            "ls-files",
            "-v",
            "--stage",
            "-z",
            "--",
        ])
        .args(paths)
        .current_dir(git_root);
    let output = git.output(command)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "failed to inspect reverse patch index entries: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let records = if output.stdout.is_empty() {
        &[][..]
    } else {
        output
            .stdout
            .strip_suffix(&[0])
            .ok_or_else(|| invalid_reverse_index_output("unterminated git ls-files output"))?
    };

    let expected = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut entries = BTreeMap::<String, Vec<ReverseIndexEntry>>::new();
    if records.is_empty() {
        return Ok(entries);
    }
    for record in records.split(|byte| *byte == 0) {
        let Some(separator) = record.iter().position(|byte| *byte == b'\t') else {
            return Err(invalid_reverse_index_output(
                "git ls-files returned a malformed stage record",
            ));
        };
        let (header, path_with_separator) = record.split_at(separator);
        let path = &path_with_separator[1..];
        let path = std::str::from_utf8(path)
            .map_err(|_| invalid_reverse_index_output("git ls-files returned a non-UTF-8 path"))?;
        if !expected.contains(path) {
            return Err(invalid_reverse_index_output(
                "git ls-files returned an unexpected path",
            ));
        }
        let header = std::str::from_utf8(header).map_err(|_| {
            invalid_reverse_index_output("git ls-files returned a non-UTF-8 stage header")
        })?;
        let mut fields = header.split_ascii_whitespace();
        let tag = fields
            .next()
            .filter(|tag| tag.len() == 1)
            .map(|tag| tag.as_bytes()[0])
            .ok_or_else(|| invalid_reverse_index_output("missing git ls-files status tag"))?;
        let _mode = fields
            .next()
            .ok_or_else(|| invalid_reverse_index_output("missing git ls-files mode"))?;
        let _object_id = fields
            .next()
            .ok_or_else(|| invalid_reverse_index_output("missing git ls-files object ID"))?;
        let stage = fields
            .next()
            .ok_or_else(|| invalid_reverse_index_output("missing git ls-files stage"))?
            .parse::<u8>()
            .map_err(|_| invalid_reverse_index_output("invalid git ls-files stage"))?;
        if fields.next().is_some() {
            return Err(invalid_reverse_index_output(
                "git ls-files returned an ambiguous stage header",
            ));
        }
        entries
            .entry(path.to_string())
            .or_default()
            .push(ReverseIndexEntry { tag, stage });
    }
    Ok(entries)
}

fn read_name_status(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
    args: &[&str],
    operation: &str,
) -> io::Result<BTreeMap<String, u8>> {
    let output =
        run_path_list_command(git, git_root, paths, git_config_args, &[], args, operation)?;
    let fields = nul_fields(&output, operation)?;
    if fields.len() % 2 != 0 {
        return Err(invalid_reverse_index_output(
            "git diff returned an incomplete name-status record",
        ));
    }
    let expected = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut statuses = BTreeMap::new();
    for record in fields.chunks_exact(2) {
        let status = record[0];
        if status.len() != 1 || !status[0].is_ascii_alphabetic() {
            return Err(invalid_reverse_index_output(
                "git diff returned an invalid name-status value",
            ));
        }
        let path = parse_expected_path(record[1], &expected, "git diff")?;
        if statuses.insert(path, status[0]).is_some() {
            return Err(invalid_reverse_index_output(
                "git diff returned a duplicate path",
            ));
        }
    }
    Ok(statuses)
}

fn read_changed_paths(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
    extra_git_args: &[&str],
    args: &[&str],
    operation: &str,
) -> io::Result<BTreeSet<String>> {
    let output = run_path_list_command(
        git,
        git_root,
        paths,
        git_config_args,
        extra_git_args,
        args,
        operation,
    )?;
    let expected = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut changed = BTreeSet::new();
    for path in nul_fields(&output, operation)? {
        let path = parse_expected_path(path, &expected, operation)?;
        if !changed.insert(path) {
            return Err(invalid_reverse_index_output(
                "Git returned a duplicate changed path",
            ));
        }
    }
    Ok(changed)
}

fn run_path_list_command(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
    extra_git_args: &[&str],
    args: &[&str],
    operation: &str,
) -> io::Result<Vec<u8>> {
    let mut command = git.command();
    command
        .args(git_config_args)
        .args(extra_git_args)
        .args(args)
        .arg("--")
        .args(paths)
        .current_dir(git_root);
    let output = git.output(command)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "failed to {operation} (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn reverse_staging_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.into())
}

fn nul_fields<'a>(output: &'a [u8], operation: &str) -> io::Result<Vec<&'a [u8]>> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let body = output.strip_suffix(&[0]).ok_or_else(|| {
        invalid_reverse_index_output(&format!("{operation} returned unterminated output"))
    })?;
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.iter().any(|field| field.is_empty()) {
        return Err(invalid_reverse_index_output(&format!(
            "{operation} returned an empty output field"
        )));
    }
    Ok(fields)
}

fn parse_expected_path(
    path: &[u8],
    expected: &BTreeSet<&str>,
    operation: &str,
) -> io::Result<String> {
    let path = std::str::from_utf8(path).map_err(|_| {
        invalid_reverse_index_output(&format!("{operation} returned a non-UTF-8 path"))
    })?;
    if !expected.contains(path) {
        return Err(invalid_reverse_index_output(&format!(
            "{operation} returned an unexpected path"
        )));
    }
    Ok(path.to_string())
}

fn invalid_reverse_index_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
