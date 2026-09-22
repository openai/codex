//! Quota selection, sparse updates, read ordering, and compact notice formatting.

use super::*;
use crate::app_event::AppEvent;
use crate::app_event::RateLimitRefreshOrigin;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::tui::FrameRequester;
use chrono::TimeZone;
use codex_app_server_protocol::RateLimitReachedType;
use pretty_assertions::assert_eq;

fn snapshot(used_percent: i32) -> RateLimitSnapshot {
    serde_json::from_value(serde_json::json!({
        "primary": {"usedPercent": used_percent, "windowDurationMins": 300, "resetsAt": 1000},
    }))
    .unwrap()
}

fn notice_time() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(
            /*year*/ 2026, /*month*/ 9, /*day*/ 20, /*hour*/ 12,
            /*min*/ 0, /*sec*/ 0,
        )
        .single()
        .unwrap()
}

#[test]
fn notice_details_fit_without_truncating_the_warning() {
    let now = notice_time();
    let mut primary = snapshot(/*used_percent*/ 92).primary.unwrap();
    primary.resets_at = Some((now + chrono::Duration::hours(/*hours*/ 1)).timestamp());
    for (name, state, widths) in [
        (
            "usage_notice_widths",
            UsageNoticeState {
                primary: Some(primary),
                ..Default::default()
            },
            &[98, 97, 58, 57, 38, 37, 12, 11, 0][..],
        ),
        (
            "weekly_usage_notice_widths",
            UsageNoticeState {
                secondary: Some(RateLimitWindow {
                    used_percent: 98,
                    window_duration_mins: Some(10080),
                    resets_at: Some((now + chrono::Duration::days(/*days*/ 1)).timestamp()),
                }),
                ..Default::default()
            },
            &[138, 137, 76, 75, 56, 55, 16, 15][..],
        ),
    ] {
        let mut lines = Vec::new();
        for &width in widths {
            let line = state.line(width, now, ClockFormat::TwelveHour);
            assert!(
                line.as_ref()
                    .is_none_or(|line| line.width() <= usize::from(width))
            );
            lines.push(
                format!("{width} columns: {}", line.unwrap_or_default())
                    .trim_end()
                    .to_owned(),
            );
        }
        insta::assert_snapshot!(name, lines.join("\n"));
    }
}

#[test]
fn notice_shows_only_valid_future_reset_times() {
    let now = notice_time();
    let mut state = UsageNoticeState {
        primary: snapshot(/*used_percent*/ 92).primary,
        ..Default::default()
    };
    for (reset, expected) in [
        (None, ""),
        (Some(now.timestamp() - 1), ""),
        (Some(now.timestamp()), ""),
        (Some(i64::MAX), ""),
        (
            Some((now + chrono::Duration::days(/*days*/ 1)).timestamp()),
            " · resets at 12:00 on 21 Sep",
        ),
    ] {
        state.primary.as_mut().unwrap().resets_at = reset;
        assert_eq!(
            state.line(/*width*/ 160, now, ClockFormat::TwentyFourHour),
            Some(
                Line::from(format!("⚠ 5h limit: 8% left{expected} · /status"))
                    .style(crate::style::warning_notice_style().bold())
            ),
        );
    }
}

#[test]
fn notice_emphasis_tracks_utilization_without_claiming_a_hard_stop() {
    for (used, remaining, bold) in [
        (89, "11%", false),
        (90, "10%", true),
        (94, "6%", true),
        (95, "only 5%", true),
        (100, "only <1%", true),
        (101, "only <1%", true),
    ] {
        let state = UsageNoticeState {
            primary: snapshot(used).primary,
            ..Default::default()
        };
        let style = crate::style::warning_notice_style();
        assert_eq!(
            state.line(
                /*width*/ 80,
                notice_time(),
                ClockFormat::TwentyFourHour
            ),
            Some(
                Line::from(format!("⚠ 5h limit: {remaining} left · /status")).style(if bold {
                    style.bold()
                } else {
                    style
                })
            ),
        );
    }
}

#[test]
fn notice_uses_existing_plan_and_window_thresholds() {
    for (plan, minutes, used, visible) in [
        (Some(PlanType::Plus), Some(300), 49, false),
        (Some(PlanType::Plus), Some(300), 50, true),
        (Some(PlanType::Team), Some(285), 50, true),
        (Some(PlanType::Plus), Some(1440), 50, false),
        (Some(PlanType::Plus), None, 50, false),
        (Some(PlanType::Pro), Some(300), 74, false),
        (Some(PlanType::Pro), Some(300), 75, true),
        (None, Some(300), 75, true),
    ] {
        let mut state = UsageNoticeState::default();
        let mut update = snapshot(used);
        update.primary.as_mut().unwrap().window_duration_mins = minutes;
        state.update(&update, RateLimitSnapshotSource::AccountUsage, plan);
        assert_eq!(
            state.current(),
            visible.then(|| (update.primary.unwrap(), false)),
            "plan={plan:?}, minutes={minutes:?}, used={used}",
        );
    }
}

#[test]
fn most_constrained_window_wins_with_primary_tie_break() {
    let mut state = UsageNoticeState::default();
    let mut update = snapshot(/*used_percent*/ 90);
    update.secondary = Some(RateLimitWindow {
        used_percent: 95,
        window_duration_mins: Some(10080),
        resets_at: Some(2000),
    });
    for primary_used in [90, 95, 100] {
        update.primary.as_mut().unwrap().used_percent = primary_used;
        state.update(
            &update,
            RateLimitSnapshotSource::AccountUsage,
            /*plan_type*/ None,
        );
        let expected = if primary_used < 95 {
            (update.secondary.clone().unwrap(), true)
        } else {
            (update.primary.clone().unwrap(), false)
        };
        assert_eq!(state.current(), Some(expected));
    }
}

#[test]
fn sparse_updates_preserve_metadata_until_confirmed_recovery() {
    let mut state = UsageNoticeState::default();
    let initial = snapshot(/*used_percent*/ 92);
    state.update(
        &initial,
        RateLimitSnapshotSource::AccountUsage,
        /*plan_type*/ None,
    );
    let mut sparse = snapshot(/*used_percent*/ 94);
    sparse.primary.as_mut().unwrap().window_duration_mins = None;
    sparse.primary.as_mut().unwrap().resets_at = None;
    state.update(
        &sparse,
        RateLimitSnapshotSource::RollingUpdate,
        /*plan_type*/ None,
    );
    let mut expected = initial.primary.unwrap();
    expected.used_percent = 94;
    assert_eq!(state.current(), Some((expected.clone(), false)));
    sparse.primary = None;
    state.update(
        &sparse,
        RateLimitSnapshotSource::RollingUpdate,
        /*plan_type*/ None,
    );
    assert_eq!(state.current(), Some((expected.clone(), false)));
    let healthy = snapshot(/*used_percent*/ 10);
    state.update(
        &healthy,
        RateLimitSnapshotSource::RollingUpdate,
        /*plan_type*/ None,
    );
    assert_eq!(state.current(), Some((expected, false)));
    state.update(
        &healthy,
        RateLimitSnapshotSource::AccountUsage,
        /*plan_type*/ None,
    );
    assert_eq!(state.current(), None);
}

#[test]
fn authoritative_reads_replace_windows_and_metadata() {
    let mut state = UsageNoticeState {
        primary: snapshot(/*used_percent*/ 92).primary,
        secondary: snapshot(/*used_percent*/ 95).primary,
        ..Default::default()
    };
    let mut update = snapshot(/*used_percent*/ 80);
    update.primary.as_mut().unwrap().window_duration_mins = None;
    update.primary.as_mut().unwrap().resets_at = None;
    state.update(
        &update,
        RateLimitSnapshotSource::AccountUsage,
        /*plan_type*/ None,
    );
    assert_eq!(state.current(), Some((update.primary.unwrap(), false)));
    let mut empty = snapshot(/*used_percent*/ 0);
    empty.primary = None;
    state.update(
        &empty,
        RateLimitSnapshotSource::AccountUsage,
        /*plan_type*/ None,
    );
    assert_eq!(state.current(), None);
}

#[test]
fn usable_credits_suppress_notices_and_survive_sparse_updates() {
    for (has_credits, unlimited) in [(true, false), (false, true)] {
        let mut state = UsageNoticeState::default();
        let mut funded = snapshot(/*used_percent*/ 92);
        funded.credits = Some(CreditsSnapshot {
            has_credits,
            unlimited,
            balance: None,
        });
        state.update(
            &funded,
            RateLimitSnapshotSource::AccountUsage,
            /*plan_type*/ None,
        );
        let sparse = snapshot(/*used_percent*/ 96);
        state.update(
            &sparse,
            RateLimitSnapshotSource::RollingUpdate,
            /*plan_type*/ None,
        );
        assert_eq!(state.current(), None);
        state.update(
            &sparse,
            RateLimitSnapshotSource::AccountUsage,
            /*plan_type*/ None,
        );
        assert_eq!(state.current(), Some((sparse.primary.unwrap(), false)));
    }
}

#[tokio::test]
async fn notice_redraws_when_only_limit_blockers_change() {
    let (frame_requester, mut draw_rx) = FrameRequester::test_channel();
    let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
    chat.frame_requester = frame_requester;
    chat.local_settings.tui.status_line = Some(Vec::new());
    chat.local_settings.tui.terminal_title = Some(Vec::new());
    let low = snapshot(/*used_percent*/ 92);
    chat.on_rate_limit_snapshot(Some(low.clone()));
    let notice = chat.usage_notice(/*width*/ 80);
    assert!(notice.is_some());
    for (limit_id, spend_control_reached, rate_limit_reached_type) in [
        (None, Some(true), None),
        (None, None, Some(RateLimitReachedType::RateLimitReached)),
        (
            Some("codex_other".to_string()),
            None,
            Some(RateLimitReachedType::RateLimitReached),
        ),
    ] {
        while draw_rx.try_recv().is_ok() {}
        let mut blocked = low.clone();
        blocked.limit_id = limit_id;
        blocked.spend_control_reached = spend_control_reached;
        blocked.rate_limit_reached_type = rate_limit_reached_type;
        chat.on_rolling_rate_limit_snapshot(blocked);
        assert_eq!(chat.usage_notice(/*width*/ 80), None);
        assert!(draw_rx.try_recv().is_ok());
        while draw_rx.try_recv().is_ok() {}
        chat.on_rate_limit_snapshot(/*snapshot*/ None);
        assert_eq!(chat.usage_notice(/*width*/ 80), notice);
        assert!(draw_rx.try_recv().is_ok());
    }
}

#[tokio::test]
async fn overlapping_reads_keep_older_success_when_newer_read_fails() -> color_eyre::Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut session = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let low = snapshot(/*used_percent*/ 92);
    let healthy = snapshot(/*used_percent*/ 10);
    for (first_id, rolling, update, expected) in [
        (
            1,
            None,
            low.clone(),
            Some((low.primary.clone().unwrap(), false)),
        ),
        (
            3,
            Some(low.clone()),
            healthy.clone(),
            Some((low.primary.clone().unwrap(), false)),
        ),
        (5, None, healthy, None),
    ] {
        for request_id in first_id..=first_id + 1 {
            app.chat_widget
                .add_status_output(/*refreshing_rate_limits*/ true, Some(request_id));
            app.refresh_rate_limits(
                &session,
                RateLimitRefreshOrigin::StatusCommand { request_id },
            );
        }
        if let Some(rolling) = rolling {
            app.chat_widget.on_rolling_rate_limit_snapshot(rolling);
        }
        let before = app.chat_widget.usage_notice_state.current();
        for (request_id, result, expected) in [
            (first_id + 1, Err("transient failure".into()), before),
            (first_id, Ok(update), expected),
        ] {
            app.handle_event(
                &mut tui,
                &mut session,
                AppEvent::RateLimitsLoaded {
                    request_id,
                    origin: RateLimitRefreshOrigin::StatusCommand { request_id },
                    hard_stop_generation: 0,
                    result: result.map(|update| {
                        serde_json::from_value(serde_json::json!({"rateLimits": update})).unwrap()
                    }),
                },
            )
            .await?;
            assert_eq!(app.chat_widget.usage_notice_state.current(), expected);
        }
    }
    session.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn rolling_updates_invalidate_only_reads_already_started() {
    let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
    let mut low = snapshot(/*used_percent*/ 50);
    low.plan_type = Some(PlanType::Plus);
    let healthy = snapshot(/*used_percent*/ 10);
    chat.start_usage_notice_read(/*request_id*/ 1);
    chat.on_rolling_rate_limit_snapshot(low.clone());
    chat.start_usage_notice_read(/*request_id*/ 2);
    chat.start_usage_notice_read(/*request_id*/ 3);
    // Read 3 fails; read 1 is stale, but read 2 began after the rolling update.
    chat.apply_usage_notice_read(/*request_id*/ 1);
    let mut stale = healthy.clone();
    stale.plan_type = Some(PlanType::Pro);
    // A legacy response can include the same quota both at the top level and in its map.
    for _ in 0..2 {
        chat.on_rate_limit_snapshot(Some(stale.clone()));
        assert_eq!(
            chat.usage_notice_state.current(),
            Some((low.primary.clone().unwrap(), false))
        );
    }
    chat.apply_usage_notice_read(/*request_id*/ 2);
    chat.on_rate_limit_snapshot(Some(healthy));
    assert_eq!(chat.usage_notice_state.current(), None);
}

#[tokio::test]
async fn notice_follows_threads_and_clears_on_account_change() {
    let (mut previous, _, _, _) = make_chatwidget_manual_with_sender().await;
    let low = snapshot(/*used_percent*/ 92);
    previous.on_rate_limit_snapshot(Some(low.clone()));
    previous.start_usage_notice_read(/*request_id*/ 1);
    previous.on_rolling_rate_limit_snapshot(low.clone());
    let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
    chat.inherit_backend_banner_state(&mut previous);
    chat.apply_usage_notice_read(/*request_id*/ 1);
    chat.on_rate_limit_snapshot(Some(snapshot(/*used_percent*/ 10)));
    let mut other = snapshot(/*used_percent*/ 99);
    other.limit_id = Some("codex_other".into());
    chat.on_rolling_rate_limit_snapshot(other);
    chat.on_rate_limit_snapshot(/*snapshot*/ None);
    assert_eq!(
        chat.usage_notice_state.current(),
        Some((low.primary.unwrap(), false))
    );
    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    assert_eq!(chat.usage_notice_state.current(), None);
}
