//! Exercises interruption while a one-shot command's completion is waiting to reach history.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::TurnStatus;
use codex_config::Constrained;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::CommandStartInput;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::TurnLifecycleContributor;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::CommandExecutionStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::user_input::UserInput;
use codex_rollout::RolloutRecorder;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Notify;
use tokio::time::timeout;

const CALL_ID: &str = "gated-command";

#[derive(Default)]
struct PauseCommandCompletion {
    entered: Notify,
    release: Notify,
}

impl TurnLifecycleContributor for PauseCommandCompletion {
    fn on_item_completed<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a TurnItem,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let TurnItem::CommandExecution(command) = item
                && command.id == CALL_ID
            {
                // The start is already published, but completion has not reached persistence.
                self.entered.notify_one();
                self.release.notified().await;
            }
        })
    }
}

#[tokio::test]
async fn cancelling_approval_stops_running_sibling_and_preserves_completed_output() -> Result<()> {
    skip_if_no_network!(Ok(()));
    core_test_support::skip_if_target_windows!(Ok(()), "runs a POSIX sleep command");

    #[derive(Default)]
    struct SiblingResults {
        completed: Notify,
        release_first: Notify,
    }

    impl ToolLifecycleContributor for SiblingResults {
        fn on_command_start<'a>(&'a self, input: CommandStartInput<'a>) -> ToolLifecycleFuture<'a> {
            Box::pin(async move {
                if input.call_id == CALL_ID {
                    self.completed.notified().await;
                }
            })
        }

        fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
            Box::pin(async move {
                match input.call_id {
                    "sibling" => self.release_first.notified().await,
                    "finished" => {
                        assert_eq!(input.outcome, ToolCallOutcome::Completed { success: true });
                        self.completed.notify_one();
                    }
                    _ => {}
                }
            })
        }
    }

    let server = responses::start_mock_server().await;
    let gate = Arc::new(PauseCommandCompletion::default());
    let results = Arc::new(SiblingResults::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.turn_lifecycle_contributor(gate.clone());
    extensions.tool_lifecycle_contributor(results.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[features]\nunified_exec = false\nshell_tool = true\n",
            ),
        )
        .with_config(|config| config.approvals_reviewer = ApprovalsReviewer::User)
        .build_with_auto_env(&server)
        .await?;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("parallel"),
                responses::ev_exec_command_call_with_args(
                    "sibling",
                    &json!({"cmd": "sleep 120", "timeout_ms": 120_000}),
                ),
                responses::ev_exec_command_call_with_args(
                    "finished",
                    &json!({"cmd": "echo sibling-output", "login": false}),
                ),
                responses::ev_exec_command_call_with_args(
                    CALL_ID,
                    &json!({"cmd": "echo pending", "sandbox_permissions": "require_escalated",
                    "justification": "Run outside the sandbox?"}),
                ),
                responses::ev_completed("parallel"),
            ]),
            responses::sse(vec![responses::ev_completed("follow-up")]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run the commands".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(
                codex_protocol::protocol::ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::OnRequest),
                    permission_profile: Some(PermissionProfile::workspace_write()),
                    ..Default::default()
                },
            ),
        )
        .await?;
    let mut started = false;
    let mut prompt = None;
    while !started || prompt.is_none() {
        match test.codex.next_event().await?.msg {
            EventMsg::ExecCommandBegin(event) if event.call_id == "sibling" => {
                assert!(event.process_id.is_some());
                started = true;
            }
            EventMsg::ExecApprovalRequest(event) if event.call_id == CALL_ID => {
                prompt = Some(event)
            }
            _ => {}
        }
    }
    let prompt = prompt.unwrap();
    test.codex
        .submit(Op::ExecApproval {
            id: prompt.effective_approval_id(),
            turn_id: Some(prompt.turn_id),
            decision: ReviewDecision::Abort,
        })
        .await?;

    gate.entered.notified().await;
    // The sibling should stop even while history is waiting to record the decline.
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::ExecCommandEnd(end) if end.call_id == "sibling"),
    )
    .await;
    // Results drain in call order. Hold the first result and the decline beyond
    // the abort grace period, with the successful second result queued behind it.
    tokio::time::pause();
    tokio::time::sleep(Duration::from_secs(1)).await;
    tokio::time::resume();
    results.release_first.notify_one();
    gate.release.notify_one();
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ItemCompleted(completed)
            if matches!(&completed.item, TurnItem::CommandExecution(command)
                if command.id == CALL_ID && command.status == CommandExecutionStatus::Declined))
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    test.submit_text_turn("What did the finished command print?")
        .await?;
    let requests = requests.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1]
        .function_call_output_text("finished")
        .context("finished command output")?;
    assert!(output.contains("sibling-output"), "{output}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupted_one_shot_launch_failure_completes_and_persists() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let gate = Arc::new(PauseCommandCompletion::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.turn_lifecycle_contributor(gate.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[features]\nunified_exec = false\nshell_tool = true\n",
            ),
        )
        .with_config(|config| {
            config.features.disable(Feature::ShellSnapshot).unwrap();
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await?;
    let request = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("launch"),
            responses::ev_function_call(
                CALL_ID,
                "exec_command",
                &json!({
                    "cmd": "echo unreachable",
                    "workdir": "missing-work-directory",
                    "login": false,
                })
                .to_string(),
            ),
            responses::ev_completed("launch"),
        ]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the command.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    timeout(Duration::from_secs(/*secs*/ 30), gate.entered.notified()).await?;
    let mut events = Vec::new();
    test.codex.submit(Op::Interrupt).await?;
    let aborted = wait_for_event(&test.codex, |event| {
        events.push(event.clone());
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;

    // One-shot cancellation drops execution when there is no process handle. Release only
    // after the turn is aborted, so publishing inline would lose this completion.
    gate.release.notify_one();
    wait_for_event(&test.codex, |event| {
        events.push(event.clone());
        matches!(event, EventMsg::ExecCommandEnd(event) if event.call_id == CALL_ID)
    })
    .await;
    test.codex.submit(Op::Shutdown).await?;
    wait_for_event(&test.codex, |event| {
        events.push(event.clone());
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;
    let started = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::ItemStarted(event) if event.item.id() == CALL_ID => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    let completed = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::ItemCompleted(event) if event.item.id() == CALL_ID => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!((started.len(), completed.len()), (1, 1));
    assert_eq!(started[0].turn_id, completed[0].turn_id);
    let TurnItem::CommandExecution(command) = &completed[0].item else {
        anyhow::bail!("expected command completion");
    };
    assert_eq!(
        (
            command.status,
            command.process_id.as_ref(),
            command.exit_code,
            command.duration
        ),
        (
            CommandExecutionStatus::Failed,
            None,
            Some(-1),
            Some(Duration::ZERO)
        )
    );
    assert!(
        command
            .aggregated_output
            .as_deref()
            .context("launch diagnostic")?
            .starts_with("Failed to create unified exec process:")
    );

    let rollout_path = test.codex.rollout_path().context("rollout path")?;
    let (items, _, parse_errors) = RolloutRecorder::load_rollout_items(&rollout_path).await?;
    assert_eq!(parse_errors, 0);
    let persisted_completions = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::EventMsg {
                event: EventMsg::ItemCompleted(event),
                ..
            } if event.item.id() == CALL_ID => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(&persisted_completions)?,
        serde_json::to_value(&completed)?
    );
    let mut history = ThreadHistoryBuilder::new();
    for item in &items {
        history.handle_rollout_item(item);
    }
    let turn = history
        .turn_snapshot(&completed[0].turn_id)
        .context("interrupted turn in persisted history")?;
    assert_eq!(turn.status, TurnStatus::Interrupted);
    let commands = turn
        .item_payloads()
        .filter(|item| item.id() == CALL_ID)
        .collect::<Vec<_>>();
    // History reconstructs persisted fields; model_context is only present on live items.
    assert_eq!(
        commands,
        vec![&ThreadItemPayload::from(
            persisted_completions[0].item.clone()
        )]
    );
    assert!(
        matches!(aborted, EventMsg::TurnAborted(event) if event.turn_id.as_deref() == Some(completed[0].turn_id.as_str()))
    );
    request.single_request();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_during_launch_failure_preserves_the_failed_command() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let gate = Arc::new(PauseCommandCompletion::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.turn_lifecycle_contributor(gate.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_config(|config| {
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("enable unified exec");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("disable sandboxing");
        })
        .build_with_auto_env(&server)
        .await?;
    let request = responses::mount_sse_once(&server, responses::sse(vec![
        responses::ev_response_created("launch"),
        responses::ev_exec_command_call_with_args(CALL_ID,
            &json!({"cmd": "echo unreachable", "workdir": "missing-work-directory", "login": false})),
        responses::ev_completed("launch"),
    ])).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the command.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;

    gate.entered.notified().await;
    test.codex.submit(Op::Interrupt).await?;
    let EventMsg::TurnAborted(aborted) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await
    else {
        unreachable!()
    };

    // Try to close the writer while the command completion is paused before its append.
    test.codex.submit(Op::Shutdown).await?;
    let shutdown = timeout(Duration::from_secs(1), test.codex.wait_until_terminated()).await;
    gate.release.notify_one();
    test.codex.wait_until_terminated().await;

    let path = test.codex.rollout_path().context("rollout path")?;
    let (items, _, errors) = RolloutRecorder::load_rollout_items(&path).await?;
    assert_eq!(errors, 0);
    let mut history = ThreadHistoryBuilder::new();
    for item in &items {
        history.handle_rollout_item(item);
    }
    let turn = history
        .turn_snapshot(aborted.turn_id.as_deref().context("turn id")?)
        .context("interrupted turn in persisted history")?;
    assert_eq!(turn.status, TurnStatus::Interrupted);
    let commands = turn
        .item_payloads()
        .filter_map(|item| match item {
            ThreadItemPayload::CommandExecution { id, status, .. } => Some((id.as_str(), status)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        vec![(
            CALL_ID,
            &codex_app_server_protocol::CommandExecutionStatus::Failed
        )]
    );
    assert!(
        shutdown.is_err(),
        "session closed while failure publication was paused"
    );
    request.single_request();
    Ok(())
}
