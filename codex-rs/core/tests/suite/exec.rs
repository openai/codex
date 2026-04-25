#![cfg(target_os = "macos")]

use codex_core::exec::ExecCapturePolicy;
use codex_core::exec::ExecParams;
use codex_core::exec::process_exec_tool_call;
use codex_core::sandboxing::SandboxPermissions;
use codex_core::spawn::CODEX_SANDBOX_ENV_VAR;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::Result;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxType;
use codex_sandboxing::get_platform_sandbox;
use core_test_support::PathExt;
use std::collections::HashMap;
use tempfile::TempDir;

fn skip_test() -> bool {
    if std::env::var(CODEX_SANDBOX_ENV_VAR) == Ok("seatbelt".to_string()) {
        eprintln!("{CODEX_SANDBOX_ENV_VAR} is set to 'seatbelt', skipping test.");
        return true;
    }

    false
}

#[expect(clippy::expect_used)]
async fn run_test_cmd<I, S>(tmp: TempDir, command: I) -> Result<ExecToolCallOutput>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let sandbox_type = get_platform_sandbox(/*windows_sandbox_enabled*/ false)
        .expect("should be able to get sandbox type");
    assert_eq!(sandbox_type, SandboxType::MacosSeatbelt);
    let cwd = tmp.path().abs();

    let params = ExecParams {
        command: command.into_iter().map(Into::into).collect(),
        cwd: cwd.clone(),
        expiration: 1000.into(),
        capture_policy: ExecCapturePolicy::ShellTool,
        env: HashMap::new(),
        network: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        justification: None,
        arg0: None,
    };

    let policy = SandboxPolicy::new_read_only_policy();
    let permission_profile = PermissionProfile::from_legacy_sandbox_policy(&policy);

    process_exec_tool_call(
        params,
        &permission_profile,
        &cwd,
        &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await
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

#[tokio::test]
async fn openpty_works_under_real_exec_seatbelt_path() {
    if skip_test() {
        return;
    }

    let python = match which::which("python3") {
        Ok(path) => path,
        Err(_) => {
            eprintln!("python3 not found in PATH, skipping test.");
            return;
        }
    };

    let tmp = TempDir::new().expect("should be able to create temp dir");
    let cmd = vec![
        python.to_string_lossy().into_owned(),
        "-c".to_string(),
        r#"import os

master, slave = os.openpty()
os.write(slave, b"ping")
assert os.read(master, 4) == b"ping""#
            .to_string(),
    ];

    let output = run_test_cmd(tmp, cmd).await.unwrap();
    assert_eq!(output.stdout.text, "");
    assert_eq!(output.stderr.text, "");
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
        "/usr/bin/touch",
        path.to_str().expect("should be able to get path"),
    ];

    assert!(run_test_cmd(tmp, cmd).await.is_err());
}
