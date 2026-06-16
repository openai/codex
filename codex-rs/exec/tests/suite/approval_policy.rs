#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;

async fn run_exec_with_auto_review_config(extra_args: &[&str]) -> anyhow::Result<String> {
    let test = test_codex_exec();
    std::fs::write(
        test.home_path().join("config.toml"),
        r#"
approval_policy = "on-request"
approvals_reviewer = "auto_review"
"#,
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "done"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    let mut cmd = test.cmd_with_server(&server);
    let output = cmd
        .arg("--skip-git-repo-check")
        .args(extra_args)
        .arg("check approval mode")
        .output()?;

    assert!(output.status.success(), "exec run failed: {output:?}");

    Ok(String::from_utf8(output.stderr)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_preserves_on_request_for_auto_review_config() -> anyhow::Result<()> {
    let stderr = run_exec_with_auto_review_config(&[]).await?;
    assert!(
        stderr.contains("approval: on-request"),
        "stderr missing preserved auto-review approval mode: {stderr}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_bypass_preserves_never_for_auto_review_config() -> anyhow::Result<()> {
    let stderr =
        run_exec_with_auto_review_config(&["--dangerously-bypass-approvals-and-sandbox"]).await?;
    assert!(
        stderr.contains("approval: never"),
        "stderr missing bypass approval mode: {stderr}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_full_auto_preserves_never_for_auto_review_config() -> anyhow::Result<()> {
    let stderr = run_exec_with_auto_review_config(&["--full-auto"]).await?;
    assert!(
        stderr.contains("approval: never"),
        "stderr missing full-auto approval mode: {stderr}"
    );

    Ok(())
}
