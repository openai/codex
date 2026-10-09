//! Partial answers retain their phase in live events and subsequent model requests.

use super::super::guardian_retained_context::ForkTestLifecycle;
use super::*;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::ThreadHistoryMode;
use core_test_support::ThreadIdle;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use test_case::test_case;

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

#[test_case(false, false, ThreadHistoryMode::Legacy; "filtered")]
#[test_case(true, false, ThreadHistoryMode::Legacy; "preserved legacy")]
#[test_case(true, false, ThreadHistoryMode::Paginated; "preserved paginated")]
#[test_case(true, true, ThreadHistoryMode::Paginated; "preserved code mode")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_answers_survive_forked_child_requests(
    preserve_fork_prefix: bool,
    code_mode: bool,
    history_mode: ThreadHistoryMode,
) -> Result<()> {
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
    mount_sse_sequence(
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
                if code_mode {
                    ev_custom_tool_call("spawn-worker", "exec",
                        r#"text(await tools.collaboration__spawn_agent({task_name: "worker", message: "Check both inherited results.", fork_turns: "all"}));"#)
                } else { ev_function_call_with_namespace(
                    "spawn-worker",
                    "collaboration",
                    "spawn_agent",
                    &json!({
                        "task_name": "worker",
                        "message": "Check both inherited results.",
                        "fork_turns": "all",
                    })
                    .to_string(),
                ) },
                ev_completed("spawn"),
            ]),
        ],
    )
    .await;
    let child_gate = Arc::new(ForkTestLifecycle::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(child_gate.clone());
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let test = test_codex()
        .with_model(if preserve_fork_prefix {
            "gpt-5.6-sol"
        } else {
            "test-gpt-5.1"
        })
        .with_history_mode(history_mode)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.multi_agent_v2.preserve_fork_prefix = preserve_fork_prefix;
            config.multi_agent_v2.non_code_mode_only = !code_mode;
            if code_mode {
                config
                    .features
                    .enable(Feature::CodeMode)
                    .expect("enable Code Mode");
                config
                    .features
                    .enable(Feature::MultiAgentV2DynamicTools)
                    .expect("enable agent tools in Code Mode");
            }
            if preserve_fork_prefix {
                config.update_plan_enabled = true;
                config.multi_agent_v2.root_agent_usage_hint_text =
                    Some("You are /root. Coordinate the investigation.".into());
                config.multi_agent_v2.subagent_usage_hint_text =
                    Some("You are a worker. Report to your parent.".into());
                config.developer_instructions = Some("Coordinate the investigation.".into());
                config.multi_agent_v2.subagent_developer_instructions =
                    Some("Check both results independently.".into());
            }
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
    let parent_id = test.session_configured.thread_id.to_string();
    responses::mount_sse_once_match(
        &server,
        wiremock::matchers::header("thread-id", parent_id.as_str()),
        done.clone(),
    )
    .await;
    let root_id = parent_id.clone();
    responses::mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| {
            request
                .headers
                .get("thread-id")
                .is_some_and(|id| id != root_id.as_str())
        },
        if preserve_fork_prefix {
            // Keep the child active until the parent is idle, making completion mail deterministic.
            sse(vec![
                ev_function_call_with_namespace(
                    "child-pause",
                    "functions",
                    "update_plan",
                    r#"{"plan":[{"step":"Check results","status":"in_progress"}]}"#,
                ),
                ev_completed("child-pause"),
            ])
        } else {
            done
        },
    )
    .await;
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
    if preserve_fork_prefix {
        child_gate.entered.notified().await;
    } else {
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    }
    ThreadIdle::wait(&test.codex).await;
    let mut requests = responses::received_responses_requests(&server).await;
    requests
        .sort_by_key(|request| request.header("thread-id").as_deref() != Some(parent_id.as_str()));
    let child_requests = requests
        .iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["thread_id"] == child_id.to_string()
        })
        .cloned()
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
    let mut expected_messages = vec![
        (
            json!("partial_answer"),
            json!([{"type":"output_text", "text":"The first result is ready."}]),
        ),
        (
            json!("final_answer"),
            json!([{"type":"output_text", "text":"The second result is ready."}]),
        ),
    ];
    if preserve_fork_prefix {
        expected_messages.insert(
            1,
            (
                json!("commentary"),
                json!([{"type":"output_text", "text":"Checking the remaining result."}]),
            ),
        );
        let parent_input = requests[1].input();
        assert_eq!(
            responses::strip_response_item_ids_from_json(json!(
                &child_requests[0].input()[..parent_input.len()]
            )),
            responses::strip_response_item_ids_from_json(json!(parent_input))
        );
        assert_eq!(body["tools"], requests[1].body_json()["tools"]);
        assert_eq!(
            body["instructions"],
            requests[1].body_json()["instructions"]
        );
        assert!(child_requests[0].body_contains_text("Check both results independently."));
        if code_mode {
            assert_eq!(
                child_requests[0].custom_tool_call_output_content_and_success("spawn-worker"),
                Some((Some("aborted".into()), None))
            );
            assert!(
                requests[2]
                    .custom_tool_call_output("spawn-worker")
                    .to_string()
                    .contains("/root/worker")
            );
        } else {
            insta::assert_snapshot!(
                "preserved_answer_fork",
                context_snapshot::format_request_history_snapshot(
                    "A worker inherits the parent prefix unchanged and receives its own instructions. Requests are grouped by parent, then child.",
                    &requests,
                    &ContextSnapshotOptions::default().rewrite_known_segments()
                )
            );
        }
    } else {
        insta::assert_snapshot!(
            "partial_answer_fork",
            context_snapshot::format_request_history_snapshot(
                "A forked worker receives both stable answer fragments, with their phases preserved and parent commentary filtered out.",
                &child_requests,
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
    }
    assert_eq!(assistant_messages, expected_messages);
    child.shutdown_and_wait().await?;
    Ok(())
}
