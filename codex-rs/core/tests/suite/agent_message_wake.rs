//! Agent-message wakes preserve stop, Plan-mode, and turn-completion boundaries.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use codex_core::NotSubmittedReason;
use codex_core::StartIfIdleSubmission;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_protocol::AgentPath;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::ThreadIdle;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[derive(Default)]
struct PauseOnce {
    gate: Mutex<Option<oneshot::Receiver<()>>>,
    entered: Notify,
}

impl PauseOnce {
    fn arm(&self) -> oneshot::Sender<()> {
        let (release, gate) = oneshot::channel();
        assert!(
            self.gate
                .lock()
                .expect("pause lock")
                .replace(gate)
                .is_none()
        );
        release
    }

    async fn pause(&self) {
        let gate = self.gate.lock().expect("pause lock").take();
        if let Some(gate) = gate {
            self.entered.notify_one();
            gate.await.expect("test should release the pause");
        }
    }
}

#[derive(Default)]
struct WakeBarriers {
    completion: PauseOnce,
    discovery: PauseOnce,
}

impl codex_extension_api::TurnLifecycleContributor for WakeBarriers {
    fn on_turn_stop<'a>(
        &'a self,
        _input: codex_extension_api::TurnStopInput<'a>,
    ) -> codex_extension_api::ExtensionFuture<'a, ()> {
        Box::pin(self.completion.pause())
    }
}

impl ExternalAuth for WakeBarriers {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            self.discovery.pause().await;
            Ok(CodexAuth::from_api_key("dummy"))
        })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[derive(Clone, Copy)]
enum WakeBoundary {
    Interrupt,
    Plan,
}

#[derive(Clone, Copy)]
enum AgentMessageWake {
    Enabled,
    Disabled,
}

#[test_case(WakeBoundary::Interrupt, AgentMessageWake::Enabled; "interrupt_during_discovery")]
#[test_case(WakeBoundary::Plan, AgentMessageWake::Enabled; "plan_mode_during_discovery")]
#[test_case(WakeBoundary::Interrupt, AgentMessageWake::Disabled; "disabled_interrupt_preserves_status_and_explicit_followup")]
#[test_case(WakeBoundary::Plan, AgentMessageWake::Disabled; "disabled_plan_allows_explicit_followup")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_agent_message_wake_respects_control_changes(
    boundary: WakeBoundary,
    setting: AgentMessageWake,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let wakes_on_agent_messages = matches!(setting, AgentMessageWake::Enabled);
    let server = responses::start_mock_server().await;
    let mut answer = responses::ev_assistant_message("answer", "Finished.");
    answer["item"]["phase"] = json!("final_answer");
    let response = responses::sse(vec![answer, responses::ev_completed("response")]);
    // The third response also lets a regressed wake finish, so the turn-count
    // assertion below fails instead of waiting for that unwanted turn.
    let expected_requests = if wakes_on_agent_messages { 3 } else { 2 };
    let mock = responses::mount_sse_sequence(&server, vec![response; expected_requests]).await;
    let barriers = Arc::new(WakeBarriers::default());
    let release_completion = barriers.completion.arm();
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_lifecycle_contributor(barriers.clone());
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            for feature in [Feature::Collab, Feature::MultiAgentV2] {
                config.features.enable(feature).expect("enable agent mail");
            }
            if wakes_on_agent_messages {
                config.features.enable(Feature::WakeOnAgentMessages)
            } else {
                config.features.disable(Feature::WakeOnAgentMessages)
            }
            .expect("configure agent-message wakes");
        })
        .build_with_auto_env(&server)
        .await?;
    let input = |text: &str| {
        TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.to_owned(),
            text_elements: Vec::new(),
        }])
    };
    let TurnInputSubmission::Started {
        turn_id: initial_turn_id,
        ..
    } = test
        .codex
        .start_or_steer_turn(input("Finish the initial work."))
        .await?
    else {
        anyhow::bail!("the initial input must start a turn");
    };
    timeout(
        Duration::from_secs(10),
        barriers.completion.entered.notified(),
    )
    .await?;

    let trigger_turn = matches!(setting, AgentMessageWake::Disabled);
    let mail = |text: &str| Op::InterAgentCommunication {
        communication: InterAgentCommunication::new(
            AgentPath::root().join("worker").expect("valid worker path"),
            AgentPath::root(),
            Vec::new(),
            text.to_owned(),
            trigger_turn,
        ),
        start_options: Default::default(),
    };
    let first_mail = "First deferred worker update.";
    let second_mail = "Worker update after stop.";
    test.codex.submit(mail(first_mail)).await?;
    // The settings reply fences the preceding mailbox operation while the old
    // turn still owns admission, so only its completion can start the wake.
    test.codex
        .update_thread_settings(ThreadSettingsOverrides::default())
        .await?;
    test.thread_manager
        .auth_manager()
        .set_external_auth(barriers.clone())
        .await?;
    let release_discovery = barriers.discovery.arm();
    release_completion.send(()).expect("completion is paused");
    timeout(
        Duration::from_secs(10),
        barriers.discovery.entered.notified(),
    )
    .await?;
    // Queue-only mail allows an idle notification before discovery. Explicit
    // follow-ups suppress it until they have been delivered.
    if wakes_on_agent_messages {
        ThreadIdle::wait(&test.codex).await;
    }

    let expected_mail = match boundary {
        WakeBoundary::Interrupt => {
            let previous_status = test.codex.agent_status().await;
            test.codex.submit(Op::Interrupt).await?;
            if !wakes_on_agent_messages {
                test.codex
                    .update_thread_settings(ThreadSettingsOverrides::default())
                    .await?;
                assert_eq!(test.codex.agent_status().await, previous_status);
            }
            test.codex.submit(mail(second_mail)).await?;
            test.codex
                .update_thread_settings(ThreadSettingsOverrides::default())
                .await?;
            vec![first_mail, second_mail]
        }
        WakeBoundary::Plan => {
            let mut collaboration_mode = test.codex.config_snapshot().await.collaboration_mode;
            collaboration_mode.mode = ModeKind::Plan;
            test.codex
                .update_thread_settings(ThreadSettingsOverrides {
                    collaboration_mode: Some(collaboration_mode),
                    ..Default::default()
                })
                .await?;
            assert_eq!(
                test.codex.config_snapshot().await.collaboration_mode.mode,
                ModeKind::Plan
            );
            vec![first_mail]
        }
    };
    release_discovery
        .send(())
        .expect("control change must complete while wake discovery is paused");

    let (mail_request_index, final_turn_id) = if wakes_on_agent_messages {
        // StartIfIdle cannot replace the reserved wake. Acceptance is the barrier
        // proving that the pending wake relinquished admission; no sleep is needed.
        let user_turn_id = timeout(Duration::from_secs(10), async {
            loop {
                match test
                    .codex
                    .start_turn_if_idle(input("Read the deferred worker updates now."))
                    .await?
                {
                    StartIfIdleSubmission::Started { turn_id, .. } => {
                        return Ok::<_, anyhow::Error>(turn_id);
                    }
                    StartIfIdleSubmission::NotSubmitted {
                        reason: NotSubmittedReason::NotIdle,
                    } => tokio::task::yield_now().await,
                    other => anyhow::bail!("unexpected explicit user turn result: {other:?}"),
                }
            }
        })
        .await??;
        wait_for_event(
            &test.codex,
            |event| matches!(event, EventMsg::TurnComplete(done) if done.turn_id == user_turn_id),
        )
        .await;
        ThreadIdle::wait(&test.codex).await;

        let requests = mock.requests();
        assert_eq!(
            requests.len(),
            2,
            "agent mail must not start an automatic turn"
        );
        assert!(requests[0].inputs_of_type("agent_message").is_empty());
        assert_eq!(
            requests[1].body_json()["client_metadata"]["turn_id"],
            user_turn_id
        );
        // Fresh user input is sampled before queued mail. The final answer defers
        // that mail, and completion records it for the next explicit user turn.
        let TurnInputSubmission::Started {
            turn_id: next_user_turn_id,
            ..
        } = test
            .codex
            .start_or_steer_turn(input("Summarize the recorded worker updates."))
            .await?
        else {
            anyhow::bail!("the next explicit user input must start a new turn");
        };
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(done) if done.turn_id == next_user_turn_id)
        })
        .await;
        ThreadIdle::wait(&test.codex).await;
        (2, next_user_turn_id)
    } else {
        // With the feature disabled, explicit follow-ups still run. In the
        // interrupt case, the second follow-up also delivers the preserved first.
        let EventMsg::TurnComplete(completed) = wait_for_event(
            &test.codex,
            |event| matches!(event, EventMsg::TurnComplete(done) if done.turn_id != initial_turn_id),
        )
        .await
        else {
            unreachable!("waited for the explicit follow-up to complete");
        };
        ThreadIdle::wait(&test.codex).await;
        (1, completed.turn_id)
    };
    test.codex.shutdown_and_wait().await?;

    let requests = responses::received_responses_requests(&server).await;
    assert_eq!(requests.len(), expected_requests);
    assert_eq!(
        requests[mail_request_index].body_json()["client_metadata"]["turn_id"],
        final_turn_id
    );
    let messages = requests[mail_request_index].inputs_of_type("agent_message");
    let actual_mail = messages
        .iter()
        .map(|message| {
            assert_eq!(message["author"], "/root/worker");
            assert_eq!(message["recipient"], "/root");
            message["content"].clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual_mail,
        expected_mail
            .iter()
            .map(|text| json!([{"type": "input_text", "text": text}]))
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test_case(AgentMessageWake::Enabled; "enabled")]
#[test_case(AgentMessageWake::Disabled; "disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_during_completion_preserves_running_status(
    setting: AgentMessageWake,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("answer", "Finished."),
            responses::ev_completed("response"),
        ]),
    )
    .await;
    let barriers = Arc::new(WakeBarriers::default());
    let release_completion = barriers.completion.arm();
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_lifecycle_contributor(barriers.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            match setting {
                AgentMessageWake::Enabled => config.features.enable(Feature::WakeOnAgentMessages),
                AgentMessageWake::Disabled => config.features.disable(Feature::WakeOnAgentMessages),
            }
            .expect("configure agent-message wakes");
        })
        .build_with_auto_env(&server)
        .await?;
    let TurnInputSubmission::Started { turn_id, .. } = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Finish the work.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?
    else {
        anyhow::bail!("the initial input must start a turn");
    };
    // The stop callback runs after the task is detached, before TurnComplete
    // is persisted. This is a finishing turn, not an unstarted wake reservation.
    timeout(
        Duration::from_secs(10),
        barriers.completion.entered.notified(),
    )
    .await?;
    assert_eq!(test.codex.agent_status().await, AgentStatus::Running);
    test.codex.submit(Op::Interrupt).await?;
    test.codex
        .update_thread_settings(ThreadSettingsOverrides::default())
        .await?;
    let status_while_finishing = test.codex.agent_status().await;

    release_completion.send(()).expect("completion is paused");
    let completed = wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::TurnComplete(done) if done.turn_id == turn_id),
    )
    .await;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    test.codex.shutdown_and_wait().await?;

    // Interrupted would make a child eligible for capacity eviction while its
    // completion callback still owns the rollout writer.
    assert_eq!(status_while_finishing, AgentStatus::Running);
    let terminal_events = std::fs::read_to_string(rollout_path)?
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::EventMsg(
                event @ (EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_)),
            ) => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(terminal_events)?,
        serde_json::to_value(vec![completed])?
    );
    response.single_request();
    Ok(())
}
