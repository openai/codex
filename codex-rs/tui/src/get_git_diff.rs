//! Utility to compute the current Git diff for the working directory.
//!
//! The implementation mirrors the behaviour of the TypeScript version in
//! `codex-cli`: it returns the diff for tracked changes as well as any
//! untracked files. When the current directory is not inside a Git
//! repository, the function returns `Ok((false, String::new()))`.

use std::path::Path;
use std::time::Duration;

use crate::workspace_command::WorkspaceCommand;
use crate::workspace_command::WorkspaceCommandExecutor;
use crate::workspace_command::WorkspaceCommandOutput;

const DIFF_COMMAND_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);

/// Return value of [`get_git_diff`].
///
/// * `bool` – Whether the current working directory is inside a Git repo.
/// * `String` – The concatenated diff (may be empty).
pub(crate) async fn get_git_diff(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Result<(bool, String), String> {
    // First check if we are inside a Git repository.
    if !inside_git_repo(runner, cwd).await? {
        return Ok((false, String::new()));
    }

    // Run tracked diff and untracked file listing in parallel.
    let (tracked_diff_res, untracked_output_res) = tokio::join!(
        run_git_capture_diff(runner, cwd, &["diff", "--color"]),
        run_git_capture_stdout(runner, cwd, &["ls-files", "--others", "--exclude-standard"]),
    );
    let tracked_diff = tracked_diff_res?;
    let untracked_output = untracked_output_res?;

    let mut untracked_diff = String::new();
    let null_device: &Path = if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    };

    let null_path = null_device.to_str().unwrap_or("/dev/null");
    for file in untracked_output
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let args = ["diff", "--color", "--no-index", "--", null_path, file];
        let diff = run_git_capture_diff(runner, cwd, &args).await?;
        untracked_diff.push_str(&diff);
    }

    Ok((true, format!("{tracked_diff}{untracked_diff}")))
}

/// Helper that executes `git` with the given `args` and returns `stdout` as a
/// UTF-8 string. Any non-zero exit status is considered an *error*.
async fn run_git_capture_stdout(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    args: &[&str],
) -> Result<String, String> {
    let output = run_git_command(runner, cwd, args).await?;
    if output.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "git {:?} failed with status {}",
            args, output.exit_code
        ))
    }
}

/// Like [`run_git_capture_stdout`] but treats exit status 1 as success and
/// returns stdout. Git returns 1 for diffs when differences are present.
async fn run_git_capture_diff(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    args: &[&str],
) -> Result<String, String> {
    let output = run_git_command(runner, cwd, args).await?;
    if output.success() || output.exit_code == 1 {
        Ok(output.stdout)
    } else {
        Err(format!(
            "git {:?} failed with status {}",
            args, output.exit_code
        ))
    }
}

/// Determine if the current directory is inside a Git repository.
async fn inside_git_repo(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Result<bool, String> {
    let output = run_git_command(runner, cwd, &["rev-parse", "--is-inside-work-tree"]).await?;
    Ok(output.success())
}

async fn run_git_command(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    args: &[&str],
) -> Result<WorkspaceCommandOutput, String> {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push("git".to_string());
    argv.extend(args.iter().map(|arg| (*arg).to_string()));
    runner
        .run(
            WorkspaceCommand::new(argv)
                .cwd(cwd.to_path_buf())
                .timeout(DIFF_COMMAND_TIMEOUT)
                .disable_output_cap(),
        )
        .await
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_command::WorkspaceCommandError;
    use pretty_assertions::assert_eq;
    use std::collections::VecDeque;
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::sync::Mutex;

    #[tokio::test]
    async fn get_git_diff_returns_not_git_for_non_git_cwd() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![response(
            &["git", "rev-parse", "--is-inside-work-tree"],
            /*exit_code*/ 128,
            "",
        )]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((false, String::new())));
        assert_commands(
            &runner.commands(),
            &[&["git", "rev-parse", "--is-inside-work-tree"]],
            &cwd,
        );
    }

    #[tokio::test]
    async fn get_git_diff_concatenates_tracked_and_untracked_diffs() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                &["git", "rev-parse", "--is-inside-work-tree"],
                /*exit_code*/ 0,
                "true\n",
            ),
            response(
                &["git", "diff", "--color"],
                /*exit_code*/ 1,
                "tracked\n",
            ),
            response(
                &["git", "ls-files", "--others", "--exclude-standard"],
                /*exit_code*/ 0,
                "new.txt\n",
            ),
            response(
                &[
                    "git",
                    "diff",
                    "--color",
                    "--no-index",
                    "--",
                    null_device(),
                    "new.txt",
                ],
                /*exit_code*/ 1,
                "untracked\n",
            ),
        ]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((true, "tracked\nuntracked\n".to_string())));
        assert_commands(
            &runner.commands(),
            &[
                &["git", "rev-parse", "--is-inside-work-tree"],
                &["git", "diff", "--color"],
                &["git", "ls-files", "--others", "--exclude-standard"],
                &[
                    "git",
                    "diff",
                    "--color",
                    "--no-index",
                    "--",
                    null_device(),
                    "new.txt",
                ],
            ],
            &cwd,
        );
    }

    #[tokio::test]
    async fn get_git_diff_accepts_diff_exit_code_one() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                &["git", "rev-parse", "--is-inside-work-tree"],
                /*exit_code*/ 0,
                "true\n",
            ),
            response(
                &["git", "diff", "--color"],
                /*exit_code*/ 1,
                "tracked\n",
            ),
            response(
                &["git", "ls-files", "--others", "--exclude-standard"],
                /*exit_code*/ 0,
                "",
            ),
        ]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((true, "tracked\n".to_string())));
    }

    #[tokio::test]
    async fn get_git_diff_rejects_unexpected_git_diff_status() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                &["git", "rev-parse", "--is-inside-work-tree"],
                /*exit_code*/ 0,
                "true\n",
            ),
            response(&["git", "diff", "--color"], /*exit_code*/ 2, ""),
            response(
                &["git", "ls-files", "--others", "--exclude-standard"],
                /*exit_code*/ 0,
                "",
            ),
        ]);

        let error = get_git_diff(&runner, &cwd)
            .await
            .expect_err("unexpected git diff status should fail");

        assert!(
            error.contains("git [\"diff\", \"--color\"] failed with status 2"),
            "unexpected error: {error}",
        );
    }

    fn response(argv: &[&str], exit_code: i32, stdout: &str) -> FakeResponse {
        FakeResponse {
            argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
            output: WorkspaceCommandOutput {
                exit_code,
                stdout: stdout.to_string(),
                stderr: String::new(),
            },
        }
    }

    fn null_device() -> &'static str {
        if cfg!(windows) { "NUL" } else { "/dev/null" }
    }

    fn assert_commands(commands: &[WorkspaceCommand], expected: &[&[&str]], cwd: &Path) {
        let actual: Vec<Vec<String>> = commands
            .iter()
            .map(|command| command.argv.clone())
            .collect();
        let expected: Vec<Vec<String>> = expected
            .iter()
            .map(|argv| argv.iter().map(|arg| (*arg).to_string()).collect())
            .collect();
        assert_eq!(actual, expected);

        for command in commands {
            assert_eq!(command.cwd.as_deref(), Some(cwd));
            assert_eq!(command.timeout, DIFF_COMMAND_TIMEOUT);
            assert!(command.disable_output_cap);
        }
    }

    struct FakeResponse {
        argv: Vec<String>,
        output: WorkspaceCommandOutput,
    }

    struct FakeRunner {
        responses: Mutex<VecDeque<FakeResponse>>,
        commands: Mutex<Vec<WorkspaceCommand>>,
    }

    impl FakeRunner {
        fn new(responses: Vec<FakeResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                commands: Mutex::new(Vec::new()),
            }
        }

        fn commands(&self) -> Vec<WorkspaceCommand> {
            self.commands.lock().expect("commands lock").clone()
        }
    }

    impl WorkspaceCommandExecutor for FakeRunner {
        fn run(
            &self,
            command: WorkspaceCommand,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<WorkspaceCommandOutput, WorkspaceCommandError>>
                    + Send
                    + '_,
            >,
        > {
            Box::pin(async move {
                let mut responses = self.responses.lock().expect("responses lock");
                let response = responses.pop_front().expect("missing fake response");
                assert_eq!(command.argv, response.argv);
                self.commands.lock().expect("commands lock").push(command);
                Ok(response.output)
            })
        }
    }
}
