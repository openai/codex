use codex_core::approval::CommandDecision;
use codex_core::approval::assess_command_safety;
use codex_core::exec::SandboxType;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use pretty_assertions::assert_eq as pretty_assert_eq;
use std::collections::HashSet;

fn cmd(parts: &[&str]) -> Vec<String> {
    parts.iter().map(ToString::to_string).collect()
}

fn approved_cache(commands: &[&[&str]]) -> HashSet<Vec<String>> {
    commands
        .iter()
        .map(|command| cmd(command))
        .collect::<HashSet<Vec<String>>>()
}

// Helper to get the platform-specific sandbox type for assertions.
fn get_platform_sandbox_type() -> SandboxType {
    if cfg!(target_os = "macos") {
        SandboxType::MacosSeatbelt
    } else if cfg!(target_os = "linux") {
        SandboxType::LinuxSeccomp
    } else {
        SandboxType::None
    }
}

#[test]
fn test_command_safety_assessments() {
    struct TestCase {
        command: &'static [&'static str],
        decision: CommandDecision,
        description: &'static str,
    }

    let cases: &[TestCase] = &[
        TestCase {
            command: &["ls", "-l"],
            decision: CommandDecision::permit(SandboxType::None, false),
            description: "Read-only commands should be permitted.",
        },
        TestCase {
            command: &["cat", "file"],
            decision: CommandDecision::permit(SandboxType::None, false),
            description: "Read-only commands should be permitted.",
        },
        TestCase {
            command: &["rm", "-rf", "/"],
            decision: CommandDecision::require_approval(),
            description: "High-risk commands should require approval.",
        },
        TestCase {
            command: &["touch", "file"], // Unrecognized
            decision: CommandDecision::require_approval(),
            description: "Unrecognized commands should require approval to prevent wrapped dangerous commands.",
        },
        TestCase {
            command: &["npm", "install"], // Unrecognized
            decision: CommandDecision::require_approval(),
            description: "Unrecognized package manager commands should require approval.",
        },
        TestCase {
            command: &["git", "status"],
            decision: CommandDecision::permit(SandboxType::None, false),
            description: "Read-only VCS commands should be permitted.",
        },
        TestCase {
            command: &["git", "commit", "-m", "msg"],
            decision: CommandDecision::permit(get_platform_sandbox_type(), false),
            description: "VCS modifications should be sandboxed.",
        },
        TestCase {
            command: &["bash", "-c", "ls && rm -f file"],
            decision: CommandDecision::require_approval(),
            description: "Pipelines with mixed risk should take the highest risk.",
        },
    ];

    for case in cases {
        let result = assess_command_safety(
            &cmd(case.command),
            AskForApproval::OnRequest,
            &SandboxPolicy::ReadOnly,
            &approved_cache(&[]),
            false,
        );
        pretty_assert_eq!(result, case.decision, "Failed case: {}", case.description);
    }
}

#[test]
fn test_invalid_and_edge_case_inputs() {
    // Empty command is unrecognized, so it should require approval.
    let result = assess_command_safety(
        &[],
        AskForApproval::OnRequest,
        &SandboxPolicy::ReadOnly,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(result, CommandDecision::require_approval());

    // Command with empty string is also unrecognized and should require approval.
    let result = assess_command_safety(
        &cmd(&["", "arg"]),
        AskForApproval::OnRequest,
        &SandboxPolicy::ReadOnly,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(result, CommandDecision::require_approval());

    // A complex pipeline with the highest risk in the middle.
    let result = assess_command_safety(
        &cmd(&["bash", "-c", "ls && rm -f file && git status"]),
        AskForApproval::OnRequest,
        &SandboxPolicy::ReadOnly,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(result, CommandDecision::require_approval());
}

#[test]
fn test_danger_full_access_policy() {
    // Even in DangerFullAccess mode, unrecognized commands require approval because
    // we can't assess their safety (prevents sudo/wrapper bypasses).
    let result = assess_command_safety(
        &cmd(&["touch", "file"]), // Unrecognized
        AskForApproval::OnRequest,
        &SandboxPolicy::DangerFullAccess,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(result, CommandDecision::require_approval());

    // Commands that are explicitly high-risk should also require approval.
    let result = assess_command_safety(
        &cmd(&["rm", "-rf", "/"]),
        AskForApproval::OnRequest,
        &SandboxPolicy::DangerFullAccess,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(result, CommandDecision::require_approval());
}

#[test]
fn test_never_approve_policy() {
    // With AskForApproval::Never, high-risk commands should be denied outright.
    let result = assess_command_safety(
        &cmd(&["rm", "-rf", "/"]),
        AskForApproval::Never,
        &SandboxPolicy::ReadOnly,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(
        result,
        CommandDecision::deny(
            "Destructive command detected; rejected by user approval settings".to_string()
        )
    );

    // Safe commands should still be permitted.
    let result = assess_command_safety(
        &cmd(&["ls"]),
        AskForApproval::Never,
        &SandboxPolicy::ReadOnly,
        &approved_cache(&[]),
        false,
    );
    pretty_assert_eq!(result, CommandDecision::permit(SandboxType::None, false));
}

#[test]
fn test_adversarial_inputs() {
    struct TestCase {
        command: &'static [&'static str],
        decision: CommandDecision,
        description: &'static str,
    }

    let cases: &[TestCase] = &[
        TestCase {
            command: &["sh", "-c", "rm -rf /"],
            decision: CommandDecision::require_approval(),
            description: "Dangerous command via `sh` should be caught.",
        },
        TestCase {
            command: &["env", "bash", "-c", "rm -rf /"],
            decision: CommandDecision::require_approval(),
            description: "Commands prefixed with `env` should be treated as Unrecognized and require approval.",
        },
        TestCase {
            // The shell_parser rejects command substitution, so the command is Unrecognized.
            // Unrecognized commands require approval.
            command: &["bash", "-c", "echo $(rm -rf /)"],
            decision: CommandDecision::require_approval(),
            description: "Command substitution should be rejected by the parser and require approval.",
        },
        TestCase {
            // The shell_parser rejects redirection, so the command is Unrecognized.
            // Unrecognized commands require approval.
            command: &["bash", "-c", "echo hello > /tmp/world"],
            decision: CommandDecision::require_approval(),
            description: "Redirection should be rejected by the parser and require approval.",
        },
    ];

    for case in cases {
        let result = assess_command_safety(
            &cmd(case.command),
            AskForApproval::OnRequest,
            &SandboxPolicy::ReadOnly,
            &approved_cache(&[]),
            false,
        );
        pretty_assert_eq!(result, case.decision, "Failed case: {}", case.description);
    }
}
