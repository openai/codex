use super::*;
use crate::app::agents_overview_usage::AgentsOverviewUsage;
use crate::app::agents_overview_usage::usage_lines;
use crate::chatwidget::ThreadUsageOutcome;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::ThreadUsage;
use codex_app_server_protocol::TokenUsageBreakdown;
use codex_protocol::account::PlanType;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn selected_usage_is_cached_and_account_changes_discard_old_results() -> Result<()> {
    let mut app = make_test_app().await;
    let app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    app.chat_widget.update_account_state(
        /*status_account_display*/ None,
        Some(PlanType::Business),
        /*has_chatgpt_account*/ false,
        /*has_codex_backend_auth*/ true,
    );
    let selected = ThreadId::from_u128(/*value*/ 1);
    let other = ThreadId::new();
    let threads = [(selected, "Review parser"), (other, "Other task")].map(|(id, name)| {
        overview_thread(id, /*parent_thread_id*/ None, name, ThreadStatus::Idle)
    });
    app.agents_overview.threads = threads
        .iter()
        .cloned()
        .map(|thread| (ThreadId::from_string(&thread.id).unwrap(), Some(thread)))
        .collect();
    let view = app.agents_overview_view(threads.to_vec(), Some(selected));
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let thread_id = selected;
    let request_id = Uuid::new_v4();
    let result = ThreadUsageOutcome::Available(serde_json::from_value(serde_json::json!({
        "threadId": selected.to_string(),
        "estimatedUsageCreditsMicros": 3_400_000,
        "estimatedUsageUsdMicros": 140_000,
        "groups": [{"estimatedUsageCreditsMicros": 3_400_000, "inputTokens": 12_000, "outputTokens": 3_000}]
    })).unwrap());
    let tui = crate::tui::test_support::make_test_tui()?;
    app.agents_overview.pending_usage = Some((thread_id, request_id));
    app.refresh_agents_overview_usage(&app_server, tui.frame_requester());
    assert_eq!(
        app.agents_overview.pending_usage,
        Some((thread_id, request_id))
    );
    app.finish_agents_overview_usage(thread_id, request_id, Ok(result.clone()));
    app.refresh_agents_overview_usage(&app_server, tui.frame_requester());
    assert_eq!(app.agents_overview.pending_usage, None);

    let ThreadUsageOutcome::Available(mut expected) = result.clone() else {
        panic!("expected usage estimate")
    };
    for (credits, cost, expected_credits) in [(0, 200_000, 3_400_000), (5_000_000, 0, 5_000_000)] {
        let mut settlement = expected.clone();
        settlement.estimated_usage_credits_micros = credits;
        settlement.estimated_usage_usd_micros = Some(cost);
        settlement.groups.clear();
        expected.estimated_usage_credits_micros = expected_credits;
        expected.estimated_usage_usd_micros = Some(200_000);
        app.agents_overview.pending_usage = Some((thread_id, request_id));
        app.finish_agents_overview_usage(
            thread_id,
            request_id,
            Ok(ThreadUsageOutcome::Available(settlement)),
        );
        assert_eq!(
            app.agents_overview.usage[&selected].estimate,
            Some(expected.clone())
        );
    }

    let tokens = TokenUsageBreakdown {
        total_tokens: 17_000,
        input_tokens: 13_000,
        cached_input_tokens: 1_000,
        cache_write_input_tokens: 0,
        output_tokens: 4_000,
        reasoning_output_tokens: 0,
    };
    app.agents_overview.threads.remove(&selected);
    app.agents_overview.request_id = Some(request_id);
    app.track_agents_overview_notification(&ServerNotification::ThreadTokenUsageUpdated(
        ThreadTokenUsageUpdatedNotification {
            thread_id: selected.to_string(),
            turn_id: "turn".into(),
            token_usage: ThreadTokenUsage {
                total: tokens.clone(),
                last: tokens.clone(),
                model_context_window: None,
            },
        },
    ));
    app.apply_agents_overview_thread_refresh(
        &app_server,
        request_id,
        Ok(AgentsOverviewThreadRefresh {
            threads: HashMap::from([(selected, Some(threads[0].clone()))]),
            last_messages: HashMap::new(),
            recent_seed_complete: true,
        }),
    );
    assert_eq!(
        app.agents_overview.usage[&selected].tokens,
        Some(tokens.clone())
    );
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("Tokens: 13K in · 4K out")
    );
    let mut view = app.agents_overview_view(threads.to_vec(), Some(selected));
    let row = view
        .rows
        .iter_mut()
        .find(|row| row.thread_id == selected)
        .unwrap();
    row.group = AgentsOverviewGroup::NeedsYou;
    row.details.lines = vec![
        Line::default(),
        "Needs attention".into(),
        "Which dependency version should I use?".into(),
        "Open task to review.".into(),
    ];
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let project = test_path_display("/tmp/project");
    let group = format!(
        "/tmp/project  2{}",
        " ".repeat(project.len().saturating_sub("/tmp/project".len()))
    );
    insta::assert_snapshot!(
        "agents_overview_usage",
        render_bottom_popup(&app.chat_widget, /*width*/ 96)
            .replace(&format!("{project}  2"), &group)
            .replace(&project, "/tmp/project")
            .replace("fwd del", "del")
    );
    for notification in [
        ServerNotification::ThreadReverted(codex_app_server_protocol::ThreadRevertedNotification {
            thread_id: selected.to_string(),
        }),
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: selected.to_string(),
        }),
    ] {
        app.agents_overview.usage.get_mut(&selected).unwrap().tokens = Some(tokens.clone());
        app.track_agents_overview_notification(&notification);
        assert_eq!(app.agents_overview.usage[&selected].tokens, None);
    }
    assert_eq!(
        app.agents_overview.usage[&selected].estimate,
        Some(expected.clone())
    );
    let other_usage = app.agents_overview.usage.entry(other).or_default();
    other_usage.estimate = Some(expected.clone());
    other_usage.tokens = Some(tokens.clone());
    app.agents_overview.pending_usage = Some((thread_id, request_id));
    app.finish_agents_overview_usage(thread_id, request_id, Ok(ThreadUsageOutcome::Disabled));
    assert!(
        app.agents_overview
            .usage
            .values()
            .all(|usage| usage.estimate.is_none())
    );
    assert_eq!(
        app.agents_overview.usage[&other].tokens,
        Some(tokens.clone())
    );
    // A different selection must not retry an unavailable account-wide capability.
    let view = app.agents_overview_view(threads.to_vec(), Some(other));
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.refresh_agents_overview_usage(&app_server, tui.frame_requester());
    assert!(app.agents_overview.usage_disabled);
    assert_eq!(app.agents_overview.pending_usage, None);
    for event in [
        AppServerEvent::Lagged { skipped: 1 },
        AppServerEvent::ServerNotification(Box::new(ServerNotification::AccountUpdated(
            AccountUpdatedNotification {
                auth_mode: None,
                plan_type: None,
            },
        ))),
    ] {
        app.agents_overview
            .usage
            .entry(selected)
            .or_default()
            .tokens = Some(tokens.clone());
        app.agents_overview.pending_usage = Some((thread_id, request_id));
        app.agents_overview.usage_disabled = true;
        app.handle_app_server_event(&app_server, event).await;
        app.finish_agents_overview_usage(thread_id, request_id, Ok(result.clone()));
        assert!(app.agents_overview.usage.is_empty());
        assert_eq!(app.agents_overview.pending_usage, None);
        assert!(!app.agents_overview.usage_disabled);
    }
    app.agents_overview.usage_disabled = true;
    app.agents_overview.pending_usage = Some((thread_id, request_id));
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: test_path_buf("/tmp/test.sock").abs(),
        },
    };
    assert!(app.begin_reconnect());
    app.finish_agents_overview_usage(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(expected)),
    );
    assert!(app.agents_overview.usage.is_empty());
    assert_eq!(app.agents_overview.pending_usage, None);
    assert!(!app.agents_overview.usage_disabled);
    Ok(())
}

#[test]
fn incomplete_billing_groups_do_not_display_partial_token_totals() {
    let estimate: ThreadUsage = serde_json::from_value(serde_json::json!({
        "threadId": "thread", "estimatedUsageCreditsMicros": 0, "estimatedUsageUsdMicros": null,
        "groups": [
            {"estimatedUsageCreditsMicros": 0, "inputTokens": 100, "outputTokens": 10},
            {"estimatedUsageCreditsMicros": 0, "inputTokens": null, "outputTokens": 20}
        ]
    }))
    .unwrap();
    let mut usage = AgentsOverviewUsage::default();
    usage.estimate = Some(estimate);
    let text = usage_lines(&usage)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    assert_eq!(text, vec!["Tokens: 30 out", "Est. usage: 0 credits"]);
}
