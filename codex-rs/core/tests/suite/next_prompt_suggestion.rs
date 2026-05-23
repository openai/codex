use anyhow::Result;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suggest_next_prompt_samples_history_without_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let first_turn_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "first turn complete"),
        ev_completed("resp-1"),
    ]);
    let second_turn_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-2", "second turn complete"),
        ev_completed("resp-2"),
    ]);
    let suggestion_response = sse(vec![
        ev_response_created("resp-suggestion"),
        ev_rate_limits(),
        ev_assistant_message("msg-suggestion", "run the tests"),
        ev_completed_with_tokens("resp-suggestion", /*total_tokens*/ 33),
    ]);
    let responses = mount_sse_sequence(
        &server,
        vec![
            first_turn_response,
            second_turn_response,
            suggestion_response,
        ],
    )
    .await;

    let test = test_codex().build(&server).await?;
    test.submit_turn("first task").await?;
    test.submit_turn("second task").await?;

    let suggestion = test
        .codex
        .suggest_next_prompt(CancellationToken::new())
        .await?;
    assert_eq!(suggestion, Some("run the tests".to_string()));

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let suggestion_request = &requests[2];
    let suggestion_body = suggestion_request.body_json();
    assert_eq!(suggestion_body["tools"], json!([]));
    assert_eq!(suggestion_body["parallel_tool_calls"], false);

    let user_texts = suggestion_request.message_input_texts("user");
    let suggestion_prompt = user_texts
        .last()
        .expect("suggestion request should append a contextual user prompt");
    assert!(suggestion_prompt.contains("<next_prompt_suggestion>"));
    assert!(suggestion_prompt.contains("Reply with ONLY the suggestion"));
    assert!(suggestion_prompt.contains("</next_prompt_suggestion>"));

    let token_event = wait_for_event(&test.codex, |msg| {
        matches!(msg, EventMsg::TokenCount(ev)
            if ev.info.is_none() && ev.rate_limits.is_some())
    })
    .await;
    let EventMsg::TokenCount(token_count) = token_event else {
        unreachable!("wait_for_event predicate only accepts TokenCount");
    };
    assert_eq!(token_count.info, None);
    assert_eq!(
        token_count
            .rate_limits
            .expect("rate limits should be recorded")
            .primary
            .expect("primary rate limit should be present")
            .used_percent,
        42.0
    );
    assert_eq!(test.codex.token_usage_info().await, None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suggest_next_prompt_skips_early_history_without_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let first_turn_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "first turn complete"),
        ev_completed("resp-1"),
    ]);
    let responses = mount_sse_sequence(&server, vec![first_turn_response]).await;

    let test = test_codex().build(&server).await?;
    test.submit_turn("first task").await?;

    let suggestion = test
        .codex
        .suggest_next_prompt(CancellationToken::new())
        .await?;
    assert_eq!(suggestion, None);
    assert_eq!(responses.requests().len(), 1);

    Ok(())
}

fn ev_rate_limits() -> serde_json::Value {
    json!({
        "type": "codex.rate_limits",
        "plan_type": "plus",
        "rate_limits": {
            "allowed": true,
            "limit_reached": false,
            "primary": {
                "used_percent": 42,
                "window_minutes": 60,
                "reset_at": 1700000000
            },
            "secondary": null
        },
        "code_review_rate_limits": null,
        "credits": null,
        "promo": null
    })
}
