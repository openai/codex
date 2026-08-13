use super::THREAD_USAGE_RETRY_DELAYS;
use super::THREAD_USAGE_SETTLEMENT_DELAYS;
use super::ThreadUsageOutcome;
use crate::app_event::AppEvent;
use crate::chatwidget::ReplayKind;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::status::format_estimated_usd_micros;
use crate::tui::FrameRequester;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadUsage;
use codex_app_server_protocol::ThreadUsageBreakdownGroup;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType;
use pretty_assertions::assert_eq;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc::UnboundedReceiver;

fn thread_usage_request_id(events: &mut UnboundedReceiver<AppEvent>) -> u64 {
    match events.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected a thread usage request, got {event:?}"),
    }
}

fn thread_usage(
    thread_id: ThreadId,
    estimated_usage_credits_micros: i64,
    estimated_usage_usd_micros: Option<i64>,
) -> ThreadUsage {
    ThreadUsage {
        thread_id: thread_id.to_string(),
        estimated_usage_credits_micros,
        estimated_usage_usd_micros,
        groups: Vec::new(),
    }
}

#[test]
fn estimated_cost_formats_dollars_and_sub_cent_values() {
    assert_eq!(
        format_estimated_usd_micros(/*micros*/ 0),
        Some("~$0.00".to_string())
    );
    assert_eq!(
        format_estimated_usd_micros(/*micros*/ 1),
        Some("~$0.000001".to_string())
    );
    assert_eq!(
        format_estimated_usd_micros(/*micros*/ 400),
        Some("~$0.0004".to_string())
    );
    assert_eq!(
        format_estimated_usd_micros(/*micros*/ 1_820_000),
        Some("~$1.82".to_string())
    );
    assert_eq!(
        format_estimated_usd_micros(/*micros*/ 995_000),
        Some("~$1.00".to_string())
    );
    assert_eq!(format_estimated_usd_micros(/*micros*/ -1), None);
}

#[tokio::test]
async fn temporary_thread_usage_failures_have_bounded_retries() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.refresh_status_line();

    for attempt in 0..=THREAD_USAGE_RETRY_DELAYS.len() {
        let request_id = match rx.try_recv() {
            Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
            event => panic!("expected retry attempt {attempt}, got {event:?}"),
        };
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Err("thread usage temporarily unavailable".to_string()),
        ));

        if attempt < THREAD_USAGE_RETRY_DELAYS.len() {
            assert!(chat.thread_usage.retry_due_at.is_some());
            chat.thread_usage.retry_due_at = Some(Instant::now());
            chat.refresh_thread_usage_if_settlement_due();
        }
    }

    assert!(chat.thread_usage.retry_due_at.is_none());
    assert!(rx.try_recv().is_err(), "retries must stop after the bound");
}

#[tokio::test]
async fn status_history_survives_exhausted_billing_retries() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.refresh_status_line();
    let mut request_id = thread_usage_request_id(&mut rx);

    chat.add_status_output(
        /*refreshing_rate_limits*/ false, /*request_id*/ None,
    );
    let first_status_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected the first /status card, got {event:?}"),
    };

    for attempt in 0..=THREAD_USAGE_RETRY_DELAYS.len() {
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Err("thread usage temporarily unavailable".to_string()),
        ));
        assert_eq!(chat.thread_usage.status_history_handles.len(), 1);

        if attempt < THREAD_USAGE_RETRY_DELAYS.len() {
            chat.thread_usage.retry_due_at = Some(Instant::now());
            chat.refresh_thread_usage_if_settlement_due();
            request_id = thread_usage_request_id(&mut rx);
        }
    }

    assert_eq!(chat.thread_usage.retry_due_at, None);
    chat.add_status_output(
        /*refreshing_rate_limits*/ false, /*request_id*/ None,
    );
    let retry_status_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected a fresh /status card, got {event:?}"),
    };
    request_id = thread_usage_request_id(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(thread_usage(
            thread_id,
            /*estimated_usage_credits_micros*/ 50_000_000,
            Some(2_100_000),
        ))),
    ));

    for status_cell in [&first_status_cell, &retry_status_cell] {
        let rendered = status_cell
            .display_lines(/*width*/ 90)
            .into_iter()
            .flat_map(|line| line.spans)
            .map(|span| span.content.into_owned())
            .collect::<String>();
        assert!(rendered.contains("50 credits · ~$2.10"));
    }
    assert!(chat.thread_usage.status_history_handles.is_empty());
}

#[tokio::test]
async fn status_history_updates_again_after_billing_settles() {
    for (selected_item, stale_credits_micros, stale_usd_micros) in [
        ("estimated-thread-cost", 40_000_000, 1_820_000),
        ("thread-credits", 50_000_000, 1_820_000),
        ("estimated-thread-cost", 40_000_000, 2_100_000),
    ] {
        let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.has_codex_backend_auth = true;
        chat.plan_type = Some(PlanType::Business);
        chat.config.tui_status_line = Some(vec![selected_item.to_string()]);
        chat.refresh_status_line();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 40_000_000,
                Some(1_820_000),
            ))),
        ));

        chat.refresh_thread_usage_after_turn();
        let request_id = thread_usage_request_id(&mut rx);
        chat.add_status_output(
            /*refreshing_rate_limits*/ false, /*request_id*/ None,
        );
        let status_cell = match rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => cell,
            event => panic!("expected an immediate /status card, got {event:?}"),
        };
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                stale_credits_micros,
                Some(stale_usd_micros),
            ))),
        ));
        assert_eq!(chat.thread_usage.status_history_handles.len(), 1);

        chat.thread_usage.settlement_refresh_due_at = Some(Instant::now());
        chat.refresh_thread_usage_if_settlement_due();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 50_000_000,
                Some(2_100_000),
            ))),
        ));

        let rendered = status_cell
            .display_lines(/*width*/ 90)
            .into_iter()
            .flat_map(|line| line.spans)
            .map(|span| span.content.into_owned())
            .collect::<String>();
        assert!(rendered.contains("50 credits · ~$2.10"));
        assert!(chat.thread_usage.status_history_handles.is_empty());
    }
}

#[tokio::test]
async fn replayed_turn_completions_do_not_start_live_billing_refreshes() {
    for replay_kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        for (status, error) in [
            (TurnStatus::Completed, None),
            (TurnStatus::Interrupted, None),
            (TurnStatus::Failed, None),
            (
                TurnStatus::Failed,
                Some(TurnError {
                    message: "the replayed turn failed".to_string(),
                    codex_error_info: Some(CodexErrorInfo::ServerOverloaded),
                    additional_details: None,
                }),
            ),
        ] {
            let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
            let thread_id = ThreadId::new();
            chat.thread_id = Some(thread_id);
            chat.has_codex_backend_auth = true;
            chat.plan_type = Some(PlanType::Business);
            chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
            chat.refresh_status_line();
            let request_id = thread_usage_request_id(&mut rx);
            assert!(chat.finish_thread_usage_refresh(
                thread_id,
                request_id,
                Ok(ThreadUsageOutcome::Available(thread_usage(
                    thread_id,
                    /*estimated_usage_credits_micros*/ 46_000_000,
                    Some(1_820_000),
                ))),
            ));

            chat.replay_thread_turns(
                vec![Turn {
                    id: "replayed-turn".to_string(),
                    items: Vec::new(),
                    items_view: TurnItemsView::NotLoaded,
                    status: status.clone(),
                    error,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                }],
                replay_kind,
            );

            assert!(
                !std::iter::from_fn(|| rx.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::RefreshThreadUsage { .. })),
                "{replay_kind:?} replay of {status:?} must not start live billing settlement"
            );
            assert_eq!(chat.thread_usage.settlement_baseline_credits_micros, None);
            assert_eq!(chat.thread_usage.settlement_baseline_usd_micros, None);
            assert_eq!(chat.thread_usage.settlement_refresh_due_at, None);
            assert!(!chat.thread_usage.replaying_turn_completion);
        }
    }
}

#[tokio::test]
async fn replayed_errors_do_not_start_live_billing_refreshes() {
    for replay_kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.has_codex_backend_auth = true;
        chat.plan_type = Some(PlanType::Business);
        chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
        chat.refresh_status_line();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 46_000_000,
                Some(1_820_000),
            ))),
        ));

        let notification = ServerNotification::Error(ErrorNotification {
            error: TurnError {
                message: "the replayed turn failed".to_string(),
                codex_error_info: Some(CodexErrorInfo::ServerOverloaded),
                additional_details: None,
            },
            will_retry: false,
            thread_id: thread_id.to_string(),
            turn_id: "replayed-turn".to_string(),
        });
        chat.handle_server_notification(notification.clone(), Some(replay_kind));
        assert!(
            !std::iter::from_fn(|| rx.try_recv().ok())
                .any(|event| matches!(event, AppEvent::RefreshThreadUsage { .. })),
            "{replay_kind:?} error replay must not start live billing settlement"
        );
        assert!(!chat.thread_usage.replaying_turn_completion);

        chat.handle_server_notification(notification, /*replay_kind*/ None);
        assert!(
            std::iter::from_fn(|| rx.try_recv().ok())
                .any(|event| matches!(event, AppEvent::RefreshThreadUsage { .. })),
            "a later live error must still refresh billing"
        );
    }
}

#[tokio::test]
async fn transient_zero_cost_preserves_fresh_credits_and_breakdowns() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec![
        "thread-credits".to_string(),
        "estimated-thread-cost".to_string(),
    ]);
    chat.config.tui_terminal_title = chat.config.tui_status_line.clone();
    chat.refresh_status_surfaces();
    let request_id = thread_usage_request_id(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(thread_usage(
            thread_id,
            /*estimated_usage_credits_micros*/ 46_000_000,
            Some(1_820_000),
        ))),
    ));

    chat.refresh_thread_usage_after_turn();
    let request_id = thread_usage_request_id(&mut rx);
    let fresh_usage = ThreadUsage {
        groups: vec![ThreadUsageBreakdownGroup {
            model: Some("gpt-5.4".to_string()),
            reasoning_effort: Some("high".to_string()),
            speed: Some("fast".to_string()),
            estimated_usage_credits_micros: 50_000_000,
            net_new_input_tokens: Some(80),
            cached_input_tokens: Some(20),
            input_tokens: Some(100),
            output_tokens: Some(40),
            total_tokens: Some(140),
        }],
        ..thread_usage(
            thread_id,
            /*estimated_usage_credits_micros*/ 50_000_000,
            Some(0),
        )
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(fresh_usage.clone())),
    ));

    assert_eq!(
        chat.estimated_thread_usage(),
        Some(&ThreadUsage {
            estimated_usage_usd_micros: Some(1_820_000),
            ..fresh_usage
        })
    );
    assert_eq!(
        chat.status_line_text(),
        Some("50 credits · ~$1.82".to_string())
    );
    assert_eq!(
        chat.last_terminal_title,
        Some("50 credits | ~$1.82".to_string())
    );
    assert!(chat.thread_usage.settlement_refresh_due_at.is_some());
}

#[tokio::test]
async fn transient_zero_credits_preserves_credits_only_estimates() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["thread-credits".to_string()]);
    chat.config.tui_terminal_title = chat.config.tui_status_line.clone();
    chat.refresh_status_surfaces();
    let request_id = thread_usage_request_id(&mut rx);
    let previous_usage = ThreadUsage {
        groups: vec![ThreadUsageBreakdownGroup {
            model: Some("gpt-5.4".to_string()),
            reasoning_effort: Some("high".to_string()),
            speed: Some("fast".to_string()),
            estimated_usage_credits_micros: 46_000_000,
            net_new_input_tokens: Some(80),
            cached_input_tokens: Some(20),
            input_tokens: Some(100),
            output_tokens: Some(40),
            total_tokens: Some(140),
        }],
        ..thread_usage(
            thread_id, /*estimated_usage_credits_micros*/ 46_000_000,
            /*estimated_usage_usd_micros*/ None,
        )
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(previous_usage.clone())),
    ));

    chat.refresh_thread_usage_after_turn();
    let request_id = thread_usage_request_id(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(thread_usage(
            thread_id, /*estimated_usage_credits_micros*/ 0,
            /*estimated_usage_usd_micros*/ None,
        ))),
    ));

    assert_eq!(chat.estimated_thread_usage(), Some(&previous_usage));
    assert_eq!(chat.status_line_text(), Some("46 credits".to_string()));
    assert_eq!(chat.last_terminal_title, Some("46 credits".to_string()));
    assert!(chat.thread_usage.settlement_refresh_due_at.is_some());
}

#[tokio::test]
async fn cost_settlement_waits_for_new_or_missing_usd_estimates() {
    for (previous_usd_micros, stale_usd_micros) in [
        (Some(1_820_000), Some(1_820_000)),
        (None, None),
        (None, Some(0)),
    ] {
        let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.has_codex_backend_auth = true;
        chat.plan_type = Some(PlanType::Business);
        chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
        chat.refresh_status_line();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 40_000_000,
                previous_usd_micros,
            ))),
        ));

        chat.refresh_thread_usage_after_turn();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 50_000_000,
                stale_usd_micros,
            ))),
        ));
        assert_eq!(
            chat.thread_usage.settlement_baseline_credits_micros,
            Some(40_000_000)
        );
        assert_eq!(
            chat.thread_usage.settlement_baseline_usd_micros,
            previous_usd_micros
        );
        assert!(chat.thread_usage.settlement_refresh_due_at.is_some());

        chat.thread_usage.settlement_refresh_due_at = Some(Instant::now());
        chat.refresh_thread_usage_if_settlement_due();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 50_000_000,
                Some(2_100_000),
            ))),
        ));

        assert_eq!(chat.status_line_text(), Some("~$2.10".to_string()));
        assert_eq!(chat.thread_usage.settlement_baseline_credits_micros, None);
        assert_eq!(chat.thread_usage.settlement_baseline_usd_micros, None);
        assert_eq!(chat.thread_usage.settlement_refresh_due_at, None);
    }
}

#[tokio::test]
async fn credits_only_settlement_completes_without_usd_estimates() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_terminal_title = Some(vec!["thread-credits".to_string()]);
    chat.refresh_terminal_title();
    let request_id = thread_usage_request_id(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(thread_usage(
            thread_id, /*estimated_usage_credits_micros*/ 40_000_000,
            /*estimated_usage_usd_micros*/ None,
        ))),
    ));

    chat.refresh_thread_usage_after_turn();
    let request_id = thread_usage_request_id(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(thread_usage(
            thread_id, /*estimated_usage_credits_micros*/ 50_000_000,
            /*estimated_usage_usd_micros*/ None,
        ))),
    ));

    assert_eq!(chat.last_terminal_title, Some("50 credits".to_string()));
    assert_eq!(chat.thread_usage.settlement_baseline_credits_micros, None);
    assert_eq!(chat.thread_usage.settlement_baseline_usd_micros, None);
    assert_eq!(chat.thread_usage.settlement_refresh_due_at, None);
}

#[tokio::test]
async fn combined_billing_surfaces_wait_for_credits_and_cost() {
    for (status_line_item, title_item) in [
        ("thread-credits", "estimated-thread-cost"),
        ("estimated-thread-cost", "thread-credits"),
    ] {
        let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.has_codex_backend_auth = true;
        chat.plan_type = Some(PlanType::Business);
        chat.config.tui_status_line = Some(vec![status_line_item.to_string()]);
        chat.config.tui_terminal_title = Some(vec![title_item.to_string()]);
        chat.refresh_status_surfaces();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 40_000_000,
                Some(1_820_000),
            ))),
        ));

        chat.refresh_thread_usage_after_turn();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 50_000_000,
                Some(1_820_000),
            ))),
        ));
        assert!(chat.thread_usage.settlement_refresh_due_at.is_some());

        chat.thread_usage.settlement_refresh_due_at = Some(Instant::now());
        chat.refresh_thread_usage_if_settlement_due();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 50_000_000,
                Some(2_100_000),
            ))),
        ));

        assert_eq!(chat.thread_usage.settlement_baseline_credits_micros, None);
        assert_eq!(chat.thread_usage.settlement_baseline_usd_micros, None);
        assert_eq!(chat.thread_usage.settlement_refresh_due_at, None);
    }
}

#[tokio::test]
async fn billing_surfaces_render_for_every_supported_enterprise_plan() {
    for plan_type in [
        PlanType::Business,
        PlanType::EnterpriseCbpUsageBased,
        PlanType::EnterpriseCbpAutomation,
    ] {
        let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.has_codex_backend_auth = true;
        chat.plan_type = Some(plan_type);
        chat.config.tui_status_line = Some(vec![
            "thread-credits".to_string(),
            "estimated-thread-cost".to_string(),
        ]);
        chat.config.tui_terminal_title = chat.config.tui_status_line.clone();
        chat.refresh_status_surfaces();
        let request_id = thread_usage_request_id(&mut rx);
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(thread_usage(
                thread_id,
                /*estimated_usage_credits_micros*/ 5_200_000,
                Some(210_000),
            ))),
        ));

        assert_eq!(
            chat.status_line_text(),
            Some("5.2 credits · ~$0.21".to_string())
        );
        assert_eq!(
            chat.last_terminal_title,
            Some("5.2 credits | ~$0.21".to_string())
        );
        assert!(rx.try_recv().is_err(), "billing surfaces share a request");
    }
}

#[tokio::test]
async fn early_draw_rearms_thread_usage_retry_after_immediate_redraw() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.refresh_status_line();

    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected initial thread usage request, got {event:?}"),
    };
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    chat.frame_requester = frame_requester;

    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Err("thread usage temporarily unavailable".to_string()),
    ));
    let retry_due_at = chat
        .thread_usage
        .retry_due_at
        .expect("temporary failures schedule a retry");
    let scheduled = std::iter::from_fn(|| frame_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(scheduled.iter().any(|deadline| *deadline >= retry_due_at));
    assert!(scheduled.iter().any(|deadline| *deadline < retry_due_at));

    chat.pre_draw_tick();

    let rearmed = std::iter::from_fn(|| frame_rx.try_recv().ok())
        .find(|deadline| *deadline >= retry_due_at)
        .expect("the immediate redraw must re-arm the pending billing retry");
    assert!(rearmed.duration_since(retry_due_at) < Duration::from_secs(/*secs*/ 1));
}

#[tokio::test]
async fn unchanged_thread_usage_has_bounded_settlement_refreshes() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.refresh_status_line();

    let initial_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected initial thread usage request, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        initial_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));

    chat.refresh_thread_usage_after_turn();
    for attempt in 0..=THREAD_USAGE_SETTLEMENT_DELAYS.len() {
        let request_id = match rx.try_recv() {
            Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
            event => panic!("expected settlement attempt {attempt}, got {event:?}"),
        };
        assert!(chat.finish_thread_usage_refresh(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(ThreadUsage {
                thread_id: thread_id.to_string(),
                estimated_usage_credits_micros: 46_000_000,
                estimated_usage_usd_micros: Some(1_820_000),
                groups: Vec::new(),
            })),
        ));

        if attempt < THREAD_USAGE_SETTLEMENT_DELAYS.len() {
            assert!(chat.thread_usage.settlement_refresh_due_at.is_some());
            chat.thread_usage.settlement_refresh_due_at = Some(Instant::now());
            chat.refresh_thread_usage_if_settlement_due();
        }
    }

    assert!(chat.thread_usage.settlement_refresh_due_at.is_none());
    assert!(
        chat.thread_usage
            .settlement_baseline_credits_micros
            .is_none()
    );
    assert!(rx.try_recv().is_err(), "settlement polling must stop");
}

#[tokio::test]
async fn early_draw_rearms_thread_usage_settlement_after_immediate_redraw() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.refresh_status_line();

    let initial_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected initial thread usage request, got {event:?}"),
    };
    let usage = ThreadUsage {
        thread_id: thread_id.to_string(),
        estimated_usage_credits_micros: 46_000_000,
        estimated_usage_usd_micros: Some(1_820_000),
        groups: Vec::new(),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        initial_request_id,
        Ok(ThreadUsageOutcome::Available(usage.clone())),
    ));

    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    chat.frame_requester = frame_requester;
    chat.refresh_thread_usage_after_turn();
    let settlement_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected immediate post-turn thread usage request, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        settlement_request_id,
        Ok(ThreadUsageOutcome::Available(usage)),
    ));
    let settlement_due_at = chat
        .thread_usage
        .settlement_refresh_due_at
        .expect("unchanged post-turn billing schedules a settlement refresh");
    let scheduled = std::iter::from_fn(|| frame_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        scheduled
            .iter()
            .any(|deadline| *deadline >= settlement_due_at)
    );
    assert!(
        scheduled
            .iter()
            .any(|deadline| *deadline < settlement_due_at)
    );

    chat.pre_draw_tick();

    let rearmed = std::iter::from_fn(|| frame_rx.try_recv().ok())
        .find(|deadline| *deadline >= settlement_due_at)
        .expect("the immediate redraw must re-arm the pending billing settlement");
    assert!(rearmed.duration_since(settlement_due_at) < Duration::from_secs(/*secs*/ 1));
}

#[tokio::test]
async fn thread_usage_settlement_requires_request_started_after_turn_completion() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.refresh_status_line();

    let initial_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected initial thread usage request, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        initial_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 40_000_000,
            estimated_usage_usd_micros: Some(1_600_000),
            groups: Vec::new(),
        })),
    ));

    chat.thread_usage.status_requested = true;
    chat.request_thread_usage();
    let pre_turn_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected pre-turn thread usage request, got {event:?}"),
    };
    chat.refresh_thread_usage_after_turn();
    assert_eq!(chat.thread_usage.settlement_request_id, None);
    assert_eq!(
        chat.thread_usage.settlement_baseline_credits_micros,
        Some(40_000_000)
    );

    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        pre_turn_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));
    let post_turn_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected a request started after turn completion, got {event:?}"),
    };
    assert_eq!(
        chat.thread_usage.settlement_baseline_credits_micros,
        Some(46_000_000)
    );
    assert!(chat.thread_usage.settlement_refresh_due_at.is_some());

    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        post_turn_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));
    assert!(chat.thread_usage.settlement_refresh_due_at.is_some());

    chat.thread_usage.settlement_refresh_due_at = Some(Instant::now());
    chat.refresh_thread_usage_if_settlement_due();
    let settlement_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected another post-turn settlement request, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        settlement_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(2_100_000),
            groups: Vec::new(),
        })),
    ));
    assert_eq!(chat.thread_usage.settlement_baseline_credits_micros, None);
    assert_eq!(chat.thread_usage.settlement_refresh_due_at, None);
}

#[tokio::test]
async fn deselecting_billing_surfaces_cancels_polling_and_preserves_status_cache() {
    let (mut chat, _sender, mut rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(PlanType::Business);
    chat.config.tui_status_line = Some(vec!["estimated-thread-cost".to_string()]);
    chat.config.tui_terminal_title = Some(vec!["thread-credits".to_string()]);
    chat.refresh_status_surfaces();

    let initial_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected initial thread usage request, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        initial_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));

    chat.thread_usage.status_requested = true;
    chat.request_thread_usage();
    assert!(chat.thread_usage.status_requested);
    let status_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected explicit /status refresh, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        status_request_id,
        Err("thread usage temporarily unavailable".to_string()),
    ));
    assert!(!chat.thread_usage.status_requested);
    assert!(chat.thread_usage.retry_due_at.is_some());

    chat.refresh_thread_usage_after_turn();
    let settlement_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected post-turn thread usage refresh, got {event:?}"),
    };
    assert!(chat.thread_usage.settlement_refresh_due_at.is_some());

    chat.config.tui_status_line = Some(Vec::new());
    chat.config.tui_terminal_title = Some(Vec::new());
    chat.refresh_status_surfaces();

    assert!(!chat.thread_usage.status_requested);
    assert!(!chat.thread_usage.requested);
    assert_eq!(chat.thread_usage.pending_request_id, None);
    assert_eq!(chat.thread_usage.retry_due_at, None);
    assert_eq!(chat.thread_usage.settlement_refresh_due_at, None);
    assert_eq!(chat.thread_usage.settlement_baseline_credits_micros, None);
    assert_eq!(
        chat.estimated_thread_usage(),
        Some(&ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })
    );
    assert!(!chat.finish_thread_usage_refresh(
        thread_id,
        settlement_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(2_100_000),
            groups: Vec::new(),
        })),
    ));
    chat.refresh_thread_usage_if_settlement_due();
    assert!(rx.try_recv().is_err(), "deselected billing must not poll");
}
