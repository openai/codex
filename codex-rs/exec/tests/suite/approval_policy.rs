#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;

async fn run_exec_with_config(
    config_toml: &str,
    extra_args: &[&str],
) -> anyhow::Result<(String, responses::ResponseMock)> {
    let test = test_codex_exec();
    std::fs::write(test.home_path().join("config.toml"), config_toml)?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "done"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    let mut cmd = test.cmd_with_server(&server);
    let output = cmd
        .arg("--skip-git-repo-check")
        .args(extra_args)
        .arg("check approval mode")
        .output()?;

    assert!(output.status.success(), "exec run failed: {output:?}");

    Ok((String::from_utf8(output.stderr)?, mock))
}

async fn run_exec_with_auto_review_config(extra_args: &[&str]) -> anyhow::Result<String> {
    let (stderr, _) = run_exec_with_config(
        r#"
approval_policy = "on-request"
approvals_reviewer = "auto_review"
"#,
        extra_args,
    )
    .await?;
    Ok(stderr)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_without_a_thread_uses_fresh_exec_policy() -> anyhow::Result<()> {
    let (stderr, _) =
        run_exec_with_config("approval_policy = 'on-request'", &["resume", "--last"]).await?;
    assert!(stderr.contains("approval: on-request"), "{stderr}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_warns_about_human_approval_for_granular_policy() -> anyhow::Result<()> {
    for request_permissions in [true, false] {
        let policy = format!(
            "approval_policy={{granular={{sandbox_approval=false,rules=false,request_permissions={request_permissions},mcp_elicitations=false}}}}"
        );
        let (stderr, _) = run_exec_with_config("", &["-c", &policy]).await?;
        assert!(
            stderr.contains("WARNING: This thread is configured to ask a human"),
            "{stderr}"
        );
    }
    Ok(())
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
async fn exec_approve_for_me_flag_sets_approval_mode_and_sandbox() -> anyhow::Result<()> {
    let (stderr, _) = run_exec_with_config("", &["--approve-for-me"]).await?;
    assert!(
        stderr.contains("approval: on-request"),
        "stderr missing --approve-for-me approval mode: {stderr}"
    );
    assert!(
        stderr.contains("sandbox: workspace-write"),
        "stderr missing --approve-for-me sandbox mode: {stderr}"
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
async fn exec_review_keeps_parent_approval_policy_and_disables_delegate_approvals()
-> anyhow::Result<()> {
    let (stderr, mock) =
        run_exec_with_config("approval_policy = 'on-request'", &["review"]).await?;
    assert!(stderr.contains("approval: on-request"), "{stderr}");
    assert!(
        mock.single_request()
            .message_input_texts("developer")
            .iter()
            .any(|text| text.contains("Approval policy is currently never"))
    );
    Ok(())
}
