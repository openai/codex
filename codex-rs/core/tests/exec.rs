#![cfg(target_os = "macos")]
#![expect(clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Arc;

use codex_core::exec::ExecParams;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::protocol::SandboxPolicy;
use codex_core::spawn::CODEX_SANDBOX_ENV_VAR;
use tempfile::TempDir;
use tokio::sync::Notify;

use codex_core::get_platform_sandbox;

async fn run_test_cmd(tmp: TempDir, cmd: Vec<&str>, should_be_ok: bool) {
    if std::env::var(CODEX_SANDBOX_ENV_VAR) == Ok("seatbelt".to_string()) {
        eprintln!("{CODEX_SANDBOX_ENV_VAR} is set to 'seatbelt', skipping test.");
        return;
    }

    let sandbox_type = get_platform_sandbox().expect("should be able to get sandbox type");
    assert_eq!(sandbox_type, SandboxType::MacosSeatbelt);

    let params = ExecParams {
        command: cmd.iter().map(|s| s.to_string()).collect(),
        cwd: tmp.path().to_path_buf(),
        timeout_ms: Some(1000),
        env: HashMap::new(),
    };

    let ctrl_c = Arc::new(Notify::new());
    let policy = SandboxPolicy::new_read_only_policy();

    let result = process_exec_tool_call(params, sandbox_type, ctrl_c, &policy, &None, None).await;

    assert!(result.is_ok() == should_be_ok);
}

/// Command succeeds with exit code 0 normally
#[tokio::test]
async fn exit_code_0_succeeds() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["echo", "hello"];

    run_test_cmd(tmp, cmd, true).await
}

/// Command not found returns exit code 127, this is not considered a sandbox error
#[tokio::test]
async fn exit_command_not_found_is_ok() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["/bin/bash", "-c", "nonexistent_command_12345"];
    run_test_cmd(tmp, cmd, true).await
}

/// Writing a file fails and should be considered a sandbox error
#[tokio::test]
async fn write_file_fails_as_sandbox_error() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let path = tmp.path().join("test.txt");
    let cmd = vec![
        "/user/bin/touch",
        path.to_str().expect("should be able to get path"),
    ];

    run_test_cmd(tmp, cmd, false).await;
}
