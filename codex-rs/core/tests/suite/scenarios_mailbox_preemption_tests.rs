//! Mailbox scheduling preserves the current response when deferral is enabled.
//! Queued mail still reaches the next request, alongside completed tool results.

use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
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

#[derive(Clone, Copy)]
enum Boundary {
    Reasoning,
    Commentary,
}

#[test_case(Boundary::Reasoning, false; "reasoning_default")]
#[test_case(Boundary::Commentary, false; "commentary_default")]
#[test_case(Boundary::Reasoning, true; "reasoning_deferred")]
#[test_case(Boundary::Commentary, true; "commentary_deferred")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mailbox_preemption_preserves_response_when_deferred(
    boundary: Boundary,
    defer_mailbox_preemption: bool,
) -> anyhow::Result<()> {
    let (release, gate) = oneshot::channel();
    let (added, done, boundary_name) = match boundary {
        Boundary::Reasoning => (
            responses::ev_reasoning_item_added("boundary", &["Preparing the next action"]),
            responses::ev_reasoning_item("boundary", &["Preparing the next action"], &[]),
            "reasoning",
        ),
        Boundary::Commentary => {
            let mut done = responses::ev_assistant_message("boundary", "I will update the plan.");
            done["item"]["phase"] = json!("commentary");
            (
                responses::ev_message_item_added("boundary", ""),
                done,
                "commentary",
            )
        }
    };
    let (streaming, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_response_created("first"), added]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                body: responses::sse(vec![
                    done,
                    responses::ev_function_call(
                        "planned-action",
                        "update_plan",
                        r#"{"plan":[{"step":"Reply to the user","status":"in_progress"}]}"#,
                    ),
                    responses::ev_completed("first"),
                ]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("follow-up"),
                responses::ev_assistant_message("answer", "The worker update is now included."),
                responses::ev_completed("follow-up"),
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
                .set_enabled(Feature::DeferMailboxPreemption, defer_mailbox_preemption)
                .expect("set mailbox policy");
            // The gated server captures uncompressed request bodies for assertions.
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("disable request compression");
        })
        .build_with_auto_env(&config_server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Plan the next action while the worker finishes.",
        )]))
        .await?;
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "boundary"),
    )
    .await;
    for message in ["Worker found the result.", "Worker checked the result."] {
        test.codex
            .submit(Op::InterAgentCommunication {
                communication: InterAgentCommunication::new(
                    AgentPath::root().join("worker").expect("worker path"),
                    AgentPath::root(),
                    Vec::new(),
                    message.to_string(),
                    /*trigger_turn*/ false,
                ),
                start_options: Default::default(),
            })
            .await?;
    }
    // The barrier confirms that both messages were queued before finishing the item.
    test.codex
        .submit(Op::RealtimeConversationListVoices)
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    release.send(()).expect("release response");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 2);
    let input = requests[1]["input"].as_array().expect("follow-up input");
    let output = input
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == "planned-action");
    assert_eq!(
        output.map(|item| &item["output"]),
        defer_mailbox_preemption.then_some(&json!("Plan updated")),
    );
    let messages = input
        .iter()
        .filter(|item| item["type"] == "agent_message")
        .map(|item| item["content"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec![
            json!([{"type": "input_text", "text": "Worker found the result."}]),
            json!([{"type": "input_text", "text": "Worker checked the result."}]),
        ],
    );
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(
        format!("mailbox_preemption_{boundary_name}_deferred_{defer_mailbox_preemption}"),
        context_snapshot::format_context_snapshot(
            "Worker messages arrive during a response. Deferred mailbox preemption lets the planned tool execute before the next request receives both messages.",
            &entries,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    streaming.shutdown().await;
    Ok(())
}
