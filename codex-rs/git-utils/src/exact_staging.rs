//! Non-recursive staging for already-confined repository paths.

use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::process::Stdio;

use crate::exact_index_policy::ExactIndexPolicy;
use crate::exact_index_policy::resolve_exact_index_policy;
use crate::guarded_config::GuardedGitConfig;
use crate::patch_paths::validate_patch_path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StagePathsResult {
    pub(crate) exit_code: i32,
    pub(crate) stderr: String,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
enum StagingMode {
    ComposedApply,
    ReverseApply,
    Standalone,
}

enum SparseCheckoutPolicy {
    Verified { excluded: BTreeSet<String> },
    Refuse { result: StagePathsResult },
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
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn update_index_exact_paths_from_apply(
    config: &mut GuardedGitConfig<'_>,
    paths: &[String],
    content_filter_paths: &[String],
) -> io::Result<StagePathsResult> {
    update_index_exact_paths_common(
        config,
        paths,
        content_filter_paths,
        StagingMode::ComposedApply,
    )
}

pub(crate) fn update_index_exact_paths_for_reverse_apply(
    config: &mut GuardedGitConfig<'_>,
    paths: &[String],
    content_filter_paths: &[String],
) -> io::Result<StagePathsResult> {
    update_index_exact_paths_common(
        config,
        paths,
        content_filter_paths,
        StagingMode::ReverseApply,
    )
}

pub(crate) fn update_index_exact_paths_standalone(
    config: &mut GuardedGitConfig<'_>,
    paths: &[String],
    content_filter_paths: &[String],
) -> io::Result<StagePathsResult> {
    update_index_exact_paths_common(config, paths, content_filter_paths, StagingMode::Standalone)
}

fn update_index_exact_paths_common(
    config: &mut GuardedGitConfig<'_>,
    paths: &[String],
    content_filter_paths: &[String],
    mode: StagingMode,
) -> io::Result<StagePathsResult> {
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
    if !matches!(mode, StagingMode::Standalone) {
        config.ensure_apply_filter_path_subset(content_filter_paths)?;
    }
    if paths.is_empty() {
        return Ok(StagePathsResult {
            exit_code: 0,
            stderr: String::new(),
        });
    }

    let (paths, content_filter_paths, skip_worktree, assume_unchanged) =
        match resolve_exact_index_policy(config, paths, content_filter_paths)? {
            ExactIndexPolicy::Proceed {
                paths,
                content_filter_paths,
            } => (
                paths,
                content_filter_paths,
                BTreeSet::new(),
                BTreeSet::new(),
            ),
            ExactIndexPolicy::Flagged {
                paths,
                content_filter_paths,
                skip_worktree,
                assume_unchanged,
            } => (paths, content_filter_paths, skip_worktree, assume_unchanged),
            ExactIndexPolicy::Refuse { stderr } => {
                return Ok(StagePathsResult {
                    exit_code: 1,
                    stderr,
                });
            }
        };
    if !matches!(mode, StagingMode::Standalone) {
        config.ensure_apply_filter_path_subset(&content_filter_paths)?;
    }

    if (matches!(mode, StagingMode::Standalone) && !skip_worktree.is_empty())
        || !assume_unchanged.is_empty()
    {
        let mut reasons = Vec::new();
        if !skip_worktree.is_empty() {
            reasons.push(format!(
                "skip-worktree path(s): {}",
                quote_paths(&skip_worktree)
            ));
        }
        if !assume_unchanged.is_empty() {
            reasons.push(format!(
                "assume-unchanged path(s): {}",
                quote_paths(&assume_unchanged)
            ));
        }
        return Ok(StagePathsResult {
            exit_code: 1,
            stderr: format!("refusing to stage {}", reasons.join("; ")),
        });
    }

    let ignored = ignored_untracked_paths(config, &paths)?;
    let sparse_excluded = match sparse_checkout_policy(config, &paths)? {
        SparseCheckoutPolicy::Verified { excluded } => excluded,
        SparseCheckoutPolicy::Refuse { result } => return Ok(result),
    };
    let non_sparse_skip_worktree = skip_worktree
        .difference(&sparse_excluded)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !non_sparse_skip_worktree.is_empty() {
        return Ok(StagePathsResult {
            exit_code: 1,
            stderr: format!(
                "refusing to stage skip-worktree path(s): {}",
                quote_paths(&non_sparse_skip_worktree)
            ),
        });
    }
    let mut exclusions = ignored.clone();
    exclusions.extend(sparse_excluded.iter().cloned());
    let mut exclusion_reasons = Vec::new();
    if !ignored.is_empty() {
        exclusion_reasons.push(format!(
            "refusing to stage ignored untracked path(s): {}",
            quote_paths(&ignored)
        ));
    }
    if !sparse_excluded.is_empty() {
        exclusion_reasons.push(format!(
            "refusing to stage path(s) outside the sparse-checkout definition: {}",
            quote_paths(&sparse_excluded)
        ));
    }
    let exclusion_stderr = exclusion_reasons.join("; ");
    if !exclusions.is_empty() && !matches!(mode, StagingMode::ComposedApply) {
        return Ok(StagePathsResult {
            exit_code: 1,
            stderr: exclusion_stderr,
        });
    }
    // Composed staging is intentionally best effort for ignored and sparse
    // paths. Treat those expected omissions as successful after every fallible
    // probe has completed, so its caller cannot report an error after the
    // eligible subset has already changed the index. Reverse application uses
    // the all-or-nothing mode above because every staging candidate is needed
    // to align the index before the final patch command.
    let paths = paths
        .into_iter()
        .filter(|path| !exclusions.contains(path))
        .collect::<Vec<_>>();
    let content_filter_paths = content_filter_paths
        .into_iter()
        .filter(|path| !exclusions.contains(path))
        .collect::<Vec<_>>();

    config.authorize_git_add_filter_paths(&content_filter_paths)?;
    if paths.is_empty() {
        return Ok(StagePathsResult {
            exit_code: 0,
            stderr: exclusion_stderr,
        });
    }
    let mut command = config.update_index_literal_pathspecs_command()?;
    command
        .disable_optional_locks()
        .args(["--add", "--remove", "--"])
        .args(&paths);
    let output = command.output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !exclusions.is_empty() {
        if !stderr.is_empty() && !stderr.ends_with('\n') {
            stderr.push('\n');
        }
        stderr.push_str(&exclusion_stderr);
    }
    Ok(StagePathsResult { exit_code, stderr })
}

fn ignored_untracked_paths(
    config: &GuardedGitConfig<'_>,
    paths: &[String],
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
    let mut command = config.check_ignore_command()?;
    command
        .disable_optional_locks()
        // `check-ignore --stdin` consumes pathnames rather than pathspecs and
        // rejects the global literal-pathspec mode as unsupported magic.
        .args(["--stdin", "-z"])
        .stdin(Stdio::from(input));
    let output = command.output()?;
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
    config: &GuardedGitConfig<'_>,
    paths: &[String],
) -> io::Result<SparseCheckoutPolicy> {
    if !config.read_bool("core.sparseCheckout")?.unwrap_or(false) {
        return Ok(SparseCheckoutPolicy::Verified {
            excluded: BTreeSet::new(),
        });
    }

    // Do not let an older Git fall back to a PATH-resolved
    // `git-sparse-checkout` helper. Repository-controlled PATH entries are
    // outside the trusted executable boundary. `--list-cmds=builtins` is a
    // main-program capability query and cannot dispatch an external command.
    let builtins = config.list_builtin_commands()?;
    let sparse_checkout_is_builtin = builtins.status.success()
        && std::str::from_utf8(&builtins.stdout)
            .is_ok_and(|output| output.lines().any(|command| command == "sparse-checkout"));
    if !sparse_checkout_is_builtin {
        return Ok(SparseCheckoutPolicy::Refuse {
            result: StagePathsResult {
                exit_code: builtins.status.code().unwrap_or(-1).max(1),
                stderr:
                    "unable to verify sparse-checkout staging policy with a built-in Git command"
                        .to_string(),
            },
        });
    }

    let input = nul_path_input(paths)?;
    let mut command = config.sparse_checkout_command()?;
    command
        .disable_optional_locks()
        .args(["check-rules", "-z"])
        .stdin(Stdio::from(input));
    let output = command.output()?;
    if !output.status.success() {
        return Ok(SparseCheckoutPolicy::Refuse {
            result: StagePathsResult {
                exit_code: output.status.code().unwrap_or(-1),
                stderr: format!(
                    "unable to verify sparse-checkout staging policy: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            },
        });
    }
    let included = parse_reported_paths(&output.stdout, paths, "sparse-checkout")?;
    let excluded = paths
        .iter()
        .filter(|path| !included.contains(path.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    Ok(SparseCheckoutPolicy::Verified { excluded })
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
