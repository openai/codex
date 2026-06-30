use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use tokio::time::Duration;
use tokio::time::timeout;

use crate::git_config::GitConfigEntry;
use crate::git_config::parse_effective_config;

pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_FILTER_CONFIG_PATTERN: &str = r"^filter\..*\.(clean|smudge|process)$";
/// Timeout for internal Git commands to prevent freezing on large repositories.
pub(crate) const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

const ISOLATED_GIT_ENVIRONMENT: [&str; 10] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_PREFIX",
    "GIT_LITERAL_PATHSPECS",
    "GIT_GLOB_PATHSPECS",
    "GIT_NOGLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
    "GIT_EXEC_PATH",
];

/// Keep internal worktree operations bound to their explicit cwd and pathspec
/// semantics instead of inheriting repository, index, or pathspec selectors.
/// Deliberately leave Git config channels intact: callers may rely on normal
/// system/global configuration, and executable helpers are probed separately.
pub(crate) fn isolate_git_command_environment(command: &mut Command) {
    for name in ISOLATED_GIT_ENVIRONMENT {
        command.env_remove(name);
    }
}

pub(crate) fn isolate_tokio_git_command_environment(command: &mut tokio::process::Command) {
    for name in ISOLATED_GIT_ENVIRONMENT {
        command.env_remove(name);
    }
}

pub(crate) async fn has_configured_executable_filters_from(git: &Path, cwd: &Path) -> Option<bool> {
    let mut command = TokioCommand::new(git);
    isolate_tokio_git_command_environment(&mut command);
    command
        .args([
            "config",
            "--null",
            "--show-scope",
            "--show-origin",
            "--includes",
            "--get-regexp",
            EXECUTABLE_FILTER_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .kill_on_drop(true);
    let output = match timeout(GIT_COMMAND_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        _ => return None,
    };
    if !output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return None;
    }

    let entries = parse_effective_config(&output.stdout).ok()?;
    Some(config_entries_have_untrusted_filters(&entries))
}

pub(crate) fn ensure_no_executable_git_filters(
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<()> {
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args([
            "config",
            "--null",
            "--show-scope",
            "--show-origin",
            "--includes",
            "--get-regexp",
            EXECUTABLE_FILTER_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .output()?;
    if !output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return Err(io::Error::other(format!(
            "git config probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let entries = parse_effective_config(&output.stdout)?;
    if config_entries_have_untrusted_filters(&entries) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to run an internal Git worktree operation with an executable Git filter configured",
        ));
    }
    Ok(())
}

fn config_entries_have_untrusted_filters(entries: &BTreeMap<String, GitConfigEntry>) -> bool {
    entries.values().any(|entry| !entry.value.is_empty())
}

#[cfg(test)]
#[path = "safe_git_tests.rs"]
mod tests;
