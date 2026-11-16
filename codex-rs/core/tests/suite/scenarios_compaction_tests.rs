//! Snapshots context retention across compaction, interruption, restart, and normal inference.

use super::*;
use codex_core::TurnInputSubmission;
use codex_protocol::AgentPath;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::turn_input::InjectedResponseItem;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::strip_metadata_from_json;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_during_remote_compaction_preserves_summary_on_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    const SUMMARY: &str = "REMOTE_COMPACTION_CHECKPOINT";
    const POST_TURN_SUMMARY: &str = "POST_TURN_COMPACTION_CHECKPOINT";
    const FINAL_ANSWER: &str = "Fixed escaping in parser.rs; all tests pass.";
    let chunk = |gate, events| StreamingSseChunk {
        gate,
        body: sse(events),
    };
    let (release_summary, summary_gate) = oneshot::channel();
    let (release_post_summary, post_summary_gate) = oneshot::channel();
    let (server, _) = start_streaming_sse_server(vec![
        vec![chunk(
            None,
            vec![
                ev_function_call(
                    "plan",
                    "update_plan",
                    r#"{"plan":[{"step":"Fix escaping in parser.rs","status":"in_progress"}]}"#,
                ),
                ev_completed_with_tokens("first", /*total_tokens*/ 200_000),
            ],
        )],
        vec![
            chunk(None, vec![ev_response_created("compact")]),
            chunk(
                Some(summary_gate),
                vec![
                    compaction_output(SUMMARY),
                    ev_completed_with_tokens("compact", /*total_tokens*/ 50),
                ],
            ),
        ],
        vec![chunk(
            None,
            vec![
                ev_assistant_message("answer", FINAL_ANSWER),
                ev_completed_with_tokens("answer", /*total_tokens*/ 200_000),
            ],
        )],
        vec![
            chunk(None, vec![ev_response_created("post-compact")]),
            chunk(
                Some(post_summary_gate),
                vec![
                    compaction_output(POST_TURN_SUMMARY),
                    ev_completed_with_tokens("post-compact", /*total_tokens*/ 50),
                ],
            ),
        ],
        vec![chunk(
            None,
            vec![
                ev_assistant_message("resumed", "Continuing with the escaping fix."),
                ev_completed_with_tokens("resume", /*total_tokens*/ 60),
            ],
        )],
    ])
    .await;
    let config_server = start_mock_server().await;
    let base_url = format!("{}/v1", server.uri());
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.supports_websockets = false;
            config.model_auto_compact_token_limit = Some(100_000);
            config.model_post_turn_compact_threshold_percent = 50;
            config.update_plan_enabled = true;
            config.features.enable(Feature::MultiAgentV2).unwrap();
            config
                .features
                .disable(Feature::WakeOnAgentMessages)
                .unwrap();
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .unwrap();
            config.multi_agent_v2.interrupt_at_item_boundary = true;
        })
        .build_with_auto_env(&config_server)
        .await?;
    let child = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: test.session_configured.thread_id,
                depth: 1,
                agent_path: Some(AgentPath::try_from("/root/worker").expect("child path")),
                agent_nickname: None,
                agent_role: None,
            })),
            ..test.start_thread_options().await
        })
        .await?
        .thread;
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
            "Find and fix the bug.",
        )]))
        .await?;
    let EventMsg::TurnStarted(started) =
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnStarted(_))).await
    else {
        unreachable!("waited for turn start");
    };
    let mut previous_turn_id = started.turn_id;
    let next = |event: &EventMsg| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::ItemCompleted(item) if matches!(item.item, TurnItem::ContextCompaction(_)))
            || matches!(
                event,
                EventMsg::TurnAborted(_) | EventMsg::TurnStarted(_) | EventMsg::TurnComplete(_)
            )
    };
    // The same child stops during mid-turn compaction, then after its completed answer.
    for (request_count, release_summary, expected_answer) in [
        (2, release_summary, None),
        (4, release_post_summary, Some(FINAL_ANSWER)),
    ] {
        server.wait_for_request_count(request_count).await;
        child.submit(Op::InterruptAgent).await?;
        // This reply confirms the interrupt was accepted while the summary is still sampling.
        child
            .update_thread_settings(ThreadSettingsOverrides::default())
            .await?;
        assert_eq!(child.agent_status().await, AgentStatus::Running);
        let (reply, resumed) = oneshot::channel();
        child
            .submit(Op::TurnInput {
                request: Box::new(TurnInputRequest::user_input(vec![text("Resume.")])),
                mode: codex_protocol::turn_input::TurnInputMode::StartOrSteer,
                reply,
            })
            .await?;
        release_summary
            .send(())
            .expect("finish sampling the summary");
        assert!(matches!(
            wait_for_event(&child, next).await,
            EventMsg::ItemCompleted(_)
        ));
        let terminal = wait_for_event(&child, next).await;
        if let Some(answer) = expected_answer {
            let EventMsg::TurnComplete(completed) = terminal else {
                panic!("completed answer was lost: {terminal:?}");
            };
            assert_eq!(completed.last_agent_message.as_deref(), Some(answer));
            assert_eq!(completed.turn_id, previous_turn_id);
        } else {
            assert!(matches!(terminal,
                EventMsg::TurnAborted(event) if event.reason == TurnAbortReason::Interrupted));
        }
        let TurnInputSubmission::Started { turn_id, .. } = resumed.await?? else {
            panic!("Resume must start a new turn");
        };
        assert_ne!(turn_id, previous_turn_id);
        let EventMsg::TurnStarted(started) = wait_for_event(&child, next).await else {
            panic!("expected resumed turn start");
        };
        assert_eq!(started.turn_id, turn_id);
        previous_turn_id = turn_id;
    }
    assert!(matches!(
        wait_for_event(&child, next).await,
        EventMsg::TurnComplete(_)
    ));

    let requests = server
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<serde_json::Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 5);
    for (request_index, summary) in [(2, SUMMARY), (4, POST_TURN_SUMMARY)] {
        assert_eq!(
            requests[request_index - 1]["input"]
                .as_array()
                .expect("compaction input")
                .last(),
            Some(&json!({"type": "compaction_trigger"})),
        );
        let resumed_input = requests[request_index]["input"]
            .as_array()
            .expect("resumed input");
        let summaries = resumed_input
            .iter()
            .filter(|item| item["type"] == "compaction")
            .map(|item| item["encrypted_content"].clone())
            .collect::<Vec<_>>();
        assert_eq!(summaries, vec![json!(summary)]);
        let resume_messages = resumed_input
            .iter()
            .filter(|item| item["role"] == "user")
            .filter_map(|item| item["content"].as_array())
            .flatten()
            .filter_map(|content| content["text"].as_str())
            .filter(|text| *text == "Resume.")
            .collect::<Vec<_>>();
        assert_eq!(resume_messages, vec!["Resume."; request_index / 2]);
    }
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "A child interrupted during mid-turn and post-turn remote compaction keeps both checkpoints, delivers its completed answer, and handles each Resume in a separate turn.",
        &entries,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    ));
    child.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_slash_compact_preserves_context_across_windows() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("plan", "The release plan has three steps."),
                ev_completed("plan-response"),
            ]),
            sse(vec![
                compaction_output("REMOTE_SLASH_COMPACT_CHECKPOINT"),
                ev_completed("compact-response"),
            ]),
            sse(vec![
                compaction_output("RECOMPACTED_CHECKPOINT"),
                ev_completed("recompact-response"),
            ]),
            sse(vec![
                ev_assistant_message("continue", "Start by validating the release candidate."),
                ev_completed("continue-response"),
            ]),
            sse(vec![
                compaction_output("AFTER_MAIL_WAS_SAMPLED"),
                ev_completed("sampled-compact-response"),
            ]),
        ],
    )
    .await;
    let configure = |config: &mut Config| {
        configure_scenario_catalog(config);
        config.workspace_roots = vec![config.cwd.clone()];
        config.model_auto_compact_token_limit = Some(i64::MAX);
    };
    let mut builder = test_codex()
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(configure);
    let mut test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Plan the release.").await?;
    let mail = json!({
        "type": "agent_message", "id": "agent_pending-result",
        "author": "/root/worker", "recipient": "/root",
        "content": [{"type": "input_text", "text": "Message Type: FINAL_ANSWER\nThe worker found a release blocker."}],
    });
    let call: InjectedResponseItem = serde_json::from_value(json!({
        "item": {"type": "function_call", "call_id": "release-check", "name": "check_release", "arguments": "{}"},
        "retention": "always"
    }))?;
    let result: InjectedResponseItem = serde_json::from_value(json!({
        "item": {"type": "function_call_output", "call_id": "release-check", "output": "The release candidate passed."},
        "retention": "always"
    }))?;
    test.codex
        .inject_response_items_with_retention(vec![
            call,
            result,
            serde_json::from_value::<ResponseItem>(mail.clone())?.into(),
        ])
        .await?;
    for compaction in 0..2 {
        test.codex.submit(Op::Compact).await?;
        wait_for_event(&test.codex, |event| {
            assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        if compaction == 0 {
            builder = builder.with_model("gpt-6-astra").with_config(configure);
            test = builder.restart_with_auto_env(&server, &test).await?;
        }
    }

    test.submit_turn("Continue with the first step.").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let history = test.codex.conversation_history_snapshot().await;
    assert!(
        !history
            .items()
            .any(|item| matches!(item, ResponseItem::AgentMessage { .. }))
    );
    let requests = mock.requests();
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
    assert_eq!(
        requests[3].inputs_of_type("compaction")[0]["encrypted_content"],
        "RECOMPACTED_CHECKPOINT"
    );
    assert_eq!(
        strip_metadata_from_json(json!(requests[3].inputs_of_type("agent_message"))),
        json!([mail]),
    );

    insta::assert_snapshot!(
        "remote_slash_compact",
        context_snapshot::format_context_snapshot(
            "Pending mail survives repeated /compact and cold resume, then normal inference acknowledges it for subsequent compaction.",
            &[
                SnapshotEntry::captured(&requests[0]).labeled("Before /compact"),
                SnapshotEntry::captured(&requests[1]).labeled("Remote /compact request"),
                SnapshotEntry::captured(&requests[2]).labeled("Repeated /compact after resume"),
                SnapshotEntry::captured(&requests[3]).labeled("First turn after /compact"),
                SnapshotEntry::captured(&requests[4]).labeled("Compaction after mail was sampled"),
            ],
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
