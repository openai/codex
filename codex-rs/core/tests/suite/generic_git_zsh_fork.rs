use anyhow::Result;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
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
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::zsh_fork::build_unified_exec_zsh_fork_test;
use core_test_support::zsh_fork::build_zsh_fork_test;
use core_test_support::zsh_fork::zsh_fork_runtime;
use serde_json::json;

#[derive(Clone, Copy, Debug)]
enum AgentExecTool {
    Shell,
    UnifiedExec,
}

async fn assert_generic_git_uses_one_parent_approval(tool: AgentExecTool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let Some(runtime) = zsh_fork_runtime("generic Git single approval test")? else {
        return Ok(());
    };
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::UnlessTrusted;
    let permission_profile = PermissionProfile::workspace_write();
    let test = match tool {
        AgentExecTool::Shell => {
            build_zsh_fork_test(
                &server,
                runtime,
                approval_policy,
                permission_profile,
                |_home| {},
            )
            .await?
        }
        AgentExecTool::UnifiedExec => {
            build_unified_exec_zsh_fork_test(
                &server,
                runtime,
                approval_policy,
                permission_profile,
                |_home| {},
            )
            .await?
        }
    };
    let call_id = match tool {
        AgentExecTool::Shell => "generic-git-zsh-fork-shell",
        AgentExecTool::UnifiedExec => "generic-git-zsh-fork-unified-exec",
    };
    let (tool_name, args) = match tool {
        AgentExecTool::Shell => (
            "shell_command",
            json!({
                "command": "git status --short",
                "timeout_ms": 30_000,
            }),
        ),
        AgentExecTool::UnifiedExec => (
            "exec_command",
            json!({
                "cmd": "git status --short",
                "yield_time_ms": 30_000,
            }),
        ),
    };
    let _first_response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-generic-git-zsh-fork-1"),
            ev_function_call(call_id, tool_name, &serde_json::to_string(&args)?),
            ev_completed("resp-generic-git-zsh-fork-1"),
        ]),
    )
    .await;
    let _results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-generic-git-zsh-fork-1", "done"),
            ev_completed("resp-generic-git-zsh-fork-2"),
        ]),
    )
    .await;

    submit_turn(&test, approval_policy).await?;
    let approval = expect_approval_or_completion(&test).await;
    let EventMsg::ExecApprovalRequest(approval) = approval else {
        panic!("{tool:?} generic Git completed without parent approval");
    };
    assert_eq!(
        approval.approval_id, None,
        "{tool:?} first approval must be the parent tool command"
    );
    assert_eq!(approval.proposed_execpolicy_amendment, None);
    assert!(
        approval
            .command
            .iter()
            .any(|argument| argument.contains("git status --short")),
        "unexpected {tool:?} parent approval command: {:?}",
        approval.command
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;

    match expect_approval_or_completion(&test).await {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(approval) => panic!(
            "{tool:?} emitted a redundant child approval after parent approval: {:?}",
            approval.command
        ),
        event => panic!("unexpected {tool:?} event: {event:?}"),
    }

    Ok(())
}

async fn submit_turn(test: &TestCodex, approval_policy: AskForApproval) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) = turn_permission_fields(
        test.session_configured.permission_profile.clone(),
        test.cwd.path(),
    );
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run repository-sensitive Git through zsh fork".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;
    Ok(())
}

async fn expect_approval_or_completion(test: &TestCodex) -> EventMsg {
    wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_generic_git_uses_one_parent_approval() -> Result<()> {
    assert_generic_git_uses_one_parent_approval(AgentExecTool::Shell).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_generic_git_uses_one_parent_approval() -> Result<()> {
    assert_generic_git_uses_one_parent_approval(AgentExecTool::UnifiedExec).await
}
