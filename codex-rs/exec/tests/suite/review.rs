#![allow(clippy::unwrap_used, clippy::expect_used)]
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::prelude::*;

/// Test that ensures the review command works without requiring prompts.
/// This test catches the regression where global prompt validation broke review commands.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_command_works_without_prompt() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // Test that `codex exec review --uncommitted` works without prompts
    // This would fail if global prompt validation incorrectly required prompts for all commands
    test.cmd()
        .arg("review")
        .arg("--uncommitted")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure() // Expected to fail due to no auth, but NOT due to "No prompt provided" error
        .stderr(predicate::str::contains("No prompt provided").not()); // Ensure it's NOT failing due to prompt validation

    Ok(())
}

/// Test that ensures review command with --base works without prompts
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_command_with_base_works_without_prompt() -> anyhow::Result<()> {
    let test = test_codex_exec();

    test.cmd()
        .arg("review")
        .arg("--base")
        .arg("HEAD~1")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure() // Expected to fail due to no auth, but NOT due to "No prompt provided" error
        .stderr(predicate::str::contains("No prompt provided").not());

    Ok(())
}

/// Test that ensures review command with --commit works without prompts
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_command_with_commit_works_without_prompt() -> anyhow::Result<()> {
    let test = test_codex_exec();

    test.cmd()
        .arg("review")
        .arg("--commit")
        .arg("HEAD")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure() // Expected to fail due to no auth, but NOT due to "No prompt provided" error
        .stderr(predicate::str::contains("No prompt provided").not());

    Ok(())
}

/// Test that review command still properly handles custom prompts when provided
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_command_with_custom_prompt_works() -> anyhow::Result<()> {
    let test = test_codex_exec();

    test.cmd()
        .arg("review")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("Check for security issues")
        .assert()
        .failure() // Expected to fail due to no auth, but NOT due to "No prompt provided" error
        .stderr(predicate::str::contains("No prompt provided").not());

    Ok(())
}
