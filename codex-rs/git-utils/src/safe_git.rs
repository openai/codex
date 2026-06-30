use std::io;
use std::path::Path;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use tokio::time::Duration;
use tokio::time::timeout;

pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_FILTER_CONFIG_PATTERN: &str = r"^filter\..*\.(clean|smudge|process)$";
pub(crate) const EXECUTABLE_PATCH_CONFIG_PATTERN: &str =
    r"^(filter\..*\.(clean|smudge|process)|merge\..*\.driver)$";
/// Timeout for internal Git commands to prevent freezing on large repositories.
pub(crate) const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

const ISOLATED_GIT_ENVIRONMENT: [&str; 9] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_PREFIX",
    "GIT_LITERAL_PATHSPECS",
    "GIT_GLOB_PATHSPECS",
    "GIT_NOGLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
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

    Some(config_output_has_untrusted_executable_helpers(
        &output.stdout,
    ))
}

pub(crate) fn ensure_no_executable_git_config(
    cwd: &Path,
    pattern: &str,
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
            "--includes",
            "--get-regexp",
            pattern,
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
    if config_output_has_untrusted_executable_helpers(&output.stdout) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to run an internal Git worktree operation with executable Git helpers configured",
        ));
    }
    Ok(())
}

pub(crate) fn config_output_has_untrusted_executable_helpers(stdout: &[u8]) -> bool {
    let mut fields = stdout.split(|byte| *byte == 0);
    loop {
        let Some(scope) = fields.next() else {
            return false;
        };
        if scope.is_empty() {
            return fields.any(|field| !field.is_empty());
        }
        let Some(entry) = fields.next() else {
            return true;
        };
        let Some(value_separator) = entry.iter().position(|byte| *byte == b'\n') else {
            return true;
        };
        let key = &entry[..value_separator];
        let value = &entry[value_separator + 1..];
        let trusted_scope = scope == b"system" || scope == b"global";
        // Repositories choose merge drivers through `.gitattributes`, so even
        // a globally configured driver is repository-triggerable. Filters at
        // system/global scope remain trusted to preserve normal Git LFS and
        // user normalization behavior.
        let merge_driver = key.starts_with(b"merge.") && key.ends_with(b".driver");
        if !value.is_empty() && (merge_driver || !trusted_scope) {
            return true;
        }
    }
}

#[cfg(test)]
#[path = "safe_git_tests.rs"]
mod tests;
