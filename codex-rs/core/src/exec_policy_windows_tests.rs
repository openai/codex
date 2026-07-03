use super::*;
use pretty_assertions::assert_eq;
use std::sync::LazyLock;

static TRUSTED_WINDOWS_POWERSHELL_EXE: LazyLock<String> = LazyLock::new(|| {
    codex_shell_command::powershell::try_find_powershell_executable_blocking()
        .expect("Windows PowerShell must be installed")
        .as_path()
        .to_str()
        .expect("the Windows PowerShell path must be valid UTF-8")
        .to_string()
});

#[tokio::test]
async fn evaluates_powershell_inner_commands_against_prompt_rules() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["echo"], decision="prompt")"#.to_string()),
            command: vec![
                TRUSTED_WINDOWS_POWERSHELL_EXE.to_string(),
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
                TRUSTED_WINDOWS_POWERSHELL_EXE.to_string(),
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

#[tokio::test]
async fn namespace_literal_alias_with_equivalent_host_mapping_requires_approval() {
    let requirement = exec_approval_requirement_for_command(ExecApprovalRequirementScenario {
        policy_src: Some(
            r#"
prefix_rule(pattern = ["git.exe."], decision = "allow")
host_executable(name = "git", paths = ["C:\\trusted\\git.exe"])
"#
            .to_string(),
        ),
        command: vec![
            r"\\?\C:\attacker\git.exe.".to_string(),
            "status".to_string(),
        ],
        approval_policy: AskForApproval::UnlessTrusted,
        permission_profile: PermissionProfile::Disabled,
        sandbox_permissions: SandboxPermissions::UseDefault,
        prefix_rule: None,
    })
    .await;

    assert!(
        matches!(&requirement, ExecApprovalRequirement::NeedsApproval { .. }),
        "namespace executable outside the mapped paths must require approval: {requirement:?}"
    );
}

#[test]
fn commands_for_exec_policy_keeps_bare_powershell_alias_opaque() {
    let command = vec![
        "powershell.exe".to_string(),
        "-NoProfile".to_string(),
        "-Command".to_string(),
        "echo blocked".to_string(),
    ];

    assert_eq!(
        commands_for_exec_policy(&command),
        ExecPolicyCommands {
            commands: vec![command],
            used_complex_parsing: false,
            command_origin: ExecPolicyCommandOrigin::Generic,
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
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::UseDefault,
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::Generic,
            },
        ),
        "command is forbidden because approval policy is never and there is no Windows sandbox to rely on"
    );
}

#[test]
fn writable_windows_policy_without_sandbox_backend_still_requires_approval() {
    let command = vec!["cmd.exe".to_string(), "/c".to_string(), "dir".to_string()];
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    assert_eq!(
        Decision::Forbidden,
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::Never,
                permission_profile: &permission_profile,
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
                TRUSTED_WINDOWS_POWERSHELL_EXE.to_string(),
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
