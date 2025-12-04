use std::env;
use std::fs;
use std::process::Stdio;

use color_eyre::eyre::Report;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use shlex::split as shlex_split;
use tempfile::Builder;
use tokio::process::Command;

/// Resolve the editor command from environment variables.
/// Prefers `VISUAL` over `EDITOR`.
pub(crate) fn resolve_editor_command() -> Result<Vec<String>> {
    let raw = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .wrap_err("VISUAL/EDITOR not set")?;
    let parts = shlex_split(&raw).ok_or_else(|| Report::msg("failed to parse editor command"))?;
    if parts.is_empty() {
        return Err(Report::msg("editor command is empty"));
    }
    Ok(parts)
}

/// Write `seed` to a temp file, launch the editor command, and return the updated content.
///
/// Returns `Ok(None)` when the file is left empty after the editor exits.
pub(crate) async fn run_editor(seed: &str, editor_cmd: &[String]) -> Result<Option<String>> {
    if editor_cmd.is_empty() {
        return Err(Report::msg("editor command is empty"));
    }

    let tempfile = Builder::new().suffix(".md").tempfile()?;
    fs::write(tempfile.path(), seed)?;

    let mut cmd = Command::new(&editor_cmd[0]);
    if editor_cmd.len() > 1 {
        cmd.args(&editor_cmd[1..]);
    }
    let status = cmd
        .arg(tempfile.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(Report::msg(format!("editor exited with status {status}")));
    }

    let contents = fs::read_to_string(tempfile.path())?;
    if contents.is_empty() {
        Ok(None)
    } else {
        Ok(Some(contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
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
        assert!(resolve_editor_command().is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
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
        assert_eq!(result, Some("edited".to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_editor_returns_none_when_file_empty() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let script_path = dir.path().join("edit.sh");
        fs::write(&script_path, "#!/bin/sh\n: > \"$1\"\n").unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();

        let cmd = vec![script_path.to_string_lossy().to_string()];
        let result = run_editor("seed", &cmd).await.unwrap();
        assert_eq!(result, None);
    }
}
