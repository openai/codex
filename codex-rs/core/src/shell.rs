use crate::bash::try_parse_bash;
use crate::bash::try_parse_word_only_commands_sequence;
use codex_protocol::models::ShellToolCallParams;
use serde::Deserialize;
use serde::Serialize;
use shlex;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ZshShell {
    shell_path: String,
    zshrc_path: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BashShell {
    shell_path: String,
    bashrc_path: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PowerShellConfig {
    exe: String, // Executable name or path, e.g. "pwsh" or "powershell.exe".
    bash_exe_fallback: Option<PathBuf>, // In case the model generates a bash command.
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Shell {
    Zsh(ZshShell),
    Bash(BashShell),
    PowerShell(PowerShellConfig),
    Unknown,
}

impl Shell {
    pub fn format_default_shell_invocation(&self, command: Vec<String>) -> Option<Vec<String>> {
        match self {
            Shell::Zsh(zsh) => {
                format_shell_invocation_with_rc(&command, &zsh.shell_path, &zsh.zshrc_path)
            }
            Shell::Bash(bash) => {
                format_shell_invocation_with_rc(&command, &bash.shell_path, &bash.bashrc_path)
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

fn format_shell_invocation_with_rc(
    command: &Vec<String>,
    shell_path: &str,
    rc_path: &str,
) -> Option<Vec<String>> {
    let joined = strip_bash_lc(command)
        .or_else(|| shlex::try_join(command.iter().map(|s| s.as_str())).ok())?;

    let rc_command = if std::path::Path::new(rc_path).exists() {
        format!("source {rc_path} && ({joined})")
    } else {
        joined
    };

    Some(vec![shell_path.to_string(), "-lc".to_string(), rc_command])
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

pub fn rewrite_cd_prefix_into_workdir(mut params: ShellToolCallParams) -> ShellToolCallParams {
    if params.workdir.is_some() {
        return params;
    }

    if let Some((script_idx, script)) = extract_shell_script(&params.command)
        && let Some((workdir, rest)) = split_cd_prefix_from_script(script)
    {
        params.workdir = Some(workdir);
        params.command[script_idx] = rest;
        return params;
    }

    if let Some((workdir, rest_command)) = split_cd_prefix_from_tokens(&params.command) {
        params.workdir = Some(workdir);
        params.command = rest_command;
    }

    params
}

fn extract_shell_script(command: &[String]) -> Option<(usize, &str)> {
    match command {
        [shell, flag, script]
            if looks_like_shell_wrapper(shell) && (flag == "-lc" || flag == "-c") =>
        {
            Some((2, script.as_str()))
        }
        _ => None,
    }
}

fn looks_like_shell_wrapper(shell: &str) -> bool {
    let name = Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(shell);

    matches!(name, "bash" | "sh" | "zsh")
}

fn split_cd_prefix_from_script(script: &str) -> Option<(String, String)> {
    let tree = try_parse_bash(script)?;
    let mut commands = try_parse_word_only_commands_sequence(&tree, script)?;
    if commands.is_empty() {
        return None;
    }

    commands.reverse();
    let mut iter = commands.into_iter();
    let cd_command = iter.next()?;
    if cd_command.first().map(String::as_str) != Some("cd") {
        return None;
    }

    if cd_command.len() != 2 {
        return None;
    }

    if iter.any(|cmd| cmd.first().map(String::as_str) == Some("cd")) {
        return None;
    }

    let workdir = cd_command.get(1)?.clone();
    if workdir.is_empty() {
        return None;
    }

    let tokens = shlex::split(script)?;
    let rest_tokens = tokens_after_first_command(&tokens)?;
    let rest_script = shlex::try_join(rest_tokens.iter().map(|s| s.as_str())).ok()?;

    Some((workdir, rest_script))
}

fn split_cd_prefix_from_tokens(command: &[String]) -> Option<(String, Vec<String>)> {
    if command.len() < 4 {
        return None;
    }

    if command.first()?.as_str() != "cd" {
        return None;
    }

    if command.get(2)?.as_str() != "&&" {
        return None;
    }

    let workdir = command.get(1)?.clone();
    let remainder = command.get(3..)?.to_vec();
    if remainder.is_empty()
        || remainder
            .first()
            .is_some_and(|first| first.as_str() == "cd" || first.starts_with("cd"))
    {
        return None;
    }

    Some((workdir, remainder))
}

fn tokens_after_first_command(tokens: &[String]) -> Option<&[String]> {
    let mut idx = 0;
    while idx < tokens.len() && tokens[idx] != "&&" {
        idx += 1;
    }

    if idx == tokens.len() {
        return None;
    }

    let rest_start = idx + 1;
    if rest_start >= tokens.len() {
        return None;
    }

    Some(&tokens[rest_start..])
}

#[cfg(test)]
mod rewrite_cd_prefix_tests {
    use super::rewrite_cd_prefix_into_workdir;
    use codex_protocol::models::ShellToolCallParams;
    use shlex;

    fn shell_params(command: &[&str], workdir: Option<&str>) -> ShellToolCallParams {
        ShellToolCallParams {
            command: command.iter().map(|s| s.to_string()).collect(),
            workdir: workdir.map(|s| s.to_string()),
            timeout_ms: None,
            with_escalated_permissions: None,
            justification: None,
        }
    }

    #[test]
    fn cd_prefix_in_bash_script_is_promoted_to_workdir() {
        let params = shell_params(&["bash", "-lc", "cd foo && echo hi"], None);
        let updated = rewrite_cd_prefix_into_workdir(params);
        assert_eq!(
            updated.command,
            vec!["bash".to_string(), "-lc".to_string(), "echo hi".to_string()]
        );
        assert_eq!(updated.workdir.as_deref(), Some("foo"));
    }

    #[test]
    fn cd_prefix_with_quoted_path_sets_workdir() {
        let params = shell_params(&["bash", "-lc", "  cd 'foo bar' && ls"], None);
        let updated = rewrite_cd_prefix_into_workdir(params);
        assert_eq!(
            updated.command,
            vec!["bash".to_string(), "-lc".to_string(), "ls".to_string()]
        );
        assert_eq!(updated.workdir.as_deref(), Some("foo bar"));
    }

    #[test]
    fn cd_prefix_with_ampersands_inside_string_is_promoted() {
        let params = shell_params(
            &["bash", "-lc", "cd foo && echo \"checking && markers\""],
            None,
        );
        let updated = rewrite_cd_prefix_into_workdir(params);
        assert_eq!(updated.workdir.as_deref(), Some("foo"));

        let script = updated.command.get(2).expect("bash script");
        let tokens = shlex::split(script).expect("split script");
        assert_eq!(
            tokens,
            vec!["echo".to_string(), "checking && markers".to_string()]
        );
    }

    #[test]
    fn cd_prefix_with_or_connector_is_left_unchanged() {
        let params = shell_params(
            &[
                "bash",
                "-lc",
                "cd and_and || echo \"couldn't find the dir for &&\"",
            ],
            None,
        );
        let updated = rewrite_cd_prefix_into_workdir(params.clone());
        assert_eq!(updated, params);
    }

    #[test]
    fn existing_workdir_is_left_intact() {
        let params = shell_params(&["bash", "-lc", "cd foo && ls"], Some("already"));
        let updated = rewrite_cd_prefix_into_workdir(params.clone());
        assert_eq!(updated, params);
    }

    #[test]
    fn cd_prefix_without_shell_wrapper_is_promoted() {
        let params = shell_params(&["cd", "foo", "&&", "git", "status"], None);
        let updated = rewrite_cd_prefix_into_workdir(params);
        assert_eq!(
            updated.command,
            vec!["git".to_string(), "status".to_string()]
        );
        assert_eq!(updated.workdir.as_deref(), Some("foo"));
    }

    #[test]
    fn cd_prefix_with_additional_cd_is_left_unchanged() {
        let params = shell_params(&["bash", "-lc", "cd foo && cd bar && ls"], None);
        let updated = rewrite_cd_prefix_into_workdir(params.clone());
        assert_eq!(updated, params);
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
pub async fn default_user_shell() -> Shell {
    Shell::Unknown
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
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
    async fn test_run_with_profile_zshrc_not_exists() {
        let shell = Shell::Zsh(ZshShell {
            shell_path: "/bin/zsh".to_string(),
            zshrc_path: "/does/not/exist/.zshrc".to_string(),
        });
        let actual_cmd = shell.format_default_shell_invocation(vec!["myecho".to_string()]);
        assert_eq!(
            actual_cmd,
            Some(vec![
                "/bin/zsh".to_string(),
                "-lc".to_string(),
                "myecho".to_string()
            ])
        );
    }

    #[tokio::test]
    async fn test_run_with_profile_bashrc_not_exists() {
        let shell = Shell::Bash(BashShell {
            shell_path: "/bin/bash".to_string(),
            bashrc_path: "/does/not/exist/.bashrc".to_string(),
        });
        let actual_cmd = shell.format_default_shell_invocation(vec!["myecho".to_string()]);
        assert_eq!(
            actual_cmd,
            Some(vec![
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "myecho".to_string()
            ])
        );
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
            let shell = Shell::Bash(BashShell {
                shell_path: shell_path.to_string(),
                bashrc_path: bashrc_path.to_str().unwrap().to_string(),
            });

            let actual_cmd = shell
                .format_default_shell_invocation(input.iter().map(|s| s.to_string()).collect());
            let expected_cmd = expected_cmd
                .iter()
                .map(|s| s.replace("BASHRC_PATH", bashrc_path.to_str().unwrap()))
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
            let shell = Shell::Zsh(ZshShell {
                shell_path: shell_path.to_string(),
                zshrc_path: zshrc_path.to_str().unwrap().to_string(),
            });

            let actual_cmd = shell
                .format_default_shell_invocation(input.iter().map(|s| s.to_string()).collect());
            let expected_cmd = expected_cmd
                .iter()
                .map(|s| s.replace("ZSHRC_PATH", zshrc_path.to_str().unwrap()))
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
