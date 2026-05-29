use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_protocol::ThreadId;
use codex_protocol::shell_environment::CODEX_ENV_FILE_ENV_VAR;
use codex_protocol::shell_environment::CODEX_THREAD_ID_ENV_VAR;
use tempfile::TempPath;
use tokio::process::Command;

use crate::shell::Shell;
use crate::shell::ShellType;

/// Session-owned script that hooks can populate with exported shell state.
///
/// Only lifecycle hooks receive the writable file path. After SessionStart
/// hooks finish, Codex captures supported exported variables and passes those
/// values to later commands without exposing the writable path.
pub(crate) struct ShellEnvFile {
    path: TempPath,
    exports: Mutex<HashMap<String, Option<String>>>,
}

impl ShellEnvFile {
    pub(crate) fn for_session(thread_id: ThreadId) -> Result<Option<Self>> {
        #[cfg(windows)]
        {
            let _ = thread_id;
            // TODO: Support a Windows shell environment persistence contract,
            // likely with PowerShell- and cmd-compatible formats.
            Ok(None)
        }
        #[cfg(not(windows))]
        {
            Self::new(thread_id).map(Some)
        }
    }

    #[cfg(not(windows))]
    fn new(thread_id: ThreadId) -> Result<Self> {
        let file = tempfile::Builder::new()
            .prefix(&format!("codex-env-{thread_id}."))
            .suffix(".sh")
            .tempfile()
            .context("failed to create temporary shell env file")?;
        Ok(Self {
            path: file.into_temp_path(),
            exports: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub(crate) fn insert_path_into_env(&self, env: &mut HashMap<String, String>) {
        env.insert(
            CODEX_ENV_FILE_ENV_VAR.to_string(),
            self.path().to_string_lossy().to_string(),
        );
    }

    /// Sources the hook-writable env file once and stores the resulting
    /// environment diff.
    ///
    /// The temp file remains an input channel for SessionStart hooks, but later
    /// commands receive only captured variable changes. Running both a baseline
    /// environment dump and a sourced dump lets shell syntax such as command
    /// substitution behave naturally without keeping `CODEX_ENV_FILE` available
    /// after hook execution.
    pub(crate) async fn capture_exports(
        &self,
        shell: &Shell,
        cwd: &Path,
        base_env: &HashMap<String, String>,
    ) -> Result<()> {
        if !matches!(
            shell.shell_type,
            ShellType::Zsh | ShellType::Bash | ShellType::Sh
        ) {
            return Ok(());
        }

        let mut capture_env = base_env.clone();
        capture_env.remove(CODEX_ENV_FILE_ENV_VAR);
        self.insert_path_into_env(&mut capture_env);

        let baseline = capture_env_from_shell(shell, cwd, DUMP_ENV_SCRIPT, &capture_env).await?;
        let captured = capture_env_from_shell(
            shell,
            cwd,
            SOURCE_ENV_FILE_AND_DUMP_ENV_SCRIPT,
            &capture_env,
        )
        .await?;
        let exports = diff_env(&baseline, &captured);
        *self
            .exports
            .lock()
            .map_err(|_| anyhow!("shell env exports lock poisoned"))? = exports;
        Ok(())
    }

    /// Applies captured SessionStart environment changes to a command
    /// environment without exposing the writable env-file path.
    ///
    /// Explicit shell-environment policy values are layered back on top so
    /// configured overrides keep precedence, and runtime-owned values such as
    /// the Codex thread id are preserved rather than accepting hook-written
    /// replacements.
    pub(crate) fn apply_exports(
        &self,
        env: &mut HashMap<String, String>,
        explicit_env_overrides: &HashMap<String, String>,
    ) {
        let thread_id = env.get(CODEX_THREAD_ID_ENV_VAR).cloned();
        let exports = self
            .exports
            .lock()
            .map(|exports| exports.clone())
            .unwrap_or_default();
        for (key, value) in exports {
            if ignored_capture_key(&key) {
                continue;
            }
            match value {
                Some(value) => {
                    env.insert(key, value);
                }
                None => {
                    env.remove(&key);
                }
            }
        }
        for (key, value) in explicit_env_overrides {
            env.insert(key.clone(), value.clone());
        }
        if let Some(thread_id) = thread_id {
            env.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id);
        }
        env.remove(CODEX_ENV_FILE_ENV_VAR);
    }
}

const DUMP_ENV_SCRIPT: &str = r#"if [ -x /usr/bin/env ]; then
  /usr/bin/env -0
else
  env -0
fi"#;

const SOURCE_ENV_FILE_AND_DUMP_ENV_SCRIPT: &str = r#"if [ -n "${CODEX_ENV_FILE:-}" ] && [ -f "$CODEX_ENV_FILE" ]; then
  if . "$CODEX_ENV_FILE" >/dev/null 2>&1; then
    :
  fi
fi
if [ -x /usr/bin/env ]; then
  /usr/bin/env -0
else
  env -0
fi"#;

async fn capture_env_from_shell(
    shell: &Shell,
    cwd: &Path,
    script: &str,
    env: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let output = Command::new(&shell.shell_path)
        .current_dir(cwd)
        .arg("-c")
        .arg(script)
        .env_clear()
        .envs(env)
        .output()
        .await
        .with_context(|| format!("failed to run {}", shell.shell_path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to capture shell environment with {}: {stderr}",
            shell.shell_path.display()
        );
    }
    parse_env_output(&output.stdout)
}

fn parse_env_output(output: &[u8]) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for entry in output.split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        let Some(separator) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let key = String::from_utf8(entry[..separator].to_vec())
            .context("captured shell environment key was not UTF-8")?;
        let value = String::from_utf8(entry[separator + 1..].to_vec())
            .context("captured shell environment value was not UTF-8")?;
        env.insert(key, value);
    }
    Ok(env)
}

fn diff_env(
    baseline: &HashMap<String, String>,
    captured: &HashMap<String, String>,
) -> HashMap<String, Option<String>> {
    let mut exports = HashMap::new();
    for (key, value) in captured {
        if ignored_capture_key(key) {
            continue;
        }
        if baseline.get(key) != Some(value) {
            exports.insert(key.clone(), Some(value.clone()));
        }
    }
    for key in baseline.keys() {
        if ignored_capture_key(key) {
            continue;
        }
        if !captured.contains_key(key) {
            exports.insert(key.clone(), None);
        }
    }
    exports
}

fn ignored_capture_key(key: &str) -> bool {
    matches!(
        key,
        CODEX_ENV_FILE_ENV_VAR | CODEX_THREAD_ID_ENV_VAR | "PWD" | "OLDPWD" | "SHLVL" | "_"
    )
}

#[cfg(all(test, not(windows)))]
#[path = "shell_env_file_tests.rs"]
mod tests;
