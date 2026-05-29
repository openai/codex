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
    capture: ShellEnvCapture,
    exports: Mutex<HashMap<String, Option<String>>>,
}

impl ShellEnvFile {
    pub(crate) fn for_session(thread_id: ThreadId, shell: &Shell) -> Result<Option<Self>> {
        let Some(capture) = ShellEnvCapture::for_shell_type(&shell.shell_type) else {
            return Ok(None);
        };
        Self::new(thread_id, capture).map(Some)
    }

    fn new(thread_id: ThreadId, capture: ShellEnvCapture) -> Result<Self> {
        let file = tempfile::Builder::new()
            .prefix(&format!("codex-env-{thread_id}."))
            .suffix(capture.file_suffix())
            .tempfile()
            .context("failed to create temporary shell env file")?;
        Ok(Self {
            path: file.into_temp_path(),
            capture,
            exports: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub(crate) fn insert_path_into_env(&self, env: &mut HashMap<String, String>) {
        insert_env_var(
            env,
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
        if !self.capture.supports_shell_type(&shell.shell_type) {
            return Ok(());
        }

        let mut capture_env = base_env.clone();
        remove_env_var(&mut capture_env, CODEX_ENV_FILE_ENV_VAR);
        self.insert_path_into_env(&mut capture_env);

        let baseline = self
            .capture
            .capture_env_from_shell(shell, cwd, ShellEnvCaptureMode::Baseline, &capture_env)
            .await?;
        let captured = self
            .capture
            .capture_env_from_shell(shell, cwd, ShellEnvCaptureMode::SourceEnvFile, &capture_env)
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
        let thread_id = get_env_var(env, CODEX_THREAD_ID_ENV_VAR).cloned();
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
                    insert_env_var(env, key, value);
                }
                None => {
                    remove_env_var(env, &key);
                }
            }
        }
        for (key, value) in explicit_env_overrides {
            insert_env_var(env, key.clone(), value.clone());
        }
        if let Some(thread_id) = thread_id {
            insert_env_var(env, CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id);
        }
        remove_env_var(env, CODEX_ENV_FILE_ENV_VAR);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellEnvCapture {
    Posix,
    PowerShell,
}

#[derive(Clone, Copy, Debug)]
enum ShellEnvCaptureMode {
    Baseline,
    SourceEnvFile,
}

impl ShellEnvCapture {
    fn for_shell_type(shell_type: &ShellType) -> Option<Self> {
        match shell_type {
            ShellType::Zsh | ShellType::Bash | ShellType::Sh => Some(Self::Posix),
            ShellType::PowerShell => Some(Self::PowerShell),
            ShellType::Cmd => None,
        }
    }

    fn supports_shell_type(self, shell_type: &ShellType) -> bool {
        Self::for_shell_type(shell_type) == Some(self)
    }

    fn file_suffix(self) -> &'static str {
        match self {
            Self::Posix => ".sh",
            Self::PowerShell => ".ps1",
        }
    }

    async fn capture_env_from_shell(
        self,
        shell: &Shell,
        cwd: &Path,
        mode: ShellEnvCaptureMode,
        env: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let script = self.capture_script(mode);
        let output = Command::new(&shell.shell_path)
            .current_dir(cwd)
            .args(self.capture_args(script))
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
        self.parse_env_output(&output.stdout)
    }

    fn capture_script(self, mode: ShellEnvCaptureMode) -> &'static str {
        match (self, mode) {
            (Self::Posix, ShellEnvCaptureMode::Baseline) => POSIX_DUMP_ENV_SCRIPT,
            (Self::Posix, ShellEnvCaptureMode::SourceEnvFile) => {
                POSIX_SOURCE_ENV_FILE_AND_DUMP_ENV_SCRIPT
            }
            (Self::PowerShell, ShellEnvCaptureMode::Baseline) => POWERSHELL_DUMP_ENV_SCRIPT,
            (Self::PowerShell, ShellEnvCaptureMode::SourceEnvFile) => {
                POWERSHELL_SOURCE_ENV_FILE_AND_DUMP_ENV_SCRIPT
            }
        }
    }

    fn capture_args(self, script: &str) -> Vec<&str> {
        match self {
            Self::Posix => vec!["-c", script],
            Self::PowerShell => vec!["-NoLogo", "-NoProfile", "-Command", script],
        }
    }

    fn parse_env_output(self, output: &[u8]) -> Result<HashMap<String, String>> {
        match self {
            Self::Posix => parse_posix_env_output(output),
            Self::PowerShell => parse_powershell_env_output(output),
        }
    }
}

const POSIX_DUMP_ENV_SCRIPT: &str = r#"if [ -x /usr/bin/env ]; then
  /usr/bin/env -0
else
  env -0
fi"#;

const POSIX_SOURCE_ENV_FILE_AND_DUMP_ENV_SCRIPT: &str = r#"if [ -n "${CODEX_ENV_FILE:-}" ] && [ -f "$CODEX_ENV_FILE" ]; then
  if . "$CODEX_ENV_FILE" >/dev/null 2>&1; then
    :
  fi
fi
if [ -x /usr/bin/env ]; then
  /usr/bin/env -0
else
  env -0
fi"#;

const POWERSHELL_DUMP_ENV_SCRIPT: &str = r#"$items = [ordered]@{}
Get-ChildItem Env: | Sort-Object Name | ForEach-Object {
  $items[$_.Name] = $_.Value
}
ConvertTo-Json -InputObject $items -Compress -Depth 2"#;

const POWERSHELL_SOURCE_ENV_FILE_AND_DUMP_ENV_SCRIPT: &str = r#"$envFile = $env:CODEX_ENV_FILE
if (![string]::IsNullOrEmpty($envFile) -and (Test-Path -LiteralPath $envFile -PathType Leaf)) {
  try {
    . $envFile *> $null
  } catch {
  }
}
$items = [ordered]@{}
Get-ChildItem Env: | Sort-Object Name | ForEach-Object {
  $items[$_.Name] = $_.Value
}
ConvertTo-Json -InputObject $items -Compress -Depth 2"#;

fn parse_posix_env_output(output: &[u8]) -> Result<HashMap<String, String>> {
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

fn parse_powershell_env_output(output: &[u8]) -> Result<HashMap<String, String>> {
    let output =
        std::str::from_utf8(output).context("captured PowerShell environment was not UTF-8")?;
    let output = output.trim();
    if output.is_empty() {
        return Ok(HashMap::new());
    }
    let output: HashMap<String, String> =
        serde_json::from_str(output).context("failed to parse captured PowerShell environment")?;
    Ok(output)
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
        if get_env_var(baseline, key) != Some(value) {
            exports.insert(key.clone(), Some(value.clone()));
        }
    }
    for key in baseline.keys() {
        if ignored_capture_key(key) {
            continue;
        }
        if !contains_env_var(captured, key) {
            exports.insert(key.clone(), None);
        }
    }
    exports
}

fn ignored_capture_key(key: &str) -> bool {
    [
        CODEX_ENV_FILE_ENV_VAR,
        CODEX_THREAD_ID_ENV_VAR,
        "PWD",
        "OLDPWD",
        "SHLVL",
        "_",
    ]
    .iter()
    .any(|ignored| env_key_eq(key, ignored))
}

fn get_env_var<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a String> {
    env.iter()
        .find(|(candidate, _)| env_key_eq(candidate, key))
        .map(|(_, value)| value)
}

fn contains_env_var(env: &HashMap<String, String>, key: &str) -> bool {
    get_env_var(env, key).is_some()
}

fn insert_env_var(env: &mut HashMap<String, String>, key: String, value: String) {
    if let Some(existing) = env
        .keys()
        .find(|candidate| env_key_eq(candidate, &key))
        .cloned()
    {
        env.remove(&existing);
    }

    env.insert(key, value);
}

fn remove_env_var(env: &mut HashMap<String, String>, key: &str) {
    if let Some(existing) = env
        .keys()
        .find(|candidate| env_key_eq(candidate, key))
        .cloned()
    {
        env.remove(&existing);
    }
}

fn env_key_eq(candidate: &str, key: &str) -> bool {
    #[cfg(windows)]
    {
        candidate.eq_ignore_ascii_case(key)
    }

    #[cfg(not(windows))]
    {
        candidate == key
    }
}

#[cfg(test)]
#[path = "shell_env_file_tests.rs"]
mod tests;
