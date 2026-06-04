use super::*;
use pretty_assertions::assert_eq;
use std::path::Path;

#[tokio::test]
async fn evaluates_powershell_inner_commands_against_prompt_rules() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["echo"], decision="prompt")"#.to_string()),
            command: vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "echo blocked".to_string(),
            ],
            approval_policy: AskForApproval::Never,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: PROMPT_CONFLICT_REASON.to_string(),
        },
    )
    .await;
}

#[tokio::test]
async fn evaluates_powershell_inner_commands_against_allow_rules() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["echo"], decision="allow")"#.to_string()),
            command: vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "echo blocked".to_string(),
            ],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[test]
fn commands_for_exec_policy_parses_powershell_shell_wrapper() {
    let command = vec![
        "powershell.exe".to_string(),
        "-NoProfile".to_string(),
        "-Command".to_string(),
        "echo blocked".to_string(),
    ];

    assert_eq!(
        commands_for_exec_policy(&command),
        ExecPolicyCommands {
            commands: vec![vec!["echo".to_string(), "blocked".to_string()]],
            used_complex_parsing: false,
            command_origin: ExecPolicyCommandOrigin::PowerShell,
        }
    );
}

#[test]
fn unmatched_safe_powershell_words_are_allowed() {
    let command = vec!["Get-Content".to_string(), "Cargo.toml".to_string()];

    assert_eq!(
        Decision::Allow,
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::UnlessTrusted,
                permission_profile: &PermissionProfile::read_only(),
                sandbox_cwd: Path::new("/tmp"),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::UseDefault,
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::PowerShell,
            },
        )
    );
}

#[test]
fn read_only_windows_sandbox_runs_unmatched_commands_under_sandbox() {
    let command = vec!["cmd.exe".to_string(), "/c".to_string(), "dir".to_string()];

    for windows_sandbox_level in [
        WindowsSandboxLevel::RestrictedToken,
        WindowsSandboxLevel::Elevated,
    ] {
        assert_eq!(
            Decision::Allow,
            render_decision_for_unmatched_command(
                &command,
                UnmatchedCommandContext {
                    approval_policy: AskForApproval::Never,
                    permission_profile: &PermissionProfile::read_only(),
                    file_system_sandbox_policy: &read_only_file_system_sandbox_policy(),
                    sandbox_cwd: Path::new("/tmp"),
                    windows_sandbox_level,
                    sandbox_permissions: SandboxPermissions::UseDefault,
                    used_complex_parsing: false,
                    command_origin: ExecPolicyCommandOrigin::Generic,
                },
            )
        );
    }
}

#[test]
fn read_only_windows_policy_without_sandbox_backend_still_requires_approval() {
    let command = vec!["cmd.exe".to_string(), "/c".to_string(), "dir".to_string()];

    assert_eq!(
        Decision::Forbidden,
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::Never,
                permission_profile: &PermissionProfile::read_only(),
                file_system_sandbox_policy: &read_only_file_system_sandbox_policy(),
                sandbox_cwd: Path::new("/tmp"),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::UseDefault,
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::Generic,
            },
        )
    );
}

#[tokio::test]
async fn unmatched_dangerous_powershell_inner_commands_require_approval() {
    let inner_command = vec![
        "Remove-Item".to_string(),
        "test".to_string(),
        "-Force".to_string(),
    ];

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Remove-Item test -Force".to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(inner_command)),
        },
    )
    .await;
}
