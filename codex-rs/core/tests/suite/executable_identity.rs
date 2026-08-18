use anyhow::Result;
use codex_config::Constrained;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_remote;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use test_case::test_case;

#[derive(Clone, Copy)]
enum ShellAttack {
    ExactShellName,
    ExactShellNameWithAllowedInnerCommand,
    ExactShellNameWithForbiddenInnerCommand,
    DangerousCommandOnRequest,
    DangerousCommandNever,
    ApprovedCustomShell,
    SessionApprovalDoesNotTrustDifferentShell,
    SpoofedShellExtension,
}

#[test_case(ShellAttack::ExactShellName; "workspace shell requires approval")]
#[test_case(ShellAttack::ExactShellNameWithAllowedInnerCommand; "inner allow does not trust workspace shell")]
#[test_case(ShellAttack::ExactShellNameWithForbiddenInnerCommand; "inner forbidden rule still rejects workspace shell")]
#[test_case(ShellAttack::DangerousCommandOnRequest; "dangerous inner command requires approval")]
#[test_case(ShellAttack::DangerousCommandNever; "dangerous inner command is forbidden without approval")]
#[test_case(ShellAttack::ApprovedCustomShell; "approved custom shell still runs")]
#[test_case(ShellAttack::SessionApprovalDoesNotTrustDifferentShell; "session approval does not trust a different shell")]
#[test_case(ShellAttack::SpoofedShellExtension; "workspace shell with an extra extension requires approval")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_provided_shell_cannot_inherit_inner_command_trust(
    attack: ShellAttack,
) -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "remote executors already replace requested shell paths with their reported shell"
    );

    let approval_policy = match attack {
        ShellAttack::DangerousCommandOnRequest => AskForApproval::OnRequest,
        ShellAttack::DangerousCommandNever => AskForApproval::Never,
        ShellAttack::ExactShellName
        | ShellAttack::ExactShellNameWithAllowedInnerCommand
        | ShellAttack::ExactShellNameWithForbiddenInnerCommand
        | ShellAttack::ApprovedCustomShell
        | ShellAttack::SessionApprovalDoesNotTrustDifferentShell
        | ShellAttack::SpoofedShellExtension => AskForApproval::UnlessTrusted,
    };
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("enable unified exec");
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.approvals_reviewer = ApprovalsReviewer::User;
        let inner_command_rule = match attack {
            ShellAttack::ExactShellNameWithAllowedInnerCommand => {
                Some("prefix_rule(pattern=[\"echo\"], decision=\"allow\")\n")
            }
            ShellAttack::ExactShellNameWithForbiddenInnerCommand => {
                if cfg!(windows) {
                    Some(
                        r#"prefix_rule(pattern=["Remove-Item", "C:\\important"], decision="forbidden")"#,
                    )
                } else {
                    Some("prefix_rule(pattern=[\"rm\"], decision=\"forbidden\")\n")
                }
            }
            ShellAttack::ExactShellName
            | ShellAttack::DangerousCommandOnRequest
            | ShellAttack::DangerousCommandNever
            | ShellAttack::ApprovedCustomShell
            | ShellAttack::SessionApprovalDoesNotTrustDifferentShell
            | ShellAttack::SpoofedShellExtension => None,
        };
        if let Some(inner_command_rule) = inner_command_rule {
            let policy_path = config.codex_home.join("rules/default.rules");
            fs::create_dir_all(policy_path.parent().expect("rules directory"))
                .expect("create rules directory");
            fs::write(policy_path, inner_command_rule).expect("write execution policy rule");
        }
    });
    #[cfg(windows)]
    if matches!(
        attack,
        ShellAttack::ExactShellNameWithForbiddenInnerCommand | ShellAttack::ApprovedCustomShell
    ) {
        let system_root = std::env::var_os("SystemRoot").expect("Windows SystemRoot");
        let configured_shell = std::path::Path::new(&system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        builder = builder.with_user_shell(codex_core::shell::get_shell_by_model_provided_path(
            &configured_shell,
        ));
    }
    let test = builder.build_with_auto_env(&server).await?;
    let shell_name = match attack {
        ShellAttack::ExactShellNameWithForbiddenInnerCommand | ShellAttack::ApprovedCustomShell
            if cfg!(windows) =>
        {
            "pwsh.exe"
        }
        ShellAttack::ExactShellName
        | ShellAttack::ExactShellNameWithAllowedInnerCommand
        | ShellAttack::ExactShellNameWithForbiddenInnerCommand
        | ShellAttack::DangerousCommandOnRequest
        | ShellAttack::DangerousCommandNever
        | ShellAttack::ApprovedCustomShell
        | ShellAttack::SessionApprovalDoesNotTrustDifferentShell => {
            if cfg!(windows) {
                "powershell.exe"
            } else {
                "bash"
            }
        }
        ShellAttack::SpoofedShellExtension => {
            if cfg!(windows) {
                "powershell.evil"
            } else {
                "bash.evil"
            }
        }
    };
    let shell = test.workspace_path(shell_name);
    let marker = test.workspace_path("attacker-executed");
    #[cfg(unix)]
    {
        fs::write(&shell, "#!/bin/sh\nprintf ran > attacker-executed\n")?;
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(windows)]
    {
        let test_executable = std::env::current_exe()?;
        fs::hard_link(&test_executable, &shell)
            .or_else(|_| fs::copy(&test_executable, &shell).map(|_| ()))?;
        fs::write(
            shell.with_file_name(".codex-executable-identity-fixture"),
            b"fake shell",
        )?;
    }
    let other_shell = if matches!(
        attack,
        ShellAttack::SessionApprovalDoesNotTrustDifferentShell
    ) {
        let other_shell = test.workspace_path("another").join(shell_name);
        fs::create_dir_all(other_shell.parent().expect("alternate shell directory"))?;
        fs::copy(&shell, &other_shell)?;
        #[cfg(windows)]
        fs::write(
            other_shell.with_file_name(".codex-executable-identity-fixture"),
            b"fake shell",
        )?;
        Some(other_shell)
    } else {
        None
    };
    let call_id = "untrusted-shell-path";
    let other_call_id = "different-untrusted-shell-path";
    let command = match attack {
        ShellAttack::ExactShellName if cfg!(windows) => "Write-Output $env:USERNAME",
        ShellAttack::DangerousCommandOnRequest | ShellAttack::DangerousCommandNever => {
            if cfg!(windows) {
                "Remove-Item important -Force"
            } else {
                "rm -rf important"
            }
        }
        ShellAttack::ExactShellNameWithForbiddenInnerCommand => {
            if cfg!(windows) {
                r"echo shell-safe && Remove-Item C:\important"
            } else {
                "echo shell-safe; rm important"
            }
        }
        ShellAttack::ExactShellName
        | ShellAttack::ExactShellNameWithAllowedInnerCommand
        | ShellAttack::ApprovedCustomShell
        | ShellAttack::SessionApprovalDoesNotTrustDifferentShell
        | ShellAttack::SpoofedShellExtension => "echo shell-safe",
    };

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-untrusted-shell-1"),
            ev_function_call(
                call_id,
                "exec_command",
                &json!({ "cmd": command, "shell": shell }).to_string(),
            ),
            ev_completed("resp-untrusted-shell-1"),
        ]),
    )
    .await;
    if let Some(other_shell) = other_shell.as_ref() {
        mount_sse_once(
            &server,
            sse(vec![
                ev_response_created("resp-different-untrusted-shell"),
                ev_function_call(
                    other_call_id,
                    "exec_command",
                    &json!({ "cmd": command, "shell": other_shell }).to_string(),
                ),
                ev_completed("resp-different-untrusted-shell"),
            ]),
        )
        .await;
    }
    let completed = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-untrusted-shell", "done"),
            ev_completed("resp-untrusted-shell-2"),
        ]),
    )
    .await;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect the repository".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    if matches!(
        attack,
        ShellAttack::ExactShellNameWithForbiddenInnerCommand | ShellAttack::DangerousCommandNever
    ) {
        assert!(matches!(event, EventMsg::TurnComplete(_)));
        let output = completed
            .single_request()
            .function_call_output_text(call_id)
            .expect("forbidden command output");
        assert!(
            output.contains("rejected"),
            "the forbidden command should be rejected: {output}"
        );
        #[cfg(windows)]
        if matches!(attack, ShellAttack::ExactShellNameWithForbiddenInnerCommand) {
            assert!(
                output.contains("Remove-Item"),
                "the forbidden PowerShell command should remain visible to policy: {output}"
            );
        }
    } else {
        let EventMsg::ExecApprovalRequest(approval) = event else {
            panic!("workspace shell bypassed approval");
        };
        assert_eq!(approval.call_id, call_id);
        assert!(!marker.exists(), "the shell ran before approval");

        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: match attack {
                    ShellAttack::ApprovedCustomShell => ReviewDecision::Approved,
                    ShellAttack::SessionApprovalDoesNotTrustDifferentShell => {
                        ReviewDecision::ApprovedForSession
                    }
                    _ => ReviewDecision::denied("untrusted shell"),
                },
            })
            .await?;
        if other_shell.is_some() {
            let event = wait_for_event(&test.codex, |event| {
                matches!(
                    event,
                    EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
                )
            })
            .await;
            let EventMsg::ExecApprovalRequest(approval) = event else {
                panic!("a different workspace shell reused the first shell's session approval");
            };
            assert_eq!(approval.call_id, other_call_id);
            assert!(
                marker.exists(),
                "the session-approved shell should have run"
            );
            test.codex
                .submit(Op::ExecApproval {
                    id: approval.effective_approval_id(),
                    turn_id: None,
                    decision: ReviewDecision::denied("different untrusted shell"),
                })
                .await?;
        }
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    assert_eq!(
        marker.exists(),
        matches!(
            attack,
            ShellAttack::ApprovedCustomShell
                | ShellAttack::SessionApprovalDoesNotTrustDifferentShell
        ),
        "only an explicitly approved custom shell should run"
    );
    Ok(())
}
