use super::*;
use codex_network_proxy::BlockedRequest;
use codex_network_proxy::BlockedRequestArgs;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use pretty_assertions::assert_eq;
use std::time::Duration;

fn make_exec_output(
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    aggregated: &str,
) -> ExecToolCallOutput {
    ExecToolCallOutput {
        exit_code,
        stdout: StreamOutput::new(stdout.to_string()),
        stderr: StreamOutput::new(stderr.to_string()),
        aggregated_output: StreamOutput::new(aggregated.to_string()),
        duration: Duration::from_millis(1),
        timed_out: false,
    }
}

#[test]
fn preserves_legacy_boolean_denial_keywords() {
    for keyword in [
        "operation not permitted",
        "permission denied",
        "read-only file system",
        "seccomp",
        "sandbox",
        "landlock",
        "failed to write file",
    ] {
        let output = make_exec_output(/*exit_code*/ 1, "", keyword, "");

        assert_eq!(
            is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output),
            true,
            "{keyword}"
        );
    }
}

#[test]
fn preserves_legacy_boolean_denial_ordering() {
    let quick_reject_without_keyword =
        make_exec_output(/*exit_code*/ 127, "", "command not found", "");
    let quick_reject_with_keyword =
        make_exec_output(/*exit_code*/ 127, "", "Permission denied", "");
    let zero_exit_with_keyword =
        make_exec_output(/*exit_code*/ 0, "", "Operation not permitted", "");
    let non_sandbox_with_keyword =
        make_exec_output(/*exit_code*/ 1, "", "Operation not permitted", "");

    assert!(!is_likely_sandbox_denied(
        SandboxType::LinuxSeccomp,
        &quick_reject_without_keyword
    ));
    assert!(is_likely_sandbox_denied(
        SandboxType::LinuxSeccomp,
        &quick_reject_with_keyword
    ));
    assert!(!is_likely_sandbox_denied(
        SandboxType::LinuxSeccomp,
        &zero_exit_with_keyword
    ));
    assert!(!is_likely_sandbox_denied(
        SandboxType::None,
        &non_sandbox_with_keyword
    ));
}

#[test]
fn classifies_filesystem_violation_with_path() {
    let output = make_exec_output(
        /*exit_code*/ 1,
        "",
        "bash: /private/tmp/denied: Operation not permitted",
        "",
    );

    assert_eq!(
        classify_filesystem_sandbox_violation(SandboxType::MacosSeatbelt, &output),
        Some(FileSystemSandboxViolation {
            sandbox_type: SandboxType::MacosSeatbelt,
            reason: FileSystemSandboxViolationReason::OperationNotPermitted,
            path: Some("/private/tmp/denied".to_string()),
            output_snippet: "bash: /private/tmp/denied: Operation not permitted".to_string(),
        })
    );
}

#[test]
fn classifies_filesystem_violation_from_aggregated_output() {
    let output = make_exec_output(
        /*exit_code*/ 101,
        "",
        "",
        "cargo failed: Read-only file system when writing target",
    );

    assert_eq!(
        classify_filesystem_sandbox_violation(SandboxType::MacosSeatbelt, &output),
        Some(FileSystemSandboxViolation {
            sandbox_type: SandboxType::MacosSeatbelt,
            reason: FileSystemSandboxViolationReason::ReadOnlyFileSystem,
            path: None,
            output_snippet: "cargo failed: Read-only file system when writing target".to_string(),
        })
    );
}

#[cfg(unix)]
#[test]
fn classifies_linux_sigsys_exit() {
    let output = make_exec_output(
        /*exit_code*/ EXIT_CODE_SIGNAL_BASE + libc::SIGSYS,
        "",
        "",
        "",
    );

    assert_eq!(
        classify_filesystem_sandbox_violation(SandboxType::LinuxSeccomp, &output),
        Some(FileSystemSandboxViolation {
            sandbox_type: SandboxType::LinuxSeccomp,
            reason: FileSystemSandboxViolationReason::SignalSyscall,
            path: None,
            output_snippet: String::new(),
        })
    );
}

#[test]
fn preserves_boolean_denial_semantics_for_non_sandbox_mode() {
    let output = make_exec_output(/*exit_code*/ 1, "", "Operation not permitted", "");

    assert!(!is_likely_sandbox_denied(SandboxType::None, &output));
}

#[test]
fn converts_blocked_request_to_network_violation() {
    let blocked = BlockedRequest::new(BlockedRequestArgs {
        host: "example.com".to_string(),
        reason: "not_allowed".to_string(),
        client: Some("curl".to_string()),
        method: Some("CONNECT".to_string()),
        mode: None,
        protocol: "https".to_string(),
        decision: Some("block".to_string()),
        source: Some("policy".to_string()),
        port: Some(443),
    });

    assert_eq!(
        NetworkSandboxViolation::from_blocked_request(&blocked),
        NetworkSandboxViolation {
            host: "example.com".to_string(),
            reason: "not_allowed".to_string(),
            client: Some("curl".to_string()),
            method: Some("CONNECT".to_string()),
            protocol: "https".to_string(),
            decision: Some("block".to_string()),
            source: Some("policy".to_string()),
            port: Some(443),
            timestamp: blocked.timestamp,
        }
    );
}
