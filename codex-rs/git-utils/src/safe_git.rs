use std::io;
use std::path::Path;
use std::process::Command;

pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_FILTER_CONFIG_PATTERN: &str = r"^filter\..*\.(clean|smudge|process)$";
pub(crate) const EXECUTABLE_PATCH_CONFIG_PATTERN: &str =
    r"^(filter\..*\.(clean|smudge|process)|merge\..*\.driver)$";

pub(crate) fn ensure_no_executable_git_config(cwd: &Path, pattern: &str) -> io::Result<()> {
    let output = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["config", "--null", "--name-only", "--get-regexp", pattern])
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
    if config_output_has_entries(&output.stdout) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to run an internal Git worktree operation with executable Git helpers configured",
        ));
    }
    Ok(())
}

pub(crate) fn config_output_has_entries(stdout: &[u8]) -> bool {
    stdout.iter().any(|byte| *byte != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nonempty_null_delimited_config_output() {
        assert!(!config_output_has_entries(b""));
        assert!(!config_output_has_entries(b"\0"));
        assert!(config_output_has_entries(b"filter.example.clean\0"));
    }
}
