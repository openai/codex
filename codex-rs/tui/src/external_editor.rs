use std::env;
use std::fs;
use std::process::Stdio;

use color_eyre::eyre::Report;
use color_eyre::eyre::Result;
use tempfile::Builder;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub(crate) enum EditorError {
    #[error("neither VISUAL nor EDITOR is set")]
    MissingEditor,
    #[cfg(not(windows))]
    #[error("failed to parse editor command")]
    ParseFailed,
    #[error("editor command is empty")]
    EmptyCommand,
}

/// Tries to resolve the full path to a Windows program, respecting PATH + PATHEXT.
/// Falls back to the original program name if resolution fails.
#[cfg(windows)]
fn resolve_windows_program(program: &str) -> std::path::PathBuf {
    // On Windows, `Command::new("code")` will not resolve `code.cmd` shims on PATH.
    // Use `which` so we respect PATH + PATHEXT (e.g., `code` -> `code.cmd`).
    which::which(program).unwrap_or_else(|_| std::path::PathBuf::from(program))
}

/// Resolve the editor command from environment variables.
/// Prefers `VISUAL` over `EDITOR`.
pub(crate) fn resolve_editor_command() -> std::result::Result<Vec<String>, EditorError> {
    let raw = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .map_err(|_| EditorError::MissingEditor)?;
    let parts = {
        #[cfg(windows)]
        {
            winsplit::split(&raw)
        }
        #[cfg(not(windows))]
        {
            shlex::split(&raw).ok_or(EditorError::ParseFailed)?
        }
    };
    if parts.is_empty() {
        return Err(EditorError::EmptyCommand);
    }
    Ok(parts)
}

/// Write `seed` to a temp file, launch the editor command, and return the updated content.
pub(crate) async fn run_editor(seed: &str, editor_cmd: &[String]) -> Result<String> {
    if editor_cmd.is_empty() {
        return Err(Report::msg("editor command is empty"));
    }

    // Convert to TempPath immediately so no file handle stays open on Windows.
    let temp_path = Builder::new().suffix(".md").tempfile()?.into_temp_path();
    fs::write(&temp_path, seed)?;

    let mut cmd = {
        #[cfg(windows)]
        {
            // handles .cmd/.bat shims
            Command::new(resolve_windows_program(&editor_cmd[0]))
        }
        #[cfg(not(windows))]
        {
            Command::new(&editor_cmd[0])
        }
    };
    if editor_cmd.len() > 1 {
        cmd.args(&editor_cmd[1..]);
    }
    let status = cmd
        .arg(&temp_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(Report::msg(format!("editor exited with status {status}")));
    }

    let contents = fs::read_to_string(&temp_path)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    #[cfg(unix)]
    use tempfile::tempdir;

    struct EnvGuard {
        visual: Option<String>,
        editor: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                visual: env::var("VISUAL").ok(),
                editor: env::var("EDITOR").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            restore_env("VISUAL", self.visual.take());
            restore_env("EDITOR", self.editor.take());
        }
    }

    fn restore_env(key: &str, value: Option<String>) {
        match value {
            Some(val) => unsafe { env::set_var(key, val) },
            None => unsafe { env::remove_var(key) },
        }
    }

    #[test]
    #[serial]
    fn resolve_editor_prefers_visual() {
        let _guard = EnvGuard::new();
        unsafe {
            env::set_var("VISUAL", "vis");
            env::set_var("EDITOR", "ed");
        }
        let cmd = resolve_editor_command().unwrap();
        assert_eq!(cmd, vec!["vis".to_string()]);
    }

    #[test]
    #[serial]
    fn resolve_editor_errors_when_unset() {
        let _guard = EnvGuard::new();
        unsafe {
            env::remove_var("VISUAL");
            env::remove_var("EDITOR");
        }
        assert!(matches!(
            resolve_editor_command(),
            Err(EditorError::MissingEditor)
        ));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn run_editor_returns_updated_content() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let script_path = dir.path().join("edit.sh");
        fs::write(&script_path, "#!/bin/sh\nprintf \"edited\" > \"$1\"\n").unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();

        let cmd = vec![script_path.to_string_lossy().to_string()];
        let result = run_editor("seed", &cmd).await.unwrap();
        assert_eq!(result, "edited".to_string());
    }
}
