use shlex;

#[derive(Debug, PartialEq, Eq)]
pub struct ZshShell {
    shell_path: String,
    zshrc_path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Shell {
    Zsh(ZshShell),
    Unknown,
}

impl Shell {
    pub fn format_default_shell_invocation(&self, command: Vec<String>) -> Option<Vec<String>> {
        match self {
            Shell::Zsh(zsh) => {
                if !std::path::Path::new(&zsh.zshrc_path).exists() {
                    return None;
                }

                let mut result = vec![zsh.shell_path.clone(), "-c".to_string()];
                if let Ok(joined) = shlex::try_join(command.iter().map(|s| s.as_str())) {
                    result.push(format!("source {} && ({joined})", zsh.zshrc_path));
                } else {
                    return None;
                }
                Some(result)
            }
            Shell::Unknown => None,
        }
    }
}

#[cfg(target_os = "macos")]
pub async fn default_user_shell() -> Shell {
    use tokio::process::Command;
    use whoami;

    let user = whoami::username();
    let home = format!("/Users/{user}");
    let output = Command::new("dscl")
        .args([".", "-read", &home, "UserShell"])
        .output()
        .await
        .ok();
    match output {
        Some(o) => {
            if !o.status.success() {
                return Shell::Unknown;
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if let Some(shell_path) = line.strip_prefix("UserShell: ") {
                    if shell_path.ends_with("/zsh") {
                        return Shell::Zsh(ZshShell {
                            shell_path: shell_path.to_string(),
                            zshrc_path: format!("{home}/.zshrc"),
                        });
                    }
                }
            }

            Shell::Unknown
        }
        _ => Shell::Unknown,
    }
}

#[cfg(not(target_os = "macos"))]
pub async fn default_user_shell() -> Shell {
    Shell::Unknown
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::*;
    use std::process::Command;

    #[tokio::test]
    #[expect(clippy::unwrap_used)]
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
        assert_eq!(actual_cmd, None);
    }

    #[expect(clippy::unwrap_used)]
    #[tokio::test]
    async fn test_run_with_profile_escaping_and_execution() {
        let shell_path = "/bin/zsh";

        let cases = vec![
            (
                vec!["myecho"],
                vec![shell_path, "-c", "source ZSHRC_PATH && (myecho)"],
                Some("It works!\n"),
            ),
            (
                vec!["bash", "-lc", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-c",
                    "source ZSHRC_PATH && (bash -lc \"echo 'single' \\\"double\\\"\")",
                ],
                Some("single double\n"),
            ),
        ];
        for (input, expected_cmd, expected_output) in cases {
            use std::collections::HashMap;
            use std::path::PathBuf;
            use std::sync::Arc;

            use tokio::sync::Notify;

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
                },
                SandboxType::None,
                Arc::new(Notify::new()),
                &SandboxPolicy::DangerFullAccess,
                &None,
            )
            .await
            .unwrap();

            assert_eq!(output.exit_code, 0, "input: {input:?} output: {output:?}");
            if let Some(expected) = expected_output {
                assert_eq!(
                    output.stdout, expected,
                    "input: {input:?} output: {output:?}"
                );
            }
        }
    }
}
