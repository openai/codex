//! Delegated review keeps current-turn skills when scheduled instructions coalesce.

use super::*;
use codex_core::context::GuardianReviewEvidence;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use core_test_support::responses::mount_sse_once;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_root_projection_uses_latest_turn_skills() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::GuardianThreadContext,
            ] {
                config.features.enable(feature).unwrap();
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
    let mut first_prompt = String::new();
    for index in 0..2 {
        let events = if index == 0 {
            vec![ev_completed("first-heartbeat")]
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
        if index == 1 {
            mount_completion(&server, root_id, SPAWN_CALL_ID).await;
            mount_sse_once_match(
                &server,
                move |request: &wiremock::Request| is_worker_request(request, root_id),
                sse(vec![ev_completed("worker-complete")]),
            )
            .await;
        }
        let prompt = format!(
            "<heartbeat>\n  <automation_id>monitor</automation_id>\n  <current_time_iso>2026-09-23T00:{index:02}:00Z</current_time_iso>\n  <instructions>\nMonitor only.\n  </instructions>\n</heartbeat>\n"
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
    assert_eq!(
        (snapshot.messages, snapshot.trusted_skill_paths),
        (
            vec![
                GuardianRootMessage::RetainedContextScope,
                GuardianRootMessage::User(first_prompt)
            ],
            vec!["/skills/run-1/SKILL.md".to_owned()]
        ),
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
