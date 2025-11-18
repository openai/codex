use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ZshShell {
    pub(crate) shell_path: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BashShell {
    pub(crate) shell_path: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PowerShellConfig {
    pub(crate) shell_path: PathBuf, // Executable name or path, e.g. "pwsh" or "powershell.exe".
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Shell {
    Zsh(ZshShell),
    Bash(BashShell),
    PowerShell(PowerShellConfig),
    Unknown,
}

impl Shell {
    pub fn name(&self) -> Option<String> {
        match self {
            Shell::Zsh(ZshShell { shell_path, .. }) | Shell::Bash(BashShell { shell_path, .. }) => {
                std::path::Path::new(shell_path)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
            }
            Shell::PowerShell(ps) => ps
                .shell_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string()),
            Shell::Unknown => None,
        }
    }

    /// Takes a string of shell and returns the full list of command args to
    /// use with `exec()` to run the shell command.
    pub fn derive_exec_args(&self, command: &str, use_login_shell: bool) -> Vec<String> {
        match self {
            Shell::Zsh(ZshShell { shell_path, .. }) | Shell::Bash(BashShell { shell_path, .. }) => {
                let arg = if use_login_shell { "-lc" } else { "-c" };
                vec![
                    shell_path.to_string_lossy().to_string(),
                    arg.to_string(),
                    command.to_string(),
                ]
            }
            Shell::PowerShell(ps) => {
                let mut args = vec![
                    ps.shell_path.to_string_lossy().to_string(),
                    "-NoLogo".to_string(),
                ];
                if !use_login_shell {
                    args.push("-NoProfile".to_string());
                }

                args.push("-Command".to_string());
                args.push(command.to_string());
                args
            }
            Shell::Unknown => shlex::split(command).unwrap_or_else(|| vec![command.to_string()]),
        }
    }
}

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

fn get_zsh_shell(path: Option<&PathBuf>) -> Option<ZshShell> {
    let shell_path = get_shell_path(ShellType::Zsh, path, "zsh", vec!["/bin/zsh"]);

    shell_path.map(|shell_path| ZshShell { shell_path })
}

fn get_bash_shell(path: Option<&PathBuf>) -> Option<BashShell> {
    let shell_path = get_shell_path(ShellType::Bash, path, "bash", vec!["/bin/bash"]);

    shell_path.map(|shell_path| BashShell { shell_path })
}

fn get_powershell_shell(path: Option<&PathBuf>) -> Option<PowerShellConfig> {
    let shell_path = get_shell_path(
        ShellType::PowerShell,
        path,
        "pwsh",
        vec!["/usr/local/bin/pwsh"],
    )
    .or_else(|| get_shell_path(ShellType::PowerShell, path, "powershell", vec![]));

    shell_path.map(|shell_path| PowerShellConfig { shell_path })
}

pub fn get_shell_by_model_provided_path(shell_path: &PathBuf) -> Shell {
    detect_shell_type(shell_path)
        .and_then(|shell_type| get_shell(shell_type, Some(shell_path)))
        .unwrap_or(Shell::Unknown)
}

pub fn get_shell(shell_type: ShellType, path: Option<&PathBuf>) -> Option<Shell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell(path).map(Shell::Zsh),
        ShellType::Bash => get_bash_shell(path).map(Shell::Bash),
        ShellType::PowerShell => get_powershell_shell(path).map(Shell::PowerShell),
    }
}

pub fn detect_shell_type(shell_path: &PathBuf) -> Option<ShellType> {
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
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

pub async fn default_user_shell() -> Shell {
    if cfg!(windows) {
        get_shell(ShellType::PowerShell, None).unwrap_or(Shell::Unknown)
    } else {
        get_user_shell_path()
            .and_then(|shell| detect_shell_type(&shell))
            .and_then(|shell_type| get_shell(shell_type, None))
            .unwrap_or(Shell::Unknown)
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
            detect_shell_type(&PathBuf::from("pwsh.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    #[cfg(target_os = "macos")]
    fn detects_zsh() {
        let zsh_shell = get_shell(ShellType::Zsh, None).unwrap();

        let ZshShell { shell_path } = match zsh_shell {
            Shell::Zsh(zsh_shell) => zsh_shell,
            _ => panic!("expected zsh shell"),
        };

        assert_eq!(shell_path, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn detects_bash() {
        let bash_shell = get_shell(ShellType::Bash, None).unwrap();
        let BashShell { shell_path } = match bash_shell {
            Shell::Bash(bash_shell) => bash_shell,
            _ => panic!("expected bash shell"),
        };

        assert!(
            shell_path == PathBuf::from("/bin/bash")
                || shell_path == PathBuf::from("/usr/bin/bash"),
            "shell path: {shell_path:?}",
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
                default_user_shell().await,
                Shell::Zsh(ZshShell {
                    shell_path: PathBuf::from(shell_path),
                })
            );
        }
    }

    #[tokio::test]
    async fn detects_powershell_as_default() {
        if !cfg!(windows) {
            return;
        }

        let powershell_shell = default_user_shell().await;
        let PowerShellConfig { shell_path } = match powershell_shell {
            Shell::PowerShell(powershell_shell) => powershell_shell,
            _ => panic!("expected powershell shell"),
        };

        assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
    }

    #[test]
    fn finds_poweshell() {
        if !cfg!(windows) {
            return;
        }

        let powershell_shell = get_shell(ShellType::PowerShell, None).unwrap();
        let PowerShellConfig { shell_path } = match powershell_shell {
            Shell::PowerShell(powershell_shell) => powershell_shell,
            _ => panic!("expected powershell shell"),
        };

        assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
    }
}
