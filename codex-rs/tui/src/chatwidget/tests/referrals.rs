use super::*;
use codex_chatgpt::referrals::ReferralIdentity;
use codex_chatgpt::referrals::ReferralOffer;
use codex_chatgpt::referrals::ReferralRewardStatus;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

use crate::app_event::ReferralInviteResult;
use crate::chatwidget::referrals::REFERRAL_OFFER_MAX_AGE;

#[tokio::test]
async fn usage_menu_adds_eligible_referral_offer() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Usage);
    let request_id = referral_refresh_request(&mut rx);
    chat.finish_referral_offer_refresh(request_id, Some(test_offer()));

    assert_chatwidget_snapshot!(
        "usage_menu_with_referral_offer",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn referral_popup_flow_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let offer = test_offer();
    chat.start_referral_offer_refresh();
    let request_id = referral_refresh_request(&mut rx);
    chat.finish_referral_offer_refresh(request_id, Some(offer));

    chat.show_referral_email_prompt();
    let email_prompt = render_bottom_popup(&chat, /*width*/ 80);
    chat.show_referral_confirmation("friend@example.com".to_string());
    let confirmation = render_bottom_popup(&chat, /*width*/ 50);
    let confirmed_offer = test_offer();
    let (send_request_id, _) = chat
        .start_referral_send("friend@example.com", confirmed_offer)
        .expect("eligible offer starts send");
    let sending = render_bottom_popup(&chat, /*width*/ 80);
    chat.finish_referral_send(
        send_request_id,
        ReferralInviteResult::Sent(ReferralRewardStatus::Included),
    );
    let success = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        chat.bottom_pane
            .dismiss_active_view_if_id("referral-invite"),
        "success popup should be dismissible"
    );
    assert_ne!(
        chat.bottom_pane.active_view_id(),
        Some("referral-invite"),
        "closing success should not reveal stale referral UI"
    );

    assert_snapshot!(
        "referral_popup_flow",
        format!(
            "EMAIL\n{email_prompt}\nCONFIRMATION\n{confirmation}\nSENDING\n{sending}\nSUCCESS\n{success}"
        )
    );
}

#[tokio::test]
async fn account_change_invalidates_referral_offer() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let offer = test_offer();
    chat.start_referral_offer_refresh();
    let stale_request_id = referral_refresh_request(&mut rx);

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    chat.start_referral_offer_refresh();
    let current_request_id = referral_refresh_request(&mut rx);
    assert_ne!(stale_request_id, current_request_id);
    chat.finish_referral_offer_refresh(stale_request_id, Some(offer));

    assert_eq!(
        chat.start_referral_send("friend@example.com", test_offer()),
        None
    );
}

#[tokio::test]
async fn account_change_dismisses_open_referral_prompt() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.start_referral_offer_refresh();
    let request_id = referral_refresh_request(&mut rx);
    chat.finish_referral_offer_refresh(request_id, Some(test_offer()));
    chat.show_referral_email_prompt();
    assert_eq!(chat.bottom_pane.active_view_id(), Some("referral-invite"));

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );

    assert_ne!(
        chat.bottom_pane.active_view_id(),
        Some("referral-invite"),
        "account changes should dismiss open referral prompts"
    );
}

#[tokio::test]
async fn referral_refresh_preserves_last_loaded_offer() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.start_referral_offer_refresh();
    let request_id = referral_refresh_request(&mut rx);
    chat.finish_referral_offer_refresh(request_id, Some(test_offer()));

    chat.start_referral_offer_refresh();
    let _pending_request_id = referral_refresh_request(&mut rx);

    assert!(
        chat.referral_menu_item().is_some(),
        "refreshing should not discard the last loaded offer"
    );
}

#[tokio::test]
async fn expired_referral_offer_is_not_selectable() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.start_referral_offer_refresh();
    let request_id = referral_refresh_request(&mut rx);
    chat.finish_referral_offer_refresh(request_id, Some(test_offer()));

    chat.referral_state.offer_loaded_at =
        Some(Instant::now() - REFERRAL_OFFER_MAX_AGE - Duration::from_secs(1));

    assert!(
        chat.referral_menu_item().is_none(),
        "expired referral offers should be hidden"
    );
    assert_eq!(
        chat.start_referral_send("friend@example.com", test_offer()),
        None
    );
}

#[tokio::test]
async fn changed_referral_offer_requires_reconfirmation_before_send() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.start_referral_offer_refresh();
    let request_id = referral_refresh_request(&mut rx);
    let confirmed_offer = test_offer();
    chat.finish_referral_offer_refresh(request_id, Some(confirmed_offer.clone()));

    let mut replacement_offer = test_offer();
    replacement_offer.rules = vec!["Different terms apply.".to_string()];
    chat.start_referral_offer_refresh();
    let request_id = referral_refresh_request(&mut rx);
    chat.finish_referral_offer_refresh(request_id, Some(replacement_offer));

    assert_eq!(
        chat.start_referral_send("friend@example.com", confirmed_offer),
        None,
        "changed referral offers should require a new confirmation"
    );
}

fn test_offer() -> ReferralOffer {
    ReferralOffer {
        description: "Invite someone to Codex. Rewards may apply.".to_string(),
        rules: vec!["Your friend must be new to Codex.".to_string()],
        grant_action: Some(serde_json::json!("usage_reset")),
        grant_amount: Some(serde_json::json!(1)),
        requires_explicit_confirmation: true,
        identity: ReferralIdentity {
            user_id: "user-1".to_string(),
            account_id: "account-1".to_string(),
        },
    }
}

fn referral_refresh_request(rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> Uuid {
    let event = rx.try_recv().expect("referral refresh event");
    let AppEvent::RefreshReferralOffer(request_id) = event else {
        panic!("expected referral refresh, got {event:?}");
    };
    request_id
}
