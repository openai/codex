use super::*;

fn token_usage_info(total_tokens: i64) -> TokenUsageInfo {
    TokenUsageInfo {
        total_token_usage: TokenUsage {
            total_tokens,
            ..TokenUsage::default()
        },
        last_token_usage: TokenUsage {
            total_tokens,
            ..TokenUsage::default()
        },
        model_context_window: Some(100_000),
    }
}

fn page_with_turn(id: &str, items_view: TurnItemsView) -> TurnsPage {
    TurnsPage {
        data: vec![turn_with_view(id, items_view, TurnStatus::Completed)],
        next_cursor: Some("older-page".to_string()),
        backwards_cursor: None,
    }
}

fn assert_page_coverage(
    label: &str,
    buffered: &BufferedThreadEvent,
    page: &TurnsPage,
    expected: bool,
) {
    assert_eq!(
        event_is_represented(buffered, &[], Some(page), ResumePayloadMode::Full),
        expected,
        "{label}"
    );
}

#[test]
fn buffered_token_usage_dedupes_only_the_reconciled_final_snapshot() {
    let usage_a = token_usage_info(/*total_tokens*/ 10);
    let usage_b = token_usage_info(/*total_tokens*/ 20);
    let mut buffered = vec![represented_buffered_event(
        "turn-a",
        EventMsg::TokenCount(TokenCountEvent {
            info: Some(usage_a.clone()),
            rate_limits: Some(RateLimitSnapshot {
                limit_id: Some("codex".to_string()),
                limit_name: None,
                primary: None,
                secondary: None,
                credits: None,
                individual_limit: None,
                plan_type: None,
                rate_limit_reached_type: None,
            }),
        }),
    )];

    assert!(!should_replay_reconciled_token_usage(
        &buffered,
        Some(&usage_a),
        Some("turn-a"),
    ));
    assert!(
        should_replay_reconciled_token_usage(&buffered, Some(&usage_b), Some("turn-b")),
        "an externally reconciled newer snapshot must follow an older buffered update"
    );
    assert!(
        should_replay_reconciled_token_usage(&buffered, Some(&usage_a), Some("turn-b")),
        "equal usage still needs replay when the persisted owner advanced to a new turn"
    );
    assert!(
        should_replay_reconciled_token_usage(&[], Some(&usage_a), /*reconciled_turn_id*/ None,),
        "a captured snapshot with no recoverable owner still uses the sender's fallback"
    );

    buffered.push(represented_buffered_event(
        "turn-a",
        EventMsg::TokenCount(TokenCountEvent {
            info: None,
            rate_limits: None,
        }),
    ));
    assert!(
        !should_replay_reconciled_token_usage(&buffered, Some(&usage_a), Some("turn-a")),
        "a later rate-limit-only event must not hide the last buffered usage snapshot"
    );
    assert!(
        !full_turns_cover_event(&buffered[0], &[]),
        "the buffered event still delivers its rate-limit side effect exactly once"
    );
}

#[test]
fn resume_payload_coverage_replays_events_omitted_by_initial_turns_page() {
    let buffered_message = represented_buffered_event(
        "latest-turn",
        EventMsg::AgentMessage(AgentMessageEvent {
            message: "buffered".to_string(),
            phase: None,
            memory_citation: None,
        }),
    );
    for (label, id, view, expected) in [
        (
            "omitted turn replays",
            "older-turn",
            TurnItemsView::Full,
            false,
        ),
        (
            "summary does not cover arbitrary item",
            "latest-turn",
            TurnItemsView::Summary,
            false,
        ),
        (
            "full item view covers item",
            "latest-turn",
            TurnItemsView::Full,
            true,
        ),
    ] {
        assert_page_coverage(
            label,
            &buffered_message,
            &page_with_turn(id, view),
            expected,
        );
    }

    let buffered_completion =
        represented_buffered_event("latest-turn", turn_complete_event("latest-turn"));
    for (label, id, expected) in [
        ("omitted turn metadata replays", "older-turn", false),
        ("terminal metadata needs no items", "latest-turn", true),
    ] {
        assert_page_coverage(
            label,
            &buffered_completion,
            &page_with_turn(id, TurnItemsView::NotLoaded),
            expected,
        );
    }

    let buffered_usage = represented_buffered_event(
        "latest-turn",
        EventMsg::TokenCount(TokenCountEvent {
            info: None,
            rate_limits: None,
        }),
    );
    assert_page_coverage(
        "turn page never covers token usage",
        &buffered_usage,
        &page_with_turn("latest-turn", TurnItemsView::NotLoaded),
        /*expected*/ false,
    );
}
