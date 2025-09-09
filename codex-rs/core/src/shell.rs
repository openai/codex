use serde::Deserialize;
use serde::Serialize;
use shlex;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::trace;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
/// This structure cannot derive Clone or this will break the Drop implementation.
pub struct ShellSnapshot {
    pub(crate) path: PathBuf,
}

impl ShellSnapshot {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for ShellSnapshot {
    fn drop(&mut self) {
        delete_shell_snapshot(&self.path);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PosixShell {
    pub(crate) shell_path: String,
    pub(crate) rc_path: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) shell_snapshot: Option<Arc<ShellSnapshot>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PowerShellConfig {
    exe: String, // Executable name or path, e.g. "pwsh" or "powershell.exe".
    bash_exe_fallback: Option<PathBuf>, // In case the model generates a bash command.
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Shell {
    Posix(PosixShell),
    PowerShell(PowerShellConfig),
    Unknown,
}

impl Shell {
    pub fn format_default_shell_invocation(&self, command: Vec<String>) -> Option<Vec<String>> {
        match self {
            Shell::Posix(shell) => {
                let joined = strip_bash_lc(&command)
                    .or_else(|| shlex::try_join(command.iter().map(|s| s.as_str())).ok())?;

                let mut source_path = Path::new(&shell.rc_path);

                let session_cmd = if let Some(shell_snapshot) = &shell.shell_snapshot
                    && shell_snapshot.path.exists()
                {
                    source_path = shell_snapshot.path.as_path();
                    "-c".to_string()
                } else {
                    "-lc".to_string()
                };

                let source_path_str = source_path.to_string_lossy().to_string();
                let quoted_source_path = shlex::try_quote(&source_path_str).ok()?;
                let rc_command =
                    format!("[ -f {quoted_source_path} ] && . {quoted_source_path}; ({joined})");

                Some(vec![shell.shell_path.clone(), session_cmd, rc_command])
            }
            Shell::PowerShell(ps) => {
                // If model generated a bash command, prefer a detected bash fallback
                if let Some(script) = strip_bash_lc(&command) {
                    return match &ps.bash_exe_fallback {
                        Some(bash) => Some(vec![
                            bash.to_string_lossy().to_string(),
                            "-lc".to_string(),
                            script,
                        ]),

                        // No bash fallback → run the script under PowerShell.
                        // It will likely fail (except for some simple commands), but the error
                        // should give a clue to the model to fix upon retry that it's running under PowerShell.
                        None => Some(vec![
                            ps.exe.clone(),
                            "-NoProfile".to_string(),
                            "-Command".to_string(),
                            script,
                        ]),
                    };
                }

                // Not a bash command. If model did not generate a PowerShell command,
                // turn it into a PowerShell command.
                let first = command.first().map(String::as_str);
                if first != Some(ps.exe.as_str()) {
                    // TODO (CODEX_2900): Handle escaping newlines.
                    if command.iter().any(|a| a.contains('\n') || a.contains('\r')) {
                        return Some(command);
                    }

                    let joined = shlex::try_join(command.iter().map(|s| s.as_str())).ok();
                    return joined.map(|arg| {
                        vec![
                            ps.exe.clone(),
                            "-NoProfile".to_string(),
                            "-Command".to_string(),
                            arg,
                        ]
                    });
                }

                // Model generated a PowerShell command. Run it.
                Some(command)
            }
            Shell::Unknown => None,
        }
    }

    pub fn name(&self) -> Option<String> {
        match self {
            Shell::Posix(shell) => Path::new(&shell.shell_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Shell::PowerShell(ps) => Some(ps.exe.clone()),
            Shell::Unknown => None,
        }
    }

    pub fn get_snapshot(&self) -> Option<Arc<ShellSnapshot>> {
        match self {
            Shell::Posix(shell) => shell.shell_snapshot.clone(),
            _ => None,
        }
    }
}

fn strip_bash_lc(command: &Vec<String>) -> Option<String> {
    match command.as_slice() {
        // exactly three items
        [first, second, third]
            // first two must be "bash", "-lc"
            if first == "bash" && second == "-lc" =>
        {
            Some(third.clone())
        }
        _ => None,
    }
}

#[cfg(unix)]
async fn detect_default_user_shell(session_id: Uuid, codex_home: &Path) -> Shell {
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

            let rc_path = if shell_path.ends_with("/zsh") {
                format!("{home_path}/.zshrc")
            } else if shell_path.ends_with("/bash") {
                format!("{home_path}/.bashrc")
            } else {
                return Shell::Unknown;
            };

            let snapshot_path = snapshots::ensure_posix_snapshot(
                &shell_path,
                &rc_path,
                Path::new(&home_path),
                codex_home,
                session_id,
            )
            .await;
            if snapshot_path.is_none() {
                trace!("failed to prepare posix snapshot; using live profile");
            }
            let shell_snapshot =
                snapshot_path.map(|snapshot| Arc::new(ShellSnapshot::new(snapshot)));

            return Shell::Posix(PosixShell {
                shell_path,
                rc_path,
                shell_snapshot,
            });
        }
    }
    Shell::Unknown
}

#[cfg(unix)]
pub async fn default_user_shell(session_id: Uuid, codex_home: &Path) -> Shell {
    detect_default_user_shell(session_id, codex_home).await
}

#[cfg(target_os = "windows")]
pub async fn default_user_shell(_session_id: Uuid, _codex_home: &Path) -> Shell {
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
    let bash_exe = if Command::new("bash.exe")
        .arg("--version")
        .output()
        .await
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        which::which("bash.exe").ok()
    } else {
        None
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
pub async fn default_user_shell(_session_id: Uuid, _codex_home: &Path) -> Shell {
    Shell::Unknown
}

#[cfg(unix)]
mod snapshots {
    use super::*;

    fn zsh_profile_paths(home: &Path) -> Vec<PathBuf> {
        [".zshenv", ".zprofile", ".zshrc", ".zlogin"]
            .into_iter()
            .map(|name| home.join(name))
            .collect()
    }

    fn posix_profile_source_script(home: &Path) -> String {
        zsh_profile_paths(home)
            .into_iter()
            .map(|profile| {
                let profile_string = profile.to_string_lossy().into_owned();
                let quoted = shlex::try_quote(&profile_string)
                    .map(|cow| cow.into_owned())
                    .unwrap_or(profile_string.clone());

                format!("[ -f {quoted} ] && . {quoted}")
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub(crate) async fn ensure_posix_snapshot(
        shell_path: &str,
        rc_path: &str,
        home: &Path,
        codex_home: &Path,
        session_id: Uuid,
    ) -> Option<PathBuf> {
        let snapshot_path = codex_home.join(format!("shell_snapshots/snapshot_{session_id}.zsh"));

        // Check if an update in the profile requires to re-generate the snapshot.
        let snapshot_is_stale = async {
            let snapshot_metadata = tokio::fs::metadata(&snapshot_path).await.ok()?;
            let snapshot_modified = snapshot_metadata.modified().ok()?;

            for profile in zsh_profile_paths(home) {
                let Ok(profile_metadata) = tokio::fs::metadata(&profile).await else {
                    continue;
                };

                let Ok(profile_modified) = profile_metadata.modified() else {
                    return Some(true);
                };

                if profile_modified > snapshot_modified {
                    return Some(true);
                }
            }

            Some(false)
        }
        .await
        .unwrap_or(true);

        if !snapshot_is_stale {
            return Some(snapshot_path);
        }

        match regenerate_posix_snapshot(shell_path, rc_path, home, &snapshot_path).await {
            Ok(()) => Some(snapshot_path),
            Err(err) => {
                tracing::warn!("failed to generate posix snapshot: {err}");
                None
            }
        }
    }

    async fn regenerate_posix_snapshot(
        shell_path: &str,
        rc_path: &str,
        home: &Path,
        snapshot_path: &Path,
    ) -> std::io::Result<()> {
        // Use `emulate -L sh` instead of `set -o posix` so we work on zsh builds
        // that disable that option. Guard `alias -p` with `|| true` so the script
        // keeps a zero exit status even if aliases are disabled.
        let mut capture_script = String::new();
        let profile_sources = posix_profile_source_script(home);
        if !profile_sources.is_empty() {
            capture_script.push_str(&format!("{profile_sources}; "));
        }

        let zshrc = home.join(rc_path);

        capture_script.push_str(
            &format!(". {}; setopt posixbuiltins; export -p; {{ alias | sed 's/^/alias /'; }} 2>/dev/null || true", zshrc.display()),
        );
        let output = tokio::process::Command::new(shell_path)
            .arg("-lc")
            .arg(capture_script)
            .env("HOME", home)
            .output()
            .await?;

        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "snapshot capture exited with status {}",
                output.status
            )));
        }

        let mut contents = String::from("# Generated by Codex. Do not edit.\n");

        contents.push_str(&String::from_utf8_lossy(&output.stdout));
        contents.push('\n');

        if let Some(parent) = snapshot_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let tmp_path = snapshot_path.with_extension("tmp");
        tokio::fs::write(&tmp_path, contents).await?;

        // Restrict the snapshot to user read/write so that environment variables or aliases
        // that may contain secrets are not exposed to other users on the system.
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(&tmp_path, permissions).await?;

        tokio::fs::rename(&tmp_path, snapshot_path).await?;
        Ok(())
    }
}

pub(crate) fn delete_shell_snapshot(path: &Path) {
    if let Err(err) = std::fs::remove_file(path) {
        trace!("failed to delete shell snapshot {path:?}: {err}");
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    #[tokio::test]
    async fn test_run_with_profile_zshrc_not_exists() {
        let shell = Shell::Posix(PosixShell {
            shell_path: "/bin/zsh".to_string(),
            rc_path: "/does/not/exist/.zshrc".to_string(),
            shell_snapshot: None,
        });
        let actual_cmd = shell.format_default_shell_invocation(vec!["myecho".to_string()]);
        assert_eq!(
            actual_cmd,
            Some(vec![
                "/bin/zsh".to_string(),
                "-lc".to_string(),
                "[ -f /does/not/exist/.zshrc ] && . /does/not/exist/.zshrc; (myecho)".to_string(),
            ])
        );
    }

    #[tokio::test]
    async fn test_run_with_profile_bash_escaping_and_execution() {
        let shell_path = "/bin/bash";

        let cases = vec![
            (
                vec!["myecho"],
                vec![
                    shell_path,
                    "-lc",
                    "[ -f BASHRC_PATH ] && . BASHRC_PATH; (myecho)",
                ],
                Some("It works!\n"),
            ),
            (
                vec!["bash", "-lc", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-lc",
                    "[ -f BASHRC_PATH ] && . BASHRC_PATH; (echo 'single' \"double\")",
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
            let shell = Shell::Posix(PosixShell {
                shell_path: shell_path.to_string(),
                rc_path: bashrc_path.to_str().unwrap().to_string(),
                shell_snapshot: None,
            });

            let actual_cmd = shell
                .format_default_shell_invocation(input.iter().map(|s| s.to_string()).collect());
            let expected_cmd = expected_cmd
                .iter()
                .map(|s| {
                    s.replace("BASHRC_PATH", bashrc_path.to_str().unwrap())
                        .to_string()
                })
                .collect();

            assert_eq!(actual_cmd, Some(expected_cmd));

            let output = process_exec_tool_call(
                ExecParams {
                    command: actual_cmd.unwrap(),
                    cwd: PathBuf::from(temp_home.path()),
                    timeout_ms: None,
                    env: HashMap::from([(
                        "HOME".to_string(),
                        temp_home.path().to_str().unwrap().to_string(),
                    )]),
                    with_escalated_permissions: None,
                    justification: None,
                },
                SandboxType::None,
                &SandboxPolicy::DangerFullAccess,
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
    use super::*;
    use crate::shell::snapshots::ensure_posix_snapshot;

    #[tokio::test]
    async fn test_snapshot_generation_uses_session_id_and_cleanup() {
        let shell_path = "/bin/zsh";

        let temp_home = tempfile::tempdir().unwrap();
        let codex_home = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_home.path().join(".zshrc"),
            "export SNAPSHOT_TEST_VAR=1\nalias snapshot_test_alias='echo hi'\n",
        )
        .unwrap();

        let session_id = Uuid::new_v4();
        let snapshot_path = ensure_posix_snapshot(
            shell_path,
            ".zshrc",
            temp_home.path(),
            codex_home.path(),
            session_id,
        )
        .await
        .expect("snapshot path");

        let filename = snapshot_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(filename.contains(&session_id.to_string()));
        assert!(snapshot_path.exists());

        let snapshot_path_second = ensure_posix_snapshot(
            shell_path,
            ".zshrc",
            temp_home.path(),
            codex_home.path(),
            session_id,
        )
        .await
        .expect("snapshot path");
        assert_eq!(snapshot_path, snapshot_path_second);

        let contents = std::fs::read_to_string(&snapshot_path).unwrap();
        assert!(contents.contains("alias snapshot_test_alias='echo hi'"));
        assert!(contents.contains("SNAPSHOT_TEST_VAR=1"));

        delete_shell_snapshot(&snapshot_path);
        assert!(!snapshot_path.exists());
    }

    #[test]
    fn format_default_shell_invocation_prefers_snapshot_when_available() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot.zsh");
        std::fs::write(&snapshot_path, "export SNAPSHOT_READY=1").unwrap();

        let shell = Shell::Posix(PosixShell {
            shell_path: "/bin/zsh".to_string(),
            rc_path: {
                let path = temp_dir.path().join(".zshrc");
                std::fs::write(&path, "# test zshrc").unwrap();
                path.to_string_lossy().to_string()
            },
            shell_snapshot: Some(Arc::new(ShellSnapshot::new(snapshot_path.clone()))),
        });

        let invocation = shell.format_default_shell_invocation(vec!["echo".to_string()]);
        let expected_command = vec!["/bin/zsh".to_string(), "-c".to_string(), {
            let snapshot_path = snapshot_path.to_string_lossy();
            format!("[ -f {snapshot_path} ] && . {snapshot_path}; (echo)")
        }];

        assert_eq!(invocation, Some(expected_command));
    }

    #[tokio::test]
    async fn test_run_with_profile_escaping_and_execution() {
        let shell_path = "/bin/zsh";

        let cases = vec![
            (
                vec!["myecho"],
                vec![
                    shell_path,
                    "-lc",
                    "[ -f ZSHRC_PATH ] && . ZSHRC_PATH; (myecho)",
                ],
                Some("It works!\n"),
            ),
            (
                vec!["myecho"],
                vec![
                    shell_path,
                    "-lc",
                    "[ -f ZSHRC_PATH ] && . ZSHRC_PATH; (myecho)",
                ],
                Some("It works!\n"),
            ),
            (
                vec!["bash", "-c", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-lc",
                    "[ -f ZSHRC_PATH ] && . ZSHRC_PATH; (bash -c \"echo 'single' \\\"double\\\"\")",
                ],
                Some("single double\n"),
            ),
            (
                vec!["bash", "-lc", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-lc",
                    "[ -f ZSHRC_PATH ] && . ZSHRC_PATH; (echo 'single' \"double\")",
                ],
                Some("single double\n"),
            ),
        ];
        for (input, expected_cmd, expected_output) in cases {
            use std::collections::HashMap;
            use std::path::PathBuf;

            use crate::exec::ExecParams;
            use crate::exec::SandboxType;
            use crate::exec::process_exec_tool_call;
            use crate::protocol::SandboxPolicy;

            // create a temp directory with a zshrc file in it
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
            let shell = Shell::Posix(PosixShell {
                shell_path: shell_path.to_string(),
                rc_path: zshrc_path.to_str().unwrap().to_string(),
                shell_snapshot: None,
            });

            let actual_cmd = shell
                .format_default_shell_invocation(input.iter().map(|s| s.to_string()).collect());
            let expected_cmd = expected_cmd
                .iter()
                .map(|s| {
                    s.replace("ZSHRC_PATH", zshrc_path.to_str().unwrap())
                        .to_string()
                })
                .collect();

            assert_eq!(actual_cmd, Some(expected_cmd));
            // Actually run the command and check output/exit code
            let output = process_exec_tool_call(
                ExecParams {
                    command: actual_cmd.unwrap(),
                    cwd: PathBuf::from(temp_home.path()),
                    timeout_ms: None,
                    env: HashMap::from([(
                        "HOME".to_string(),
                        temp_home.path().to_str().unwrap().to_string(),
                    )]),
                    with_escalated_permissions: None,
                    justification: None,
                },
                SandboxType::None,
                &SandboxPolicy::DangerFullAccess,
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
        let cases = vec![
            (
                Shell::PowerShell(PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: None,
                }),
                vec!["bash", "-lc", "echo hello"],
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                Shell::PowerShell(PowerShellConfig {
                    exe: "powershell.exe".to_string(),
                    bash_exe_fallback: None,
                }),
                vec!["bash", "-lc", "echo hello"],
                vec!["powershell.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                Shell::PowerShell(PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                }),
                vec!["bash", "-lc", "echo hello"],
                vec!["bash.exe", "-lc", "echo hello"],
            ),
            (
                Shell::PowerShell(PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                }),
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
                Shell::PowerShell(PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                }),
                vec!["echo", "hello"],
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                Shell::PowerShell(PowerShellConfig {
                    exe: "pwsh.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                }),
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
                vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"],
            ),
            (
                // TODO (CODEX_2900): Handle escaping newlines for powershell invocation.
                Shell::PowerShell(PowerShellConfig {
                    exe: "powershell.exe".to_string(),
                    bash_exe_fallback: Some(PathBuf::from("bash.exe")),
                }),
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

        for (shell, input, expected_cmd) in cases {
            let actual_cmd = shell
                .format_default_shell_invocation(input.iter().map(|s| s.to_string()).collect());
            assert_eq!(
                actual_cmd,
                Some(expected_cmd.iter().map(|s| s.to_string()).collect())
            );
        }
    }
}
