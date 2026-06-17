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
            "refusing to run an internal Git worktree operation with repository- or command-scoped executable Git helpers configured",
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
        let value = &entry[value_separator + 1..];
        let trusted_scope = scope == b"system" || scope == b"global";
        if !trusted_scope && !value.is_empty() {
            return true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_untrusted_nonempty_helper_values() {
        assert!(!config_output_has_untrusted_executable_helpers(b""));
        assert!(!config_output_has_untrusted_executable_helpers(b"\0"));
        assert!(config_output_has_untrusted_executable_helpers(
            b"local\0filter.example.clean\nhelper\0"
        ));
        assert!(config_output_has_untrusted_executable_helpers(
            b"command\0merge.example.driver\nhelper\0"
        ));
    }

    #[test]
    fn allows_trusted_or_disabled_helper_values() {
        assert!(!config_output_has_untrusted_executable_helpers(
            b"global\0filter.lfs.process\ngit-lfs filter-process\0"
        ));
        assert!(!config_output_has_untrusted_executable_helpers(
            b"system\0merge.trusted.driver\ntrusted-driver\0"
        ));
        assert!(!config_output_has_untrusted_executable_helpers(
            b"local\0filter.example.clean\n\0"
        ));
    }

    #[test]
    fn rejects_malformed_probe_output() {
        assert!(config_output_has_untrusted_executable_helpers(b"local\0"));
        assert!(config_output_has_untrusted_executable_helpers(
            b"local\0filter.example.clean\0"
        ));
    }
}
