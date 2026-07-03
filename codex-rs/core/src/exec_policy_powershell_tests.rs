use super::*;

fn trusted_windows_powershell() -> String {
    codex_shell_command::powershell::try_find_powershell_executable_blocking()
        .expect("Windows PowerShell must be installed")
        .as_path()
        .to_str()
        .expect("the Windows PowerShell path must be valid UTF-8")
        .to_string()
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

#[tokio::test]
async fn rejects_every_noninspectable_or_untrusted_powershell_state() {
    let cases = [
        (
            "untrusted parsed runtime",
            vec_str(&["powershell.EXE.CmD", "-Command", "echo allowed"]),
            Some(r#"prefix_rule(pattern=["echo"], decision="allow")"#),
            "the PowerShell runtime is not a protected system executable",
        ),
        (
            "trusted empty body",
            vec![
                trusted_windows_powershell(),
                "-Command".into(),
                String::new(),
            ],
            None,
            "the PowerShell script could not be inspected",
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
