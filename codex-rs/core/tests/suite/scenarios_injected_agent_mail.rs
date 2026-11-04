//! Injected agent messages share native delivery boundaries while retaining their wire payloads.

use codex_core::InputWaitOutcome;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_features::Feature;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn injected_mail_respects_delivery_boundaries() -> anyhow::Result<()> {
    let (release_boundary, boundary_gate) = oneshot::channel();
    let (release_final, final_gate) = oneshot::channel();
    let (release_completion, completion_gate) = oneshot::channel();
    let chunk = |gate, events| StreamingSseChunk {
        gate,
        body: responses::sse(events),
    };
    let mut final_answer =
        responses::ev_assistant_message("answer", "The worker update is included.");
    final_answer["item"]["phase"] = json!("final_answer");
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            chunk(None, vec![
                responses::ev_response_created("first"),
                responses::ev_reasoning_item_added("boundary", &["Preparing the plan"]),
            ]),
            chunk(Some(boundary_gate), vec![
                responses::ev_reasoning_item("boundary", &["Preparing the plan"], &[]),
                responses::ev_function_call(
                    "stale-plan", "update_plan",
                    r#"{"plan":[{"step":"Act before reading the worker update","status":"in_progress"}]}"#,
                ),
                responses::ev_completed("first"),
            ]),
        ],
        vec![
            chunk(None, vec![
                responses::ev_response_created("after-mail"),
                responses::ev_message_item_added("answer", ""),
            ]),
            chunk(Some(final_gate), vec![
                final_answer,
                // This event proves that final-answer handling has closed delivery.
                responses::ev_reasoning_item_added("after-final", &["Finishing"]),
            ]),
            chunk(Some(completion_gate), vec![
                responses::ev_reasoning_item("after-final", &["Finishing"], &[]),
                responses::ev_completed("after-mail"),
            ]),
        ],
        vec![chunk(None, vec![
            responses::ev_response_created("next-task"),
            responses::ev_assistant_message("next-answer", "The remaining updates are included."),
            responses::ev_completed("next-task"),
        ])],
    ]).await;
    let config_server = responses::start_mock_server().await;
    let base_url = format!("{}/v1", server.uri());
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.update_plan_enabled = true;
            config
                .features
                .disable(Feature::DeferMailboxPreemption)
                .expect("enable preemption");
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("capture request bodies");
        })
        .build_with_auto_env(&config_server)
        .await?;
    let mail = |id: &str, text: &str| -> ResponseItem {
        serde_json::from_value(json!({
            "type": "agent_message", "id": id,
            "author": "/root/worker", "recipient": "/root",
            "content": [{"type": "input_text", "text": text}],
        }))
        .expect("agent message")
    };
    let idle_mail = mail("agent_idle-mail", "The worker started checking.");
    let active_mail: ResponseItem = serde_json::from_value(json!({
        "type": "agent_message", "id": "agent_active-mail",
        "author": "/root/worker", "recipient": "/root",
        "content": [
            {"type": "input_text", "text": "Use the updated worker result."},
            {"type": "encrypted_content", "encrypted_content": "opaque-worker-result"},
        ],
    }))?;
    let before_final_mail = mail(
        "agent_before-final-mail",
        "Another check finished while you answered.",
    );
    let after_final_mail = mail(
        "agent_after-final-mail",
        "The worker finished the remaining checks.",
    );

    test.codex
        .inject_response_items(vec![idle_mail.clone()])
        .await?;
    assert!(
        server.requests().await.is_empty(),
        "idle injection must not start a turn"
    );
    let TurnInputSubmission::Started { turn_id, .. } = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Plan the next action.",
        )]))
        .await?
    else {
        panic!("initial task should start a turn")
    };
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "boundary"),
    )
    .await;

    let (outcome, ()) = tokio::try_join!(biased;
        async { anyhow::Ok(test.codex.wait_for_input_activity(&turn_id, Duration::from_secs(10)).await) },
        async { test.codex.inject_response_items(vec![active_mail.clone()]).await?; anyhow::Ok(()) },
    )?;
    assert_eq!(outcome, InputWaitOutcome::MailboxActivity);
    release_boundary
        .send(())
        .expect("release sampling boundary");
    wait_for_event(&test.codex, |event| {
        assert!(
            !matches!(event, EventMsg::PlanUpdate(_)),
            "stale plan must not execute"
        );
        matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "answer")
    })
    .await;

    // Mail already pending when the final answer finishes must not request another response.
    test.codex
        .inject_response_items(vec![before_final_mail.clone()])
        .await?;
    release_final.send(()).expect("release final answer");
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "after-final"),
    )
    .await;
    // A later arrival must not reopen delivery, including at a trailing reasoning boundary.
    test.codex
        .inject_response_items(vec![after_final_mail.clone()])
        .await?;
    release_completion.send(()).expect("release completion");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        server.requests().await.len(),
        2,
        "queued mail must wait for another task"
    );

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Continue with the remaining worker updates.",
        )]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;

    let requests = server
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 3);
    for (request, expected) in requests.iter().zip([
        vec![idle_mail.clone()],
        vec![idle_mail.clone(), active_mail.clone()],
        vec![idle_mail, active_mail, before_final_mail, after_final_mail],
    ]) {
        let input = request["input"].as_array().expect("request input");
        let actual = input
            .iter()
            .filter(|item| item["type"] == "agent_message")
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            responses::strip_metadata_from_json(json!(actual)),
            responses::strip_metadata_from_json(serde_json::to_value(expected)?)
        );
        assert!(!input.iter().any(|item| item["call_id"] == "stale-plan"));
    }
    let next_input = requests[2]["input"].as_array().expect("next task input");
    let last_mail = next_input
        .iter()
        .rposition(|item| item["type"] == "agent_message")
        .unwrap();
    let next_task = next_input
        .iter()
        .rposition(|item| item["role"] == "user")
        .unwrap();
    assert!(
        last_mail < next_task,
        "deferred mail precedes the next task"
    );
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "Injected agent messages enter idle history, interrupt active sampling, and remain for the next task once the final answer closes delivery.",
        &entries,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    ));
    server.shutdown().await;
    Ok(())
}

#[test_case::test_case(true; "instant interrupt enabled")]
#[test_case::test_case(false; "instant interrupt disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn injected_agent_message_interrupting_sampling_survives_mid_turn_compaction(
    instant_interrupt: bool,
) -> anyhow::Result<()> {
    let (release_boundary, boundary_gate) = oneshot::channel();
    let chunk = |gate, events| StreamingSseChunk {
        gate,
        body: responses::sse(events),
    };
    let long_reasoning = "reasoning ".repeat(400);
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            chunk(
                None,
                vec![
                    responses::ev_response_created("first"),
                    responses::ev_reasoning_item_added("boundary", &[long_reasoning.as_str()]),
                ],
            ),
            chunk(
                Some(boundary_gate),
                vec![responses::ev_reasoning_item(
                    "boundary",
                    &[long_reasoning.as_str()],
                    &[],
                )],
            ),
        ],
        vec![chunk(
            None,
            vec![
                responses::ev_response_created("compact"),
                responses::ev_assistant_message("summary", "AUTO_COMPACT_SUMMARY"),
                responses::ev_completed_with_tokens("compact", /*total_tokens*/ 50),
            ],
        )],
        vec![chunk(
            None,
            vec![
                responses::ev_response_created("after-compact"),
                responses::ev_assistant_message("answer", "The worker update is included."),
                responses::ev_completed_with_tokens("after-compact", /*total_tokens*/ 60),
            ],
        )],
    ])
    .await;
    let config_server = responses::start_mock_server().await;
    let base_url = format!("{}/v1", server.uri());
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.model_provider.name = "OpenAI (test)".to_string();
            config.model_provider.base_url = Some(base_url);
            config.model_provider.supports_websockets = false;
            config.model_auto_compact_token_limit = Some(200);
            config
                .features
                .set_enabled(Feature::InstantInterrupt, instant_interrupt)
                .expect("set instant interrupt");
            config
                .features
                .disable(Feature::DeferMailboxPreemption)
                .expect("enable preemption");
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("capture request bodies");
        })
        .build_with_auto_env(&config_server)
        .await?;
    let message: ResponseItem = serde_json::from_value(json!({
        "type": "agent_message", "id": "agent_active-before-compaction",
        "author": "/root/worker", "recipient": "/root",
        "content": [{"type": "input_text", "text": "Use the completed worker result."}],
    }))?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Plan the next action.",
        )]))
        .await?;
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "boundary"),
    )
    .await;
    test.codex
        .inject_response_items(vec![message.clone()])
        .await?;
    release_boundary
        .send(())
        .expect("release sampling boundary");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;

    let requests = server
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 3);
    let after_compact_input = requests[2]["input"].as_array().expect("request input");
    let actual = after_compact_input
        .iter()
        .filter(|item| item["type"] == "agent_message")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        responses::strip_metadata_from_json(json!(actual)),
        responses::strip_metadata_from_json(serde_json::to_value([message])?)
    );
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "An injected agent message that interrupts sampling is included in the first request after mid-turn compaction.",
        &entries,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    ));
    server.shutdown().await;
    Ok(())
}
