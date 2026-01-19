use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::ReviewTarget;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_reasoning_item_added;
use core_test_support::responses::ev_reasoning_summary_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_subagent_ignores_legacy_deltas() {
    skip_if_no_network!();

    // Single response with reasoning summary deltas.
    let sse_stream = sse(vec![
        ev_response_created("resp-1"),
        ev_reasoning_item_added("reason-1", &["initial"]),
        ev_reasoning_summary_text_delta("think-1"),
        ev_completed("resp-1"),
    ]);

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![sse_stream]).await;

    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("build test codex");

    // Kick off review (delegated).
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .expect("submit review");

    let mut reasoning_delta_count = 0;
    let mut legacy_reasoning_delta_count = 0;

    loop {
        let ev = wait_for_event(&test.codex, |_| true).await;
        match ev {
            EventMsg::ReasoningContentDelta(_) => reasoning_delta_count += 1,
            EventMsg::AgentReasoningDelta(_) => legacy_reasoning_delta_count += 1,
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert_eq!(reasoning_delta_count, 1, "expected one new reasoning delta");
    assert_eq!(
        legacy_reasoning_delta_count, 1,
        "expected one legacy reasoning delta"
    );
}
