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

fn powershell_command(script: &str) -> Vec<String> {
    vec![
        TRUSTED_WINDOWS_POWERSHELL_EXE.to_string(),
        "-NoProfile".to_string(),
        "-Command".to_string(),
        script.to_string(),
    ]
}

fn prefix_rule_for(command: &[String], decision: &str) -> String {
    let pattern = command
        .iter()
        .map(|word| format!(r#""{}""#, starlark_string(word)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(r#"prefix_rule(pattern=[{pattern}], decision="{decision}")"#)
}

fn skip_outer(command: &[String], bypass_sandbox: bool) -> ExecApprovalRequirement {
    ExecApprovalRequirement::Skip {
        bypass_sandbox,
        proposed_execpolicy_amendment: (!bypass_sandbox)
            .then(|| ExecPolicyAmendment::new(command.to_vec())),
    }
}

fn prompt_outer(command: &[String]) -> ExecApprovalRequirement {
    ExecApprovalRequirement::NeedsApproval {
        reason: None,
        proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command.to_vec())),
    }
}

async fn windows_requirement(
    policy_src: String,
    command: &[String],
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_permissions: SandboxPermissions,
) -> ExecApprovalRequirement {
    ExecPolicyManager::new(policy_from_src(Some(&policy_src)))
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command,
            approval_policy,
            permission_profile,
            windows_sandbox_level,
            sandbox_permissions,
            prefix_rule: None,
        })
        .await
}

async fn default_windows_requirement(
    policy_src: String,
    command: &[String],
) -> ExecApprovalRequirement {
    windows_requirement(
        policy_src,
        command,
        AskForApproval::UnlessTrusted,
        PermissionProfile::read_only(),
        WindowsSandboxLevel::RestrictedToken,
        SandboxPermissions::UseDefault,
    )
    .await
}

fn external_profile() -> PermissionProfile {
    PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    }
}

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
async fn inner_allows_for_runtime_resolved_names_remain_sandboxed() {
    for (script, inner) in [
        ("echo blocked", "echo"),
        ("Get-Content Cargo.toml", "Get-Content"),
        ("Invoke-ProfileHook test", "Invoke-ProfileHook"),
        ("codex-path-helper --version", "codex-path-helper"),
    ] {
        let command = powershell_command(script);
        assert_eq!(
            default_windows_requirement(
                format!(r#"prefix_rule(pattern=["{inner}"], decision="allow")"#),
                &command,
            )
            .await,
            skip_outer(&command, false),
        );
    }
}

#[tokio::test]
async fn only_exact_outer_rules_can_bypass_runtime_resolution() {
    let command = powershell_command("Remove-Item target -Force");
    let executable = starlark_string(&command[0]);
    let rest = command[1..]
        .iter()
        .map(|word| format!(r#""{}""#, starlark_string(word)))
        .collect::<Vec<_>>()
        .join(", ");
    let mismatched = powershell_command("Remove-Item other -Force");
    let cases = [
        (prefix_rule_for(&command[..1], "allow"), Some(false)),
        (prefix_rule_for(&command, "allow"), Some(true)),
        (
            format!(
                "host_executable(name = \"powershell\", paths = [\"{executable}\"])\n\
             prefix_rule(pattern=[\"powershell\", {rest}], decision=\"allow\")"
            ),
            Some(true),
        ),
        (
            format!(
                "prefix_rule(pattern=[[\"{executable}\", \"C:\\\\other\\\\powershell.exe\"], \
             [\"-NoProfile\", \"-noprofile\"], \"-Command\", \
             \"Remove-Item target -Force\"], decision=\"allow\")"
            ),
            Some(true),
        ),
        (prefix_rule_for(&mismatched, "allow"), None),
    ];

    for (policy_src, bypass_sandbox) in cases {
        assert_eq!(
            default_windows_requirement(policy_src, &command).await,
            bypass_sandbox.map_or_else(
                || prompt_outer(&command),
                |bypass| skip_outer(&command, bypass)
            ),
        );
    }
}

#[tokio::test]
async fn full_outer_allow_does_not_bypass_extended_unsupported_wrapper() {
    use SandboxPermissions::*;
    use WindowsSandboxLevel::*;

    let command = powershell_command("Get-Content Cargo.toml");
    let policy_src = prefix_rule_for(&command, "allow");
    let mut extended = command.clone();
    extended.push("trailing-runtime-argument".to_string());

    for (profile, level, permissions, prompts, offers_amendment) in [
        (
            PermissionProfile::read_only(),
            RestrictedToken,
            UseDefault,
            false,
            true,
        ),
        (
            PermissionProfile::read_only(),
            RestrictedToken,
            RequireEscalated,
            true,
            false,
        ),
        (
            PermissionProfile::read_only(),
            RestrictedToken,
            WithAdditionalPermissions,
            true,
            false,
        ),
        (
            PermissionProfile::read_only(),
            Disabled,
            UseDefault,
            true,
            true,
        ),
        (
            PermissionProfile::Disabled,
            RestrictedToken,
            UseDefault,
            false,
            true,
        ),
    ] {
        assert_eq!(
            windows_requirement(
                policy_src.clone(),
                &extended,
                AskForApproval::UnlessTrusted,
                profile,
                level,
                permissions,
            )
            .await,
            if prompts {
                if offers_amendment {
                    prompt_outer(&extended)
                } else {
                    ExecApprovalRequirement::NeedsApproval {
                        reason: None,
                        proposed_execpolicy_amendment: None,
                    }
                }
            } else if offers_amendment {
                skip_outer(&extended, false)
            } else {
                ExecApprovalRequirement::Skip {
                    bypass_sandbox: false,
                    proposed_execpolicy_amendment: None,
                }
            },
        );
    }
}

#[tokio::test]
async fn explicit_inner_and_outer_restrictions_remain_strictest() {
    let command = powershell_command("Get-Content Cargo.toml");
    let outer_allow = prefix_rule_for(&command, "allow");
    let inner_allow = r#"prefix_rule(pattern=["Get-Content"], decision="allow")"#;
    let rendered = render_shlex_command(&command);

    for (inner, decision) in [
        (true, "prompt"),
        (true, "forbidden"),
        (false, "prompt"),
        (false, "forbidden"),
    ] {
        let (policy_src, forbidden_prefix) = if inner {
            (
                format!(
                    "{outer_allow}\nprefix_rule(pattern=[\"Get-Content\"], decision=\"{decision}\")"
                ),
                "Get-Content".to_string(),
            )
        } else {
            (
                format!("{inner_allow}\n{}", prefix_rule_for(&command, decision)),
                rendered.clone(),
            )
        };
        let expected = if decision == "prompt" {
            ExecApprovalRequirement::NeedsApproval {
                reason: Some(format!("`{rendered}` requires approval by policy")),
                proposed_execpolicy_amendment: None,
            }
        } else {
            ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{rendered}` rejected: policy forbids commands starting with `{forbidden_prefix}`"
                ),
            }
        };
        assert_eq!(
            windows_requirement(
                policy_src,
                &command,
                AskForApproval::OnRequest,
                PermissionProfile::read_only(),
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::UseDefault,
            )
            .await,
            expected,
        );
    }
}

#[tokio::test]
async fn outer_authority_tracks_permission_deltas_and_missing_managed_sandbox() {
    use SandboxPermissions::*;
    use WindowsSandboxLevel::*;

    let command = powershell_command("Get-Content Cargo.toml");
    let inner_allow = r#"prefix_rule(pattern=["Get-Content"], decision="allow")"#.to_string();
    let denied_read_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: FileSystemAccessMode::Deny,
        }]),
        NetworkSandboxPolicy::Restricted,
    );

    for (profile, level, permissions, prompts, offers_amendment) in [
        (
            PermissionProfile::read_only(),
            RestrictedToken,
            RequireEscalated,
            true,
            false,
        ),
        (
            PermissionProfile::read_only(),
            Disabled,
            UseDefault,
            true,
            true,
        ),
        (
            denied_read_profile,
            RestrictedToken,
            RequireEscalated,
            false,
            false,
        ),
        (
            PermissionProfile::read_only(),
            RestrictedToken,
            WithAdditionalPermissions,
            true,
            false,
        ),
        (
            PermissionProfile::Disabled,
            RestrictedToken,
            RequireEscalated,
            false,
            false,
        ),
        (
            PermissionProfile::Disabled,
            RestrictedToken,
            WithAdditionalPermissions,
            false,
            false,
        ),
        (external_profile(), RestrictedToken, UseDefault, false, true),
        (
            external_profile(),
            RestrictedToken,
            RequireEscalated,
            true,
            false,
        ),
        (
            external_profile(),
            RestrictedToken,
            WithAdditionalPermissions,
            true,
            false,
        ),
    ] {
        assert_eq!(
            windows_requirement(
                inner_allow.clone(),
                &command,
                AskForApproval::OnRequest,
                profile,
                level,
                permissions,
            )
            .await,
            if prompts {
                if offers_amendment {
                    prompt_outer(&command)
                } else {
                    ExecApprovalRequirement::NeedsApproval {
                        reason: None,
                        proposed_execpolicy_amendment: None,
                    }
                }
            } else if offers_amendment {
                skip_outer(&command, false)
            } else {
                ExecApprovalRequirement::Skip {
                    bypass_sandbox: false,
                    proposed_execpolicy_amendment: None,
                }
            },
        );
    }

    let exact_allow = format!("{inner_allow}\n{}", prefix_rule_for(&command, "allow"));
    for (level, permissions) in [
        (RestrictedToken, RequireEscalated),
        (RestrictedToken, WithAdditionalPermissions),
        (Disabled, UseDefault),
    ] {
        assert_eq!(
            windows_requirement(
                exact_allow.clone(),
                &command,
                AskForApproval::OnRequest,
                PermissionProfile::read_only(),
                level,
                permissions,
            )
            .await,
            skip_outer(&command, true),
        );
    }
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
    let command = powershell_command("Remove-Item test -Force");

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: command.clone(),
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        },
    )
    .await;
}

#[tokio::test]
async fn mixed_powershell_inner_commands_use_the_strictest_decision() {
    let command = powershell_command("echo safe; Remove-Item target -Force");

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["echo"], decision="allow")"#.to_string()),
            command: command.clone(),
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        },
    )
    .await;
}
