#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::string::ToString;

use codex_core::exec::ExecParams;
use codex_core::exec::ExecToolCallOutput;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::protocol::SandboxPolicy;
use codex_core::spawn::CODEX_SANDBOX_ENV_VAR;
use tempfile::TempDir;

use codex_core::error::Result;

use codex_core::get_platform_sandbox;

fn skip_test() -> bool {
    if std::env::var(CODEX_SANDBOX_ENV_VAR) == Ok("seatbelt".to_string()) {
        eprintln!("{CODEX_SANDBOX_ENV_VAR} is set to 'seatbelt', skipping test.");
        return true;
    }

    false
}

#[expect(clippy::expect_used)]
async fn run_test_cmd(tmp: TempDir, cmd: Vec<&str>) -> Result<ExecToolCallOutput> {
    let sandbox_type = get_platform_sandbox().expect("should be able to get sandbox type");
    assert_eq!(sandbox_type, SandboxType::MacosSeatbelt);

    let params = ExecParams {
        command: cmd.iter().map(ToString::to_string).collect(),
        cwd: tmp.path().to_path_buf(),
        expiration: 1000.into(),
        env: HashMap::new(),
        with_escalated_permissions: None,
        justification: None,
        arg0: None,
    };

    let policy = SandboxPolicy::new_read_only_policy();

    process_exec_tool_call(params, &policy, tmp.path(), &None, None).await
}

/// Command succeeds with exit code 0 normally
#[tokio::test]
async fn exit_code_0_succeeds() {
    if skip_test() {
        return;
    }

    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["echo", "hello"];

    let output = run_test_cmd(tmp, cmd).await.unwrap();
    assert_eq!(output.stdout.text, "hello\n");
    assert_eq!(output.stderr.text, "");
    assert_eq!(output.stdout.truncated_after_lines, None);
}

/// Command succeeds with exit code 0 normally
#[tokio::test]
async fn truncates_output_lines() {
    if skip_test() {
        return;
    }

    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["seq", "300"];

    let output = run_test_cmd(tmp, cmd).await.unwrap();

    let expected_output = (1..=300)
        .map(|i| format!("{i}\n"))
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(output.stdout.text, expected_output);
    assert_eq!(output.stdout.truncated_after_lines, None);
}

/// Command succeeds with exit code 0 normally
#[tokio::test]
async fn truncates_output_bytes() {
    if skip_test() {
        return;
    }

    let tmp = TempDir::new().expect("should be able to create temp dir");
    // each line is 1000 bytes
    let cmd = vec!["bash", "-lc", "seq 15 | awk '{printf \"%-1000s\\n\", $0}'"];

    let output = run_test_cmd(tmp, cmd).await.unwrap();

    assert!(output.stdout.text.len() >= 15000);
    assert_eq!(output.stdout.truncated_after_lines, None);
}

/// Command not found returns exit code 127, this is not considered a sandbox error
#[tokio::test]
async fn exit_command_not_found_is_ok() {
    if skip_test() {
        return;
    }

    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec!["/bin/bash", "-c", "nonexistent_command_12345"];
    run_test_cmd(tmp, cmd).await.unwrap();
}

/// Writing a file fails and should be considered a sandbox error
#[tokio::test]
async fn write_file_fails_as_sandbox_error() {
    if skip_test() {
        return;
    }

    let tmp = TempDir::new().expect("should be able to create temp dir");
    let path = tmp.path().join("test.txt");
    let cmd = vec![
        "/user/bin/touch",
        path.to_str().expect("should be able to get path"),
    ];

    assert!(run_test_cmd(tmp, cmd).await.is_err());
}
