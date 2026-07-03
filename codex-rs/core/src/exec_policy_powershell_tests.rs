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

fn untrusted_powershell_command(script: &str) -> Vec<String> {
    vec![
        "powershell.exe".to_string(),
        "-Command".to_string(),
        script.to_string(),
    ]
}

fn absolute_untrusted_powershell_command(script: &str) -> Vec<String> {
    vec![
        r"C:\workspace\powershell.exe".to_string(),
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
    requirement_with_options(
        policy_src,
        command,
        approval_policy,
        permission_profile,
        WindowsSandboxLevel::RestrictedToken,
        sandbox_permissions,
        /*prefix_rule*/ None,
    )
    .await
}

async fn requirement_with_options(
    policy_src: Option<&str>,
    command: &[String],
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_permissions: SandboxPermissions,
    prefix_rule: Option<Vec<String>>,
) -> ExecApprovalRequirement {
    let policy = policy_from_src(policy_src);
    ExecPolicyManager::new(policy)
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command,
            approval_policy,
            permission_profile,
            windows_sandbox_level,
            sandbox_permissions,
            prefix_rule,
        })
        .await
}

fn composed_untrusted_requirement(
    policy_src: Option<&str>,
    outer_argv: &[String],
    commands: &[Vec<String>],
    approval_policy: AskForApproval,
    permission_profile: &PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_permissions: SandboxPermissions,
) -> ExecApprovalRequirement {
    let policy = policy_from_src(policy_src);
    create_untrusted_powershell_approval_requirement(
        policy.as_ref(),
        outer_argv,
        commands,
        UnmatchedCommandContext {
            approval_policy,
            permission_profile,
            windows_sandbox_level,
            sandbox_permissions,
            used_complex_parsing: false,
            command_origin: ExecPolicyCommandOrigin::PowerShell,
        },
    )
}

fn one_shot(reason: Option<String>) -> ExecApprovalRequirement {
    ExecApprovalRequirement::NeedsOneShotApproval { reason }
}

fn untrusted_skip(bypass_sandbox: bool) -> ExecApprovalRequirement {
    ExecApprovalRequirement::Skip {
        bypass_sandbox,
        proposed_execpolicy_amendment: None,
    }
}

#[test]
fn untrusted_parsed_state_retains_exact_outer_argv_and_trusted_state_does_not() {
    let outer_argv = vec_str(&[
        r"\\?\C:\Workspace\PoWeRsHeLl.ExE. ",
        "-NoProfile",
        "-Command",
        "echo First; Write-Output SECOND",
    ]);
    let commands = vec![
        vec_str(&["echo", "First"]),
        vec_str(&["Write-Output", "SECOND"]),
    ];

    let Some(powershell_policy::PreparedPowerShell::Parsed(untrusted)) =
        powershell_policy::prepare_classified(
            &outer_argv,
            PowerShellExecPolicyParse::UntrustedRuntime {
                outcome: PowerShellExecPolicyParseOutcome::Commands(commands.clone()),
            },
        )
    else {
        panic!("nonempty inspected untrusted PowerShell should remain policy-evaluable");
    };
    pretty_assertions::assert_eq!(untrusted.commands(), commands);
    pretty_assertions::assert_eq!(
        untrusted.untrusted_outer_argv(),
        Some(outer_argv.as_slice())
    );

    let Some(powershell_policy::PreparedPowerShell::Parsed(trusted)) =
        powershell_policy::prepare_classified(
            &outer_argv,
            PowerShellExecPolicyParse::TrustedRuntime {
                outcome: PowerShellExecPolicyParseOutcome::Commands(vec![vec_str(&[
                    "Get-Location",
                ])]),
            },
        )
    else {
        panic!("trusted parsed PowerShell should remain parsed");
    };
    pretty_assertions::assert_eq!(trusted.untrusted_outer_argv(), None);
}

#[test]
fn untrusted_opaque_failed_and_empty_parser_states_remain_terminal() {
    let command = untrusted_powershell_command("echo blocked");
    let rendered = render_shlex_command(&command);
    let cases = [
        (
            PowerShellExecPolicyParseOutcome::Commands(Vec::new()),
            "the protected system parser returned an empty command while inspecting an untrusted PowerShell wrapper",
        ),
        (
            PowerShellExecPolicyParseOutcome::Commands(vec![Vec::new()]),
            "the protected system parser returned an empty command while inspecting an untrusted PowerShell wrapper",
        ),
        (
            PowerShellExecPolicyParseOutcome::Commands(vec![vec![String::new()]]),
            "the protected system parser returned an empty command while inspecting an untrusted PowerShell wrapper",
        ),
        (
            PowerShellExecPolicyParseOutcome::Unsupported,
            "an untrusted PowerShell wrapper could not be inspected with the protected system parser",
        ),
        (
            PowerShellExecPolicyParseOutcome::Failed,
            "the protected system parser failed while inspecting an untrusted PowerShell wrapper",
        ),
    ];

    for (outcome, reason) in cases {
        let Some(powershell_policy::PreparedPowerShell::Terminal(requirement)) =
            powershell_policy::prepare_classified(
                &command,
                PowerShellExecPolicyParse::UntrustedRuntime { outcome },
            )
        else {
            panic!("untrusted opaque, failed, and empty states must remain terminal");
        };
        pretty_assertions::assert_eq!(
            requirement,
            ExecApprovalRequirement::Forbidden {
                reason: format!("`{rendered}` rejected: {reason}"),
            },
        );
    }
}

#[test]
fn untrusted_outer_and_every_inner_command_compose_with_strictest_wins() {
    let outer = vec_str(&[r"C:\workspace\powershell.exe", "-Command", "echo allowed"]);
    let rendered = render_shlex_command(&outer);
    let echo = vec_str(&["echo", "allowed"]);
    let later = vec_str(&["madeup-later", "value"]);
    let full_outer_allow = prefix_rule_for(&outer, "allow");
    let short_outer_allow = prefix_rule_for(&outer[..1], "allow");
    let outer_prompt = prefix_rule_for(&outer, "prompt");
    let outer_forbidden = prefix_rule_for(&outer, "forbidden");
    let echo_allow = prefix_rule_for(&echo[..1], "allow");
    let echo_prompt = prefix_rule_for(&echo[..1], "prompt");
    let echo_forbidden = prefix_rule_for(&echo[..1], "forbidden");
    let later_prompt = prefix_rule_for(&later[..1], "prompt");
    let later_forbidden = prefix_rule_for(&later[..1], "forbidden");
    let policy_prompt_reason = Some(format!("`{rendered}` requires approval by policy"));

    let cases = vec![
        (
            "complete outer and every inner explicit allow",
            format!("{full_outer_allow}\n{echo_allow}"),
            vec![echo.clone()],
            untrusted_skip(true),
        ),
        (
            "short outer allow remains sandbox preserving",
            format!("{short_outer_allow}\n{echo_allow}"),
            vec![echo.clone()],
            untrusted_skip(false),
        ),
        (
            "heuristic inner allow cannot establish full authority",
            full_outer_allow.clone(),
            vec![vec_str(&["Get-Location"])],
            untrusted_skip(false),
        ),
        (
            "inner prompt dominates outer allow",
            format!("{full_outer_allow}\n{echo_prompt}"),
            vec![echo.clone()],
            one_shot(policy_prompt_reason.clone()),
        ),
        (
            "outer prompt dominates inner allow",
            format!("{outer_prompt}\n{echo_allow}"),
            vec![echo.clone()],
            one_shot(policy_prompt_reason.clone()),
        ),
        (
            "unmatched outer prompts even with inner allow",
            echo_allow.clone(),
            vec![echo.clone()],
            one_shot(None),
        ),
        (
            "outer forbidden is terminal",
            format!("{outer_forbidden}\n{echo_allow}"),
            vec![echo.clone()],
            ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{rendered}` rejected: policy forbids commands starting with `{rendered}`"
                ),
            },
        ),
        (
            "inner forbidden is terminal",
            format!("{full_outer_allow}\n{echo_forbidden}"),
            vec![echo.clone()],
            ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{rendered}` rejected: policy forbids commands starting with `echo`"
                ),
            },
        ),
        (
            "later statement prompt participates",
            format!("{full_outer_allow}\n{echo_allow}\n{later_prompt}"),
            vec![echo.clone(), later.clone()],
            one_shot(policy_prompt_reason),
        ),
        (
            "later statement forbidden participates",
            format!("{full_outer_allow}\n{echo_allow}\n{later_forbidden}"),
            vec![echo, later],
            ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{rendered}` rejected: policy forbids commands starting with `madeup-later`"
                ),
            },
        ),
        (
            "nested wrapper does not inherit outer authority",
            full_outer_allow,
            vec![vec_str(&["powershell.exe", "-Command", "Get-Location"])],
            untrusted_skip(false),
        ),
    ];

    for (name, policy_src, commands, expected) in cases {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&policy_src),
                &outer,
                &commands,
                AskForApproval::OnRequest,
                &PermissionProfile::read_only(),
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::UseDefault,
            ),
            expected,
            "{name}",
        );
    }
}

#[test]
fn untrusted_unmatched_outer_and_mixed_prompt_causes_follow_approval_policy() {
    let outer = untrusted_powershell_command("echo allowed");
    let echo = vec_str(&["echo", "allowed"]);
    let echo_allow = prefix_rule_for(&echo[..1], "allow");

    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&echo_allow),
            &outer,
            std::slice::from_ref(&echo),
            AskForApproval::Never,
            &PermissionProfile::Disabled,
            WindowsSandboxLevel::RestrictedToken,
            SandboxPermissions::UseDefault,
        ),
        ExecApprovalRequirement::Forbidden {
            reason: format!(
                "`{}` rejected: blocked by policy",
                render_shlex_command(&outer)
            ),
        },
    );
    for approval_policy in [AskForApproval::OnRequest, AskForApproval::UnlessTrusted] {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&echo_allow),
                &outer,
                std::slice::from_ref(&echo),
                approval_policy,
                &PermissionProfile::Disabled,
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::UseDefault,
            ),
            one_shot(None),
            "{approval_policy:?}",
        );
    }

    let outer_prompt = prefix_rule_for(&outer, "prompt");
    let mixed_policy = format!("{outer_prompt}\n{echo_allow}");
    for (rules, sandbox_approval, expected) in [
        (
            false,
            false,
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_RULES_APPROVAL_REASON.to_string(),
            },
        ),
        (
            false,
            true,
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_RULES_APPROVAL_REASON.to_string(),
            },
        ),
        (
            true,
            false,
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
            },
        ),
        (
            true,
            true,
            one_shot(Some(format!(
                "`{}` requires approval by policy",
                render_shlex_command(&outer)
            ))),
        ),
    ] {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&mixed_policy),
                &outer,
                std::slice::from_ref(&echo),
                granular(rules, sandbox_approval),
                &PermissionProfile::read_only(),
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::RequireEscalated,
            ),
            expected,
            "rules={rules}, sandbox_approval={sandbox_approval}",
        );
    }

    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&mixed_policy),
            &outer,
            std::slice::from_ref(&echo),
            granular(/*rules*/ true, /*sandbox_approval*/ false),
            &PermissionProfile::read_only(),
            WindowsSandboxLevel::RestrictedToken,
            SandboxPermissions::UseDefault,
        ),
        one_shot(Some(format!(
            "`{}` requires approval by policy",
            render_shlex_command(&outer)
        ))),
        "a rule-only prompt does not require sandbox approval",
    );

    let full_outer_allow = prefix_rule_for(&outer, "allow");
    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&full_outer_allow),
            &outer,
            std::slice::from_ref(&echo),
            granular(/*rules*/ false, /*sandbox_approval*/ true),
            &PermissionProfile::read_only(),
            WindowsSandboxLevel::RestrictedToken,
            SandboxPermissions::RequireEscalated,
        ),
        one_shot(None),
        "a sandbox-only prompt does not require rules approval",
    );
}

#[test]
fn untrusted_permission_and_windows_backend_gates_require_composed_authority() {
    use SandboxPermissions as SP;
    use WindowsSandboxLevel as WSL;

    let outer = absolute_untrusted_powershell_command("Get-Content Cargo.toml");
    let inner = vec_str(&["Get-Content", "Cargo.toml"]);
    let partial_policy = format!(
        "{}\n{}",
        prefix_rule_for(&outer[..1], "allow"),
        prefix_rule_for(&inner[..1], "allow")
    );
    let full_policy = format!(
        "{}\n{}",
        prefix_rule_for(&outer, "allow"),
        prefix_rule_for(&inner[..1], "allow")
    );
    let denied_read_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: FileSystemAccessMode::Deny,
        }]),
        NetworkSandboxPolicy::Restricted,
    );
    let external_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };

    for (name, profile, level, permissions, prompts) in [
        (
            "managed default",
            PermissionProfile::read_only(),
            WSL::RestrictedToken,
            SP::UseDefault,
            false,
        ),
        (
            "managed escalation",
            PermissionProfile::read_only(),
            WSL::RestrictedToken,
            SP::RequireEscalated,
            true,
        ),
        (
            "additional permissions",
            PermissionProfile::read_only(),
            WSL::RestrictedToken,
            SP::WithAdditionalPermissions,
            true,
        ),
        (
            "missing managed backend",
            PermissionProfile::read_only(),
            WSL::Disabled,
            SP::UseDefault,
            true,
        ),
        (
            "denied-read escalation",
            denied_read_profile,
            WSL::RestrictedToken,
            SP::RequireEscalated,
            false,
        ),
        (
            "disabled profile",
            PermissionProfile::Disabled,
            WSL::Disabled,
            SP::RequireEscalated,
            true,
        ),
        (
            "external default",
            external_profile.clone(),
            WSL::RestrictedToken,
            SP::UseDefault,
            false,
        ),
        (
            "external escalation",
            external_profile,
            WSL::RestrictedToken,
            SP::RequireEscalated,
            true,
        ),
    ] {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&partial_policy),
                &outer,
                std::slice::from_ref(&inner),
                AskForApproval::OnRequest,
                &profile,
                level,
                permissions,
            ),
            if prompts {
                one_shot(None)
            } else {
                untrusted_skip(false)
            },
            "{name}",
        );
    }

    for (level, permissions) in [
        (WSL::RestrictedToken, SP::RequireEscalated),
        (WSL::RestrictedToken, SP::WithAdditionalPermissions),
        (WSL::Disabled, SP::UseDefault),
    ] {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&full_policy),
                &outer,
                std::slice::from_ref(&inner),
                AskForApproval::OnRequest,
                &PermissionProfile::read_only(),
                level,
                permissions,
            ),
            untrusted_skip(true),
            "composed authority should cover {level:?} and {permissions:?}",
        );
    }
}

#[test]
fn untrusted_without_filesystem_containment_requires_complete_composed_authority() {
    let outer = absolute_untrusted_powershell_command("Get-Location");
    let inner = vec_str(&["Get-Location"]);
    let partial_policy = format!(
        "{}\n{}",
        prefix_rule_for(&outer[..1], "allow"),
        prefix_rule_for(&inner, "allow")
    );
    let full_policy = format!(
        "{}\n{}",
        prefix_rule_for(&outer, "allow"),
        prefix_rule_for(&inner, "allow")
    );
    let disabled = PermissionProfile::Disabled;

    for (name, approval_policy, expected) in [
        (
            "on request",
            AskForApproval::OnRequest,
            one_shot(/*reason*/ None),
        ),
        (
            "unless trusted",
            AskForApproval::UnlessTrusted,
            one_shot(/*reason*/ None),
        ),
        (
            "never",
            AskForApproval::Never,
            ExecApprovalRequirement::Forbidden {
                reason: PROMPT_CONFLICT_REASON.to_string(),
            },
        ),
        (
            "granular sandbox approval disabled",
            granular(/*rules*/ false, /*sandbox_approval*/ false),
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
            },
        ),
        (
            "granular sandbox approval enabled",
            granular(/*rules*/ false, /*sandbox_approval*/ true),
            one_shot(/*reason*/ None),
        ),
    ] {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&partial_policy),
                &outer,
                std::slice::from_ref(&inner),
                approval_policy,
                &disabled,
                WindowsSandboxLevel::Disabled,
                SandboxPermissions::UseDefault,
            ),
            expected,
            "{name}",
        );
    }

    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&prefix_rule_for(&outer, "allow")),
            &outer,
            std::slice::from_ref(&inner),
            AskForApproval::OnRequest,
            &disabled,
            WindowsSandboxLevel::Disabled,
            SandboxPermissions::UseDefault,
        ),
        one_shot(/*reason*/ None),
        "a heuristic-safe inner command is not explicit authority without a sandbox",
    );

    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&full_policy),
            &outer,
            std::slice::from_ref(&inner),
            AskForApproval::OnRequest,
            &disabled,
            WindowsSandboxLevel::Disabled,
            SandboxPermissions::UseDefault,
        ),
        untrusted_skip(/*bypass_sandbox*/ true),
        "complete composed authority remains sufficient without a sandbox",
    );

    let bare_outer = untrusted_powershell_command("Get-Location");
    let bare_full_policy = format!(
        "{}\n{}",
        prefix_rule_for(&bare_outer, "allow"),
        prefix_rule_for(&inner, "allow")
    );
    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&bare_full_policy),
            &bare_outer,
            std::slice::from_ref(&inner),
            AskForApproval::OnRequest,
            &disabled,
            WindowsSandboxLevel::Disabled,
            SandboxPermissions::UseDefault,
        ),
        one_shot(/*reason*/ None),
        "a bare executable spelling cannot establish exact outer authority",
    );

    let managed_unrestricted = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Enabled,
    );
    let managed_full_disk_write = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        }]),
        NetworkSandboxPolicy::Enabled,
    );
    for (name, profile) in [
        ("managed unrestricted", managed_unrestricted),
        ("managed full-disk write", managed_full_disk_write),
    ] {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&partial_policy),
                &outer,
                std::slice::from_ref(&inner),
                AskForApproval::OnRequest,
                &profile,
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::UseDefault,
            ),
            one_shot(/*reason*/ None),
            "partial authority must prompt for {name}",
        );
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&full_policy),
                &outer,
                std::slice::from_ref(&inner),
                AskForApproval::OnRequest,
                &profile,
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::UseDefault,
            ),
            untrusted_skip(/*bypass_sandbox*/ true),
            "complete authority remains sufficient for {name}",
        );
    }

    pretty_assertions::assert_eq!(
        composed_untrusted_requirement(
            Some(&partial_policy),
            &outer,
            std::slice::from_ref(&inner),
            AskForApproval::OnRequest,
            &PermissionProfile::External {
                network: NetworkSandboxPolicy::Restricted,
            },
            WindowsSandboxLevel::RestrictedToken,
            SandboxPermissions::UseDefault,
        ),
        untrusted_skip(/*bypass_sandbox*/ false),
        "an externally enforced sandbox keeps existing partial-authority behavior",
    );
}

#[test]
fn untrusted_wrapper_identity_uses_exact_outer_and_restrictive_basename_rules() {
    let path_a = vec_str(&[r"C:\workspace-a\powershell.exe", "-Command", "echo allowed"]);
    let path_b = vec_str(&[r"C:\workspace-b\powershell.exe", "-Command", "echo allowed"]);
    let inner = vec_str(&["echo", "allowed"]);
    let inner_allow = prefix_rule_for(&inner[..1], "allow");
    let basename_allow = prefix_rule_for(&vec_str(&["powershell.exe"]), "allow");
    let exact_a_allow = prefix_rule_for(&path_a, "allow");
    let basename_prompt = prefix_rule_for(&vec_str(&["powershell.exe"]), "prompt");
    let basename_forbidden = prefix_rule_for(&vec_str(&["powershell.exe"]), "forbidden");
    let mut extended = path_a.clone();
    extended.insert(1, "-NoProfile".to_string());
    let namespace_outer = vec_str(&[
        r"\\?\C:\attacker\powershell.exe.",
        "-Command",
        "echo allowed",
    ]);
    let namespace_policy = format!(
        "{}\nhost_executable(name = \"powershell\", paths = [\"C:\\\\trusted\\\\powershell.exe\"])\n{inner_allow}",
        prefix_rule_for(&vec_str(&["powershell.exe."]), "allow")
    );
    let rendered_a = render_shlex_command(&path_a);
    let cases = vec![
        (
            "basename Allow",
            path_a.clone(),
            format!("{basename_allow}\n{inner_allow}"),
            one_shot(None),
        ),
        (
            "path A rule against B",
            path_b,
            format!("{exact_a_allow}\n{inner_allow}"),
            one_shot(None),
        ),
        (
            "basename Prompt",
            path_a.clone(),
            format!("{exact_a_allow}\n{basename_prompt}\n{inner_allow}"),
            one_shot(Some(format!("`{rendered_a}` requires approval by policy"))),
        ),
        (
            "basename Forbidden",
            path_a,
            format!("{exact_a_allow}\n{basename_forbidden}\n{inner_allow}"),
            ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{rendered_a}` rejected: policy forbids commands starting with `powershell.exe`"
                ),
            },
        ),
        (
            "extended argv",
            extended,
            format!("{exact_a_allow}\n{inner_allow}"),
            one_shot(None),
        ),
        (
            "mapped namespace",
            namespace_outer,
            namespace_policy,
            one_shot(None),
        ),
    ];

    for (name, outer, policy_src, expected) in cases {
        pretty_assertions::assert_eq!(
            composed_untrusted_requirement(
                Some(&policy_src),
                &outer,
                std::slice::from_ref(&inner),
                AskForApproval::OnRequest,
                &PermissionProfile::read_only(),
                WindowsSandboxLevel::RestrictedToken,
                SandboxPermissions::UseDefault,
            ),
            expected,
            "{name}",
        );
    }
}

#[tokio::test]
async fn local_model_resolved_trusted_powershell_composes_exact_outer_and_inner() {
    let outer = powershell_command("Get-Location");
    let inner = vec_str(&["Get-Location"]);
    let full_outer_allow = prefix_rule_for(&outer, "allow");
    let short_outer_allow = prefix_rule_for(&outer[..1], "allow");
    let inner_allow = prefix_rule_for(&inner, "allow");

    pretty_assertions::assert_eq!(
        requirement_with_provenance(
            Some(&format!("{full_outer_allow}\n{inner_allow}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        untrusted_skip(/*bypass_sandbox*/ true),
        "exact outer and explicit inner authority should authorize the parsed composition",
    );

    pretty_assertions::assert_eq!(
        requirement_with_provenance(
            Some(&format!("{short_outer_allow}\n{inner_allow}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        untrusted_skip(/*bypass_sandbox*/ false),
        "partial outer authority may proceed only inside the managed sandbox",
    );

    let inner_forbidden = prefix_rule_for(&inner, "forbidden");
    pretty_assertions::assert_eq!(
        requirement_with_provenance(
            Some(&format!("{full_outer_allow}\n{inner_forbidden}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: format!(
                "`{}` rejected: policy forbids commands starting with `Get-Location`",
                render_shlex_command(&outer)
            ),
        },
        "an inner forbidden rule must dominate exact outer authority",
    );
}

#[tokio::test]
async fn untrusted_parsed_results_ignore_requested_amendments() {
    let command = absolute_untrusted_powershell_command("echo allowed");
    let inner_allow = prefix_rule_for(&vec_str(&["echo"]), "allow");
    let requested_prefix = Some(vec_str(&["echo"]));

    pretty_assertions::assert_eq!(
        requirement_with_options(
            /*policy_src*/ None,
            &command,
            AskForApproval::OnRequest,
            PermissionProfile::read_only(),
            WindowsSandboxLevel::RestrictedToken,
            SandboxPermissions::UseDefault,
            requested_prefix.clone(),
        )
        .await,
        one_shot(None),
    );

    let full_policy = format!("{}\n{inner_allow}", prefix_rule_for(&command, "allow"));
    pretty_assertions::assert_eq!(
        requirement_with_options(
            Some(&full_policy),
            &command,
            AskForApproval::OnRequest,
            PermissionProfile::read_only(),
            WindowsSandboxLevel::RestrictedToken,
            SandboxPermissions::UseDefault,
            requested_prefix,
        )
        .await,
        untrusted_skip(true),
    );
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
        "[Codex.DoesNotExist, C:/workspace/Codex.AttackerAssembly]",
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
async fn sandbox_override_on_trusted_unsupported_script_does_not_offer_amendment() {
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
                ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    proposed_execpolicy_amendment: None,
                }
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
