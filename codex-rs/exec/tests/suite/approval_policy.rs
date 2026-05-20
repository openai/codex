#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_preserves_on_request_for_auto_review_config() -> anyhow::Result<()> {
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

    let output = test
        .cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("check approval mode")
        .output()?;

    assert!(output.status.success(), "exec run failed: {output:?}");

    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("approval: on-request"),
        "stderr missing preserved auto-review approval mode: {stderr}"
    );

    Ok(())
}
