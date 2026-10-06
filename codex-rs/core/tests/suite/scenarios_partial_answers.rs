//! Partial answers retain their phase in live events and subsequent model requests.

use super::*;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_answers_preserve_phase_across_sampling_continuation() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut added = ev_message_item_added("fragment", "");
    added["item"]["phase"] = json!("partial_answer");
    let mut partial = ev_assistant_message("fragment", "The first result is ready.");
    partial["item"]["phase"] = json!("partial_answer");
    let mut continuing = ev_completed("partial-response");
    continuing["response"]["end_turn"] = json!(false);
    let mut commentary = ev_assistant_message("progress", "Checking the remaining result.");
    commentary["item"]["phase"] = json!("commentary");
    let mut final_answer = ev_assistant_message("answer", "The second result is ready.");
    final_answer["item"]["phase"] = json!("final_answer");
    let mut completed = ev_completed("final-response");
    completed["response"]["end_turn"] = json!(true);
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("partial-response"),
                added,
                ev_output_text_delta("The first result is ready."),
                partial,
                continuing,
            ]),
            sse(vec![
                ev_response_created("final-response"),
                commentary,
                final_answer,
                completed,
            ]),
            sse(vec![
                ev_response_created("follow-up-response"),
                ev_assistant_message("follow-up", "Both results are still in context."),
                ev_completed("follow-up-response"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(configure_scenario_catalog)
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
            "Give me both results.",
        )]))
        .await?;

    let started = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemStarted(event) => match &event.item {
            TurnItem::AgentMessage(item) => Some((item.id.clone(), item.phase.clone())),
            _ => None,
        },
        _ => None,
    })
    .await;
    assert_eq!(
        started,
        ("fragment".to_string(), Some(MessagePhase::PartialAnswer))
    );
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("Summarize the results.").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    let phases = |index: usize| {
        requests[index].body_json()["input"]
            .as_array()
            .expect("request input")
            .iter()
            .filter(|item| item["type"] == "message" && item["role"] == "assistant")
            .map(|item| item["phase"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(phases(/*index*/ 1), vec![json!("partial_answer")]);
    assert_eq!(
        phases(/*index*/ 2),
        vec![
            json!("partial_answer"),
            json!("commentary"),
            json!("final_answer")
        ]
    );
    insta::assert_snapshot!(
        "partial_answer_continuation",
        context_snapshot::format_request_history_snapshot(
            "A nonterminal answer continues sampling; its phase survives an ordinary follow-up.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_answers_survive_forked_child_requests() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut partial = ev_assistant_message("fragment", "The first result is ready.");
    partial["item"]["phase"] = json!("partial_answer");
    let mut commentary = ev_assistant_message("progress", "Checking the remaining result.");
    commentary["item"]["phase"] = json!("commentary");
    let mut final_answer = ev_assistant_message("answer", "The second result is ready.");
    final_answer["item"]["phase"] = json!("final_answer");
    let done = sse(vec![
        ev_assistant_message("done", "Done."),
        ev_completed("done"),
    ]);
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("results"),
                partial,
                commentary,
                final_answer,
                ev_completed("results"),
            ]),
            sse(vec![
                ev_function_call_with_namespace(
                    "spawn-worker",
                    "collaboration",
                    "spawn_agent",
                    &json!({
                        "task_name": "worker",
                        "message": "Check both inherited results.",
                        "fork_turns": "all",
                    })
                    .to_string(),
                ),
                ev_completed("spawn"),
            ]),
            // Parent continuation and child inference may arrive in either order.
            done.clone(),
            done,
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            configure_scenario_catalog(config);
            config
                .features
                .enable(Feature::Collab)
                .expect("enable agents");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable agent paths");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Give me both results.").await?;
    test.submit_turn("Have a worker check both results.")
        .await?;
    let child_id = test
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|id| *id != test.session_configured.thread_id)
        .expect("forked child");
    let child = test.thread_manager.get_thread(child_id).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let child_requests = mock
        .requests()
        .into_iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["thread_id"] == child_id.to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(child_requests.len(), 1);
    let body = child_requests[0].body_json();
    let assistant_messages = body["input"]
        .as_array()
        .expect("child request input")
        .iter()
        .filter(|item| item["type"] == "message" && item["role"] == "assistant")
        .map(|item| (item["phase"].clone(), item["content"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        assistant_messages,
        vec![
            (
                json!("partial_answer"),
                json!([{"type":"output_text", "text":"The first result is ready."}])
            ),
            (
                json!("final_answer"),
                json!([{"type":"output_text", "text":"The second result is ready."}])
            ),
        ]
    );
    insta::assert_snapshot!(
        "partial_answer_fork",
        context_snapshot::format_request_history_snapshot(
            "A forked worker receives both stable answer fragments, with their phases preserved and parent commentary filtered out.",
            &child_requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    child.shutdown_and_wait().await?;
    Ok(())
}
