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

#[test]
fn buffered_token_usage_dedupes_only_the_reconciled_final_snapshot() {
    let usage_a = token_usage_info(10);
    let usage_b = token_usage_info(20);
    let mut buffered = vec![BufferedThreadEvent {
        event: Event {
            id: "turn-a".to_string(),
            msg: EventMsg::TokenCount(TokenCountEvent {
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
        },
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    }];

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
        should_replay_reconciled_token_usage(&[], Some(&usage_a), None),
        "a captured snapshot with no recoverable owner still uses the sender's fallback"
    );

    buffered.push(BufferedThreadEvent {
        event: Event {
            id: "turn-a".to_string(),
            msg: EventMsg::TokenCount(TokenCountEvent {
                info: None,
                rate_limits: None,
            }),
        },
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    });
    assert!(
        !should_replay_reconciled_token_usage(&buffered, Some(&usage_a), Some("turn-a")),
        "a later rate-limit-only event must not hide the last buffered usage snapshot"
    );
    assert!(
        !event_is_represented(&buffered[0], &[], None, ResumePayloadMode::Full,),
        "the buffered event still delivers its rate-limit side effect exactly once"
    );
}

#[test]
fn resume_payload_coverage_replays_events_omitted_by_initial_turns_page() {
    let buffered_message = BufferedThreadEvent {
        event: Event {
            id: "latest-turn".to_string(),
            msg: EventMsg::AgentMessage(AgentMessageEvent {
                message: "buffered".to_string(),
                phase: None,
                memory_citation: None,
            }),
        },
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    };
    let mut page = TurnsPage {
        data: vec![turn_with_view(
            "older-turn",
            TurnItemsView::Full,
            TurnStatus::Completed,
        )],
        next_cursor: Some("older-page".to_string()),
        backwards_cursor: None,
    };

    assert!(
        !event_is_represented(&buffered_message, &[], Some(&page), ResumePayloadMode::Full,),
        "a paginated response that omits the event's turn must replay it"
    );

    page.data = vec![turn_with_view(
        "latest-turn",
        TurnItemsView::Summary,
        TurnStatus::Completed,
    )];
    assert!(
        !event_is_represented(&buffered_message, &[], Some(&page), ResumePayloadMode::Full,),
        "a summary page does not prove an arbitrary buffered item is present"
    );

    page.data[0].items_view = TurnItemsView::Full;
    assert!(event_is_represented(
        &buffered_message,
        &[],
        Some(&page),
        ResumePayloadMode::Full,
    ));

    let buffered_completion = BufferedThreadEvent {
        event: Event {
            id: "latest-turn".to_string(),
            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "latest-turn".to_string(),
                last_agent_message: None,
                completed_at: Some(2),
                duration_ms: Some(1_000),
                time_to_first_token_ms: None,
            }),
        },
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    };
    page.data = vec![turn_with_view(
        "older-turn",
        TurnItemsView::NotLoaded,
        TurnStatus::Completed,
    )];
    assert!(
        !event_is_represented(
            &buffered_completion,
            &[],
            Some(&page),
            ResumePayloadMode::Full,
        ),
        "turn metadata from an omitted page must still be replayed"
    );
    page.data = vec![turn_with_view(
        "latest-turn",
        TurnItemsView::NotLoaded,
        TurnStatus::Completed,
    )];
    assert!(
        event_is_represented(
            &buffered_completion,
            &[],
            Some(&page),
            ResumePayloadMode::Full,
        ),
        "terminal turn metadata is represented even when items are not loaded"
    );

    let buffered_usage = BufferedThreadEvent {
        event: Event {
            id: "latest-turn".to_string(),
            msg: EventMsg::TokenCount(TokenCountEvent {
                info: None,
                rate_limits: None,
            }),
        },
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    };
    assert!(
        !event_is_represented(&buffered_usage, &[], Some(&page), ResumePayloadMode::Full,),
        "token usage is not represented by a turn page"
    );
}
