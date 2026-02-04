use crate::shell_snapshot::ShellSnapshot;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

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
    pub fn name(&self) -> &'static str {
        match self.shell_type {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::PowerShell => "powershell",
            ShellType::Sh => "sh",
            ShellType::Cmd => "cmd",
        }
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

    /// Return the shell snapshot if existing.
    pub fn shell_snapshot(&self) -> Option<Arc<ShellSnapshot>> {
        self.shell_snapshot.borrow().clone()
    }
}

pub(crate) fn empty_shell_snapshot_receiver() -> watch::Receiver<Option<Arc<ShellSnapshot>>> {
    let (_tx, rx) = watch::channel(None);
    rx
}

impl PartialEq for Shell {
    fn eq(&self, other: &Self) -> bool {
        self.shell_type == other.shell_type && self.shell_path == other.shell_path
    }
}

impl Eq for Shell {}

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
        Some(PathBuf::from(path))
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
    if provided_path.and_then(file_exists).is_some() {
        return provided_path.cloned();
    }

    // Check if the shell we are trying to load is user's default shell
    // if just use it
    let default_shell_path = get_user_shell_path();
    if let Some(default_shell_path) = default_shell_path
        && detect_shell_type(&default_shell_path) == Some(shell_type)
        && file_exists(&default_shell_path).is_some()
    {
        return Some(default_shell_path);
    }

    if let Ok(path) = which::which(binary_name) {
        return Some(path);
    }

    for path in fallback_paths {
        //check exists
        if let Some(path) = file_exists(&PathBuf::from(path)) {
            return Some(path);
        }
    }

    None
}

fn get_zsh_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Zsh, path, "zsh", vec!["/bin/zsh"]);

    shell_path.map(|shell_path| Shell {
        shell_type: ShellType::Zsh,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_bash_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Bash, path, "bash", vec!["/bin/bash"]);

    shell_path.map(|shell_path| Shell {
        shell_type: ShellType::Bash,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_sh_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Sh, path, "sh", vec!["/bin/sh"]);

    shell_path.map(|shell_path| Shell {
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
    .or_else(|| get_shell_path(ShellType::PowerShell, path, "powershell", vec![]));

    shell_path.map(|shell_path| Shell {
        shell_type: ShellType::PowerShell,
        shell_path,
        shell_snapshot: empty_shell_snapshot_receiver(),
    })
}

fn get_cmd_shell(path: Option<&PathBuf>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Cmd, path, "cmd", vec![]);

    shell_path.map(|shell_path| Shell {
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

pub fn get_shell_by_model_provided_path(shell_path: &PathBuf) -> Shell {
    detect_shell_type(shell_path)
        .and_then(|shell_type| get_shell(shell_type, Some(shell_path)))
        .unwrap_or(ultimate_fallback_shell())
}

pub fn get_shell(shell_type: ShellType, path: Option<&PathBuf>) -> Option<Shell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell(path),
        ShellType::Bash => get_bash_shell(path),
        ShellType::PowerShell => get_powershell_shell(path),
        ShellType::Sh => get_sh_shell(path),
        ShellType::Cmd => get_cmd_shell(path),
    }
}

pub fn detect_shell_type(shell_path: &PathBuf) -> Option<ShellType> {
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
        Some("sh") => Some(ShellType::Sh),
        Some("cmd") => Some(ShellType::Cmd),
        Some("bash") => Some(ShellType::Bash),
        Some("pwsh") => Some(ShellType::PowerShell),
        Some("powershell") => Some(ShellType::PowerShell),
        _ => {
            let shell_name = shell_path.file_stem();
            if let Some(shell_name) = shell_name
                && shell_name != shell_path
            {
                detect_shell_type(&PathBuf::from(shell_name))
            } else {
                None
            }
        }
    }
}

pub fn default_user_shell() -> Shell {
    default_user_shell_from_path(get_user_shell_path())
}

fn default_user_shell_from_path(user_shell_path: Option<PathBuf>) -> Shell {
    if cfg!(windows) {
        get_shell(ShellType::PowerShell, None).unwrap_or(ultimate_fallback_shell())
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

        shell_with_fallback.unwrap_or(ultimate_fallback_shell())
    }
}

#[cfg(test)]
mod detect_shell_type_tests {
    use super::*;

    #[test]
    fn test_detect_shell_type() {
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
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from(if cfg!(windows) {
                "C:\\windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
            } else {
                "/usr/local/bin/pwsh"
            })),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("pwsh.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/sh")),
            Some(ShellType::Sh)
        );
        assert_eq!(detect_shell_type(&PathBuf::from("sh")), Some(ShellType::Sh));
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd")),
            Some(ShellType::Cmd)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd.exe")),
            Some(ShellType::Cmd)
        );
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    #[cfg(target_os = "macos")]
    fn detects_zsh() {
        let zsh_shell = get_shell(ShellType::Zsh, None).unwrap();

        let shell_path = zsh_shell.shell_path;

        assert_eq!(shell_path, Path::new("/bin/zsh"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn fish_fallback_to_zsh() {
        let zsh_shell = default_user_shell_from_path(Some(PathBuf::from("/bin/fish")));

        let shell_path = zsh_shell.shell_path;

        assert_eq!(shell_path, Path::new("/bin/zsh"));
    }

    #[test]
    fn detects_bash() {
        let bash_shell = get_shell(ShellType::Bash, None).unwrap();
        let shell_path = bash_shell.shell_path;

        assert!(
            shell_path == Path::new("/bin/bash")
                || shell_path == Path::new("/usr/bin/bash")
                || shell_path == Path::new("/usr/local/bin/bash"),
            "shell path: {shell_path:?}",
        );
    }

    #[test]
    fn detects_sh() {
        let sh_shell = get_shell(ShellType::Sh, None).unwrap();
        let shell_path = sh_shell.shell_path;
        assert!(
            shell_path == Path::new("/bin/sh") || shell_path == Path::new("/usr/bin/sh"),
            "shell path: {shell_path:?}",
        );
    }

    #[test]
    fn can_run_on_shell_test() {
        let cmd = "echo \"Works\"";
        if cfg!(windows) {
            assert!(shell_works(
                get_shell(ShellType::PowerShell, None),
                "Out-String 'Works'",
                true,
            ));
            assert!(shell_works(get_shell(ShellType::Cmd, None), cmd, true,));
            assert!(shell_works(Some(ultimate_fallback_shell()), cmd, true));
        } else {
            assert!(shell_works(Some(ultimate_fallback_shell()), cmd, true));
            assert!(shell_works(get_shell(ShellType::Zsh, None), cmd, false));
            assert!(shell_works(get_shell(ShellType::Bash, None), cmd, true));
            assert!(shell_works(get_shell(ShellType::Sh, None), cmd, true));
        }
    }

    fn shell_works(shell: Option<Shell>, command: &str, required: bool) -> bool {
        if let Some(shell) = shell {
            let args = shell.derive_exec_args(command, false);
            let output = Command::new(args[0].clone())
                .args(&args[1..])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert!(String::from_utf8_lossy(&output.stdout).contains("Works"));
            true
        } else {
            !required
        }
    }

    #[test]
    fn derive_exec_args() {
        let test_bash_shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };
        assert_eq!(
            test_bash_shell.derive_exec_args("echo hello", false),
            vec!["/bin/bash", "-c", "echo hello"]
        );
        assert_eq!(
            test_bash_shell.derive_exec_args("echo hello", true),
            vec!["/bin/bash", "-lc", "echo hello"]
        );

        let test_zsh_shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };
        assert_eq!(
            test_zsh_shell.derive_exec_args("echo hello", false),
            vec!["/bin/zsh", "-c", "echo hello"]
        );
        assert_eq!(
            test_zsh_shell.derive_exec_args("echo hello", true),
            vec!["/bin/zsh", "-lc", "echo hello"]
        );

        let test_powershell_shell = Shell {
            shell_type: ShellType::PowerShell,
            shell_path: PathBuf::from("pwsh.exe"),
            shell_snapshot: empty_shell_snapshot_receiver(),
        };
        assert_eq!(
            test_powershell_shell.derive_exec_args("echo hello", false),
            vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"]
        );
        assert_eq!(
            test_powershell_shell.derive_exec_args("echo hello", true),
            vec!["pwsh.exe", "-Command", "echo hello"]
        );
    }

    #[tokio::test]
    async fn test_current_shell_detects_zsh() {
        let shell = Command::new("sh")
            .arg("-c")
            .arg("echo $SHELL")
            .output()
            .unwrap();

        let shell_path = String::from_utf8_lossy(&shell.stdout).trim().to_string();
        if shell_path.ends_with("/zsh") {
            assert_eq!(
                default_user_shell(),
                Shell {
                    shell_type: ShellType::Zsh,
                    shell_path: PathBuf::from(shell_path),
                    shell_snapshot: empty_shell_snapshot_receiver(),
                }
            );
        }
    }

    #[tokio::test]
    async fn detects_powershell_as_default() {
        if !cfg!(windows) {
            return;
        }

        let powershell_shell = default_user_shell();
        let shell_path = powershell_shell.shell_path;

        assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
    }

    #[test]
    fn finds_poweshell() {
        if !cfg!(windows) {
            return;
        }

        let powershell_shell = get_shell(ShellType::PowerShell, None).unwrap();
        let shell_path = powershell_shell.shell_path;

        assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
    }
}
