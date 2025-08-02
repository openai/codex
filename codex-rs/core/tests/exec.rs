#![cfg(target_os = "macos")]
#![expect(clippy::expect_used)]
#![expect(clippy::unwrap_used)]

use std::collections::HashMap;
use std::sync::Arc;

use codex_core::error::CodexErr;
use codex_core::error::SandboxErr;
use codex_core::exec::ExecParams;
use codex_core::exec::ExecToolCallOutput;
use codex_core::exec::process_exec_tool_call;
use codex_core::protocol::SandboxPolicy;
use tempfile::TempDir;
use tokio::sync::Notify;

use codex_core::get_platform_sandbox;

/// Command succeeds with exit code 0 normally
#[tokio::test]
async fn exit_code_0_succeeds() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["echo", "hello"];
    let output = run_test_cmd(tmp, cmd)
        .await
        .expect("command should return successfully");

    assert_eq!(output.exit_code, 0);
}

/// Command not found returns exit code 127, this is not considered a sandbox error
#[tokio::test]
async fn exit_command_not_found_is_propagated() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["/bin/bash", "-c", "nonexistent_command_12345"];
    let output = run_test_cmd(tmp, cmd)
        .await
        .expect("command should return successfully");

    assert_eq!(output.exit_code, 127);
}

/// Writing a file fails and should be considered a sandbox error
#[tokio::test]
async fn write_file_fails_as_sandbox_error() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["/bin/bash", "-c", "touch", "/tmp/test.txt"];
    let result = run_test_cmd(tmp, cmd).await;

    assert!(result.is_err());
    assert!(matches!(
        result.err().unwrap(),
        CodexErr::Sandbox(SandboxErr::Denied(_, _, _))
    ));
}

async fn run_test_cmd(
    tmp: TempDir,
    cmd: Vec<&'static str>,
) -> Result<ExecToolCallOutput, CodexErr> {
    let sandbox_type = get_platform_sandbox().expect("should be able to get sandbox type");

    let params = ExecParams {
        command: cmd.iter().map(|s| s.to_string()).collect(),
        cwd: tmp.path().to_path_buf(),
        timeout_ms: Some(100),
        env: HashMap::new(),
    };

    let ctrl_c = Arc::new(Notify::new());
    let policy = SandboxPolicy::new_read_only_policy();

    process_exec_tool_call(params, sandbox_type, ctrl_c, &policy, &None, None).await
}
