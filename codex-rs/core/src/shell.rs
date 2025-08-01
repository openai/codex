use std::collections::HashMap;

use shlex;
use tokio::process::Command;

use crate::exec::ExecParams;

#[derive(Debug, PartialEq, Eq)]
pub struct ZshShell {
    shell_path: String,
    env: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Shell {
    Zsh(ZshShell),
    Unknown,
}

impl Shell {
    pub fn format_default_shell_invocation(&self, params: ExecParams) -> Option<ExecParams> {
        match self {
            Shell::Zsh(zsh) => {
                let mut result = vec![zsh.shell_path.clone(), "-c".to_string()];
                let command = params.command;

                let joined = strip_bash_lc(&command)
                    .or_else(|| shlex::try_join(command.iter().map(|s| s.as_str())).ok());

                if let Some(joined) = joined {
                    result.push(format!("({joined})"));
                } else {
                    return None;
                }

                Some(ExecParams {
                    command: result,
                    env: {
                        let mut env = params.env.clone();
                        env.extend(zsh.env.clone());
                        env
                    },
                    ..params
                })
            }
            Shell::Unknown => None,
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

#[cfg(target_os = "macos")]
pub async fn default_user_shell() -> Shell {
    use tokio::process::Command;
    use tracing::warn;
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
                        let zshrc_path = format!("{home}/.zshrc");
                        let mut collect_env_args = vec!["-lc".to_string()];

                        if std::path::Path::new(&zshrc_path).exists() {
                            collect_env_args
                                .push(format!("source {zshrc_path} >/dev/null 2>&1; printenv"));
                        } else {
                            collect_env_args.push("printenv".to_string());
                        }

                        let env = match collect_env(shell_path, collect_env_args).await {
                            Ok(env) => env,
                            Err(e) => {
                                warn!("Failed to collect env: {e}");
                                HashMap::new()
                            }
                        };

                        return Shell::Zsh(ZshShell {
                            shell_path: shell_path.to_string(),
                            env,
                        });
                    }
                }
            }

            Shell::Unknown
        }
        _ => Shell::Unknown,
    }
}

async fn collect_env(
    command: &str,
    args: Vec<String>,
) -> Result<HashMap<String, String>, std::io::Error> {
    let output = Command::new(command)
        .args(args)
        .env_clear()
        .output()
        .await?;
    let mut env = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            env.insert(parts[0].to_string(), parts[1].to_string());
        }
    }
    Ok(env)
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

        let shell_path = String::from_utf8_lossy(&shell.stdout).trim().to_string();
        if shell_path.ends_with("/zsh") {
            let shell = default_user_shell().await;

            if let Shell::Zsh(ZshShell {
                shell_path: actual_shell_path,
                env,
            }) = shell
            {
                assert_eq!(actual_shell_path, shell_path);
                assert!(env.contains_key("PATH"));
                assert!(env.contains_key("HOME"));
            } else {
                panic!("Expected Zsh shell, got {shell:?}");
            }
        }
    }

    #[expect(clippy::unwrap_used)]
    #[tokio::test]
    async fn test_run_with_profile_escaping_and_execution() {
        let shell_path = "/bin/zsh";

        let cases = vec![
            (
                vec!["bash", "-lc", "echo $MY_VAR"],
                vec![shell_path, "-c", "(echo $MY_VAR)"],
                Some("123\n"),
            ),
            (
                vec!["bash", "-c", "echo 'single' \"double\""],
                vec![
                    shell_path,
                    "-c",
                    "(bash -c \"echo 'single' \\\"double\\\"\")",
                ],
                Some("single double\n"),
            ),
            (
                vec!["bash", "-lc", "echo 'single' \"double\""],
                vec![shell_path, "-c", "(echo 'single' \"double\")"],
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

            let shell = Shell::Zsh(ZshShell {
                shell_path: shell_path.to_string(),
                env: HashMap::from([("MY_VAR".to_string(), "123".to_string())]),
            });

            let actual_cmd = shell
                .format_default_shell_invocation(ExecParams {
                    command: input.iter().map(|s| s.to_string()).collect(),
                    cwd: PathBuf::from("/"),
                    timeout_ms: None,
                    env: HashMap::from([("MY_OTHER_VAR".to_string(), "456".to_string())]),
                })
                .unwrap();

            let expected_cmd = expected_cmd.clone();

            assert_eq!(actual_cmd.command, expected_cmd);
            assert_eq!(
                actual_cmd.env,
                HashMap::from([
                    ("MY_VAR".to_string(), "123".to_string()),
                    ("MY_OTHER_VAR".to_string(), "456".to_string()),
                ])
            );
            // Actually run the command and check output/exit code
            let output = process_exec_tool_call(
                actual_cmd,
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
