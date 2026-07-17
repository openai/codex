use super::super::reset_credits::ResetCreditOption;
use super::super::reset_credits::reset_credit_options;
use super::*;
use chrono::TimeZone;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditOutcome;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use codex_app_server_protocol::RateLimitResetCredit;
use codex_app_server_protocol::RateLimitResetCreditStatus;
use codex_app_server_protocol::RateLimitResetCreditsSummary;
use codex_app_server_protocol::RateLimitResetType;
use pretty_assertions::assert_eq;
use uuid::Uuid;

const TEST_OVERLAY_VIEW_ID: &str = "usage-test-overlay";

fn reset_credits(available_count: i64) -> RateLimitResetCreditsSummary {
    RateLimitResetCreditsSummary {
        available_count,
        credits: None,
    }
}

fn detailed_reset_credits(
    available_count: i64,
    credits: Vec<RateLimitResetCredit>,
) -> RateLimitResetCreditsSummary {
    RateLimitResetCreditsSummary {
        available_count,
        credits: Some(credits),
    }
}

fn reset_credit(id: &str, expires_at: Option<i64>) -> RateLimitResetCredit {
    RateLimitResetCredit {
        id: id.to_string(),
        reset_type: RateLimitResetType::CodexRateLimits,
        status: RateLimitResetCreditStatus::Available,
        granted_at: 0,
        expires_at,
        title: None,
        description: None,
    }
}

fn reset_credit_with_title(id: &str, expires_at: Option<i64>, title: &str) -> RateLimitResetCredit {
    RateLimitResetCredit {
        title: Some(title.to_string()),
        ..reset_credit(id, expires_at)
    }
}

fn expiry_timestamp(day: u32, hour: u32, minute: u32) -> i64 {
    chrono::Local
        .with_ymd_and_hms(2026, 6, day, hour, minute, 0)
        .single()
        .expect("valid test timestamp")
        .timestamp()
}

fn show_rate_limit_reset_confirmation_from_event(
    chat: &mut ChatWidget,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> u64 {
    let Ok(AppEvent::OpenRateLimitResetConfirmation {
        picker_request_id,
        confirmation_gate,
        credit_id,
        reset_title,
        reset_detail,
        reset_description,
    }) = rx.try_recv()
    else {
        panic!("expected reset confirmation event");
    };
    assert!(chat.show_rate_limit_reset_confirmation(
        picker_request_id,
        confirmation_gate,
        credit_id,
        reset_title,
        reset_detail,
        reset_description,
    ));
    picker_request_id
}

#[test]
fn reset_credit_options_use_generic_copy_when_backend_copy_is_missing() {
    let mut credit = reset_credit("future-credit", /*expires_at*/ None);
    credit.reset_type = RateLimitResetType::Unknown;

    assert_eq!(
        reset_credit_options(&detailed_reset_credits(
            /*available_count*/ 1,
            vec![credit],
        )),
        vec![ResetCreditOption {
            credit_id: Some("future-credit".to_string()),
            name: "Full reset".to_string(),
            detail: Some("Does not expire.".to_string()),
            description: "Reset your current usage limits.".to_string(),
        }]
    );
}

#[tokio::test]
async fn usage_command_opens_menu_when_reset_is_available_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
    ));

    chat.dispatch_command(SlashCommand::Usage);

    assert_chatwidget_snapshot!(
        "usage_command_menu",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenTokenActivity));
}

#[tokio::test]
async fn usage_command_disables_reset_after_cached_zero_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));

    chat.dispatch_command(SlashCommand::Usage);

    assert_chatwidget_snapshot!(
        "usage_command_menu_without_resets",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::UsageMenu { request_id: 1 }
        })
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenTokenActivity));
}

#[tokio::test]
async fn usage_menu_refresh_enables_newly_available_reset() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));

    chat.dispatch_command(SlashCommand::Usage);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::UsageMenu { request_id: 1 }
        })
    );
    chat.finish_usage_menu_rate_limit_refresh(
        /*request_id*/ 1,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenRateLimitResetCredits));
}

#[tokio::test]
async fn usage_menu_refresh_failure_preserves_disabled_known_zero() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));

    chat.dispatch_command(SlashCommand::Usage);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::UsageMenu { request_id: 1 }
        })
    );
    chat.finish_usage_menu_rate_limit_refresh(
        /*request_id*/ 1,
        Vec::new(),
        Err("backend unavailable".to_string()),
    );

    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("No usage limit resets available."));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenTokenActivity));
}

#[tokio::test]
async fn account_update_invalidates_usage_menu_refresh_when_visible_state_is_unchanged() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let startup_request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        startup_request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));
    chat.dispatch_command(SlashCommand::Usage);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::UsageMenu { request_id: 1 }
        })
    );

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    chat.finish_usage_menu_rate_limit_refresh(
        /*request_id*/ 1,
        vec![snapshot(/*percent*/ 92.0)],
        Ok(reset_credits(/*available_count*/ 2)),
    );

    assert_eq!(chat.available_rate_limit_reset_credits, None);
    assert!(chat.rate_limit_snapshots_by_limit_id.is_empty());
    assert!(chat.bottom_pane.no_modal_or_popup_active());
}

#[tokio::test]
async fn usage_command_can_check_reset_availability_before_startup_refresh_finishes_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.start_rate_limit_reset_startup_check();

    chat.dispatch_command(SlashCommand::Usage);

    assert_chatwidget_snapshot!(
        "usage_command_menu_before_reset_refresh",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenRateLimitResetCredits));
}

#[tokio::test]
async fn usage_command_can_check_reset_availability_for_workspace_accounts() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Usage);

    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenRateLimitResetCredits));
}

#[tokio::test]
async fn usage_menu_rate_limit_reset_entry_opens_reset_flow() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
    ));
    chat.dispatch_command(SlashCommand::Usage);

    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenRateLimitResetCredits));
}

#[tokio::test]
async fn rate_limit_reset_popup_states_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let mut states = Vec::new();

    let loading_request_id = chat.show_rate_limit_reset_loading_popup();
    record_popup(&chat, &mut states);
    let first_expiry = expiry_timestamp(/*day*/ 18, /*hour*/ 9, /*minute*/ 39);
    let second_expiry = expiry_timestamp(/*day*/ 27, /*hour*/ 8, /*minute*/ 59);
    let mut rate_limit_snapshot = snapshot(/*percent*/ 50.0);
    rate_limit_snapshot.limit_id = Some("codex".to_string());
    rate_limit_snapshot
        .primary
        .as_mut()
        .expect("primary window")
        .window_duration_mins = Some(5 * 60);
    rate_limit_snapshot.secondary = Some(RateLimitWindow {
        used_percent: 50,
        window_duration_mins: Some(7 * 24 * 60),
        resets_at: None,
    });
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        loading_request_id,
        vec![rate_limit_snapshot],
        Ok(detailed_reset_credits(
            /*available_count*/ 2,
            vec![
                reset_credit_with_title(
                    "credit-2",
                    Some(second_expiry),
                    "Full reset (Weekly + 5 hr)",
                ),
                reset_credit_with_title(
                    "credit-1",
                    Some(first_expiry),
                    "Full reset (Weekly + 5 hr)",
                ),
            ],
        )),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let empty_request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        empty_request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let load_error_request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        load_error_request_id,
        Vec::new(),
        Err("backend unavailable".to_string()),
    ));
    record_popup(&chat, &mut states);

    dismiss_popup(&mut chat);
    let consuming_request_id = chat.show_rate_limit_reset_consuming_popup();
    record_popup(&chat, &mut states);
    assert!(!chat.finish_rate_limit_reset_consume(
        consuming_request_id,
        "redeem-1".to_string(),
        /*credit_id*/ None,
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
    ));
    record_popup(&chat, &mut states);

    assert_chatwidget_snapshot!("rate_limit_reset_popup_states", states.join("\n---\n"));
}

#[tokio::test]
async fn rate_limit_reset_picker_wraps_expiry_details_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_loading_popup();
    let expiry = expiry_timestamp(/*day*/ 18, /*hour*/ 9, /*minute*/ 39);
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(detailed_reset_credits(
            /*available_count*/ 2,
            vec![
                reset_credit("credit-2", /*expires_at*/ None),
                reset_credit("credit-1", Some(expiry)),
            ],
        )),
    ));

    assert_chatwidget_snapshot!(
        "rate_limit_reset_picker_narrow",
        render_bottom_popup(&chat, /*width*/ 44)
    );
}

#[tokio::test]
async fn rate_limit_reset_confirmation_uses_backend_copy_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let expiry = expiry_timestamp(/*day*/ 18, /*hour*/ 9, /*minute*/ 39);
    let mut credit =
        reset_credit_with_title("weekly-credit", Some(expiry), "Full reset (Weekly + 5 hr)");
    credit.description = Some("Reset your weekly and 5-hour usage limits.".to_string());
    let request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(detailed_reset_credits(
            /*available_count*/ 1,
            vec![credit],
        )),
    ));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let Ok(AppEvent::OpenRateLimitResetConfirmation {
        picker_request_id,
        confirmation_gate,
        credit_id,
        reset_title,
        reset_detail,
        reset_description,
    }) = rx.try_recv()
    else {
        panic!("expected reset confirmation event");
    };
    assert_eq!(
        (
            picker_request_id,
            credit_id.as_deref(),
            reset_title.as_str(),
            reset_detail.as_deref(),
            reset_description.as_str(),
        ),
        (
            request_id,
            Some("weekly-credit"),
            "Full reset (Weekly + 5 hr)",
            Some("Expires 09:39 on 18 Jun 2026."),
            "Reset your weekly and 5-hour usage limits.",
        )
    );
    assert!(chat.show_rate_limit_reset_confirmation(
        picker_request_id,
        confirmation_gate,
        credit_id,
        reset_title,
        reset_detail,
        reset_description,
    ));

    assert_chatwidget_snapshot!(
        "rate_limit_reset_confirmation",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn rate_limit_reset_confirmation_no_and_escape_return_to_picker() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
    ));
    let picker = render_bottom_popup(&chat, /*width*/ 80);
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    show_rate_limit_reset_confirmation_from_event(&mut chat, &mut rx);
    assert!(rx.try_recv().is_err());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), picker);

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    show_rate_limit_reset_confirmation_from_event(&mut chat, &mut rx);
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), picker);

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    show_rate_limit_reset_confirmation_from_event(&mut chat, &mut rx);
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), picker);
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn reset_picker_allows_only_one_pending_confirmation() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_loading_popup();
    let first_expiry = expiry_timestamp(/*day*/ 18, /*hour*/ 9, /*minute*/ 39);
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(detailed_reset_credits(
            /*available_count*/ 2,
            vec![
                reset_credit_with_title("credit-1", Some(first_expiry), "First reset"),
                reset_credit_with_title("credit-2", /*expires_at*/ None, "Second reset"),
            ],
        )),
    ));

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    show_rate_limit_reset_confirmation_from_event(&mut chat, &mut rx);
    assert!(rx.try_recv().is_err());

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    show_rate_limit_reset_confirmation_from_event(&mut chat, &mut rx);

    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Second reset · Does not expire."));
}

#[tokio::test]
async fn rate_limit_reset_picker_starts_with_soonest_expiries_and_keeps_all_rows_reachable() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_loading_popup();
    let first_expiry = expiry_timestamp(/*day*/ 18, /*hour*/ 9, /*minute*/ 39);
    let credits = (0..9)
        .rev()
        .map(|index| {
            reset_credit(
                &format!("credit-{index}"),
                Some(first_expiry + i64::from(index) * 86_400),
            )
        })
        .collect();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(detailed_reset_credits(/*available_count*/ 9, credits)),
    ));

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        rendered.contains("Expires 09:39 on 18 Jun 2026."),
        "{rendered}"
    );
    assert!(!rendered.contains("Full reset ("), "{rendered}");

    for _ in 0..8 {
        chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::OpenRateLimitResetConfirmation {
            picker_request_id,
            confirmation_gate: _,
            credit_id,
            reset_title,
            reset_detail,
            reset_description,
        }) if picker_request_id == request_id
            && credit_id.as_deref() == Some("credit-8")
            && reset_title == "Full reset"
            && reset_detail.as_deref() == Some("Expires 09:39 on 26 Jun 2026.")
            && reset_description == "Reset your current usage limits."
    );
}

#[tokio::test]
async fn rate_limit_reset_confirmation_can_use_reset() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(detailed_reset_credits(
            /*available_count*/ 1,
            vec![reset_credit("credit-1", /*expires_at*/ None)],
        )),
    ));

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    show_rate_limit_reset_confirmation_from_event(&mut chat, &mut rx);
    assert!(rx.try_recv().is_err());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let Ok(AppEvent::ConsumeRateLimitResetCredit {
        idempotency_key,
        credit_id,
    }) = rx.try_recv()
    else {
        panic!("expected reset consume event");
    };
    assert!(Uuid::parse_str(&idempotency_key).is_ok());
    assert_eq!(credit_id.as_deref(), Some("credit-1"));

    assert_eq!(chat.start_rate_limit_reset_consumption("wrong-key"), None);
    let consume_request_id = chat
        .start_rate_limit_reset_consumption(&idempotency_key)
        .expect("confirmed reset should start consumption");
    assert_eq!(
        chat.start_rate_limit_reset_consumption(&idempotency_key),
        None
    );
    assert!(!chat.finish_rate_limit_reset_consume(
        consume_request_id,
        idempotency_key,
        credit_id,
        Ok(consume_response(
            ConsumeAccountRateLimitResetCreditOutcome::NothingToReset,
        )),
    ));
    dismiss_popup(&mut chat);
    assert!(chat.bottom_pane.no_modal_or_popup_active());
}

#[tokio::test]
async fn rate_limit_reset_retry_reuses_idempotency_key() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(!chat.finish_rate_limit_reset_consume(
        request_id,
        "stable-redeem-id".to_string(),
        Some("credit-1".to_string()),
        Err("response lost".to_string()),
    ));

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let Ok(AppEvent::ConsumeRateLimitResetCredit {
        idempotency_key,
        credit_id,
    }) = rx.try_recv()
    else {
        panic!("expected reset retry event");
    };
    assert_eq!(
        (idempotency_key.as_str(), credit_id.as_deref()),
        ("stable-redeem-id", Some("credit-1"))
    );
    assert_eq!(chat.start_rate_limit_reset_consumption("wrong-key"), None);
    assert!(
        chat.start_rate_limit_reset_consumption(&idempotency_key)
            .is_some()
    );
}

#[tokio::test]
async fn no_credit_outcome_disables_reset_entry_in_usage_menu() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let startup_request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        startup_request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
    ));
    let consume_request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(!finish_reset_consume_outcome(
        &mut chat,
        consume_request_id,
        "redeem-1",
        ConsumeAccountRateLimitResetCreditOutcome::NoCredit,
    ));
    dismiss_popup(&mut chat);

    chat.dispatch_command(SlashCommand::Usage);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::UsageMenu { request_id: 2 }
        })
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenTokenActivity));

    chat.available_rate_limit_reset_credits = Some(2);
    let consume_request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(!chat.finish_rate_limit_reset_consume(
        consume_request_id,
        "redeem-selected".to_string(),
        Some("stale-credit".to_string()),
        Ok(consume_response(
            ConsumeAccountRateLimitResetCreditOutcome::NoCredit
        )),
    ));
    assert_eq!(chat.available_rate_limit_reset_credits, None);
    assert!(
        render_bottom_popup(&chat, /*width*/ 80).contains("That reset is no longer available.")
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenRateLimitResetCredits));
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));
    assert!(
        render_bottom_popup(&chat, /*width*/ 80)
            .contains("Usage reset. You have 0 usage limit resets left.")
    );
}

#[tokio::test]
async fn failed_post_consume_refresh_does_not_keep_stale_reset_count() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let startup_request_id = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        startup_request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
    ));
    let consume_request_id = chat.show_rate_limit_reset_consuming_popup();
    assert!(finish_reset_consume_outcome(
        &mut chat,
        consume_request_id,
        "redeem-with-refresh-error",
        ConsumeAccountRateLimitResetCreditOutcome::Reset,
    ));

    assert!(chat.finish_post_consume_reset_credits_refresh(
        consume_request_id,
        Vec::new(),
        Err("backend unavailable".to_string()),
    ));
    dismiss_popup(&mut chat);
    chat.dispatch_command(SlashCommand::Usage);

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("Check reset availability."));
    assert!(!rendered.contains("You have 2 usage limit resets available."));
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
    ));
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(TEST_OVERLAY_VIEW_ID)
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("2 usage limit resets available."));
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
    ));
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(TEST_OVERLAY_VIEW_ID)
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        render_bottom_popup(&chat, /*width*/ 80)
            .contains("Usage reset. You have 1 usage limit reset left.")
    );
}

#[tokio::test]
async fn account_change_dismisses_reset_popup_beneath_overlay() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let request_id = chat.show_rate_limit_reset_loading_popup();
    assert!(chat.finish_rate_limit_reset_credits_refresh(
        request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 1)),
    ));
    assert!(chat.show_rate_limit_reset_confirmation(
        request_id,
        Arc::new(AtomicBool::new(true)),
        /*credit_id*/ None,
        "Full reset".to_string(),
        /*reset_detail*/ None,
        "Reset your current usage limits.".to_string(),
    ));
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let Ok(AppEvent::ConsumeRateLimitResetCredit {
        idempotency_key,
        credit_id: None,
    }) = rx.try_recv()
    else {
        panic!("expected reset consume event");
    };
    show_usage_test_overlay(&mut chat);

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ false, /*has_codex_backend_auth*/ false,
    );
    assert!(!chat.show_rate_limit_reset_confirmation(
        request_id,
        Arc::new(AtomicBool::new(true)),
        /*credit_id*/ None,
        "Stale reset".to_string(),
        /*reset_detail*/ None,
        "This stale confirmation should be ignored.".to_string(),
    ));
    assert_eq!(
        chat.start_rate_limit_reset_consumption(&idempotency_key),
        None
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
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
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 0)),
    ));
    assert!(chat.pending_rate_limit_reset_hint().is_none());
}

#[tokio::test]
async fn startup_check_shows_reset_hint_for_workspace_account_with_credit() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.plan_type = Some(PlanType::Business);
    let hint_request_id = chat.start_rate_limit_reset_startup_check();

    assert!(chat.finish_rate_limit_reset_hint_refresh(
        hint_request_id,
        Vec::new(),
        Ok(reset_credits(/*available_count*/ 2)),
    ));
    assert!(chat.pending_rate_limit_reset_hint().is_some());
    assert_eq!(chat.available_rate_limit_reset_credits, Some(2));
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
        /*credit_id*/ None,
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
