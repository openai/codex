#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Context;
use assert_cmd::prelude::*;
use codex_core::CODEX_APPLY_PATCH_ARG1;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// While we may add an `apply-patch` subcommand to the `codex` CLI multitool
/// at some point, we must ensure that the smaller `codex-exec` CLI can still
/// emulate the `apply_patch` CLI.
#[test]
fn test_standalone_exec_cli_can_use_apply_patch() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let relative_path = "source.txt";
    let absolute_path = tmp.path().join(relative_path);
    fs::write(&absolute_path, "original content\n")?;

    Command::cargo_bin("codex-exec")
        .context("should find binary for codex-exec")?
        .arg(CODEX_APPLY_PATCH_ARG1)
        .arg(
            r#"*** Begin Patch
*** Update File: source.txt
@@
-original content
+modified by apply_patch
*** End Patch"#,
        )
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout("Success. Updated the following files:\nM source.txt\n")
        .stderr(predicates::str::is_empty());
    assert_eq!(
        fs::read_to_string(absolute_path)?,
        "modified by apply_patch\n"
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_apply_patch_tool() -> anyhow::Result<()> {
    use crate::suite::common::run_e2e_exec_test;
    use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;

    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

    let tmp_cwd = tempdir().expect("failed to create temp dir");
    let tmp_path = tmp_cwd.path().to_path_buf();
    run_e2e_exec_test(
        tmp_cwd.path(),
        vec![
            include_str!("../fixtures/sse_apply_patch_add.json").to_string(),
            include_str!("../fixtures/sse_apply_patch_update.json").to_string(),
            include_str!("../fixtures/sse_response_completed.json").to_string(),
        ],
    )
    .await;

    let final_path = tmp_path.join("test.md");
    let contents = std::fs::read_to_string(&final_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", final_path.display()));
    assert_eq!(contents, "Final text\n");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_apply_patch_freeform_tool() -> anyhow::Result<()> {
    use crate::suite::common::run_e2e_exec_test;
    use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;

    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

    let tmp_cwd = tempdir().expect("failed to create temp dir");
    run_e2e_exec_test(
        tmp_cwd.path(),
        vec![
            include_str!("../fixtures/sse_apply_patch_freeform_add.json").to_string(),
            include_str!("../fixtures/sse_apply_patch_freeform_update.json").to_string(),
            include_str!("../fixtures/sse_response_completed.json").to_string(),
        ],
    )
    .await;

    // Verify final file contents
    let final_path = tmp_cwd.path().join("app.py");
    let contents = std::fs::read_to_string(&final_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", final_path.display()));
    assert_eq!(
        contents,
        include_str!("../fixtures/apply_patch_freeform_final.txt")
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn test_apply_patch_context() -> anyhow::Result<()> {
    use crate::suite::common::run_e2e_exec_test;
    use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;

    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

    let tmp_cwd = tempdir().expect("failed to create temp dir");
    run_e2e_exec_test(
        tmp_cwd.path(),
        vec![
            include_str!("../fixtures/sse_apply_patch_freeform_add.json").to_string(),
            include_str!("../fixtures/sse_apply_patch_context_update.json").to_string(),
            include_str!("../fixtures/sse_response_completed.json").to_string(),
        ],
    )
    .await;

    // Verify final file contents
    let final_path = tmp_cwd.path().join("app.py");
    let contents = std::fs::read_to_string(&final_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", final_path.display()));
    assert_eq!(
        contents,
        r#"class BaseClass:
  def method():
    return True
"#
    );
    Ok(())
}
