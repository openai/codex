#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Context;
use codex_utils_cargo_bin::find_resource;
use core_test_support::test_codex_exec::test_codex_exec;

fn exec_fixture() -> anyhow::Result<std::path::PathBuf> {
    Ok(find_resource!("tests/fixtures/cli_responses_fixture.sse")?)
}

#[test]
fn not_so_yolo_uses_on_request_approvals_with_workspace_write() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;
    let repo_root = codex_utils_cargo_bin::repo_root()?;

    let output = test
        .cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .arg("--skip-git-repo-check")
        .arg("--not-so-yolo")
        .arg("-C")
        .arg(&repo_root)
        .arg("hello")
        .output()
        .context("not-so-yolo run should succeed")?;

    assert!(output.status.success(), "run failed: {output:?}");

    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("approval: on-request"),
        "stderr missing on-request approval mode: {stderr}"
    );
    let expected_sandbox = if cfg!(target_os = "windows") {
        "sandbox: read-only"
    } else {
        "sandbox: workspace-write"
    };
    assert!(
        stderr.contains(expected_sandbox),
        "stderr missing expected sandbox summary `{expected_sandbox}`: {stderr}"
    );

    Ok(())
}
