//! Effective patch-path discovery and safe staging guards.

use std::io;
use std::path::Path;

use crate::apply::run_git;
use crate::apply::safe_git_config_parts;
use crate::apply::write_temp_patch;
use crate::safe_git::EXECUTABLE_FILTER_CONFIG_PATTERN;
use crate::safe_git::ensure_no_executable_git_config;
use crate::safe_git::isolate_git_command_environment;

pub(crate) fn extract_effective_paths_from_patch(
    patch_path: &Path,
    revert: bool,
) -> io::Result<Vec<String>> {
    let forward_paths = git_apply_numstat_paths(patch_path, revert)?;
    // `git apply --numstat` reports only the destination of a rename. Parse the
    // opposite orientation too so the submodule guard covers both endpoints.
    let reverse_paths = git_apply_numstat_paths(patch_path, !revert)?;
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

/// Best-effort extraction of the paths Git would apply.
///
/// Security-sensitive callers must use the fallible internal extractor so an
/// invalid or ambiguous patch is rejected instead of becoming an empty list.
pub fn extract_paths_from_patch(diff_text: &str) -> Vec<String> {
    let Ok((tmpdir, patch_path)) = write_temp_patch(diff_text) else {
        return Vec::new();
    };
    let paths =
        extract_effective_paths_from_patch(&patch_path, /*revert*/ false).unwrap_or_default();
    drop(tmpdir);
    paths
}

fn git_apply_numstat_paths(patch_path: &Path, revert: bool) -> io::Result<Vec<String>> {
    let mut cmd = std::process::Command::new("git");
    isolate_git_command_environment(&mut cmd);
    cmd.args(["apply", "--numstat", "-z"]);
    if revert {
        cmd.arg("-R");
    }
    let out = cmd
        .arg("--")
        .arg(patch_path)
        .current_dir(patch_path.parent().unwrap_or_else(|| Path::new(".")))
        .output()?;
    if !out.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "failed to parse patch paths: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }

    parse_numstat_paths(&out.stdout)
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
        || path.contains('\\')
        || path.as_bytes().get(1) == Some(&b':')
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

/// Stage only the files that actually exist on disk for the given diff.
pub fn stage_paths(git_root: &Path, diff: &str) -> io::Result<()> {
    let (tmpdir, patch_path) = write_temp_patch(diff)?;
    let paths = extract_effective_paths_from_patch(&patch_path, /*revert*/ true)?;
    let _guard = tmpdir;
    stage_effective_paths(git_root, &paths)
}

pub(crate) fn stage_effective_paths(git_root: &Path, paths: &[String]) -> io::Result<()> {
    ensure_no_executable_git_config(git_root, EXECUTABLE_FILTER_CONFIG_PATTERN, &[])?;
    let mut existing: Vec<String> = Vec::new();
    for p in paths {
        let joined = git_root.join(p);
        if std::fs::symlink_metadata(&joined).is_ok() {
            existing.push(p.clone());
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    ensure_paths_do_not_enter_submodules(git_root, &existing)?;
    let mut args = vec![
        "--literal-pathspecs".to_string(),
        "add".to_string(),
        "--".to_string(),
    ];
    args.extend(existing);
    let config_parts = safe_git_config_parts();
    let (_code, _, _) = run_git(git_root, &config_parts, &args)?;
    // We do not hard fail staging; best-effort is OK. Return Ok even on non-zero.
    Ok(())
}

pub(crate) fn ensure_paths_do_not_enter_submodules(
    git_root: &Path,
    paths: &[String],
) -> io::Result<()> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to inspect an empty patch path set",
        ));
    }
    let mut candidates = std::collections::BTreeSet::new();
    for path in paths {
        let mut components = path.split('/').filter(|component| !component.is_empty());
        let Some(first) = components.next() else {
            continue;
        };
        let mut candidate = first.to_string();
        candidates.insert(candidate.clone());
        for component in components {
            candidate.push('/');
            candidate.push_str(component);
            candidates.insert(candidate.clone());
        }
    }

    let canonical_root = std::fs::canonicalize(git_root)?;
    let mut canonical_candidates = Vec::new();
    for candidate in &candidates {
        match std::fs::canonicalize(git_root.join(candidate)) {
            Ok(resolved) => {
                let relative = resolved.strip_prefix(&canonical_root).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "patch path alias resolves outside the Git worktree",
                    )
                })?;
                let relative = relative.to_str().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "patch path alias is not valid UTF-8",
                    )
                })?;
                if relative.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "patch path alias resolves to the Git worktree root",
                    ));
                }
                canonical_candidates.push(relative.replace(std::path::MAIN_SEPARATOR, "/"));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    candidates.extend(canonical_candidates);

    let mut args = vec![
        "ls-files".to_string(),
        "--stage".to_string(),
        "-z".to_string(),
        "--".to_string(),
    ];
    args.extend(
        candidates
            .into_iter()
            .map(|candidate| format!(":(icase,literal){candidate}")),
    );
    let config_parts = safe_git_config_parts();
    let (code, stdout, stderr) = run_git(git_root, &config_parts, &args)?;
    if code != 0 {
        return Err(io::Error::other(format!(
            "failed to inspect patch paths for submodules (exit {code}): {}",
            stderr.trim()
        )));
    }
    if stdout
        .split('\0')
        .filter(|record| !record.is_empty())
        .any(|record| record.starts_with("160000 "))
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to stage a patch path that is a submodule or enters a submodule",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "patch_paths_tests.rs"]
mod tests;
