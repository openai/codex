use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::MODEL_SPECIALTY_CYBER;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_host_windows;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::zsh_fork::zsh_fork_runtime;
use core_test_support::zsh_fork::zsh_fork_test_builder;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use test_case::test_case;

const TEST_COMMAND: &str = "git version";
const SAVED_PREFIX: &str = r#"["git", "version"]"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandTool {
    ShellCommand,
    UnifiedExec,
}

impl CommandTool {
    fn configure(self, config: &mut Config) {
        if self == Self::UnifiedExec {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("enable unified exec");
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModelSpecialty {
    Cyber,
    General,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellBackend {
    Standard,
    ZshFork,
}

fn configure_saved_prefix_and_guardian(config: &mut Config) {
    let policy_path = config.codex_home.join("rules/default.rules");
    fs::create_dir_all(policy_path.parent().expect("rules directory"))
        .expect("create rules directory");
    fs::write(
        policy_path,
        r#"prefix_rule(pattern=["git", "version"], decision="allow")"#,
    )
    .expect("write saved command prefix rule");
    config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    config
        .set_legacy_sandbox_policy(SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        })
        .expect("set workspace sandbox policy");
}

fn command_response(response_id: &str, call_id: &str, command_tool: CommandTool) -> Result<String> {
    let (tool_name, command_key, timeout_key) = match command_tool {
        CommandTool::ShellCommand => ("shell_command", "command", "timeout_ms"),
        CommandTool::UnifiedExec => ("exec_command", "cmd", "yield_time_ms"),
    };
    let mut args = json!({
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Check whether a saved prefix bypasses Guardian.",
        "prefix_rule": ["git", "version"],
    });
    args[command_key] = json!(TEST_COMMAND);
    args[timeout_key] = json!(1_000);

    Ok(sse(vec![
        ev_response_created(response_id),
        ev_function_call(call_id, tool_name, &serde_json::to_string(&args)?),
        ev_completed(response_id),
    ]))
}

fn guardian_allow_response(response_id: &str) -> String {
    sse(vec![
        ev_response_created(response_id),
        ev_assistant_message(&format!("{response_id}-message"), r#"{"outcome":"allow"}"#),
        ev_completed(response_id),
    ])
}

async fn submit_model_turn(test: &TestCodex, model: &str, prompt: &str) -> Result<()> {
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(model.to_string()),
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn(prompt).await
}

#[test_case(CommandTool::ShellCommand, ModelSpecialty::Cyber, ShellBackend::Standard; "cyber shell command is reviewed")]
#[test_case(CommandTool::UnifiedExec, ModelSpecialty::Cyber, ShellBackend::Standard; "cyber unified exec is reviewed")]
#[test_case(CommandTool::ShellCommand, ModelSpecialty::Cyber, ShellBackend::ZshFork; "cyber zsh shell command is reviewed")]
#[test_case(CommandTool::UnifiedExec, ModelSpecialty::Cyber, ShellBackend::ZshFork; "cyber zsh unified exec is reviewed")]
#[test_case(CommandTool::ShellCommand, ModelSpecialty::General, ShellBackend::Standard; "general shell command keeps saved approval")]
#[test_case(CommandTool::UnifiedExec, ModelSpecialty::General, ShellBackend::Standard; "general unified exec keeps saved approval")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_prefix_only_bypasses_guardian_for_general_models(
    command_tool: CommandTool,
    model_specialty: ModelSpecialty,
    shell_backend: ShellBackend,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(Ok(()), "Guardian command reviews require host-native paths");

    let server = start_mock_server().await;
    let builder = match shell_backend {
        ShellBackend::Standard => test_codex(),
        ShellBackend::ZshFork => {
            skip_if_host_windows!(Ok(()));
            let Some(runtime) = zsh_fork_runtime("cyber model zsh-fork saved prefix")? else {
                return Ok(());
            };
            zsh_fork_test_builder(runtime, AskForApproval::OnRequest)
        }
    };
    let mut builder = builder
        .with_model_info_override("gpt-5.4", move |model| {
            if model_specialty == ModelSpecialty::Cyber {
                model.model_specialty = Some(MODEL_SPECIALTY_CYBER.to_string());
            }
        })
        .with_config(move |config| {
            configure_saved_prefix_and_guardian(config);
            command_tool.configure(config);
            if shell_backend == ShellBackend::ZshFork && command_tool == CommandTool::UnifiedExec {
                config
                    .features
                    .enable(Feature::UnifiedExecZshFork)
                    .expect("enable unified-exec zsh fork");
            }
        });
    let test = builder.build_with_auto_env(&server).await?;
    let expected_guardian_review_count = match (model_specialty, shell_backend) {
        (ModelSpecialty::General, _) => 0,
        (ModelSpecialty::Cyber, ShellBackend::Standard) => 1,
        (ModelSpecialty::Cyber, ShellBackend::ZshFork) => 2,
    };

    let mut response_bodies = vec![command_response(
        "parent-saved-prefix-command",
        "saved-prefix-command",
        command_tool,
    )?];
    for review_index in 0..expected_guardian_review_count {
        response_bodies.push(guardian_allow_response(&format!(
            "guardian-saved-prefix-review-{review_index}"
        )));
    }
    response_bodies.push(sse_completed("parent-saved-prefix-complete"));
    let responses = mount_sse_sequence(&server, response_bodies).await;

    submit_model_turn(&test, "gpt-5.4", "run the saved-prefix command").await?;

    let requests = responses.requests();
    let guardian_request_count = requests
        .iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .count();
    assert_eq!(guardian_request_count, expected_guardian_review_count);

    let advertised_saved_prefix = requests[0]
        .message_input_texts("developer")
        .iter()
        .any(|message| message.contains(SAVED_PREFIX));
    assert_eq!(
        advertised_saved_prefix,
        model_specialty == ModelSpecialty::General,
    );
    assert!(
        responses
            .function_call_output_text("saved-prefix-command")
            .is_some_and(|output| output.contains(TEST_COMMAND)),
        "approved test command should execute",
    );

    Ok(())
}

#[test_case(CommandTool::ShellCommand; "shell command")]
#[test_case(CommandTool::UnifiedExec; "unified exec")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_model_user_approval_never_offers_a_reusable_prefix(
    command_tool: CommandTool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command approval requires host-native paths");

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.model_specialty = Some(MODEL_SPECIALTY_CYBER.to_string());
        })
        .with_config(move |config| {
            configure_saved_prefix_and_guardian(config);
            command_tool.configure(config);
            config.approvals_reviewer = ApprovalsReviewer::User;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let policy_path = test.codex_home_path().join("rules/default.rules");
    let initial_policy = fs::read_to_string(&policy_path)?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            command_response(
                "parent-one-time-approval",
                "one-time-approval",
                command_tool,
            )?,
            sse_completed("parent-one-time-complete"),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run a command with one-time approval".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                ..Default::default()
            }),
        )
        .await?;

    let EventMsg::ExecApprovalRequest(approval) = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await
    else {
        panic!("expected a one-time command approval request");
    };
    assert_eq!(approval.proposed_execpolicy_amendment, None);
    assert_eq!(
        approval.effective_available_decisions(),
        vec![ReviewDecision::Approved, ReviewDecision::Abort],
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert!(
        responses
            .function_call_output_text("one-time-approval")
            .is_some_and(|output| output.contains(TEST_COMMAND)),
    );
    assert_eq!(fs::read_to_string(policy_path)?, initial_policy);
    assert_eq!(responses.requests().len(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switching_models_suppresses_and_restores_saved_prefix_approvals() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(Ok(()), "Guardian command reviews require host-native paths");

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.model_specialty = Some(MODEL_SPECIALTY_CYBER.to_string());
        })
        .with_model("gpt-5.2")
        .with_config(configure_saved_prefix_and_guardian);
    let test = builder.build_with_auto_env(&server).await?;

    let responses = mount_sse_sequence(
        &server,
        vec![
            command_response(
                "parent-general-first-command",
                "general-first-command",
                CommandTool::ShellCommand,
            )?,
            sse_completed("parent-general-first-complete"),
            command_response(
                "parent-cyber-command",
                "cyber-command",
                CommandTool::ShellCommand,
            )?,
            guardian_allow_response("guardian-cyber-review"),
            sse_completed("parent-cyber-complete"),
            command_response(
                "parent-general-last-command",
                "general-last-command",
                CommandTool::ShellCommand,
            )?,
            sse_completed("parent-general-last-complete"),
        ],
    )
    .await;

    submit_model_turn(&test, "gpt-5.2", "run the first general-model command").await?;
    submit_model_turn(&test, "gpt-5.4", "run the cyber-model command").await?;
    submit_model_turn(&test, "gpt-5.2", "run the final general-model command").await?;

    let requests = responses.requests();
    let guardian_requests = requests
        .iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 1);
    assert!(guardian_requests[0].body_contains_text(TEST_COMMAND));

    let cyber_request = requests
        .iter()
        .find(|request| request.body_json()["model"] == "gpt-5.4")
        .expect("cyber-model request");
    let cyber_developer_messages = cyber_request.message_input_texts("developer");
    let cyber_permissions = cyber_developer_messages
        .iter()
        .rev()
        .find(|message| message.contains("<permissions instructions>"))
        .expect("refreshed cyber-model permissions instructions");
    assert!(!cyber_permissions.contains(SAVED_PREFIX));

    let final_parent_request = requests
        .iter()
        .rev()
        .find(|request| request.body_json()["model"] == "gpt-5.2")
        .expect("final general-model request");
    assert!(
        final_parent_request
            .message_input_texts("developer")
            .iter()
            .any(|message| {
                message.contains("Approved command prefix saved:") && message.contains(SAVED_PREFIX)
            }),
        "switching back should restore the original saved-prefix context",
    );

    for call_id in [
        "general-first-command",
        "cyber-command",
        "general-last-command",
    ] {
        assert!(
            responses
                .function_call_output_text(call_id)
                .is_some_and(|output| output.contains(TEST_COMMAND)),
            "approved {call_id} should execute",
        );
    }

    Ok(())
}
