//! Integration tests covering behaviour when the sandbox is given
//! `DiskWritePlatformUserTempFolder` permission.  These ensure that commands
//! executed *inside* the Linux seccomp/landlock sandbox can successfully
//! create files in the user-specific temporary directory even when `/tmp` is
//! blocked.

#![cfg(target_os = "linux")]

use std::path::PathBuf;

use codex_core::exec::{process_exec_tool_call, ExecParams, SandboxType};
use codex_core::protocol::SandboxPolicy;
use tempfile::TempDir;
use tokio::sync::Notify;

/// Helper – run a shell snippet under the sandbox returning its exit code.
async fn run_cmd(cmd: &[&str]) -> i32 {
    let params = ExecParams {
        command: cmd.iter().map(|s| s.to_string()).collect(),
        cwd: std::env::current_dir().expect("cwd exists"),
        timeout_ms: Some(1_000),
    };

    let sandbox_policy = SandboxPolicy::from(vec![
        codex_core::protocol::SandboxPermission::DiskFullReadAccess,
        codex_core::protocol::SandboxPermission::DiskWritePlatformUserTempFolder,
    ]);

    let ctrl_c = std::sync::Arc::new(Notify::new());

    let res = process_exec_tool_call(params, SandboxType::LinuxSeccomp, ctrl_c, &sandbox_policy)
        .await
        .expect("sandboxed command to run");

    res.exit_code
}

#[tokio::test]
async fn test_write_to_user_tmp() {
    // Use a disposable per-test directory and point TMPDIR at it so we know
    // exactly where the write should land.
    let tmpdir = TempDir::new().expect("create tmp");
    std::env::set_var("TMPDIR", tmpdir.path());

    // This should succeed because the sandbox includes the user-temp write
    // permission even though `/tmp` is *not* writable.
    let exit = run_cmd(&["bash", "-lc", "echo ok > $TMPDIR/hello.txt"]).await;
    assert_eq!(exit, 0, "command failed to write to TMPDIR inside sandbox");

    // Confirm the file actually exists.
    assert!(tmpdir.path().join("hello.txt").exists());
}

