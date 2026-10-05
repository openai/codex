//! Delegated review keeps user handoff context and current-turn skills across heartbeats.

use super::*;
use codex_core::context::GuardianReviewEvidence;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use core_test_support::responses::mount_sse_once;
use pretty_assertions::assert_eq;

#[test_case::test_case(0, None; "below_storage_limits")]
#[test_case::test_case(18, None; "bounded_user_history")]
#[test_case::test_case(0, Some("Should I pause restarting until you give final approval?"); "filtered_assistant_question")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_keeps_user_restriction_after_status_questions(
    unrelated_turns: usize,
    restriction_question: Option<&str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::GuardianRootHandoffContext,
            ] {
                config.features.enable(feature).expect("enable feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let root_id = test.session_configured.thread_id;
    let mut created = test.thread_manager.subscribe_thread_created();
    mount_sse_once(
        &server,
        sse(vec![
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "collaboration",
                "spawn_agent",
                &json!({"task_name": "worker", "message": INITIAL_TASK, "fork_turns": "none"})
                    .to_string(),
            ),
            ev_completed("spawn-worker"),
        ]),
    )
    .await;
    mount_completion(&server, root_id, SPAWN_CALL_ID).await;
    mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| is_worker_request(request, root_id),
        sse(vec![ev_completed("worker-complete")]),
    )
    .await;
    test.submit_text_turn("You may restart the pipeline to fix blockers. Delegate maintenance.")
        .await?;
    ThreadIdle::wait(&test.codex).await;
    let worker = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    wait_for_event(worker.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    // Exercise both the short conversation and a grant outside the last 16 messages.
    for index in 0..unrelated_turns {
        mount_sse_once(&server, sse(vec![ev_completed("website")])).await;
        test.submit_text_turn(&format!("Update website section {index}."))
            .await?;
        ThreadIdle::wait(&test.codex).await;
    }
    let restriction = if let Some(question) = restriction_question {
        test.codex
            .inject_response_items(vec![serde_json::from_value(
                ev_assistant_message("restriction-question", question)["item"].clone(),
            )?])
            .await?;
        "Yes, do that."
    } else {
        "Do not restart until the current batch finishes."
    };
    mount_sse_once(&server, sse(vec![ev_completed("restriction")])).await;
    test.submit_text_turn(restriction).await?;

    for index in 0..3 {
        mount_sse_once(&server, sse(vec![ev_completed("status-question")])).await;
        test.submit_text_turn(&format!("What is the status of batch {index}?"))
            .await?;
    }

    // Status replies fill the recent-message window without a new human instruction.
    let mut first_prompt = String::new();
    for index in 0..3 {
        mount_sse_once(
            &server,
            sse(vec![
                ev_assistant_message(
                    &format!("status-{index}"),
                    &format!("Still monitoring {index}."),
                ),
                ev_completed("heartbeat"),
            ]),
        )
        .await;
        let prompt = format!(
            "<heartbeat>\n  <automation_id>monitor</automation_id>\n  <current_time_iso>2026-09-23T00:{index:02}:00Z</current_time_iso>\n  <instructions>\nMonitor only. Do not restart.\n  </instructions>\n</heartbeat>\n"
        );
        if index == 0 {
            first_prompt = prompt.clone();
        }
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }])
                .on_start(TurnStartOptions {
                    turn_trigger: Some("automation_heartbeat_scheduled".to_owned()),
                    ..Default::default()
                }),
            )
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    let snapshot = worker
        .guardian_root_snapshot()
        .await
        .expect("root snapshot");
    let missing_instructions = snapshot
        .messages
        .contains(&GuardianRootMessage::IncompleteRootInstructions);
    let missing_assistant_context = snapshot
        .messages
        .contains(&GuardianRootMessage::IncompleteAssistantContext);
    let messages = snapshot
        .messages
        .into_iter()
        .filter(|message| {
            matches!(
                message,
                GuardianRootMessage::User(_)
                    | GuardianRootMessage::Assistant(_)
                    | GuardianRootMessage::UserInput(_)
            )
        })
        .collect::<Vec<_>>();
    let mut expected = if unrelated_turns == 0 {
        vec![GuardianRootMessage::User(
            "You may restart the pipeline to fix blockers. Delegate maintenance.".to_owned(),
        )]
    } else {
        // The 16-message cap drops the oldest inputs in order, including the original grant.
        (10..18)
            .map(|index| GuardianRootMessage::User(format!("Update website section {index}.")))
            .collect::<Vec<_>>()
    };
    expected.extend([
        GuardianRootMessage::User(restriction.to_owned()),
        GuardianRootMessage::User("What is the status of batch 0?".to_owned()),
        GuardianRootMessage::User("What is the status of batch 1?".to_owned()),
        GuardianRootMessage::User("What is the status of batch 2?".to_owned()),
        GuardianRootMessage::User(first_prompt),
        GuardianRootMessage::Assistant("Still monitoring 0.".to_owned()),
        GuardianRootMessage::Assistant("Still monitoring 1.".to_owned()),
        GuardianRootMessage::Assistant("Still monitoring 2.".to_owned()),
    ]);
    assert_eq!(
        (messages, missing_instructions, missing_assistant_context),
        (
            expected,
            unrelated_turns > 0,
            restriction_question.is_some()
        )
    );
    Ok(())
}

#[test_case::test_case(&[], 1; "existing_projection_first")]
#[test_case::test_case(&[], 3; "existing_projection_repeated")]
#[test_case::test_case(&[Feature::GuardianRootHandoffContext], 1; "handoff_filter_first")]
#[test_case::test_case(&[Feature::GuardianRootHandoffContext], 3; "handoff_filter_repeated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_root_projection_uses_latest_turn_skills(
    enabled_features: &'static [Feature],
    heartbeat_count: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(move |config| {
            for feature in [Feature::Collab, Feature::MultiAgentV2]
                .into_iter()
                .chain(enabled_features.iter().copied())
            {
                config.features.enable(feature).expect("enable feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let root_id = test.session_configured.thread_id;
    let mut created = test.thread_manager.subscribe_thread_created();
    let evidence = test
        .codex
        .thread_extension_data()
        .get_or_init(GuardianReviewEvidence::default);
    mount_sse_once(&server, sse(vec![ev_completed("monitor-request")])).await;
    test.submit_text_turn("You may restart the service.")
        .await?;
    let mut first_prompt = String::new();
    for index in 0..heartbeat_count {
        let events = if index + 1 < heartbeat_count {
            vec![
                ev_assistant_message(&format!("reply-{index}"), &format!("Run {index} finished.")),
                ev_completed("heartbeat"),
            ]
        } else {
            vec![
                ev_function_call_with_namespace(
                    SPAWN_CALL_ID,
                    "collaboration",
                    "spawn_agent",
                    &json!({"task_name": "worker", "message": INITIAL_TASK}).to_string(),
                ),
                ev_completed("spawn-worker"),
            ]
        };
        mount_sse_once(&server, sse(events)).await;
        if index + 1 == heartbeat_count {
            mount_completion(&server, root_id, SPAWN_CALL_ID).await;
            mount_sse_once_match(
                &server,
                move |request: &wiremock::Request| is_worker_request(request, root_id),
                sse(vec![ev_completed("worker-complete")]),
            )
            .await;
        }
        // Saved heartbeat instructions may be edited by the user outside the conversation.
        let prompt = format!(
            "<heartbeat>\n  <automation_id>monitor</automation_id>\n  <current_time_iso>2026-09-23T00:{index:02}:00Z</current_time_iso>\n  <instructions>\nMonitor only. Do not restart.\n  </instructions>\n</heartbeat>\n"
        );
        if index == 0 {
            first_prompt = prompt.clone();
        }
        let submission = test
            .codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }])
                .on_start(TurnStartOptions {
                    turn_trigger: Some("automation_heartbeat_scheduled".to_owned()),
                    ..Default::default()
                }),
            )
            .await?;
        let TurnInputSubmission::Started { turn_id, .. } = submission else {
            anyhow::bail!("expected a new heartbeat turn");
        };
        // The trusted-skill extension records paths against each invocation's actual turn.
        evidence.record_trusted_skill(&turn_id, format!("/skills/run-{index}/SKILL.md"));
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    let worker = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    wait_for_event(worker.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let snapshot = worker
        .guardian_root_snapshot()
        .await
        .expect("root snapshot");
    let mut expected = vec![
        GuardianRootMessage::RetainedContextScope,
        GuardianRootMessage::User("You may restart the service.".to_owned()),
        GuardianRootMessage::User(first_prompt),
    ];
    expected.extend(
        (0..heartbeat_count - 1)
            .map(|index| GuardianRootMessage::Assistant(format!("Run {index} finished."))),
    );
    assert_eq!(snapshot.messages, expected);
    assert_eq!(
        snapshot.trusted_skill_paths,
        vec![format!("/skills/run-{}/SKILL.md", heartbeat_count - 1)]
    );
    mount_sse_once(&server, sse(vec![ev_completed("human-turn")])).await;
    test.submit_text_turn("Stop monitoring.").await?;
    assert_eq!(
        worker
            .guardian_root_snapshot()
            .await
            .expect("root snapshot")
            .trusted_skill_paths,
        Vec::<String>::new(),
    );
    Ok(())
}
