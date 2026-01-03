#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;

async fn run_exec_with_server(args: &[&str], prompt: &str) -> anyhow::Result<String> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Task completed"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    let output = {
        let mut cmd = test.cmd_with_server(&server);
        cmd.arg("--skip-git-repo-check");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(prompt).output()?
    };

    assert!(output.status.success(), "run failed: {output:?}");
    Ok(String::from_utf8(output.stderr)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_read_only_sandbox_flag() -> anyhow::Result<()> {
    let stderr =
        run_exec_with_server(&["--sandbox", "read-only"], "test read-only sandbox").await?;
    assert!(stderr.contains("sandbox: read-only"), "{stderr}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_workspace_write_sandbox_flag() -> anyhow::Result<()> {
    let stderr = run_exec_with_server(
        &["--sandbox", "workspace-write"],
        "test workspace-write sandbox",
    )
    .await?;
    assert!(stderr.contains("sandbox: workspace-write"), "{stderr}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_danger_full_access_sandbox_flag() -> anyhow::Result<()> {
    let stderr = run_exec_with_server(
        &["--sandbox", "danger-full-access"],
        "test danger-full-access sandbox",
    )
    .await?;
    assert!(stderr.contains("sandbox: danger-full-access"), "{stderr}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_external_sandbox_flag_defaults_to_restricted_network() -> anyhow::Result<()> {
    let stderr =
        run_exec_with_server(&["--sandbox", "external-sandbox"], "test external sandbox").await?;
    assert!(stderr.contains("sandbox: external-sandbox"), "{stderr}");
    assert!(
        !stderr.contains("network access enabled"),
        "stderr unexpectedly claims network access enabled: {stderr}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_external_sandbox_with_enabled_network_access() -> anyhow::Result<()> {
    let stderr = run_exec_with_server(
        &[
            "--sandbox",
            "external-sandbox",
            "--network-access",
            "enabled",
        ],
        "test external sandbox network enabled",
    )
    .await?;
    assert!(
        stderr.contains("sandbox: external-sandbox (network access enabled)"),
        "{stderr}"
    );

    Ok(())
}

#[test]
fn rejects_network_access_without_external_sandbox() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let output = test
        .cmd()
        .arg("--skip-git-repo-check")
        .arg("--network-access")
        .arg("enabled")
        .arg("test")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("--network-access can only be used with --sandbox external-sandbox"),
        "{stderr}"
    );

    Ok(())
}

#[test]
fn rejects_external_sandbox_with_full_auto() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let output = test
        .cmd()
        .arg("--skip-git-repo-check")
        .arg("--full-auto")
        .arg("--sandbox")
        .arg("external-sandbox")
        .arg("test")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("--sandbox external-sandbox cannot be used with --full-auto"),
        "{stderr}"
    );

    Ok(())
}

#[test]
fn rejects_external_sandbox_with_dangerously_bypass_approvals_and_sandbox() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let output = test
        .cmd()
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("external-sandbox")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("test")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains(
            "--sandbox external-sandbox cannot be used with --dangerously-bypass-approvals-and-sandbox"
        ),
        "{stderr}"
    );

    Ok(())
}
