#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Context;
use core_test_support::responses::{
    ev_apply_patch_custom_tool_call, ev_completed, mount_sse_sequence, sse, start_mock_server,
};
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use std::env;
use std::fs;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;

/// Tests that codex can recover when the binary is deleted mid-execution.
/// This simulates what happens when users update codex (e.g., via npm) while it's running.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_binary_recovery_after_deletion() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let tmp_path = test.cwd_path().to_path_buf();

    let patch1 = r#"*** Begin Patch
*** Add File: file1.txt
+first file
*** End Patch"#;

    let patch2 = r#"*** Begin Patch
*** Add File: file2.txt
+second file
*** End Patch"#;

    let response_streams = vec![
        sse(vec![
            ev_apply_patch_custom_tool_call("request_0", patch1),
            ev_completed("request_0"),
        ]),
        sse(vec![
            ev_apply_patch_custom_tool_call("request_1", patch2),
            ev_completed("request_1"),
        ]),
        sse(vec![ev_completed("request_2")]),
    ];

    let server = start_mock_server().await;
    mount_sse_sequence(&server, response_streams).await;

    let original_binary = assert_cmd::cargo::cargo_bin("codex-exec");
    let binary_temp_dir = TempDir::new()?;
    let copied_binary = binary_temp_dir.path().join("codex-exec");

    fs::copy(&original_binary, &copied_binary)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&copied_binary)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&copied_binary, perms)?;
    }

    let original_bin_dir = original_binary.parent().unwrap();
    let original_path = env::var("PATH").unwrap_or_default();
    let path_sep = if cfg!(windows) { ";" } else { ":" };
    let new_path = if original_path.is_empty() {
        original_bin_dir.display().to_string()
    } else {
        format!("{}{}{}", original_bin_dir.display(), path_sep, original_path)
    };

    let mut cmd = Command::new(&copied_binary);
    cmd.current_dir(test.cwd_path())
        .env("CODEX_HOME", test.home_path())
        .env("CODEX_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()))
        .env("PATH", new_path)
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("danger-full-access")
        .arg("create two files")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stderr = child.stderr.take().unwrap();
    let stderr_handle = tokio::spawn(async move {
        let mut stderr_reader = BufReader::new(stderr);
        let mut output = Vec::new();
        tokio::io::copy(&mut stderr_reader, &mut output).await.ok();
        String::from_utf8_lossy(&output).to_string()
    });

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    let mut binary_deleted = false;
    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains("file1.txt") && !binary_deleted {
            binary_deleted = true;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            fs::remove_file(&copied_binary)?;
            drop(binary_temp_dir);
            break;
        }
    }

    while let Ok(Some(_)) = reader.next_line().await {}

    let status = child.wait().await?;
    let _stderr_output = stderr_handle.await?;

    assert!(status.success(), "Should succeed with recovery");

    let file2_path = tmp_path.join("file2.txt");
    assert!(
        file2_path.exists(),
        "file2.txt must exist after binary was deleted - proves PATH recovery worked"
    );

    Ok(())
}

/// Tests that codex fails gracefully with a helpful error when binary is deleted
/// and no alternative is found in PATH.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_binary_recovery_fails_when_not_in_path() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();

    let patch1 = r#"*** Begin Patch
*** Add File: file1.txt
+first file
*** End Patch"#;

    let patch2 = r#"*** Begin Patch
*** Add File: file2.txt
+second file
*** End Patch"#;

    let response_streams = vec![
        sse(vec![
            ev_apply_patch_custom_tool_call("request_0", patch1),
            ev_completed("request_0"),
        ]),
        sse(vec![
            ev_apply_patch_custom_tool_call("request_1", patch2),
            ev_completed("request_1"),
        ]),
        sse(vec![ev_completed("request_2")]),
    ];

    let server = start_mock_server().await;
    mount_sse_sequence(&server, response_streams).await;

    let original_binary = assert_cmd::cargo::cargo_bin("codex-exec");
    let binary_temp_dir = TempDir::new()?;
    let copied_binary = binary_temp_dir.path().join("codex-exec");

    fs::copy(&original_binary, &copied_binary)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&copied_binary)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&copied_binary, perms)?;
    }

    let temp_path_dir = TempDir::new()?;
    let minimal_path = temp_path_dir.path().display().to_string();

    let mut cmd = Command::new(&copied_binary);
    cmd.current_dir(test.cwd_path())
        .env("CODEX_HOME", test.home_path())
        .env("CODEX_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()))
        .env("PATH", &minimal_path)
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("danger-full-access")
        .arg("create two files")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stderr = child.stderr.take().unwrap();
    let stderr_handle = tokio::spawn(async move {
        let mut stderr_reader = BufReader::new(stderr);
        let mut output = Vec::new();
        tokio::io::copy(&mut stderr_reader, &mut output).await.ok();
        String::from_utf8_lossy(&output).to_string()
    });

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    let mut binary_deleted = false;
    while let Ok(Some(line)) = reader.next_line().await {
        if line.contains("file1.txt") && !binary_deleted {
            binary_deleted = true;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            fs::remove_file(&copied_binary)?;
            drop(binary_temp_dir);
            break;
        }
    }

    while let Ok(Some(_)) = reader.next_line().await {}

    let status = child.wait().await?;
    let stderr_output = stderr_handle.await?;

    if !status.success() {
        assert!(
            stderr_output.contains("Did you delete Codex"),
            "When failed, must show deletion error.\nStderr:\n{stderr_output}"
        );
    }

    Ok(())
}
