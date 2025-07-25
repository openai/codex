use shlex;
use std::process::Command;
use whoami;

#[derive(Debug, PartialEq, Eq)]
pub enum Shell {
    Zsh(String),
    Unknown,
}

impl Shell {
    pub fn run_with_profile(&self, command: Vec<String>) -> Option<Vec<String>> {
        match self {
            Shell::Zsh(shell_path) => {
                let mut result = vec![shell_path.clone(), "-c".to_string()];
                if let Ok(joined) = shlex::try_join(command.iter().map(|s| s.as_str())) {
                    result.push(format!("source ~/.zshrc && ({joined})"));
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
pub fn current_shell() -> Option<Shell> {
    let user = whoami::username();
    let output = Command::new("dscl")
        .args([".", "-read", &format!("/Users/{user}"), "UserShell"])
        .output()
        .ok()?;
    if !output.status.success() {
        return Some(Shell::Unknown);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(shell_path) = line.strip_prefix("UserShell: ") {
            if shell_path.ends_with("/zsh") {
                return Some(Shell::Zsh(shell_path.to_string()));
            } else {
                return Some(Shell::Unknown);
            }
        }
    }
    Some(Shell::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    #[cfg(target_os = "macos")]
    #[expect(clippy::unwrap_used)]
    fn test_current_shell_detects_zsh() {
        let output = Command::new("sh")
            .arg("-c")
            .arg("echo $SHELL")
            .output()
            .unwrap();
        let shell_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if shell_path.ends_with("/zsh") {
            assert_eq!(current_shell(), Some(Shell::Zsh(shell_path)));
        }
    }

    #[cfg(target_os = "macos")]
    #[expect(clippy::unwrap_used)]
    #[tokio::test]
    async fn test_run_with_profile_escaping_and_execution() {
        let shell_path = "/bin/zsh";
        let shell = Shell::Zsh(shell_path.to_string());
        let cases = vec![(
            vec!["bash", "-lc", "echo 'single' \"double\""],
            vec![
                shell_path,
                "-c",
                "source ~/.zshrc && (bash -lc \"echo 'single' \\\"double\\\"\")",
            ],
            Some("single double\n"),
        )];
        for (input, expected_cmd, expected_output) in cases {
            use std::collections::HashMap;
            use std::path::PathBuf;
            use std::sync::Arc;

            use tokio::sync::Notify;

            use crate::exec::ExecParams;
            use crate::exec::SandboxType;
            use crate::exec::process_exec_tool_call;
            use crate::protocol::SandboxPolicy;

            let actual_cmd = shell.run_with_profile(input.iter().map(|s| s.to_string()).collect());
            assert_eq!(
                actual_cmd,
                Some(expected_cmd.iter().map(|s| s.to_string()).collect())
            );
            // Actually run the command and check output/exit code
            let output = process_exec_tool_call(
                ExecParams {
                    command: actual_cmd.unwrap(),
                    cwd: PathBuf::from("/"),
                    timeout_ms: None,
                    env: HashMap::new(),
                },
                SandboxType::None,
                Arc::new(Notify::new()),
                &SandboxPolicy::DangerFullAccess,
                &None,
            )
            .await
            .unwrap();

            assert_eq!(output.exit_code, 0, "input: {input:?}");
            if let Some(expected) = expected_output {
                assert_eq!(output.stdout, expected, "input: {input:?}");
            }
        }
    }
}
