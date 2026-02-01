//! Shell type detection and configuration.
//!
//! This module provides types and functions for detecting the user's default shell,
//! resolving shell paths, and configuring shell execution.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
use tokio::sync::watch;

use crate::snapshot::ShellSnapshot;

/// Supported shell types.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

/// Shell configuration with path and optional snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shell {
    pub(crate) shell_type: ShellType,
    pub(crate) shell_path: PathBuf,
    #[serde(
        skip_serializing,
        skip_deserializing,
        default = "empty_shell_snapshot_receiver"
    )]
    pub(crate) shell_snapshot: watch::Receiver<Option<Arc<ShellSnapshot>>>,
}

impl Shell {
    /// Returns the shell type.
    pub fn shell_type(&self) -> &ShellType {
        &self.shell_type
    }

    /// Returns the shell path.
    pub fn shell_path(&self) -> &PathBuf {
        &self.shell_path
    }

    /// Returns the short name of the shell.
    pub fn name(&self) -> &'static str {
        match self.shell_type {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::PowerShell => "powershell",
            ShellType::Sh => "sh",
            ShellType::Cmd => "cmd",
        }
    }

    /// Derives the command arguments for executing a shell command.
    ///
    /// Returns a vector of strings suitable for use with `Command::new()`.
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
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    "/c".to_string(),
                    command.to_string(),
                ]
            }
        }
    }

    /// Returns the current shell snapshot if available.
    pub fn shell_snapshot(&self) -> Option<Arc<ShellSnapshot>> {
        self.shell_snapshot.borrow().clone()
    }

    /// Sets the shell snapshot receiver.
    pub fn set_shell_snapshot_receiver(
        &mut self,
        receiver: watch::Receiver<Option<Arc<ShellSnapshot>>>,
    ) {
        self.shell_snapshot = receiver;
    }
}

/// Creates an empty shell snapshot receiver (always None).
pub fn empty_shell_snapshot_receiver() -> watch::Receiver<Option<Arc<ShellSnapshot>>> {
    let (_tx, rx) = watch::channel(None);
    rx
}

impl PartialEq for Shell {
    fn eq(&self, other: &Self) -> bool {
        self.shell_type == other.shell_type && self.shell_path == other.shell_path
    }
}

impl Eq for Shell {}

/// Detects the shell type from a path.
///
/// Returns `None` if the shell type cannot be determined.
pub fn detect_shell_type(shell_path: &PathBuf) -> Option<ShellType> {
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
        Some("sh") => Some(ShellType::Sh),
        Some("cmd") => Some(ShellType::Cmd),
        Some("bash") => Some(ShellType::Bash),
        Some("pwsh") | Some("powershell") => Some(ShellType::PowerShell),
        _ => {
            // Try to get shell name from the file stem
            let shell_name = shell_path.file_stem();
            if let Some(shell_name) = shell_name {
                if shell_name != shell_path.as_os_str() {
                    detect_shell_type(&PathBuf::from(shell_name))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

/// Returns the user's default shell.
///
/// On Unix, this uses the passwd database. On Windows, it defaults to PowerShell.
/// Falls back to `/bin/sh` on Unix or `cmd.exe` on Windows if detection fails.
pub fn default_user_shell() -> Shell {
    default_user_shell_from_path(get_user_shell_path())
}

fn default_user_shell_from_path(user_shell_path: Option<PathBuf>) -> Shell {
    if cfg!(windows) {
        get_shell(ShellType::PowerShell, None).unwrap_or_else(ultimate_fallback_shell)
    } else {
        let user_default_shell = user_shell_path
            .and_then(|shell| detect_shell_type(&shell))
            .and_then(|shell_type| get_shell(shell_type, None));

        let shell_with_fallback = if cfg!(target_os = "macos") {
            user_default_shell
                .or_else(|| get_shell(ShellType::Zsh, None))
                .or_else(|| get_shell(ShellType::Bash, None))
        } else {
            user_default_shell
                .or_else(|| get_shell(ShellType::Bash, None))
                .or_else(|| get_shell(ShellType::Zsh, None))
        };

        shell_with_fallback.unwrap_or_else(ultimate_fallback_shell)
    }
}

/// Gets a shell of the specified type, optionally at a specific path.
pub fn get_shell(shell_type: ShellType, path: Option<&PathBuf>) -> Option<Shell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell(path),
        ShellType::Bash => get_bash_shell(path),
        ShellType::PowerShell => get_powershell_shell(path),
        ShellType::Sh => get_sh_shell(path),
        ShellType::Cmd => get_cmd_shell(path),
    }
}

/// Gets a shell by the model-provided path, detecting its type automatically.
pub fn get_shell_by_path(shell_path: &PathBuf) -> Shell {
    detect_shell_type(shell_path)
        .and_then(|shell_type| get_shell(shell_type, Some(shell_path)))
        .unwrap_or_else(ultimate_fallback_shell)
}

// Platform-specific user shell detection
#[cfg(unix)]
fn get_user_shell_path() -> Option<PathBuf> {
    use libc::getpwuid;
    use libc::getuid;
    use std::ffi::CStr;

    unsafe {
        let uid = getuid();
        let pw = getpwuid(uid);

        if !pw.is_null() {
            let shell_path = CStr::from_ptr((*pw).pw_shell)
                .to_string_lossy()
                .into_owned();
            Some(PathBuf::from(shell_path))
        } else {
            None
        }
    }
}

#[cfg(not(unix))]
fn get_user_shell_path() -> Option<PathBuf> {
    None
}

fn file_exists(path: &PathBuf) -> Option<PathBuf> {
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        Some(path.clone())
    } else {
        None
    }
}

fn get_shell_path(
    shell_type: ShellType,
    provided_path: Option<&PathBuf>,
    binary_name: &str,
    fallback_paths: Vec<&str>,
) -> Option<PathBuf> {
    // If exact provided path exists, use it
    if let Some(path) = provided_path {
        if file_exists(path).is_some() {
            return Some(path.clone());
        }
    }

    // Check if the shell we are trying to load is user's default shell
    let default_shell_path = get_user_shell_path();
    if let Some(ref default_shell_path) = default_shell_path {
        if detect_shell_type(default_shell_path) == Some(shell_type.clone()) {
            return Some(default_shell_path.clone());
        }
    }

    // Try to find via `which`
    if let Ok(path) = which::which(binary_name) {
        return Some(path);
    }

    // Try fallback paths
    for path in fallback_paths {
        if let Some(path) = file_exists(&PathBuf::from(path)) {
            return Some(path);
        }
    }

    None
}

fn get_zsh_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Zsh, path, "zsh", vec!["/bin/zsh"])?;
    Some(Shell {
        shell_type: ShellType::Zsh,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_bash_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Bash, path, "bash", vec!["/bin/bash"])?;
    Some(Shell {
        shell_type: ShellType::Bash,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_sh_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Sh, path, "sh", vec!["/bin/sh"])?;
    Some(Shell {
        shell_type: ShellType::Sh,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_powershell_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(
        ShellType::PowerShell,
        path,
        "pwsh",
        vec!["/usr/local/bin/pwsh"],
    )
    .or_else(|| get_shell_path(ShellType::PowerShell, path, "powershell", vec![]))?;

    Some(Shell {
        shell_type: ShellType::PowerShell,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_cmd_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Cmd, path, "cmd", vec![])?;
    Some(Shell {
        shell_type: ShellType::Cmd,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn ultimate_fallback_shell() -> Shell {
    if cfg!(windows) {
        Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        }
    } else {
        Shell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_type_simple() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(detect_shell_type(&PathBuf::from("fish")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("other")), None);
    }

    #[test]
    fn test_detect_shell_type_full_path() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/sh")),
            Some(ShellType::Sh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
    }

    #[test]
    fn test_detect_shell_type_with_extension() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("pwsh.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd")),
            Some(ShellType::Cmd)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd.exe")),
            Some(ShellType::Cmd)
        );
    }

    #[test]
    fn test_shell_name() {
        let shells = [
            (ShellType::Zsh, "zsh"),
            (ShellType::Bash, "bash"),
            (ShellType::Sh, "sh"),
            (ShellType::PowerShell, "powershell"),
            (ShellType::Cmd, "cmd"),
        ];

        for (shell_type, expected_name) in shells {
            let shell = Shell {
                shell_type,
                shell_path: PathBuf::from("/bin/test"),
                shell_snapshot: empty_shell_snapshot_receiver(),
            };
            assert_eq!(shell.name(), expected_name);
        }
    }

    #[test]
    fn test_derive_exec_args_bash() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };

        assert_eq!(
            shell.derive_exec_args("echo hello", false),
            vec!["/bin/bash", "-c", "echo hello"]
        );
        assert_eq!(
            shell.derive_exec_args("echo hello", true),
            vec!["/bin/bash", "-lc", "echo hello"]
        );
    }

    #[test]
    fn test_derive_exec_args_zsh() {
        let shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };

        assert_eq!(
            shell.derive_exec_args("echo hello", false),
            vec!["/bin/zsh", "-c", "echo hello"]
        );
        assert_eq!(
            shell.derive_exec_args("echo hello", true),
            vec!["/bin/zsh", "-lc", "echo hello"]
        );
    }

    #[test]
    fn test_derive_exec_args_powershell() {
        let shell = Shell {
            shell_type: ShellType::PowerShell,
            shell_path: PathBuf::from("pwsh.exe"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };

        assert_eq!(
            shell.derive_exec_args("echo hello", false),
            vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"]
        );
        assert_eq!(
            shell.derive_exec_args("echo hello", true),
            vec!["pwsh.exe", "-Command", "echo hello"]
        );
    }

    #[test]
    fn test_derive_exec_args_cmd() {
        let shell = Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };

        assert_eq!(
            shell.derive_exec_args("echo hello", false),
            vec!["cmd.exe", "/c", "echo hello"]
        );
    }

    #[test]
    fn test_shell_equality() {
        let shell1 = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };
        let shell2 = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };
        let shell3 = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };

        assert_eq!(shell1, shell2);
        assert_ne!(shell1, shell3);
    }

    #[cfg(unix)]
    #[test]
    fn test_get_shell_bash() {
        let shell = get_shell(ShellType::Bash, None);
        assert!(shell.is_some());
        let shell = shell.expect("bash should be available");
        assert_eq!(shell.shell_type, ShellType::Bash);
    }

    #[cfg(unix)]
    #[test]
    fn test_get_shell_sh() {
        let shell = get_shell(ShellType::Sh, None);
        assert!(shell.is_some());
        let shell = shell.expect("sh should be available");
        assert_eq!(shell.shell_type, ShellType::Sh);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_get_shell_zsh_macos() {
        let shell = get_shell(ShellType::Zsh, None);
        assert!(shell.is_some());
        let shell = shell.expect("zsh should be available on macOS");
        assert_eq!(shell.shell_type, ShellType::Zsh);
    }

    #[test]
    fn test_default_user_shell() {
        let shell = default_user_shell();
        // Should always return a valid shell
        assert!(!shell.shell_path.as_os_str().is_empty());
    }

    #[test]
    fn test_ultimate_fallback() {
        let shell = ultimate_fallback_shell();
        if cfg!(windows) {
            assert_eq!(shell.shell_type, ShellType::Cmd);
        } else {
            assert_eq!(shell.shell_type, ShellType::Sh);
        }
    }
}
