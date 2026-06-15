use super::*;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditOutcome;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use codex_app_server_protocol::RateLimitResetCreditsSummary;

const TEST_OVERLAY_VIEW_ID: &str = "usage-test-overlay";

#[tokio::test]
async fn usage_command_opens_reset_flow_directly() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Usage);

    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenRateLimitResetCredits));
}

#[tokio::test]
async fn usage_command_opens_token_activity_for_workspace_accounts() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Usage);

    assert_matches!(rx.try_recv(), Ok(AppEvent::RefreshTokenActivity { .. }));
}

#[tokio::test]
async fn rate_limit_reset_popup_states_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let mut states = Vec::new();

    let loading_request_id = chat.show_rate_limit_reset_loading_popup();
    record_popup(&chat, &mut states);
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        loading_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let empty_request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        empty_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 0 }),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let load_error_request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        load_error_request_id,
        Err("backend unavailable".to_string()),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let consuming_request_id = chat.show_rate_limit_reset_consuming_popup();
    record_popup(&chat, &mut states);
    assert!(!chat.finish_rate_limit_reset_consume(
        consuming_request_id,
        "redeem-1".to_string(),
        Err("request timed out".to_string()),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let nothing_request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(!finish_reset_consume_outcome(
        &mut chat,
        nothing_request_id,
        "redeem-2",
        ConsumeAccountRateLimitResetCreditOutcome::NothingToReset,
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let no_credit_request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(!finish_reset_consume_outcome(
        &mut chat,
        no_credit_request_id,
        "redeem-3",
        ConsumeAccountRateLimitResetCreditOutcome::NoCredit,
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let success_request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(finish_reset_consume_outcome(
        &mut chat,
        success_request_id,
        "redeem-4",
        ConsumeAccountRateLimitResetCreditOutcome::Reset,
    ));
    record_popup(&chat, &mut states);
    assert!(chat.finish_post_consume_reset_credits_refresh(
        success_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 1 }),
    ));
    record_popup(&chat, &mut states);

    assert_chatwidget_snapshot!("rate_limit_reset_popup_states", states.join("\n---\n"));
}

#[tokio::test]
async fn rate_limit_reset_retry_reuses_idempotency_key() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(!chat.finish_rate_limit_reset_consume(
        request_id,
        "stable-redeem-id".to_string(),
        Err("response lost".to_string()),
    ));

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::ConsumeRateLimitResetCredit { idempotency_key })
            if idempotency_key == "stable-redeem-id"
    );
}

#[tokio::test]
async fn rate_limit_reset_redemption_cannot_be_dismissed_while_in_flight() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    let request_id = chat.show_rate_limit_reset_consuming_popup();
    dismiss_popup(&mut chat);
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Using a reset..."));

    assert!(finish_reset_consume_outcome(
        &mut chat,
        request_id,
        "redeem-123",
        ConsumeAccountRateLimitResetCreditOutcome::Reset,
    ));
    dismiss_popup(&mut chat);
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Refreshing..."));

    assert!(chat.finish_post_consume_reset_credits_refresh(
        request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 1 }),
    ));
    dismiss_popup(&mut chat);
    assert!(chat.bottom_pane.no_modal_or_popup_active());
}

#[tokio::test]
async fn rate_limit_reset_redemption_allows_ctrl_c_to_quit_while_in_flight() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.show_rate_limit_reset_consuming_popup();
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Using a reset..."));
}

#[tokio::test]
async fn already_redeemed_is_an_idempotent_success() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_consuming_popup();

    assert!(finish_reset_consume_outcome(
        &mut chat,
        request_id,
        "stable-redeem-id",
        ConsumeAccountRateLimitResetCreditOutcome::AlreadyRedeemed,
    ));
    assert!(chat.finish_post_consume_reset_credits_refresh(
        request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 0 }),
    ));
    assert!(
        render_bottom_popup(&chat, /*width*/ 80)
            .contains("Usage reset. You have 0 rate-limit resets left.")
    );
}

#[tokio::test]
async fn account_change_invalidates_pending_reset_requests() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.show_rate_limit_reset_loading_popup();

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ false, /*has_codex_backend_auth*/ false,
    );

    assert!(!chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));
    assert!(chat.bottom_pane.no_modal_or_popup_active());
}

#[tokio::test]
async fn clearing_pending_reset_hint_preserves_in_flight_redemption() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let consume_request_id = chat.show_rate_limit_reset_consuming_popup();
    let hint_request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));

    chat.clear_pending_rate_limit_reset_hint();

    assert!(chat.pending_rate_limit_reset_hint().is_none());
    assert!(finish_reset_consume_outcome(
        &mut chat,
        consume_request_id,
        "redeem-after-rollback",
        ConsumeAccountRateLimitResetCreditOutcome::Reset,
    ));
}

#[tokio::test]
async fn rate_limit_reset_load_result_updates_popup_beneath_overlay() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_loading_popup();
    show_usage_test_overlay(&mut chat);

    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(TEST_OVERLAY_VIEW_ID)
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        render_bottom_popup(&chat, /*width*/ 80)
            .contains("You have 2 rate-limit resets available.")
    );
}

#[tokio::test]
async fn rate_limit_reset_success_updates_popup_beneath_overlay() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_consuming_popup();
    show_usage_test_overlay(&mut chat);

    assert!(finish_reset_consume_outcome(
        &mut chat,
        request_id,
        "redeem-covered",
        ConsumeAccountRateLimitResetCreditOutcome::Reset,
    ));
    assert!(chat.finish_post_consume_reset_credits_refresh(
        request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 1 }),
    ));
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(TEST_OVERLAY_VIEW_ID)
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        render_bottom_popup(&chat, /*width*/ 80)
            .contains("Usage reset. You have 1 rate-limit reset left.")
    );
}

#[tokio::test]
async fn account_change_dismisses_reset_popup_beneath_overlay() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.show_rate_limit_reset_loading_popup();
    show_usage_test_overlay(&mut chat);

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ false, /*has_codex_backend_auth*/ false,
    );
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(TEST_OVERLAY_VIEW_ID)
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(chat.bottom_pane.no_modal_or_popup_active());
}

#[tokio::test]
async fn startup_check_shows_available_reset_hint_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let hint_request_id = chat.start_rate_limit_reset_startup_check();

    assert!(chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));
    let rendered = lines_to_single_string(
        &chat
            .pending_rate_limit_reset_hint()
            .expect("pending reset hint")
            .display_lines(/*width*/ 80),
    );
    assert_chatwidget_snapshot!("rate_limit_reset_available_hint", rendered);
}

#[tokio::test]
async fn startup_reset_hint_waits_for_active_output_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let hint_request_id = chat.start_rate_limit_reset_startup_check();
    chat.transcript.active_cell = Some(Box::new(PlainHistoryCell::new(vec![Line::from(
        "active tool",
    )])));

    assert!(chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));

    assert!(chat.usage_history_insertion_blocked());
    assert!(drain_insert_history(&mut rx).is_empty());
    assert_chatwidget_snapshot!(
        "rate_limit_reset_hint_waits_for_active_output",
        lines_to_single_string(
            &chat
                .active_cell_transcript_lines(/*width*/ 80)
                .expect("active output with reset hint"),
        )
    );

    chat.flush_active_cell();

    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::CommitPendingUsageOutput));
}

#[tokio::test]
async fn opening_rate_limit_reset_flow_invalidates_in_flight_startup_hint() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let hint_request_id = chat.start_rate_limit_reset_startup_check();

    chat.show_rate_limit_reset_loading_popup();

    assert!(!chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));
    assert!(chat.pending_rate_limit_reset_hint().is_none());
}

#[tokio::test]
async fn starting_rate_limit_reset_redemption_clears_deferred_startup_hint() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let hint_request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 2 }),
    ));
    assert!(chat.pending_rate_limit_reset_hint().is_some());

    chat.show_rate_limit_reset_consuming_popup();

    assert!(chat.pending_rate_limit_reset_hint().is_none());
}

#[tokio::test]
async fn startup_check_omits_reset_hint_when_none_are_available() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let hint_request_id = chat.start_rate_limit_reset_startup_check();

    assert!(chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Ok(RateLimitResetCreditsSummary { available_count: 0 }),
    ));
    assert!(chat.pending_rate_limit_reset_hint().is_none());
}

fn consume_response(
    outcome: ConsumeAccountRateLimitResetCreditOutcome,
) -> ConsumeAccountRateLimitResetCreditResponse {
    ConsumeAccountRateLimitResetCreditResponse { outcome }
}

fn finish_reset_consume_outcome(
    chat: &mut ChatWidget,
    request_id: u64,
    idempotency_key: &str,
    outcome: ConsumeAccountRateLimitResetCreditOutcome,
) -> bool {
    chat.finish_rate_limit_reset_consume(
        request_id,
        idempotency_key.to_string(),
        Ok(consume_response(outcome)),
    )
}

fn record_popup(chat: &ChatWidget, states: &mut Vec<String>) {
    states.push(render_bottom_popup(chat, /*width*/ 80));
}

fn dismiss_popup(chat: &mut ChatWidget) {
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
}

fn show_usage_test_overlay(chat: &mut ChatWidget) {
    chat.bottom_pane.show_selection_view(SelectionViewParams {
        view_id: Some(TEST_OVERLAY_VIEW_ID),
        title: Some("Covering overlay".to_string()),
        items: vec![SelectionItem {
            name: "Close".to_string(),
            dismiss_on_select: true,
            ..Default::default()
        }],
        ..Default::default()
    });
}
