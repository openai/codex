//! Exercise Reserve picker focus, its status label, and the composer palette across entry and exit.

use super::helpers::normalize_snapshot_paths;
use super::*;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use pretty_assertions::assert_eq;
use serde_json::json;

fn reserve_snapshot(primary_used: i32, weekly_used: i32) -> RateLimitSnapshot {
    RateLimitSnapshot {
        limit_id: Some("base_model_inference".into()),
        limit_name: Some("gpt-reserve".into()),
        normal_model_slug: None,
        primary: Some(RateLimitWindow {
            used_percent: primary_used,
            window_duration_mins: Some(300),
            resets_at: None,
        }),
        secondary: Some(RateLimitWindow {
            used_percent: weekly_used,
            window_duration_mins: Some(10080),
            resets_at: None,
        }),
        ..snapshot(/*percent*/ 0.0)
    }
}

#[tokio::test]
async fn luna_reserve_selector_supports_arrows_enter_shortcuts_and_escape_without_losing_draft() {
    for (keys, destination) in [
        (
            vec![KeyCode::Down, KeyCode::Enter],
            Some("https://chatgpt.com/?cta_tab=personal&highlight_plan=plus#pricing"),
        ),
        (
            vec![KeyCode::Down, KeyCode::Up, KeyCode::Enter],
            Some("https://chatgpt.com/settings/usage?credits_modal=true"),
        ),
        (
            vec![KeyCode::Char('1')],
            Some("https://chatgpt.com/settings/usage?credits_modal=true"),
        ),
        (
            vec![KeyCode::Char('2')],
            Some("https://chatgpt.com/?cta_tab=personal&highlight_plan=plus#pricing"),
        ),
        (vec![KeyCode::Down, KeyCode::Esc], None),
    ] {
        let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-reserve")).await;
        chat.has_chatgpt_account = true;
        chat.apply_external_edit("saved draft".into());
        chat.on_rate_limit_snapshot(Some(reserve_snapshot(
            /*primary_used*/ 48, /*weekly_used*/ 20,
        )));
        let response = serde_json::from_value(json!({
            "accountId": "account-preview", "rateLimits": {},
            "rateLimitUpsell": {
                "banner_type": "luna_reserve", "presentation": "dismissible",
                "title": "You’re now using Luna, a faster model for simpler tasks.",
                "description": "Add credits or upgrade to continue using the most advanced models.",
                "ctas": [{"action": "add_credits", "label": "Add credits"},
                    {"action": "open_pricing_dialog", "label": "Upgrade"}]
            }
        }))
        .unwrap();
        chat.update_backend_banner(&response);
        let rendered = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 90));
        assert!(
            !rendered.contains("saved draft"),
            "selector owns input focus"
        );
        assert!(rendered.contains("esc to continue working"));
        while events.try_recv().is_ok() {}
        for key in keys {
            chat.handle_key_event(KeyEvent::from(key));
            if key == KeyCode::Down {
                insta::assert_snapshot!(
                    "luna_reserve_selector_second_choice",
                    normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 90))
                );
            }
        }
        let queued = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
        let urls: Vec<_> = queued
            .iter()
            .filter_map(|event| match event {
                AppEvent::OpenUrlInBrowser { url } => Some(url.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(urls, destination.into_iter().collect::<Vec<_>>());
        assert!(
            !queued
                .iter()
                .any(|event| matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })))
        );
        assert_eq!(chat.composer_text_with_pending(), "saved draft");
        // A periodic usage reply must not reopen a selector the user just closed.
        chat.update_backend_banner(&response);
        let rendered = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 90));
        assert!(!rendered.contains("You’re now using Luna"));
        assert!(rendered.contains("saved draft"));
        assert!(rendered.contains("Luna Reserve"));
    }
}

#[tokio::test]
async fn luna_reserve_status_tracks_the_active_model() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
    chat.has_chatgpt_account = true;
    chat.on_rate_limit_snapshot(Some(reserve_snapshot(
        /*primary_used*/ 25, /*weekly_used*/ 60,
    )));
    assert!(
        !normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
            .contains("Luna Reserve")
    );

    chat.set_model("gpt-reserve");
    let rendered = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80));
    assert!(rendered.contains("Luna Reserve default"));
    insta::assert_snapshot!("luna_reserve_usage_wide", rendered);
    insta::assert_snapshot!(
        "luna_reserve_usage_narrow",
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 34))
    );

    chat.set_model("gpt-5.6-sol");
    assert!(
        !normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
            .contains("Luna Reserve")
    );
    chat.set_model("gpt-reserve");
    assert!(
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80)).contains("Luna Reserve")
    );
}

#[tokio::test]
async fn luna_reserve_usage_survives_banner_dismissal_and_typing_during_a_turn() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(Some("gpt-reserve")).await;
    chat.has_chatgpt_account = true;
    chat.on_rate_limit_snapshot(Some(reserve_snapshot(
        /*primary_used*/ 48, /*weekly_used*/ 20,
    )));
    let response = codex_app_server_protocol::GetAccountRateLimitsResponse {
        ordinary_usage_allowed: Some(false),
        account_id: Some("account-preview".into()),
        rate_limits: snapshot(/*percent*/ 100.0),
        rate_limits_by_limit_id: None,
        rate_limit_reset_credits: None,
        rate_limit_upsell: Some(json!({
            "banner_type": "luna_reserve", "presentation": "dismissible",
            "title": "You’re now using Luna, a faster model for simpler tasks.",
            "description": "Add credits or upgrade to continue using the most advanced models.",
            "ctas": [
                {"action": "add_credits", "label": "Add credits"},
                {"action": "open_pricing_dialog", "label": "Upgrade"}
            ]
        })),
    };
    chat.update_backend_banner(&response);
    for colors in [
        DefaultColors {
            fg: (230, 230, 230),
            bg: (20, 20, 20),
        },
        DefaultColors {
            fg: (25, 25, 25),
            bg: (255, 255, 255),
        },
    ] {
        with_test_default_colors(colors, || {
            let area = Rect::new(0, 0, 80, chat.bottom_pane.desired_height(/*width*/ 80));
            let mut buffer = Buffer::empty(area);
            chat.bottom_pane.render(area, &mut buffer);
            // Do not rely on the terminal's bold default foreground or dim important copy.
            for (y, text) in buffer
                .content
                .chunks(80)
                .enumerate()
                .filter_map(|(y, row)| {
                    let text: String = row.iter().map(ratatui::buffer::Cell::symbol).collect();
                    (text.contains("You’re now using Luna")
                        || text.contains("Add credits or upgrade"))
                    .then_some((y as u16, text))
                })
            {
                let copy = &buffer[(2, y)];
                assert_eq!(copy.fg, crate::terminal_palette::rgb_color(colors.fg));
                assert!(
                    !copy.modifier.contains(ratatui::style::Modifier::DIM),
                    "{text}"
                );
            }
        });
    }
    insta::assert_snapshot!(
        "luna_reserve_usage_with_upsell",
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        !normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
            .contains("You’re now using Luna")
    );
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    let rendered = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80));
    assert_eq!(chat.composer_text_with_pending(), "x");
    insta::assert_snapshot!("luna_reserve_usage_running", rendered);
}

#[tokio::test]
async fn luna_reserve_prompt_preserves_the_composer_palette_on_exit() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(Some("gpt-reserve")).await;
    chat.has_chatgpt_account = true;
    chat.on_rate_limit_snapshot(Some(reserve_snapshot(
        /*primary_used*/ 25, /*weekly_used*/ 60,
    )));
    for colors in [
        DefaultColors {
            fg: (230, 230, 230),
            bg: (20, 20, 20),
        },
        DefaultColors {
            fg: (25, 25, 25),
            bg: (255, 255, 255),
        },
    ] {
        with_test_default_colors(colors, || {
            chat.set_model("gpt-reserve");
            let area = Rect::new(0, 0, 80, chat.bottom_pane.desired_height(/*width*/ 80));
            let mut active = Buffer::empty(area);
            chat.bottom_pane.render(area, &mut active);
            let cursor = chat.bottom_pane.cursor_pos(area).expect("composer cursor");
            let ordinary_style = crate::style::user_message_style();
            assert_eq!(active[(0, cursor.1)].bg, ordinary_style.bg.unwrap());
            assert_eq!(active[(0, cursor.1)].bg, active[(0, cursor.1 - 1)].bg);
            chat.set_model("gpt-5.6-sol");
            let area = Rect::new(0, 0, 80, chat.bottom_pane.desired_height(/*width*/ 80));
            let mut inactive = Buffer::empty(area);
            chat.bottom_pane.render(area, &mut inactive);
            let restored_cursor = chat.bottom_pane.cursor_pos(area).expect("composer cursor");
            assert_eq!(
                inactive[(0, restored_cursor.1)].bg,
                ordinary_style.bg.unwrap()
            );
            assert_eq!(restored_cursor.1, cursor.1);
        });
    }
}
