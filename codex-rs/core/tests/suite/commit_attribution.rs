use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use core_test_support::assert_regex_match;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_windows;
use core_test_support::test_codex_exec::TestCodexExecBuilder;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::json;

const CALL_ID: &str = "commit-attribution-shell-command";
const COMMIT_MESSAGE: &str = "commit attribution test";

fn git_test_env() -> [(&'static str, &'static str); 2] {
    [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
    ]
}

fn run_git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .envs(git_test_env())
        .args(args)
        .current_dir(repo)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn init_repo(repo: &Path) -> Result<()> {
    run_git(repo, &["init"])?;
    run_git(repo, &["config", "user.name", "Commit Attribution Test"])?;
    run_git(
        repo,
        &["config", "user.email", "commit-attribution@example.com"],
    )?;
    Ok(())
}

fn write_config(home: &Path, commit_attribution_line: Option<&str>) -> Result<()> {
    let mut config = String::new();
    if let Some(line) = commit_attribution_line {
        config.push_str(line);
        config.push('\n');
        config.push('\n');
    }
    config.push_str("[features]\ncodex_git_commit = true\n");
    std::fs::write(home.join("config.toml"), config).context("write config.toml")
}

async fn mount_commit_turn(server: &wiremock::MockServer, command: &str) -> Result<ResponseMock> {
    let arguments = serde_json::to_string(&json!({
        "command": command,
        "login": false,
        "timeout_ms": 10_000,
    }))?;
    Ok(mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(CALL_ID, "shell_command", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await)
}

fn run_exec(
    builder: &TestCodexExecBuilder,
    server: &wiremock::MockServer,
) -> Result<std::process::Output> {
    let mut cmd = builder.cmd_with_server(server);
    cmd.timeout(Duration::from_secs(30));
    cmd.envs(git_test_env())
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--skip-git-repo-check")
        .arg("make the requested commit");
    cmd.output().context("run codex-exec")
}

fn latest_commit_message(repo: &Path) -> Result<String> {
    run_git(repo, &["log", "-1", "--format=%B"])
}

fn assert_shell_command_succeeded(mock: &ResponseMock) {
    let Some(output) = mock.function_call_output_text(CALL_ID) else {
        panic!("shell_command output should be recorded");
    };
    assert_regex_match(
        r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\n.*$",
        &output.replace("\r\n", "\n"),
    );
}

fn codex_hooks_dir(builder: &TestCodexExecBuilder) -> std::path::PathBuf {
    builder.home_path().join("hooks").join("commit-attribution")
}

fn assert_trailer_once(message: &str, expected: &str) {
    assert!(
        message.trim_end().ends_with(expected),
        "expected commit message to end with {expected:?}, got: {message:?}"
    );
    assert_eq!(message.matches(expected).count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_cli_commit_attribution_defaults_when_unset() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let server = start_mock_server().await;
    let builder = test_codex_exec();
    init_repo(builder.cwd_path())?;
    write_config(builder.home_path(), None)?;
    let mock = mount_commit_turn(
        &server,
        &format!("git commit --allow-empty -m '{COMMIT_MESSAGE}'"),
    )
    .await?;

    let output = run_exec(&builder, &server)?;
    assert!(
        output.status.success(),
        "codex-exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_shell_command_succeeded(&mock);

    let message = latest_commit_message(builder.cwd_path())?;
    assert_trailer_once(&message, "Co-authored-by: Codex <noreply@openai.com>");
    assert!(
        codex_hooks_dir(&builder)
            .join("prepare-commit-msg")
            .exists()
    );
    assert!(codex_hooks_dir(&builder).join("commit-msg").exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_cli_commit_attribution_uses_configured_value() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let server = start_mock_server().await;
    let builder = test_codex_exec();
    init_repo(builder.cwd_path())?;
    write_config(
        builder.home_path(),
        Some(r#"commit_attribution = "AgentX <agent@example.com>""#),
    )?;
    let mock = mount_commit_turn(
        &server,
        &format!("git commit --allow-empty -m '{COMMIT_MESSAGE}'"),
    )
    .await?;

    let output = run_exec(&builder, &server)?;
    assert!(
        output.status.success(),
        "codex-exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_shell_command_succeeded(&mock);

    let message = latest_commit_message(builder.cwd_path())?;
    assert_trailer_once(&message, "Co-authored-by: AgentX <agent@example.com>");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_cli_commit_attribution_can_be_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let server = start_mock_server().await;
    let builder = test_codex_exec();
    init_repo(builder.cwd_path())?;
    write_config(builder.home_path(), Some(r#"commit_attribution = """#))?;
    let mock = mount_commit_turn(
        &server,
        &format!("git commit --allow-empty -m '{COMMIT_MESSAGE}'"),
    )
    .await?;

    let output = run_exec(&builder, &server)?;
    assert!(
        output.status.success(),
        "codex-exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_shell_command_succeeded(&mock);

    let message = latest_commit_message(builder.cwd_path())?;
    assert!(!message.contains("Co-authored-by:"));
    assert!(!codex_hooks_dir(&builder).exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_cli_commit_attribution_ignores_malformed_config() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let server = start_mock_server().await;
    let builder = test_codex_exec();
    init_repo(builder.cwd_path())?;
    write_config(builder.home_path(), Some("commit_attribution = true"))?;
    let mock = mount_commit_turn(
        &server,
        &format!("git commit --allow-empty -m '{COMMIT_MESSAGE}'"),
    )
    .await?;

    let output = run_exec(&builder, &server)?;
    assert!(
        output.status.success(),
        "codex-exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_shell_command_succeeded(&mock);

    let message = latest_commit_message(builder.cwd_path())?;
    assert_trailer_once(&message, "Co-authored-by: Codex <noreply@openai.com>");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_cli_commit_attribution_preserves_user_commit_hooks() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let server = start_mock_server().await;
    let builder = test_codex_exec();
    init_repo(builder.cwd_path())?;
    write_config(builder.home_path(), None)?;

    let user_hooks = builder.cwd_path().join("user-hooks");
    std::fs::create_dir_all(&user_hooks).context("create user hooks dir")?;
    std::fs::write(
        user_hooks.join("pre-commit"),
        "#!/usr/bin/env bash\nset -euo pipefail\nroot=\"$(git rev-parse --show-toplevel)\"\nprintf pre-commit > \"$root/pre-commit.marker\"\n",
    )?;
    std::fs::write(
        user_hooks.join("post-commit"),
        "#!/usr/bin/env bash\nset -euo pipefail\nroot=\"$(git rev-parse --show-toplevel)\"\nprintf post-commit > \"$root/post-commit.marker\"\n",
    )?;
    std::fs::write(
        user_hooks.join("prepare-commit-msg"),
        "#!/usr/bin/env bash\nset -euo pipefail\nroot=\"$(git rev-parse --show-toplevel)\"\nprintf prepare-commit-msg > \"$root/prepare-commit-msg.marker\"\nprintf '\\nuser-prepare-hook\\n' >> \"$1\"\n",
    )?;
    std::fs::write(
        user_hooks.join("commit-msg"),
        "#!/usr/bin/env bash\nset -euo pipefail\nroot=\"$(git rev-parse --show-toplevel)\"\nprintf commit-msg > \"$root/commit-msg.marker\"\n",
    )?;
    for hook_name in [
        "pre-commit",
        "post-commit",
        "prepare-commit-msg",
        "commit-msg",
    ] {
        let mut perms = std::fs::metadata(user_hooks.join(hook_name))?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
        }
        std::fs::set_permissions(user_hooks.join(hook_name), perms)?;
    }
    run_git(
        builder.cwd_path(),
        &[
            "config",
            "core.hooksPath",
            user_hooks.to_string_lossy().as_ref(),
        ],
    )?;

    let mock = mount_commit_turn(
        &server,
        &format!("git commit --allow-empty -m '{COMMIT_MESSAGE}'"),
    )
    .await?;

    let output = run_exec(&builder, &server)?;
    assert!(
        output.status.success(),
        "codex-exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_shell_command_succeeded(&mock);

    let message = latest_commit_message(builder.cwd_path())?;
    assert!(message.contains("user-prepare-hook"));
    assert_trailer_once(&message, "Co-authored-by: Codex <noreply@openai.com>");
    for marker in [
        "pre-commit.marker",
        "post-commit.marker",
        "prepare-commit-msg.marker",
        "commit-msg.marker",
    ] {
        assert!(
            builder.cwd_path().join(marker).exists(),
            "expected {marker} to be created by the user's hook"
        );
    }

    Ok(())
}
