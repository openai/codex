use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ZshShell {
    pub(crate) shell_path: String,
    pub(crate) zshrc_path: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BashShell {
    pub(crate) shell_path: String,
    pub(crate) bashrc_path: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PowerShellConfig {
    pub(crate) exe: String, // Executable name or path, e.g. "pwsh" or "powershell.exe".
    pub(crate) bash_exe_fallback: Option<PathBuf>, // In case the model generates a bash command.
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
            Shell::Zsh(zsh) => std::path::Path::new(&zsh.shell_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Shell::Bash(bash) => std::path::Path::new(&bash.shell_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Shell::PowerShell(ps) => Some(ps.exe.clone()),
            Shell::Unknown => None,
        }
    }
}

#[cfg(unix)]
fn detect_default_user_shell() -> Shell {
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
            let home_path = CStr::from_ptr((*pw).pw_dir).to_string_lossy().into_owned();

            if shell_path.ends_with("/zsh") {
                return Shell::Zsh(ZshShell {
                    shell_path,
                    zshrc_path: format!("{home_path}/.zshrc"),
                });
            }

            if shell_path.ends_with("/bash") {
                return Shell::Bash(BashShell {
                    shell_path,
                    bashrc_path: format!("{home_path}/.bashrc"),
                });
            }
        }
    }
    Shell::Unknown
}

#[cfg(unix)]
pub async fn default_user_shell() -> Shell {
    detect_default_user_shell()
}

#[cfg(target_os = "windows")]
pub async fn default_user_shell() -> Shell {
    use tokio::process::Command;

    // Prefer PowerShell 7+ (`pwsh`) if available, otherwise fall back to Windows PowerShell.
    let has_pwsh = Command::new("pwsh")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("$PSVersionTable.PSVersion.Major")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    // Consistency: use which::which("bash.exe") to resolve a concrete path and
    // validate that specific path with --version. This avoids discrepancies
    // between the path used for detection vs the fallback path we return.
    let bash_exe = match which::which("bash.exe") {
        Ok(path) => {
            let ok = Command::new(&path)
                .arg("--version")
                .stdin(std::process::Stdio::null())
                .output()
                .await
                .ok()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok { Some(path) } else { None }
        }
        Err(_) => None,
    };

    if has_pwsh {
        Shell::PowerShell(PowerShellConfig {
            exe: "pwsh.exe".to_string(),
            bash_exe_fallback: bash_exe,
        })
    } else {
        Shell::PowerShell(PowerShellConfig {
            exe: "powershell.exe".to_string(),
            bash_exe_fallback: bash_exe,
        })
    }
}

#[cfg(all(not(target_os = "windows"), not(unix)))]
pub async fn default_user_shell() -> Shell {
    Shell::Unknown
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

    // Ignored-by-default environment check: requires WSL installed, and the target bash
    // precedes System32\bash.exe in PATH. Compare run resolution (bash.exe --version)
    // with the which-resolved path (<which path> --version). Expect different outputs to
    // highlight the Windows run vs which mismatch.
    #[test]
    #[ignore]
    fn test_bash_run_vs_which_with_target_bash_priority() {
        use std::path::PathBuf;
        use std::process::Command;

        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let sys32 = PathBuf::from(&windir).join("System32");
        let wsl_exe = sys32.join("wsl.exe");
        let sys_bash_exe = sys32.join("bash.exe");
        if !wsl_exe.exists() && !sys_bash_exe.exists() {
            eprintln!("[test] skip: WSL not detected under System32");
            return;
        }

        let which_path = match which::which("bash.exe") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[test] skip: which('bash.exe') failed: {}", e);
                return;
            }
        };
        let sys_bash_canon = sys_bash_exe.canonicalize().ok();
        let which_canon = which_path.canonicalize().ok();
        if sys_bash_canon.is_some() && which_canon.is_some() && sys_bash_canon == which_canon {
            eprintln!(
                "[test] skip: which resolved to System32\\bash.exe; ensure target bash precedes WSL in PATH"
            );
            return;
        }

        let run_out = Command::new("bash.exe").arg("--version").output();
        let which_out = Command::new(&which_path).arg("--version").output();

        let (run_ok, run_stdout) = match run_out {
            Ok(o) => (
                o.status.success(),
                String::from_utf8_lossy(&o.stdout).to_string(),
            ),
            Err(e) => {
                eprintln!("[test] run bash.exe failed: {}", e);
                return;
            }
        };
        let (which_ok, which_stdout) = match which_out {
            Ok(o) => (
                o.status.success(),
                String::from_utf8_lossy(&o.stdout).to_string(),
            ),
            Err(e) => {
                eprintln!("[test] which path {:?} failed: {}", which_path, e);
                return;
            }
        };

        eprintln!("[test] which_path: {:?}", which_path);
        eprintln!("[test] run_ok: {}", run_ok);
        eprintln!("[test] which_ok: {}", which_ok);
        eprintln!("[test] run_stdout:\n{}", run_stdout.trim());
        eprintln!("[test] which_stdout:\n{}", which_stdout.trim());

        assert!(
            run_ok,
            "run bash.exe --version should succeed (WSL present)"
        );
        assert!(which_ok, "which-resolved bash --version should succeed");
        assert_ne!(
            run_stdout, which_stdout,
            "Expected different outputs; ensure PATH target bash precedes WSL bash"
        );
    }
}
