use super::*;
use crate::security_setup::Identity;
use crate::security_setup::Notice;
use serde_json::json;

fn setup_notice() -> Notice {
    serde_json::from_value(json!({
        "title":"Keep using Daybreak mode",
        "description":"Set up Advanced Account Security with a hardware security key by October 1. Already Persona-verified? Add your key before October 15 to skip re-verification.",
        "action":{"label":"Set up security","url":"https://chatgpt.com/cyber"}
    }))
    .unwrap()
}

fn setup_identity() -> Identity {
    Identity {
        account: "account".into(),
        user: "user".into(),
    }
}

#[tokio::test]
async fn security_setup_preserves_draft_action_and_dismissal_across_widgets() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
    chat.show_security_setup(setup_identity(), setup_notice());
    let rendered = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 70));
    insta::assert_snapshot!("security_setup_banner", rendered);
    let (mut replacement, mut events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
    chat.app_event_tx
        .voice_only
        .store(/*val*/ true, std::sync::atomic::Ordering::Relaxed);
    replacement.inherit_security_setup(&mut chat);
    chat = replacement;
    assert!(render_bottom_popup(&chat, /*width*/ 70).contains("Set up security"));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
    assert!(
        matches!(events.try_recv(), Ok(AppEvent::OpenUrlInBrowser { url }) if url == "https://chatgpt.com/cyber")
    );
    assert!(render_bottom_popup(&chat, /*width*/ 70).contains("Set up security"));
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!render_bottom_popup(&chat, /*width*/ 70).contains("Set up security"));

    let (mut replacement, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
    replacement.inherit_security_setup(&mut chat);
    replacement.show_security_setup(setup_identity(), setup_notice());
    assert_eq!(
        replacement.bottom_pane.inline_banner_lifecycle(),
        (true, true)
    );
    assert!(!render_bottom_popup(&replacement, /*width*/ 70).contains("Set up security"));

    replacement.invalidate_security_setup();
    replacement.show_security_setup(setup_identity(), setup_notice());
    assert!(!render_bottom_popup(&replacement, /*width*/ 70).contains("Set up security"));

    // A verified change of account starts a new notice occurrence.
    replacement.invalidate_security_setup();
    replacement.show_security_setup(
        Identity {
            account: "other-account".into(),
            ..setup_identity()
        },
        setup_notice(),
    );
    assert!(render_bottom_popup(&replacement, /*width*/ 70).contains("Set up security"));
}

#[tokio::test]
async fn security_setup_dismissal_survives_usage_banner_replacement() {
    for dismiss_security in [true, false] {
        let (mut chat, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
        chat.show_security_setup(setup_identity(), setup_notice());
        assert!(render_bottom_popup(&chat, /*width*/ 70).contains("Set up security"));
        if dismiss_security {
            chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        }

        let response = codex_app_server_protocol::GetAccountRateLimitsResponse {
            ordinary_usage_allowed: None,
            account_id: Some("account".into()),
            rate_limit_upsell: Some(json!({
                "banner_type": "selected_model_limit", "model_slug": "test-model-a",
                "title": "Selected model usage exhausted", "description": "Switch models.",
                "presentation": "dismissible", "ctas": [],
            })),
            rate_limits: snapshot(/*percent*/ 100.0),
            rate_limits_by_limit_id: None,
            rate_limit_reset_credits: None,
        };
        chat.update_backend_banner(&response);
        assert!(
            render_bottom_popup(&chat, /*width*/ 70).contains("Selected model usage exhausted")
        );
        if !dismiss_security {
            // Dismissing usage must not dismiss the security reminder instead.
            chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        }

        // Account refresh must not read the replacement banner's dismissal state.
        chat.invalidate_security_setup();
        chat.clear_backend_banner();
        chat.show_security_setup(setup_identity(), setup_notice());
        let rendered = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 70));
        assert_eq!(rendered.contains("Set up security"), !dismiss_security);
        if dismiss_security {
            insta::assert_snapshot!("security_setup_dismissed_after_usage", rendered);
        }

        let (mut replacement, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
        replacement.inherit_security_setup(&mut chat);
        replacement.invalidate_security_setup();
        replacement.show_security_setup(setup_identity(), setup_notice());
        assert_eq!(
            render_bottom_popup(&replacement, /*width*/ 70).contains("Set up security"),
            !dismiss_security
        );

        // A fresh CLI run has no inherited dismissal and reminds the same eligible user.
        let (mut fresh, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
        fresh.show_security_setup(setup_identity(), setup_notice());
        assert!(render_bottom_popup(&fresh, /*width*/ 70).contains("Set up security"));
    }
}

#[tokio::test]
async fn security_setup_after_cutoff_wraps_in_narrow_terminal() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
    let mut notice = setup_notice();
    notice.title = "Set up security for Daybreak mode".into();
    notice.description = "Set up Advanced Account Security with a hardware security key. Already Persona-verified? Add your key before October 15 to skip re-verification. You can keep using Codex while you finish setup.".into();
    chat.show_security_setup(setup_identity(), notice);
    insta::assert_snapshot!(
        "security_setup_after_cutoff",
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 48))
    );
}

#[tokio::test]
async fn security_setup_after_persona_grace_wraps_in_narrow_terminal() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
    let mut notice = setup_notice();
    notice.title = "Set up security for Daybreak mode".into();
    notice.description = "Set up Advanced Account Security with a hardware security key. Verify each new hardware security key with Persona. You can keep using Codex while you finish setup.".into();
    chat.show_security_setup(setup_identity(), notice);
    insta::assert_snapshot!(
        "security_setup_after_persona_grace",
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 48))
    );
}

#[tokio::test]
async fn security_setup_respects_applicable_backend_banner() {
    for state in ["visible", "dismissed", "another-model"] {
        let (mut chat, _events, _ops) = make_chatwidget_manual(Some("test-model-a")).await;
        let response = codex_app_server_protocol::GetAccountRateLimitsResponse {
            ordinary_usage_allowed: None,
            account_id: Some("workspace-a".into()),
            rate_limit_upsell: Some(json!({
                "banner_type": "selected_model_limit", "model_slug": "test-model-a",
                "title": "Selected model usage exhausted", "description": "Switch models.",
                "presentation": "dismissible", "ctas": [],
            })),
            rate_limits: snapshot(/*percent*/ 25.0),
            rate_limits_by_limit_id: None,
            rate_limit_reset_credits: None,
        };
        chat.update_backend_banner(&response);
        render_bottom_popup(&chat, /*width*/ 70);
        match state {
            "dismissed" => {
                chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            }
            "another-model" => chat.set_model("test-model-b"),
            "visible" => {}
            _ => unreachable!(),
        }
        chat.show_security_setup(setup_identity(), setup_notice());
        assert_eq!(
            render_bottom_popup(&chat, /*width*/ 70).contains("Set up security"),
            state != "visible"
        );
        if state == "another-model" {
            chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            chat.set_model("test-model-a");
            assert!(
                render_bottom_popup(&chat, /*width*/ 70).contains("Selected model usage exhausted")
            );
        }
    }
}
