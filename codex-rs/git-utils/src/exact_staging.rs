//! Non-recursive staging for already-confined repository paths.

use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use crate::apply::run_git;
use crate::exact_index_policy::ExactIndexPolicy;
use crate::exact_index_policy::effective_git_bool;
use crate::exact_index_policy::resolve_exact_index_policy;
use crate::git_command::GitRunner;
use crate::git_config_sources::ensure_no_worktree_config_sources;
use crate::patch_paths::validate_patch_path;
use crate::safe_git::ensure_no_selected_git_add_filters;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StagePathsResult {
    pub(crate) exit_code: i32,
    pub(crate) stderr: String,
}

/// Stage only the literal paths supplied by a caller that has already applied
/// the operation-specific containment and filesystem policy.
///
/// Unlike `git add`, `update-index` never treats a path that races from a leaf
/// to a directory as a recursive request. Callers that require staging to
/// succeed can inspect the returned status; the public forward helper retains
/// its historical best-effort handling of a non-zero Git command.
///
/// This is not a transactional filesystem-confinement primitive. A concurrent
/// strict-ancestor swap to a symlink or Windows junction can still redirect the
/// bytes read for an already-confined index pathname. It cannot make this
/// exact command recurse into, or create index entries for, descendant paths;
/// the filter neutralizer also remains in force for the entire command.
pub(crate) fn update_index_exact_paths(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    content_filter_paths: &[String],
    git_config_args: &[String],
) -> io::Result<StagePathsResult> {
    if paths.is_empty() {
        return Ok(StagePathsResult {
            exit_code: 0,
            stderr: String::new(),
        });
    }
    for path in paths {
        if path.as_bytes().contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "exact staging path contains NUL",
            ));
        }
        let _ = validate_patch_path(path.clone())?;
    }
    let exact_paths = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if content_filter_paths
        .iter()
        .any(|path| !exact_paths.contains(path.as_str()))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "content-filter staging path is not in the exact staging set",
        ));
    }

    ensure_no_worktree_config_sources(git, git_root, git_config_args)?;

    let (paths, content_filter_paths) = match resolve_exact_index_policy(
        git,
        git_root,
        paths,
        content_filter_paths,
        git_config_args,
    )? {
        ExactIndexPolicy::Proceed {
            paths,
            content_filter_paths,
        } => (paths, content_filter_paths),
        ExactIndexPolicy::Refuse { stderr } => {
            return Ok(StagePathsResult {
                exit_code: 1,
                stderr,
            });
        }
    };

    let ignored = ignored_untracked_paths(git, git_root, &paths, git_config_args)?;
    if !ignored.is_empty() {
        return Ok(StagePathsResult {
            exit_code: 1,
            stderr: format!(
                "refusing to stage ignored untracked path(s): {}",
                quote_paths(&ignored)
            ),
        });
    }
    if let Some(result) = sparse_checkout_policy(git, git_root, &paths, git_config_args)? {
        return Ok(result);
    }

    let filter_guard =
        ensure_no_selected_git_add_filters(git, git_root, &content_filter_paths, git_config_args)?;
    let mut guarded_config = git_config_args.to_vec();
    guarded_config.extend_from_slice(filter_guard.git_config_args());

    let mut update_args = vec![
        "--literal-pathspecs".to_string(),
        "update-index".to_string(),
        "--add".to_string(),
        "--remove".to_string(),
        "--".to_string(),
    ];
    update_args.extend_from_slice(&paths);
    let (exit_code, _stdout, stderr) = run_git(git, git_root, &guarded_config, &update_args)?;
    Ok(StagePathsResult { exit_code, stderr })
}

fn ignored_untracked_paths(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<BTreeSet<String>> {
    if paths.is_empty() {
        return Ok(BTreeSet::new());
    }
    // `check-ignore --stdin` still recognizes a leading `:` as pathspec
    // magic, while its command mode rejects the global literal-pathspec
    // override. A leading `./` makes every normalized repository-relative
    // name unambiguously literal without changing which file it denotes.
    let probe_paths = paths
        .iter()
        .map(|path| format!("./{path}"))
        .collect::<Vec<_>>();
    let input = nul_path_input(&probe_paths)?;
    let mut command = git.command_for_cwd(git_root)?;
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        // `check-ignore --stdin` consumes pathnames rather than pathspecs and
        // rejects the global literal-pathspec mode as unsupported magic.
        .args(["check-ignore", "--stdin", "-z"])
        .stdin(Stdio::from(input));
    let output = git.output(command)?;
    match output.status.code() {
        Some(0) => {
            let ignored = parse_reported_paths(&output.stdout, &probe_paths, "ignored-path")?
                .into_iter()
                .map(|path| path.trim_start_matches("./").to_string())
                .collect::<BTreeSet<_>>();
            if ignored.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Git ignored-path probe reported success without a path",
                ));
            }
            Ok(ignored)
        }
        Some(1) if output.stdout.is_empty() && output.stderr.is_empty() => Ok(BTreeSet::new()),
        _ => Err(io::Error::other(format!(
            "git ignored-path probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
    }
}

fn sparse_checkout_policy(
    git: &GitRunner,
    git_root: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<Option<StagePathsResult>> {
    if !effective_git_bool(git, git_root, git_config_args, "core.sparseCheckout")?.unwrap_or(false)
    {
        return Ok(None);
    }

    // Do not let an older Git fall back to a PATH-resolved
    // `git-sparse-checkout` helper. Repository-controlled PATH entries are
    // outside the trusted executable boundary. `--list-cmds=builtins` is a
    // main-program capability query and cannot dispatch an external command.
    let mut builtins_command = git.command_for_cwd(git_root)?;
    builtins_command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .arg("--list-cmds=builtins");
    let builtins = git.output(builtins_command)?;
    let sparse_checkout_is_builtin = builtins.status.success()
        && std::str::from_utf8(&builtins.stdout)
            .is_ok_and(|output| output.lines().any(|command| command == "sparse-checkout"));
    if !sparse_checkout_is_builtin {
        return Ok(Some(StagePathsResult {
            exit_code: builtins.status.code().unwrap_or(-1).max(1),
            stderr: "unable to verify sparse-checkout staging policy with a built-in Git command"
                .to_string(),
        }));
    }

    let input = nul_path_input(paths)?;
    let mut command = git.command_for_cwd(git_root)?;
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args(["sparse-checkout", "check-rules", "-z"])
        .stdin(Stdio::from(input));
    let output = git.output(command)?;
    if !output.status.success() {
        return Ok(Some(StagePathsResult {
            exit_code: output.status.code().unwrap_or(-1),
            stderr: format!(
                "unable to verify sparse-checkout staging policy: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }));
    }
    let included = parse_reported_paths(&output.stdout, paths, "sparse-checkout")?;
    let excluded = paths
        .iter()
        .filter(|path| !included.contains(path.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    if excluded.is_empty() {
        Ok(None)
    } else {
        Ok(Some(StagePathsResult {
            exit_code: 1,
            stderr: format!(
                "refusing to stage path(s) outside the sparse-checkout definition: {}",
                quote_paths(&excluded)
            ),
        }))
    }
}

fn nul_path_input(paths: &[String]) -> io::Result<std::fs::File> {
    let mut input = tempfile::tempfile()?;
    for path in paths {
        input.write_all(path.as_bytes())?;
        input.write_all(&[0])?;
    }
    input.rewind()?;
    Ok(input)
}

fn parse_reported_paths(
    output: &[u8],
    expected_paths: &[String],
    probe: &str,
) -> io::Result<BTreeSet<String>> {
    if output.is_empty() {
        return Ok(BTreeSet::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unterminated Git {probe} output"),
        ));
    };
    let expected = expected_paths
        .iter()
        .map(String::as_bytes)
        .collect::<BTreeSet<_>>();
    let mut reported = BTreeSet::new();
    for path in body.split(|byte| *byte == 0) {
        if path.is_empty() || !expected.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected Git {probe} output"),
            ));
        }
        let path = std::str::from_utf8(path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("non-UTF-8 Git {probe} output"),
            )
        })?;
        reported.insert(path.to_string());
    }
    Ok(reported)
}

fn quote_paths(paths: &BTreeSet<String>) -> String {
    paths
        .iter()
        .map(|path| format!("{path:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}
