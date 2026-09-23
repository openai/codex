//! Failed one-shot launches must finish publishing their lifecycle after interruption.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStatus;
use codex_config::Constrained;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnLifecycleContributor;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::items::CommandExecutionStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
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

const CALL_ID: &str = "interrupted-launch-failure";

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
            config.features.disable(Feature::ShellZshFork).unwrap();
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
            RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) if event.item.id() == CALL_ID => {
                Some(event)
            }
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
        .items
        .into_iter()
        .filter(|item| item.id() == CALL_ID)
        .collect::<Vec<_>>();
    // History reconstructs persisted fields; model_context is only present on live items.
    assert_eq!(
        commands,
        vec![ThreadItem::from(persisted_completions[0].item.clone())]
    );
    assert!(
        matches!(aborted, EventMsg::TurnAborted(event) if event.turn_id.as_deref() == Some(completed[0].turn_id.as_str()))
    );
    request.single_request();
    Ok(())
}
