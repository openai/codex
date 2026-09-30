use codex_exec_server::ShellInfo;
use codex_shell_command::shell_detect::DetectedShell;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

pub use codex_shell_command::shell_detect::ShellType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shell {
    pub(crate) shell_type: ShellType,
    pub(crate) shell_path: PathBuf,
}

impl Shell {
    pub fn name(&self) -> &'static str {
        self.shell_type.name()
    }

    /// Takes a string of shell and returns the full list of command args to
    /// use with `exec()` to run the shell command.
    pub fn derive_exec_args(&self, command: &str, use_login_shell: bool) -> Vec<String> {
        match self.shell_type {
            ShellType::Zsh | ShellType::Bash | ShellType::Sh => {
                let arg = if use_login_shell { "-lc" } else { "-c" };
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    arg.to_string(),
                    command.to_string(),
                ]
            }
            ShellType::PowerShell => {
                let mut args = vec![self.shell_path.to_string_lossy().to_string()];
                if !use_login_shell {
                    args.push("-NoProfile".to_string());
                }

                args.push("-Command".to_string());
                args.push(command.to_string());
                args
            }
            ShellType::Cmd => {
                let mut args = vec![self.shell_path.to_string_lossy().to_string()];
                args.push("/c".to_string());
                args.push(command.to_string());
                args
            }
        }
    }
}

impl From<DetectedShell> for Shell {
    fn from(detected: DetectedShell) -> Self {
        Self {
            shell_type: detected.shell_type,
            shell_path: detected.shell_path,
        }
    }
}

impl Shell {
    pub(crate) fn from_environment_shell_info(shell_info: ShellInfo) -> anyhow::Result<Self> {
        let shell_type = match shell_info.name.as_str() {
            "zsh" => ShellType::Zsh,
            "bash" => ShellType::Bash,
            "powershell" => ShellType::PowerShell,
            "sh" => ShellType::Sh,
            "cmd" => ShellType::Cmd,
            name => anyhow::bail!("unknown environment shell `{name}`"),
        };

        Ok(Self {
            shell_type,
            shell_path: PathBuf::from(shell_info.path),
        })
    }
}

/// The shell and startup mode Codex chose for an exec command.
#[derive(Debug, Clone)]
pub(crate) struct ShellInvocation {
    pub(crate) shell: Shell,
    pub(crate) use_login_shell: bool,
}

impl ShellInvocation {
    pub(crate) fn is_posix_login(&self) -> bool {
        self.use_login_shell
            && matches!(
                self.shell.shell_type,
                ShellType::Bash | ShellType::Zsh | ShellType::Sh
            )
    }

    /// Builds launch arguments that try to restore the executor's directories to `PATH`
    /// after login startup, so `command` and its children can find them.
    ///
    /// Call this for the POSIX login shell started by Codex when the executor reports
    /// directories and the user has not explicitly configured `PATH`. The request's login
    /// mode is unchanged, even if startup makes `PATH` read-only. Returns `None` if no
    /// directories are reported or the shell or any path cannot use this setup; keep the
    /// original arguments in that case.
    pub(crate) fn derive_exec_args_with_path_prepends(
        &self,
        command: &str,
        paths: &[PathUri],
    ) -> Option<Vec<String>> {
        if !self.is_posix_login() || paths.is_empty() {
            return None;
        }
        let mut setup = Vec::with_capacity(paths.len());
        // Prepend backwards so newly added directories retain the executor's order.
        for path in paths.iter().rev() {
            if path.infer_path_convention() != Some(PathConvention::Posix) {
                return None;
            }
            let path = path.inferred_native_path_string();
            // POSIX PATH has no way to escape a colon inside a single directory.
            if path.contains(':') {
                return None;
            }
            let path = shlex::try_quote(&path).ok()?;
            // Padding `${PATH-}` with colons makes `case` match whole entries, even at the ends;
            // `-` also handles an unset PATH. Re-export a match so child commands inherit it;
            // otherwise probe in a subshell: assigning a read-only PATH can exit the login shell
            // before the user's command runs. `${PATH:+...}` adds `:` only when PATH is nonempty,
            // so we don't make the shell search the current directory.
            setup.push(format!(
                "case \":${{PATH-}}:\" in *:{path}:*) export PATH ;; \
                 *) if (export PATH=) 2>/dev/null; then \
                 export PATH={path}${{PATH:+:\"$PATH\"}}; fi ;; esac"
            ));
        }
        let setup = setup.join("; ");
        // Keep setup on the first input line so shell diagnostics and $LINENO still refer to
        // the original script. If the quoted path contains literal newlines, decode only the
        // setup with eval in the current shell; the trailing `esac` keeps command substitution
        // from trimming any newlines in the path.
        let setup = if setup.contains('\n') {
            let escaped = setup.replace('\\', "\\\\").replace('\n', "\\n");
            let escaped = shlex::try_quote(&escaped).ok()?;
            format!("eval \"$(command -p printf '%b' {escaped})\"")
        } else {
            setup
        };
        let command = format!("{setup}; {command}");
        Some(self.shell.derive_exec_args(&command, self.use_login_shell))
    }
}

#[cfg(all(test, unix))]
fn ultimate_fallback_shell() -> Shell {
    codex_shell_command::shell_detect::ultimate_fallback_shell().into()
}

pub fn get_shell_by_model_provided_path(shell_path: &PathBuf) -> Shell {
    codex_shell_command::shell_detect::get_shell_by_model_provided_path(shell_path).into()
}

pub fn get_shell(shell_type: ShellType) -> Option<Shell> {
    codex_shell_command::shell_detect::get_shell(shell_type).map(Into::into)
}

pub fn default_user_shell() -> Shell {
    codex_shell_command::shell_detect::default_user_shell().into()
}

#[cfg(all(test, target_os = "macos"))]
fn default_user_shell_from_path(user_shell_path: Option<PathBuf>) -> Shell {
    codex_shell_command::shell_detect::default_user_shell_from_path(user_shell_path).into()
}

#[cfg(test)]
#[cfg(unix)]
#[path = "shell_tests.rs"]
mod tests;
