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
use codex_git_utils::FsmonitorOverride;
use codex_git_utils::FsmonitorProbeRunner;
use codex_git_utils::detect_fsmonitor_override;

const DIFF_COMMAND_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const DISABLE_HOOKS_CONFIG: &str = if cfg!(windows) {
    "core.hooksPath=NUL"
} else {
    "core.hooksPath=/dev/null"
};
const EXECUTABLE_FILTER_CONFIG_PATTERN: &str = r"^filter\..*\.(clean|process)$";

// `/diff` may execute Git through a remote workspace, so git-utils owns the
// probe policy while this adapter keeps command execution in the TUI layer.
// WorkspaceCommand bounds each call; `/diff` has no aggregate command deadline.
struct WorkspaceFsmonitorProbeRunner<'a> {
    runner: &'a dyn WorkspaceCommandExecutor,
    cwd: &'a Path,
}

impl FsmonitorProbeRunner for WorkspaceFsmonitorProbeRunner<'_> {
    async fn run_probe(&mut self, args: &[&str]) -> Option<Vec<u8>> {
        let argv = ["git"].into_iter().chain(args.iter().copied());
        let command = WorkspaceCommand::new(argv).cwd(self.cwd.to_path_buf());
        match self.runner.run(command).await {
            Ok(output) if output.success() => Some(output.stdout.into_bytes()),
            _ => None,
        }
    }
}

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

    // Probe once per `/diff` and reuse the result for all subsequent Git commands.
    let mut probe_runner = WorkspaceFsmonitorProbeRunner { runner, cwd };
    let fsmonitor = detect_fsmonitor_override(&mut probe_runner).await;

    // Keep `/diff` informational: repository configuration must not select executable diff helpers.
    let diff_config_overrides = diff_filter_config_overrides(runner, cwd, fsmonitor).await?;
    let (tracked_diff_res, untracked_output_res) = tokio::join!(
        run_git_capture_diff(
            runner,
            cwd,
            fsmonitor,
            &diff_config_overrides,
            &[
                "diff",
                "--no-textconv",
                "--no-ext-diff",
                "--submodule=short",
                "--ignore-submodules=dirty",
                "--color",
            ]
        ),
        run_git_capture_stdout(
            runner,
            cwd,
            fsmonitor,
            &["ls-files", "--others", "--exclude-standard"]
        ),
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
        let args = [
            "diff",
            "--no-textconv",
            "--no-ext-diff",
            "--submodule=short",
            "--ignore-submodules=dirty",
            "--color",
            "--no-index",
            "--",
            null_path,
            file,
        ];
        let diff =
            run_git_capture_diff(runner, cwd, fsmonitor, &diff_config_overrides, &args).await?;
        untracked_diff.push_str(&diff);
    }

    Ok((true, format!("{tracked_diff}{untracked_diff}")))
}

/// Helper that executes `git` with the given `args` and returns `stdout` as a
/// UTF-8 string. Any non-zero exit status is considered an *error*.
async fn run_git_capture_stdout(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    fsmonitor: FsmonitorOverride,
    args: &[&str],
) -> Result<String, String> {
    let output = run_git_command(runner, cwd, fsmonitor, &[], args).await?;
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
    fsmonitor: FsmonitorOverride,
    config_overrides: &[(String, String)],
    args: &[&str],
) -> Result<String, String> {
    let output = run_git_command(runner, cwd, fsmonitor, config_overrides, args).await?;
    if output.success() || output.exit_code == 1 {
        Ok(output.stdout)
    } else {
        Err(format!(
            "git {:?} failed with status {}",
            args, output.exit_code
        ))
    }
}

/// Return Git configuration overrides that prevent configured filter drivers
/// from executing while generating diffs.
async fn diff_filter_config_overrides(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    fsmonitor: FsmonitorOverride,
) -> Result<Vec<(String, String)>, String> {
    let args = [
        "config",
        "--null",
        "--name-only",
        "--get-regexp",
        EXECUTABLE_FILTER_CONFIG_PATTERN,
    ];
    let output = run_git_command(runner, cwd, fsmonitor, &[], &args).await?;
    if output.exit_code != 0 && output.exit_code != 1 {
        return Err(format!(
            "git {:?} failed with status {}",
            args, output.exit_code
        ));
    }

    let mut drivers = output
        .stdout
        .split('\0')
        .filter_map(|key| {
            key.strip_suffix(".clean")
                .or_else(|| key.strip_suffix(".process"))
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    drivers.sort();
    drivers.dedup();

    Ok(drivers
        .into_iter()
        .flat_map(|driver| {
            [
                (format!("{driver}.clean"), String::new()),
                (format!("{driver}.process"), String::new()),
                (format!("{driver}.required"), "false".to_string()),
            ]
        })
        .collect())
}

/// Determine if the current directory is inside a Git repository.
async fn inside_git_repo(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Result<bool, String> {
    // `rev-parse` does not inspect the worktree, and probing before this check
    // would also run extra Git commands outside repositories.
    let output = run_git_command(
        runner,
        cwd,
        FsmonitorOverride::Disabled,
        &[],
        &["rev-parse", "--is-inside-work-tree"],
    )
    .await?;
    Ok(output.success())
}

async fn run_git_command(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    fsmonitor: FsmonitorOverride,
    config_overrides: &[(String, String)],
    args: &[&str],
) -> Result<WorkspaceCommandOutput, String> {
    let argv = [
        "git",
        "-c",
        fsmonitor.git_config_arg(),
        "-c",
        DISABLE_HOOKS_CONFIG,
    ]
    .into_iter()
    .chain(args.iter().copied());
    let mut command = WorkspaceCommand::new(argv)
        .cwd(cwd.to_path_buf())
        .timeout(DIFF_COMMAND_TIMEOUT)
        .disable_output_cap();
    if !config_overrides.is_empty() {
        command = command.env("GIT_CONFIG_COUNT", config_overrides.len().to_string());
        for (index, (key, value)) in config_overrides.iter().enumerate() {
            command = command
                .env(format!("GIT_CONFIG_KEY_{index}"), key)
                .env(format!("GIT_CONFIG_VALUE_{index}"), value);
        }
    }
    runner.run(command).await.map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_command::WorkspaceCommandError;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::collections::VecDeque;
    #[cfg(unix)]
    use std::fs;
    use std::future::Future;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::pin::Pin;
    #[cfg(unix)]
    use std::process::Command as ProcessCommand;
    use std::sync::Mutex;

    #[tokio::test]
    async fn get_git_diff_returns_not_git_for_non_git_cwd() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![response(
            git_command(
                FsmonitorOverride::Disabled,
                &["rev-parse", "--is-inside-work-tree"],
            ),
            /*exit_code*/ 128,
            "",
        )]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((false, String::new())));
        assert_command_metadata(&runner.commands(), &cwd);
    }

    #[tokio::test]
    async fn get_git_diff_disables_helpers_for_tracked_and_untracked_diffs() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["rev-parse", "--is-inside-work-tree"],
                ),
                /*exit_code*/ 0,
                "true\n",
            ),
            response(
                git_probe_command(&["config", "--null", "--get", "core.fsmonitor"]),
                /*exit_code*/ 0,
                "/tmp/fsmonitor-helper\0",
            ),
            response(
                git_probe_command(&[
                    "config",
                    "--null",
                    "--type=bool",
                    "--fixed-value",
                    "--get",
                    "core.fsmonitor",
                    "/tmp/fsmonitor-helper",
                ]),
                /*exit_code*/ 128,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "config",
                        "--null",
                        "--name-only",
                        "--get-regexp",
                        EXECUTABLE_FILTER_CONFIG_PATTERN,
                    ],
                ),
                /*exit_code*/ 0,
                "filter.evil.clean\0filter.evil.process\0",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "diff",
                        "--no-textconv",
                        "--no-ext-diff",
                        "--submodule=short",
                        "--ignore-submodules=dirty",
                        "--color",
                    ],
                ),
                /*exit_code*/ 1,
                "tracked\n",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["ls-files", "--others", "--exclude-standard"],
                ),
                /*exit_code*/ 0,
                "new.txt\n",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "diff",
                        "--no-textconv",
                        "--no-ext-diff",
                        "--submodule=short",
                        "--ignore-submodules=dirty",
                        "--color",
                        "--no-index",
                        "--",
                        null_device(),
                        "new.txt",
                    ],
                ),
                /*exit_code*/ 1,
                "untracked\n",
            ),
        ]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((true, "tracked\nuntracked\n".to_string())));
        let commands = runner.commands();
        assert_command_metadata(&commands, &cwd);
        assert_eq!(commands[4].env, filter_override_env("filter.evil"));
        assert_eq!(commands[6].env, filter_override_env("filter.evil"));
    }

    #[tokio::test]
    async fn get_git_diff_preserves_builtin_fsmonitor_for_diff_workflow() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["rev-parse", "--is-inside-work-tree"],
                ),
                /*exit_code*/ 0,
                "true\n",
            ),
            response(
                git_probe_command(&["config", "--null", "--get", "core.fsmonitor"]),
                /*exit_code*/ 0,
                "true\0",
            ),
            response(
                git_probe_command(&["version", "--build-options"]),
                /*exit_code*/ 0,
                "feature: fsmonitor--daemon\n",
            ),
            response(
                git_command(
                    FsmonitorOverride::BuiltIn,
                    &[
                        "config",
                        "--null",
                        "--name-only",
                        "--get-regexp",
                        EXECUTABLE_FILTER_CONFIG_PATTERN,
                    ],
                ),
                /*exit_code*/ 1,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::BuiltIn,
                    &[
                        "diff",
                        "--no-textconv",
                        "--no-ext-diff",
                        "--submodule=short",
                        "--ignore-submodules=dirty",
                        "--color",
                    ],
                ),
                /*exit_code*/ 1,
                "tracked\n",
            ),
            response(
                git_command(
                    FsmonitorOverride::BuiltIn,
                    &["ls-files", "--others", "--exclude-standard"],
                ),
                /*exit_code*/ 0,
                "new.txt\n",
            ),
            response(
                git_command(
                    FsmonitorOverride::BuiltIn,
                    &[
                        "diff",
                        "--no-textconv",
                        "--no-ext-diff",
                        "--submodule=short",
                        "--ignore-submodules=dirty",
                        "--color",
                        "--no-index",
                        "--",
                        null_device(),
                        "new.txt",
                    ],
                ),
                /*exit_code*/ 1,
                "untracked\n",
            ),
        ]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((true, "tracked\nuntracked\n".to_string())));
        assert_command_metadata(&runner.commands(), &cwd);
    }

    #[tokio::test]
    async fn get_git_diff_accepts_diff_exit_code_one() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["rev-parse", "--is-inside-work-tree"],
                ),
                /*exit_code*/ 0,
                "true\n",
            ),
            response(
                git_probe_command(&["config", "--null", "--get", "core.fsmonitor"]),
                /*exit_code*/ 1,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "config",
                        "--null",
                        "--name-only",
                        "--get-regexp",
                        EXECUTABLE_FILTER_CONFIG_PATTERN,
                    ],
                ),
                /*exit_code*/ 1,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "diff",
                        "--no-textconv",
                        "--no-ext-diff",
                        "--submodule=short",
                        "--ignore-submodules=dirty",
                        "--color",
                    ],
                ),
                /*exit_code*/ 1,
                "tracked\n",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["ls-files", "--others", "--exclude-standard"],
                ),
                /*exit_code*/ 0,
                "",
            ),
        ]);

        let result = get_git_diff(&runner, &cwd).await;

        assert_eq!(result, Ok((true, "tracked\n".to_string())));
        assert_command_metadata(&runner.commands(), &cwd);
    }

    #[tokio::test]
    async fn get_git_diff_rejects_unexpected_git_diff_status() {
        let cwd = PathBuf::from("/workspace");
        let runner = FakeRunner::new(vec![
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["rev-parse", "--is-inside-work-tree"],
                ),
                /*exit_code*/ 0,
                "true\n",
            ),
            response(
                git_probe_command(&["config", "--null", "--get", "core.fsmonitor"]),
                /*exit_code*/ 1,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "config",
                        "--null",
                        "--name-only",
                        "--get-regexp",
                        EXECUTABLE_FILTER_CONFIG_PATTERN,
                    ],
                ),
                /*exit_code*/ 1,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &[
                        "diff",
                        "--no-textconv",
                        "--no-ext-diff",
                        "--submodule=short",
                        "--ignore-submodules=dirty",
                        "--color",
                    ],
                ),
                /*exit_code*/ 2,
                "",
            ),
            response(
                git_command(
                    FsmonitorOverride::Disabled,
                    &["ls-files", "--others", "--exclude-standard"],
                ),
                /*exit_code*/ 0,
                "",
            ),
        ]);

        let error = get_git_diff(&runner, &cwd)
            .await
            .expect_err("unexpected git diff status should fail");

        assert_eq!(
            error,
            "git [\"diff\", \"--no-textconv\", \"--no-ext-diff\", \"--submodule=short\", \"--ignore-submodules=dirty\", \"--color\"] failed with status 2"
        );
        assert_command_metadata(&runner.commands(), &cwd);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn get_git_diff_does_not_execute_configured_filters_fsmonitor_or_hooks() {
        let tempdir = tempfile::tempdir().expect("create temp directory");
        let repo = tempdir.path().join("repo");
        fs::create_dir(&repo).expect("create test repository directory");
        run_git_setup(&repo, &["init", "-q"]);
        run_git_setup(&repo, &["config", "user.name", "test"]);
        run_git_setup(&repo, &["config", "user.email", "test@example.com"]);
        fs::write(repo.join(".gitattributes"), "*.txt filter=x=y\n").expect("write attributes");
        fs::write(repo.join("tracked.txt"), "before\n").expect("write tracked file");
        fs::write(repo.join("unchanged.txt"), "unchanged\n").expect("write unchanged file");
        run_git_setup(
            &repo,
            &["add", ".gitattributes", "tracked.txt", "unchanged.txt"],
        );
        run_git_setup(&repo, &["commit", "-qm", "initial"]);

        let filter_helper = tempdir.path().join("filter-helper.sh");
        let fsmonitor_helper = tempdir.path().join("fsmonitor-helper.sh");
        let hooks_dir = tempdir.path().join("hooks");
        let hook_helper = hooks_dir.join("post-index-change");
        fs::create_dir(&hooks_dir).expect("create hooks directory");
        write_marker_helper(&filter_helper);
        write_marker_helper(&fsmonitor_helper);
        write_marker_helper(&hook_helper);
        run_git_setup(
            &repo,
            &[
                "config",
                "filter.x=y.clean",
                filter_helper.to_str().expect("filter helper path"),
            ],
        );
        run_git_setup(
            &repo,
            &[
                "config",
                "filter.x=y.process",
                filter_helper.to_str().expect("filter helper path"),
            ],
        );
        run_git_setup(&repo, &["config", "filter.x=y.required", "true"]);
        run_git_setup(
            &repo,
            &[
                "config",
                "core.fsmonitor",
                fsmonitor_helper.to_str().expect("fsmonitor helper path"),
            ],
        );
        run_git_setup(
            &repo,
            &[
                "config",
                "core.hooksPath",
                hooks_dir.to_str().expect("hooks directory path"),
            ],
        );
        std::thread::sleep(Duration::from_secs(/*secs*/ 1));
        fs::write(repo.join("unchanged.txt"), "unchanged\n").expect("refresh unchanged file");
        fs::write(repo.join("tracked.txt"), "after\n").expect("modify tracked file");

        let result = get_git_diff(&LocalRunner, &repo)
            .await
            .expect("generate diff without invoking helpers");

        assert_eq!(
            (
                result.1.contains("before"),
                result.1.contains("after"),
                filter_helper.with_extension("sh.ran").exists(),
                fsmonitor_helper.with_extension("sh.ran").exists(),
                hook_helper.with_extension("sh.ran").exists(),
            ),
            (true, true, false, false, false),
            "diff:\n{}",
            result.1
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn get_git_diff_does_not_execute_helpers_while_checking_dirty_submodules() {
        let tempdir = tempfile::tempdir().expect("create temp directory");
        let child = tempdir.path().join("child");
        let repo = tempdir.path().join("repo");
        fs::create_dir(&child).expect("create child repository directory");
        fs::create_dir(&repo).expect("create parent repository directory");
        run_git_setup(&child, &["init", "-q"]);
        run_git_setup(&child, &["config", "user.name", "test"]);
        run_git_setup(&child, &["config", "user.email", "test@example.com"]);
        fs::write(child.join(".gitattributes"), "*.txt filter=evil\n")
            .expect("write child attributes");
        fs::write(child.join("tracked.txt"), "before\n").expect("write child tracked file");
        run_git_setup(&child, &["add", ".gitattributes", "tracked.txt"]);
        run_git_setup(&child, &["commit", "-qm", "initial"]);

        run_git_setup(&repo, &["init", "-q"]);
        run_git_setup(&repo, &["config", "user.name", "test"]);
        run_git_setup(&repo, &["config", "user.email", "test@example.com"]);
        run_git_setup(
            &repo,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                child.to_str().expect("child repository path"),
                "child",
            ],
        );
        run_git_setup(&repo, &["commit", "-qm", "add submodule"]);

        let helper = tempdir.path().join("submodule-helper.sh");
        write_marker_helper(&helper);
        let checkout = repo.join("child");
        run_git_setup(
            &checkout,
            &[
                "config",
                "filter.evil.clean",
                helper.to_str().expect("submodule helper path"),
            ],
        );
        run_git_setup(&checkout, &["config", "filter.evil.required", "true"]);
        std::thread::sleep(Duration::from_secs(/*secs*/ 1));
        fs::write(checkout.join("tracked.txt"), "before\n").expect("refresh child tracked file");

        let result = get_git_diff(&LocalRunner, &repo)
            .await
            .expect("generate diff without inspecting submodule worktrees");

        assert_eq!(
            (result.1, helper.with_extension("sh.ran").exists()),
            (String::new(), false)
        );
    }

    fn git_command(fsmonitor: FsmonitorOverride, args: &[&str]) -> Vec<String> {
        [
            "git",
            "-c",
            fsmonitor.git_config_arg(),
            "-c",
            DISABLE_HOOKS_CONFIG,
        ]
        .into_iter()
        .chain(args.iter().copied())
        .map(str::to_string)
        .collect()
    }

    fn git_probe_command(args: &[&str]) -> Vec<String> {
        ["git"]
            .into_iter()
            .chain(args.iter().copied())
            .map(str::to_string)
            .collect()
    }

    fn filter_override_env(driver: &str) -> HashMap<String, Option<String>> {
        HashMap::from([
            ("GIT_CONFIG_COUNT".to_string(), Some("3".to_string())),
            (
                "GIT_CONFIG_KEY_0".to_string(),
                Some(format!("{driver}.clean")),
            ),
            ("GIT_CONFIG_VALUE_0".to_string(), Some(String::new())),
            (
                "GIT_CONFIG_KEY_1".to_string(),
                Some(format!("{driver}.process")),
            ),
            ("GIT_CONFIG_VALUE_1".to_string(), Some(String::new())),
            (
                "GIT_CONFIG_KEY_2".to_string(),
                Some(format!("{driver}.required")),
            ),
            ("GIT_CONFIG_VALUE_2".to_string(), Some("false".to_string())),
        ])
    }

    fn response(argv: Vec<String>, exit_code: i32, stdout: &str) -> FakeResponse {
        FakeResponse {
            argv,
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

    #[cfg(unix)]
    fn run_git_setup(cwd: &Path, args: &[&str]) {
        let output = ProcessCommand::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run git setup command");
        assert_eq!(
            output.status.code(),
            Some(0),
            "git setup command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn write_marker_helper(path: &Path) {
        fs::write(path, "#!/bin/sh\nprintf ran >> \"$0.ran\"\nexit 1\n")
            .expect("write helper script");
        let mut permissions = fs::metadata(path)
            .expect("read helper metadata")
            .permissions();
        permissions.set_mode(/*mode*/ 0o755);
        fs::set_permissions(path, permissions).expect("make helper executable");
    }

    fn assert_command_metadata(commands: &[WorkspaceCommand], cwd: &Path) {
        for command in commands {
            assert_eq!(command.cwd.as_deref(), Some(cwd));
            if matches!(
                command.argv.get(1).map(String::as_str),
                Some("config" | "version")
            ) {
                assert_eq!(command.env, HashMap::new());
                assert_eq!(command.timeout, Duration::from_secs(/*secs*/ 5));
                assert_eq!(command.output_bytes_cap, 64 * 1024);
                assert_eq!(command.disable_output_cap, false);
            } else {
                assert_eq!(command.timeout, DIFF_COMMAND_TIMEOUT);
                assert_eq!(command.disable_output_cap, true);
            }
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
            assert_eq!(
                self.responses.lock().expect("responses lock").len(),
                0,
                "unused fake responses"
            );
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

    #[cfg(unix)]
    struct LocalRunner;

    #[cfg(unix)]
    impl WorkspaceCommandExecutor for LocalRunner {
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
                let mut process = ProcessCommand::new(&command.argv[0]);
                process
                    .args(&command.argv[1..])
                    .current_dir(command.cwd.expect("test command cwd"));
                for (key, value) in command.env {
                    match value {
                        Some(value) => {
                            process.env(key, value);
                        }
                        None => {
                            process.env_remove(key);
                        }
                    }
                }
                let output = process.output().expect("run test command");
                Ok(WorkspaceCommandOutput {
                    exit_code: output.status.code().expect("test command exit code"),
                    stdout: String::from_utf8(output.stdout).expect("utf8 stdout"),
                    stderr: String::from_utf8(output.stderr).expect("utf8 stderr"),
                })
            })
        }
    }
}
