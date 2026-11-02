//! Follow-ups use native agent-mail sampling and final-answer boundaries.

use codex_core::StartThreadOptions;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::TurnStartOptions;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
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
use test_case::test_case;
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug)]
enum FollowupArrival {
    BeforeFinalAnswer,
    AfterFinalAnswer,
}

#[test_case(FollowupArrival::BeforeFinalAnswer; "before_final_answer")]
#[test_case(FollowupArrival::AfterFinalAnswer; "after_final_answer")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_followup_preserves_native_boundaries(
    arrival: FollowupArrival,
) -> anyhow::Result<()> {
    let (release_reasoning, reasoning_gate) = oneshot::channel();
    let (release_final, final_gate) = oneshot::channel();
    let (release_completion, completion_gate) = oneshot::channel();
    let mut final_answer = responses::ev_assistant_message("final", "The first task is finished.");
    final_answer["item"]["phase"] = json!("final_answer");
    let mut final_added = responses::ev_message_item_added("final", "");
    final_added["item"]["phase"] = json!("final_answer");
    let (streaming, _) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![
                    responses::ev_response_created("first"),
                    responses::ev_reasoning_item_added("reasoning", &["Checking the task"]),
                ]),
            },
            StreamingSseChunk {
                gate: Some(reasoning_gate),
                body: responses::sse(vec![
                    responses::ev_reasoning_item("reasoning", &["Checking the task"], &[]),
                    responses::ev_function_call(
                        "must-not-run",
                        "update_plan",
                        r#"{"plan":[{"step":"This should be preempted","status":"in_progress"}]}"#,
                    ),
                    responses::ev_completed("first"),
                ]),
            },
        ],
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_response_created("second"), final_added]),
            },
            StreamingSseChunk {
                gate: Some(final_gate),
                body: responses::sse(vec![final_answer]),
            },
            StreamingSseChunk {
                gate: Some(completion_gate),
                body: responses::sse(vec![responses::ev_completed("second")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("third"),
                responses::ev_assistant_message("next-final", "The next task is finished."),
                responses::ev_completed("third"),
            ]),
        }],
    ])
    .await;
    let config_server = responses::start_mock_server().await;
    let base_url = format!("{}/v1", streaming.uri());
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.update_plan_enabled = true;
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable native multi-agent delivery");
            config
                .features
                .disable(Feature::WakeOnAgentMessages)
                .expect("disable queue-only message wakeups");
            config
                .features
                .disable(Feature::DeferMailboxPreemption)
                .expect("enable immediate mailbox preemption");
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("disable request compression for the mock server");
        })
        .build_with_auto_env(&config_server)
        .await?;
    let child_path = AgentPath::try_from("/root/worker").expect("child path");
    let child = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: test.session_configured.thread_id,
                depth: 1,
                agent_path: Some(child_path.clone()),
                agent_nickname: None,
                agent_role: None,
            })),
            ..test.start_thread_options().await
        })
        .await?
        .thread;
    // An idle recipient must see its older queue-only mail before the new task.
    child
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::root(),
                child_path.clone(),
                Vec::new(),
                "Older queued mail".to_owned(),
                /*trigger_turn*/ false,
            ),
            start_options: Default::default(),
        })
        .await?;
    child.submit(Op::RealtimeConversationListVoices).await?;
    wait_for_event(&child, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    let message = |text: &str| {
        TurnInputRequest::new(TurnInput::InterAgentCommunication(
            InterAgentCommunication::new(
                AgentPath::root(),
                child_path.clone(),
                Vec::new(),
                text.to_owned(),
                /*trigger_turn*/ false,
            ),
        ))
        .on_start(TurnStartOptions {
            parent_turn_id: Some("parent-turn".to_owned()),
            root_turn_id: Some("root-turn".to_owned()),
            initiating_agent_path: Some(AgentPath::root()),
            ..Default::default()
        })
    };
    assert_eq!(
        child
            .start_or_steer_turn(message("Find the failing test"))
            .await?,
        TurnInputSubmission::Accepted
    );
    wait_for_event(
        &child,
        |event| matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "reasoning"),
    )
    .await;
    assert_eq!(
        child
            .start_or_steer_turn(TurnInputRequest::new(TurnInput::InterAgentCommunication(
                InterAgentCommunication::new_encrypted(
                    AgentPath::root(),
                    child_path.clone(),
                    Vec::new(),
                    "opaque-agent-message".to_owned(),
                    /*trigger_turn*/ false,
                ),
            )))
            .await?,
        TurnInputSubmission::Accepted
    );
    release_reasoning.send(()).expect("release reasoning");
    // The queued follow-up must preempt the remaining tool call at this reasoning boundary.
    wait_for_event(&child, |event| {
        assert!(
            !matches!(
                event,
                EventMsg::PlanUpdate(_) | EventMsg::TurnComplete(_) | EventMsg::Error(_)
            ),
            "{event:?}"
        );
        matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "final")
    })
    .await;
    match arrival {
        FollowupArrival::BeforeFinalAnswer => {
            assert_eq!(
                child
                    .start_or_steer_turn(message("Now check the fix"))
                    .await?,
                TurnInputSubmission::Accepted
            );
            release_final.send(()).expect("release final answer");
        }
        FollowupArrival::AfterFinalAnswer => {
            release_final.send(()).expect("release final answer");
            wait_for_event(
                &child,
                |event| matches!(event, EventMsg::ItemCompleted(item) if item.item.id() == "final"),
            )
            .await;
            assert_eq!(
                child
                    .start_or_steer_turn(message("Now check the fix"))
                    .await?,
                TurnInputSubmission::Accepted
            );
        }
    }
    release_completion.send(()).expect("release completion");
    // Neither arrival may keep the first turn open. The follow-up gets a new turn.
    let mut completed_turns = Vec::new();
    wait_for_event(&child, |event| {
        assert!(
            !matches!(event, EventMsg::PlanUpdate(_) | EventMsg::Error(_)),
            "{event:?}"
        );
        if let EventMsg::TurnComplete(done) = event {
            completed_turns.push(done.turn_id.clone());
        }
        completed_turns.len() == 2
    })
    .await;
    assert_ne!(completed_turns[0], completed_turns[1]);
    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 3);
    let first_mail = requests[0]["input"]
        .as_array()
        .expect("input array")
        .iter()
        .filter(|item| item["type"] == "agent_message")
        .map(|item| {
            item["content"][0]["text"]
                .as_str()
                .expect("plaintext message")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        first_mail,
        vec![
            "Older queued mail",
            "Message Type: NEW_TASK\nTask name: /root/worker\nSender: /root\nPayload:\nFind the failing test",
        ]
    );
    let later_mail = requests[1..]
        .iter()
        .map(|request| {
            request["input"]
                .as_array()
                .expect("input array")
                .iter()
                .rev()
                .find(|item| item["type"] == "agent_message")
                .expect("follow-up message")["content"]
                .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        later_mail,
        vec![
            json!([
                {"type": "input_text", "text": "Message Type: NEW_TASK\nTask name: /root/worker\nSender: /root\nPayload:\n"},
                {"type": "encrypted_content", "encrypted_content": "opaque-agent-message"},
            ]),
            json!([{"type": "input_text", "text": "Message Type: NEW_TASK\nTask name: /root/worker\nSender: /root\nPayload:\nNow check the fix"}]),
        ]
    );
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(
        format!("agent_followup_{arrival:?}"),
        context_snapshot::format_context_snapshot(
            "Agent follow-ups interrupt at a reasoning boundary, preserve encrypted input, and start a new turn when not consumed before the final answer completes.",
            &entries,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    child.shutdown_and_wait().await?;
    streaming.shutdown().await;
    Ok(())
}
