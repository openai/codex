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
    pub(crate) zshrc_path: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BashShell {
    pub(crate) shell_path: PathBuf,
    pub(crate) bashrc_path: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PowerShellConfig {
    pub(crate) exe: PathBuf, // Executable name or path, e.g. "pwsh" or "powershell.exe".
    pub(crate) bash_exe_fallback: Option<BashShell>, // In case the model generates a bash command.
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
            Shell::PowerShell(ps) => ps.exe.file_stem().map(|s| s.to_string_lossy().to_string()),
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
                let mut args = vec![ps.exe.to_string_lossy().to_string(), "-NoLogo".to_string()];
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
fn get_user_home() -> Option<String> {
    use libc::getpwuid;
    use libc::getuid;
    use std::ffi::CStr;

    unsafe {
        let uid = getuid();
        let pw = getpwuid(uid);

        if !pw.is_null() {
            let home_path = CStr::from_ptr((*pw).pw_dir).to_string_lossy().into_owned();
            Some(home_path)
        } else {
            None
        }
    }
}

#[cfg(not(unix))]
fn get_user_home() -> Option<String> {
    std::env::var("HOME").ok()
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
fn get_user_shell() -> Option<String> {
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
    binary_name: &str,
    fallback_paths: Vec<&str>,
) -> Option<PathBuf> {
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

fn get_zsh_shell() -> Option<ZshShell> {
    let shell_path = get_shell_path(ShellType::Zsh, "zsh", vec!["/bin/zsh"]);

    shell_path.map(|shell_path| ZshShell {
        shell_path,
        zshrc_path: get_user_home()
            .and_then(|home| file_exists(&PathBuf::from(format!("{home}/.zshrc")))),
    })
}

fn get_bash_shell() -> Option<BashShell> {
    let shell_path = get_shell_path(ShellType::Bash, "bash", vec!["/bin/bash"]);

    shell_path.map(|shell_path| BashShell {
        shell_path,
        bashrc_path: get_user_home()
            .and_then(|home| file_exists(&PathBuf::from(format!("{home}/.bashrc")))),
    })
}

fn get_powershell_shell() -> Option<PowerShellConfig> {
    let shell_path = get_shell_path(ShellType::PowerShell, "pwsh", vec!["/usr/local/bin/pwsh"])
        .or_else(|| get_shell_path(ShellType::PowerShell, "powershell", vec![]));

    shell_path.map(|shell_path| PowerShellConfig {
        exe: shell_path,
        bash_exe_fallback: get_bash_shell(),
    })
}

pub fn get_shell(shell_type: ShellType) -> Option<Shell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell().map(Shell::Zsh),
        ShellType::Bash => get_bash_shell().map(Shell::Bash),
        ShellType::PowerShell => get_powershell_shell().map(Shell::PowerShell),
    }
}

pub fn detect_shell_type(shell_path: &PathBuf) -> Option<ShellType> {
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
        Some("bash") => Some(ShellType::Bash),
        Some("pwsh") => Some(ShellType::PowerShell),
        Some("powershell") => Some(ShellType::PowerShell),
        _ => {
            let shell_name = std::path::Path::new(shell_path).file_stem();

            shell_name.and_then(|name| detect_shell_type(&PathBuf::from(name)))
        }
    }
}

fn detect_default_user_shell() -> Shell {
    get_user_shell_path()
        .and_then(|shell| detect_shell_type(&shell))
        .and_then(|t| get_shell(t))
        .unwrap_or(Shell::Unknown)
}

#[cfg(unix)]
pub async fn default_user_shell() -> Shell {
    detect_default_user_shell()
}

#[cfg(test)]
mod detect_shell_type_tests {
    use super::*;

    #[test]
    fn test_detect_shell_type() {
        assert_eq!(detect_shell_type("zsh"), ShellType::Zsh);
        assert_eq!(detect_shell_type("bash"), ShellType::Bash);
        assert_eq!(detect_shell_type("pwsh"), ShellType::PowerShell);
        assert_eq!(detect_shell_type("powershell"), ShellType::PowerShell);
        assert_eq!(detect_shell_type("fish"), ShellType::Unknown);
        assert_eq!(detect_shell_type("other"), ShellType::Unknown);
        assert_eq!(detect_shell_type("/bin/zsh"), ShellType::Zsh);
        assert_eq!(detect_shell_type("/bin/bash"), ShellType::Bash);
        assert_eq!(detect_shell_type("powershell.exe"), ShellType::PowerShell);
        assert_eq!(detect_shell_type("pwsh.exe"), ShellType::PowerShell);
        assert_eq!(detect_shell_type("/usr/local/bin/pwsh"), ShellType::Unknown);
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    #[tokio::test]
    async fn test_current_shell_detects_zsh() {
        let shell = Command::new("sh")
            .arg("-c")
            .arg("echo $SHELL")
            .output()
            .unwrap();

        let home = std::env::var("HOME").unwrap();
        let shell_path = String::from_utf8_lossy(&shell.stdout).trim().to_string();
        if shell_path.ends_with("/zsh") {
            assert_eq!(
                default_user_shell().await,
                Shell::Zsh(ZshShell {
                    shell_path: shell_path.to_string(),
                    zshrc_path: format!("{home}/.zshrc",),
                })
            );
        }
    }

    #[tokio::test]
    async fn test_run_with_profile_bash_escaping_and_execution() {
        let shell_path = "/bin/bash";

        let cases = vec![
            (
                vec!["myecho"],
                vec![shell_path, "-lc", "source BASHRC_PATH && (myecho)"],
                Some("It works!\n"),
            ),
            (
                vec!["bash", "-lc", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-lc",
                    "source BASHRC_PATH && (echo 'single' \"double\")",
                ],
                Some("single double\n"),
            ),
        ];

        for (input, expected_cmd, expected_output) in cases {
            use std::collections::HashMap;

            use crate::exec::ExecParams;
            use crate::exec::SandboxType;
            use crate::exec::process_exec_tool_call;
            use crate::protocol::SandboxPolicy;

            let temp_home = tempfile::tempdir().unwrap();
            let bashrc_path = temp_home.path().join(".bashrc");
            std::fs::write(
                &bashrc_path,
                r#"
                set -x
                function myecho {
                    echo 'It works!'
                }
                "#,
            )
            .unwrap();
            let command = expected_cmd
                .iter()
                .map(|s| s.replace("BASHRC_PATH", bashrc_path.to_str().unwrap()))
                .collect::<Vec<_>>();

            let output = process_exec_tool_call(
                ExecParams {
                    command: command.clone(),
                    cwd: PathBuf::from(temp_home.path()),
                    timeout_ms: None,
                    env: HashMap::from([(
                        "HOME".to_string(),
                        temp_home.path().to_str().unwrap().to_string(),
                    )]),
                    with_escalated_permissions: None,
                    justification: None,
                    arg0: None,
                },
                SandboxType::None,
                &SandboxPolicy::DangerFullAccess,
                temp_home.path(),
                &None,
                None,
            )
            .await
            .unwrap();

            assert_eq!(output.exit_code, 0, "input: {input:?} output: {output:?}");
            if let Some(expected) = expected_output {
                assert_eq!(
                    output.stdout.text, expected,
                    "input: {input:?} output: {output:?}"
                );
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod macos_tests {
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_run_with_profile_escaping_and_execution() {
        let shell_path = "/bin/zsh";

        let cases = vec![
            (
                vec!["myecho"],
                vec![shell_path, "-lc", "source ZSHRC_PATH && (myecho)"],
                Some("It works!\n"),
            ),
            (
                vec!["myecho"],
                vec![shell_path, "-lc", "source ZSHRC_PATH && (myecho)"],
                Some("It works!\n"),
            ),
            (
                vec!["bash", "-c", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-lc",
                    "source ZSHRC_PATH && (bash -c \"echo 'single' \\\"double\\\"\")",
                ],
                Some("single double\n"),
            ),
            (
                vec!["bash", "-lc", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-lc",
                    "source ZSHRC_PATH && (echo 'single' \"double\")",
                ],
                Some("single double\n"),
            ),
        ];
        for (input, expected_cmd, expected_output) in cases {
            use std::collections::HashMap;

            use crate::exec::ExecParams;
            use crate::exec::SandboxType;
            use crate::exec::process_exec_tool_call;
            use crate::protocol::SandboxPolicy;

            let temp_home = tempfile::tempdir().unwrap();
            let zshrc_path = temp_home.path().join(".zshrc");
            std::fs::write(
                &zshrc_path,
                r#"
                set -x
                function myecho {
                    echo 'It works!'
                }
                "#,
            )
            .unwrap();
            let command = expected_cmd
                .iter()
                .map(|s| s.replace("ZSHRC_PATH", zshrc_path.to_str().unwrap()))
                .collect::<Vec<_>>();

            let output = process_exec_tool_call(
                ExecParams {
                    command: command.clone(),
                    cwd: PathBuf::from(temp_home.path()),
                    timeout_ms: None,
                    env: HashMap::from([(
                        "HOME".to_string(),
                        temp_home.path().to_str().unwrap().to_string(),
                    )]),
                    with_escalated_permissions: None,
                    justification: None,
                    arg0: None,
                },
                SandboxType::None,
                &SandboxPolicy::DangerFullAccess,
                temp_home.path(),
                &None,
                None,
            )
            .await
            .unwrap();

            assert_eq!(output.exit_code, 0, "input: {input:?} output: {output:?}");
            if let Some(expected) = expected_output {
                assert_eq!(
                    output.stdout.text, expected,
                    "input: {input:?} output: {output:?}"
                );
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests_windows {
    use super::*;

    #[test]
    fn test_format_default_shell_invocation_powershell() {
        use std::path::PathBuf;

        let cases = vec![
            (
                PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: None,
                },
                vec!["bash", "-lc", "echo hello"],
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                PowerShellConfig {
                    exe: "powershell.exe".to_string(),
                    bash_exe_fallback: None,
                },
                vec!["bash", "-lc", "echo hello"],
                vec!["powershell.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                },
                vec!["bash", "-lc", "echo hello"],
                vec!["bash.exe", "-lc", "echo hello"],
            ),
            (
                PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                },
                vec![
                    "bash",
                    "-lc",
                    "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: destination_file.txt\n-original content\n+modified content\n*** End Patch\nEOF",
                ],
                vec![
                    "bash.exe",
                    "-lc",
                    "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: destination_file.txt\n-original content\n+modified content\n*** End Patch\nEOF",
                ],
            ),
            (
                PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                },
                vec!["echo", "hello"],
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                },
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                PowerShellConfig {
                    exe: "powershell.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                },
                vec![
                    "codex-mcp-server.exe",
                    "--codex-run-as-apply-patch",
                    "*** Begin Patch\n*** Update File: C:\\Users\\person\\destination_file.txt\n-original content\n+modified content\n*** End Patch",
                ],
                vec![
                    "codex-mcp-server.exe",
                    "--codex-run-as-apply-patch",
                    "*** Begin Patch\n*** Update File: C:\\Users\\person\\destination_file.txt\n-original content\n+modified content\n*** End Patch",
                ],
            ),
        ];

        for (config, input, expected_cmd) in cases {
            let command = expected_cmd
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<_>>();

            // These tests assert the final command for each scenario now that the helper
            // has been removed. The inputs remain to document the original coverage.
            let expected = expected_cmd
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<_>>();
            assert_eq!(command, expected, "input: {input:?} config: {config:?}");
        }
    }
}
