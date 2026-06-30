//! Effective patch-path discovery and safe staging guards.

use std::io;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::apply::run_git;
use crate::apply::safe_git_config_parts;
use crate::apply::write_temp_patch;
use crate::git_command::GitRunner;
use crate::git_config::path_is_within;
use crate::safe_git::ensure_no_selected_executable_git_filters;

pub(crate) fn extract_effective_paths_from_patch(
    git: &GitRunner,
    patch_path: &Path,
    revert: bool,
) -> io::Result<Vec<String>> {
    let forward_paths = git_apply_numstat_paths(git, patch_path, revert)?;
    // `git apply --numstat` reports only the destination of a rename. Parse the
    // opposite orientation too so the submodule guard covers both endpoints.
    let reverse_paths = git_apply_numstat_paths(git, patch_path, !revert)?;
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
    let paths = std::env::current_dir()
        .ok()
        .and_then(|cwd| GitRunner::for_cwd(&cwd).ok())
        .and_then(|git| {
            extract_effective_paths_from_patch(&git, &patch_path, /*revert*/ false).ok()
        })
        .unwrap_or_default();
    drop(tmpdir);
    paths
}

fn git_apply_numstat_paths(
    git: &GitRunner,
    patch_path: &Path,
    revert: bool,
) -> io::Result<Vec<String>> {
    let mut cmd = git.command();
    cmd.args(["apply", "--numstat", "-z"]);
    if revert {
        cmd.arg("-R");
    }
    cmd.arg("--")
        .arg(patch_path)
        .current_dir(patch_path.parent().unwrap_or_else(|| Path::new(".")));
    let out = git.output(cmd)?;
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
    path.contains('\\') || path.as_bytes().get(1) == Some(&b':')
}

#[cfg(not(windows))]
fn invalid_platform_patch_path(_path: &str) -> bool {
    false
}

/// Stage only the files that actually exist on disk for the given diff.
pub fn stage_paths(git_root: &Path, diff: &str) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(git_root)?;
    let (tmpdir, patch_path) = write_temp_patch(diff)?;
    let paths = extract_effective_paths_from_patch(&git, &patch_path, /*revert*/ true)?;
    let _guard = tmpdir;
    stage_effective_paths(&git, git_root, &paths)
}

pub(crate) fn stage_effective_paths(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
) -> io::Result<()> {
    ensure_no_selected_executable_git_filters(git, git_root, paths, &[])?;
    let guarded = classify_patch_paths(git, git_root, paths)?;
    if !guarded.exact_gitlinks.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to stage an exact submodule entry with git add",
        ));
    }
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
    let mut args = vec![
        "--literal-pathspecs".to_string(),
        "add".to_string(),
        "--".to_string(),
    ];
    args.extend(existing);
    let config_parts = safe_git_config_parts();
    let (_code, _, _) = run_git(git, git_root, &config_parts, &args)?;
    // We do not hard fail staging; best-effort is OK. Return Ok even on non-zero.
    Ok(())
}

#[cfg(test)]
pub(crate) fn ensure_paths_do_not_enter_submodules(
    git_root: &Path,
    paths: &[String],
) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(git_root)?;
    classify_patch_paths(&git, git_root, paths).map(|_| ())
}

#[derive(Debug)]
pub(crate) struct GuardedPatchPaths {
    pub(crate) exact_gitlinks: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexStageRecord {
    mode: String,
    oid: String,
    stage: u8,
    path: String,
}

pub(crate) fn classify_patch_paths(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
) -> io::Result<GuardedPatchPaths> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to inspect an empty patch path set",
        ));
    }
    let exact_leaves = paths
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut traversed_ancestors = std::collections::BTreeSet::new();
    for path in paths {
        insert_path_prefixes(path, &mut traversed_ancestors, /*include_leaf*/ false);
    }

    let canonical_root = std::fs::canonicalize(git_root)?;
    let git_metadata_dirs = canonical_git_metadata_dirs(git, &canonical_root)?;
    let mut canonical_candidates = std::collections::BTreeSet::new();
    for candidate in &traversed_ancestors {
        match std::fs::canonicalize(git_root.join(candidate)) {
            Ok(resolved) => {
                if git_metadata_dirs
                    .iter()
                    .any(|metadata_dir| path_is_within(&resolved, metadata_dir))
                {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "patch path alias resolves into Git repository metadata",
                    ));
                }
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
                insert_path_prefixes(
                    &relative.replace(std::path::MAIN_SEPARATOR, "/"),
                    &mut canonical_candidates,
                    /*include_leaf*/ true,
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let mut exact_candidates = exact_leaves.clone();
    exact_candidates.extend(traversed_ancestors.iter().cloned());
    exact_candidates.extend(canonical_candidates.iter().cloned());
    let records = read_index_stage_records(
        git,
        git_root,
        &exact_candidates,
        /*ignore_case*/ false,
        /*index_file*/ None,
    )?;
    let mut exact_gitlinks = std::collections::BTreeMap::new();
    for record in records {
        if !exact_candidates.contains(&record.path) {
            continue;
        }
        if record.stage != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "refusing to apply a patch across an unmerged index entry",
            ));
        }
        if record.mode != "160000" {
            continue;
        }
        if traversed_ancestors.contains(&record.path) || canonical_candidates.contains(&record.path)
        {
            return Err(submodule_descendant_error());
        }
        if exact_leaves.contains(&record.path) {
            exact_gitlinks.insert(record.path, record.oid);
        }
    }

    // `:(icase)` is discovery only. Confirm a differently-cased candidate by
    // filesystem identity so case-sensitive siblings remain independent.
    if !traversed_ancestors.is_empty() {
        let icase_records = read_index_stage_records(
            git,
            git_root,
            &traversed_ancestors,
            /*ignore_case*/ true,
            /*index_file*/ None,
        )?;
        for record in icase_records {
            if record.mode != "160000" || traversed_ancestors.contains(&record.path) {
                continue;
            }
            let indexed = std::fs::canonicalize(git_root.join(&record.path));
            for candidate in &traversed_ancestors {
                if !candidate.eq_ignore_ascii_case(&record.path) {
                    continue;
                }
                let requested = std::fs::canonicalize(git_root.join(candidate));
                match (&requested, &indexed) {
                    (Ok(requested), Ok(indexed))
                        if same_file::is_same_file(requested, indexed).unwrap_or(false) =>
                    {
                        return Err(submodule_descendant_error());
                    }
                    (Ok(_), Ok(_)) => {}
                    _ if filesystem_is_case_sensitive(git_root) == Some(true) => {}
                    // If either object is absent and the containing filesystem
                    // does not prove case-sensitive lookup, fail closed.
                    _ => return Err(submodule_descendant_error()),
                }
            }
        }
    }

    Ok(GuardedPatchPaths { exact_gitlinks })
}

pub(crate) fn validate_gitlink_updates(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    patch_path: &Path,
    patch_text: &str,
    revert: bool,
    guarded: &GuardedPatchPaths,
) -> io::Result<()> {
    let mentions_gitlink = patch_header_mentions_gitlink_mode(patch_text);
    if guarded.exact_gitlinks.is_empty() && !mentions_gitlink {
        return Ok(());
    }

    let config_parts = safe_git_config_parts();
    let (code, index_path, stderr) = run_git(
        git,
        git_root,
        &config_parts,
        &[
            "rev-parse".to_string(),
            "--git-path".to_string(),
            "index".to_string(),
        ],
    )?;
    if code != 0 {
        return Err(io::Error::other(format!(
            "failed to resolve parent Git index (exit {code}): {}",
            stderr.trim()
        )));
    }
    let index_path = PathBuf::from(index_path.trim_end_matches(['\r', '\n']));
    let index_path = if index_path.is_absolute() {
        index_path
    } else {
        git_root.join(index_path)
    };
    let index_parent = index_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Git index has no parent directory",
        )
    })?;
    let mut scratch = tempfile::NamedTempFile::new_in(index_parent)?;
    let index_exists = index_path.exists();
    if index_exists {
        let mut source = std::fs::File::open(&index_path)?;
        io::copy(&mut source, scratch.as_file_mut())?;
        scratch.as_file_mut().flush()?;
        scratch.as_file_mut().rewind()?;
    }
    let scratch_guard = scratch.into_temp_path();
    let scratch_path = scratch_guard.to_path_buf();
    if !index_exists {
        std::fs::remove_file(&scratch_path)?;
    }
    let mut args = vec!["apply".to_string(), "--cached".to_string()];
    if revert {
        args.push("-R".to_string());
    }
    args.push("--".to_string());
    args.push(patch_path.to_string_lossy().into_owned());
    let output = run_git_bytes(git, git_root, &config_parts, &args, Some(&scratch_path))?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "refusing a gitlink patch that cannot be validated in an isolated parent index: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }

    let candidates = paths
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let records = read_index_stage_records(
        git,
        git_root,
        &candidates,
        /*ignore_case*/ false,
        Some(&scratch_path),
    )?;
    let mut resulting_gitlinks = std::collections::BTreeMap::new();
    for record in records {
        if !candidates.contains(&record.path) {
            continue;
        }
        if record.stage != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "refusing a gitlink patch that produces an unmerged index entry",
            ));
        }
        if record.mode == "160000" {
            resulting_gitlinks.insert(record.path, record.oid);
        }
    }
    if resulting_gitlinks.keys().ne(guarded.exact_gitlinks.keys())
        || guarded.exact_gitlinks.iter().any(|(path, oid)| {
            resulting_gitlinks
                .get(path)
                .is_some_and(|resulting_oid| resulting_oid == oid)
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only object-ID updates to existing exact gitlink paths are supported",
        ));
    }
    Ok(())
}

fn read_index_stage_records(
    git: &GitRunner,
    git_root: &Path,
    candidates: &std::collections::BTreeSet<String>,
    ignore_case: bool,
    index_file: Option<&Path>,
) -> io::Result<Vec<IndexStageRecord>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec![
        "ls-files".to_string(),
        "--stage".to_string(),
        "-z".to_string(),
        "--".to_string(),
    ];
    let magic = if ignore_case {
        ":(icase,literal)"
    } else {
        ":(literal)"
    };
    args.extend(
        candidates
            .iter()
            .map(|candidate| format!("{magic}{candidate}")),
    );
    let config_parts = safe_git_config_parts();
    let output = run_git_bytes(git, git_root, &config_parts, &args, index_file)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "failed to inspect patch paths in the parent index (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_index_stage_records(&output.stdout)
}

fn run_git_bytes(
    git: &GitRunner,
    cwd: &Path,
    git_cfg: &[String],
    args: &[String],
    index_file: Option<&Path>,
) -> io::Result<std::process::Output> {
    // Keep this constructor centralized so #29470 can add its local-only
    // transport environment here during the later semantic rebase.
    let mut command = git.command();
    command.args(git_cfg).args(args).current_dir(cwd);
    match index_file {
        Some(index_file) => git.output_with_index_file(command, index_file),
        None => git.output(command),
    }
}

fn parse_index_stage_records(output: &[u8]) -> io::Result<Vec<IndexStageRecord>> {
    if !output.is_empty() && !output.ends_with(&[0]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Git returned an unterminated index record",
        ));
    }
    let mut records = Vec::new();
    for raw in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = raw.iter().position(|byte| *byte == b'\t').ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned an index record without a path",
            )
        })?;
        let metadata = std::str::from_utf8(&raw[..tab]).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned non-UTF-8 index metadata",
            )
        })?;
        let path = std::str::from_utf8(&raw[tab + 1..]).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned a non-UTF-8 index path",
            )
        })?;
        let fields = metadata.split_whitespace().collect::<Vec<_>>();
        let [mode, oid, stage] = fields.as_slice() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned malformed index metadata",
            ));
        };
        if mode.len() != 6 || !mode.bytes().all(|byte| matches!(byte, b'0'..=b'7')) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned an invalid index mode",
            ));
        }
        if oid.is_empty() || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned an invalid object ID",
            ));
        }
        let stage = stage
            .parse::<u8>()
            .ok()
            .filter(|stage| *stage <= 3)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Git returned an invalid index stage",
                )
            })?;
        records.push(IndexStageRecord {
            mode: (*mode).to_string(),
            oid: (*oid).to_string(),
            stage,
            path: path.to_string(),
        });
    }
    Ok(records)
}

fn patch_header_mentions_gitlink_mode(patch: &str) -> bool {
    let mut in_header = false;
    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            in_header = true;
            continue;
        }
        if !in_header {
            continue;
        }
        if line.starts_with("@@") || line == "GIT binary patch" {
            in_header = false;
            continue;
        }
        if matches!(
            line,
            "new file mode 160000"
                | "deleted file mode 160000"
                | "old mode 160000"
                | "new mode 160000"
        ) || line
            .strip_prefix("index ")
            .is_some_and(|index| index.ends_with(" 160000"))
        {
            return true;
        }
    }
    false
}

fn submodule_descendant_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "refusing to apply a patch path that enters a submodule",
    )
}

fn filesystem_is_case_sensitive(path: &Path) -> Option<bool> {
    if let Some(result) = case_sensitivity_from_existing_entry(&path.join(".git")) {
        return Some(result);
    }
    for existing in path.ancestors() {
        if let Some(result) = case_sensitivity_from_existing_entry(existing) {
            return Some(result);
        }
    }
    None
}

fn case_sensitivity_from_existing_entry(existing: &Path) -> Option<bool> {
    if !existing.exists() {
        return None;
    }
    let name = existing.file_name()?.to_str()?;
    let mut alternate = name.to_string();
    let (index, replacement) = alternate.char_indices().find_map(|(index, character)| {
        if character.is_ascii_lowercase() {
            Some((index, character.to_ascii_uppercase()))
        } else if character.is_ascii_uppercase() {
            Some((index, character.to_ascii_lowercase()))
        } else {
            None
        }
    })?;
    alternate.replace_range(index..index + 1, &replacement.to_string());
    let alternate_path = existing.parent()?.join(alternate);
    match std::fs::canonicalize(&alternate_path) {
        Ok(_) => same_file::is_same_file(existing, &alternate_path)
            .ok()
            .map(|same| !same),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(true),
        Err(_) => None,
    }
}

fn canonical_git_metadata_dirs(git: &GitRunner, git_root: &Path) -> io::Result<Vec<PathBuf>> {
    let config_parts = safe_git_config_parts();
    let queries = [
        vec!["rev-parse".to_string(), "--absolute-git-dir".to_string()],
        vec!["rev-parse".to_string(), "--git-common-dir".to_string()],
    ];
    let mut metadata_dirs = std::collections::BTreeSet::new();
    for args in queries {
        let (code, stdout, stderr) = run_git(git, git_root, &config_parts, &args)?;
        if code != 0 {
            return Err(io::Error::other(format!(
                "failed to resolve Git repository metadata (exit {code}): {}",
                stderr.trim()
            )));
        }
        let path = stdout.trim_end_matches(['\r', '\n']);
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
            git_root.join(path)
        };
        metadata_dirs.insert(std::fs::canonicalize(absolute)?);
    }
    Ok(metadata_dirs.into_iter().collect())
}

fn insert_path_prefixes(
    path: &str,
    prefixes: &mut std::collections::BTreeSet<String>,
    include_leaf: bool,
) {
    let components = path
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    let mut prefix = String::new();
    for (index, component) in components.iter().enumerate() {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        if include_leaf || index + 1 < components.len() {
            prefixes.insert(prefix.clone());
        }
    }
}

#[cfg(test)]
#[path = "patch_paths_tests.rs"]
mod tests;
