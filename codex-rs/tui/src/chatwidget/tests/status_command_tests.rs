use super::*;
use crate::chatwidget::ThreadUsageOutcome;
use assert_matches::assert_matches;
use codex_app_server_protocol::ThreadUsage;
use codex_app_server_protocol::ThreadUsageBreakdownGroup;
use codex_utils_path_uri::PathUri;

#[tokio::test]
async fn status_command_renders_immediately_and_refreshes_rate_limits_for_chatgpt_auth() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected status output before refresh request, got {other:?}"),
    };
    assert!(
        !rendered.contains("refreshing limits"),
        "expected /status to avoid transient refresh text in terminal history, got: {rendered}"
    );
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
        }) => request_id,
        other => panic!("expected rate-limit refresh request, got {other:?}"),
    };
    pretty_assertions::assert_eq!(request_id, 0);
}

#[tokio::test]
async fn status_command_refresh_updates_cached_limits_for_future_status_outputs() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);

    match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(_)) => {}
        other => panic!("expected status output before refresh request, got {other:?}"),
    }
    let first_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
        }) => request_id,
        other => panic!("expected rate-limit refresh request, got {other:?}"),
    };

    chat.finish_status_rate_limit_refresh(first_request_id, vec![snapshot(/*percent*/ 92.0)]);
    drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Status);
    let refreshed = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected refreshed status output, got {other:?}"),
    };
    assert!(
        refreshed.contains("8% left"),
        "expected a future /status output to use refreshed cached limits, got: {refreshed}"
    );
}

#[tokio::test]
async fn status_command_renders_immediately_without_rate_limit_refresh() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Status);

    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    assert!(
        !std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::RefreshRateLimits { .. })),
        "non-ChatGPT sessions should not request a rate-limit refresh for /status"
    );
}

#[tokio::test]
async fn status_command_uses_catalog_default_reasoning_when_config_empty() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.4")).await;
    chat.config.model_reasoning_effort = None;

    chat.dispatch_command(SlashCommand::Status);

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected status output, got {other:?}"),
    };
    assert!(
        rendered.contains("gpt-5.4 (reasoning medium, summaries auto)"),
        "expected /status to render the catalog default reasoning effort, got: {rendered}"
    );
}

#[tokio::test]
async fn status_command_renders_native_and_foreign_instruction_sources() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let (foreign_source, foreign_display) = if cfg!(windows) {
        (
            PathUri::parse("file:///remote/AGENTS.md").expect("POSIX instruction source"),
            "/remote/AGENTS.md",
        )
    } else {
        (
            PathUri::parse("file:///C:/remote/AGENTS.md").expect("Windows instruction source"),
            r"C:\remote\AGENTS.md",
        )
    };
    chat.instruction_source_paths = vec![
        PathUri::from_abs_path(&chat.config.cwd.join("AGENTS.md")),
        foreign_source,
    ];

    chat.dispatch_command(SlashCommand::Status);

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected status output, got {other:?}"),
    };
    assert!(
        rendered.contains(&format!("AGENTS.md, {foreign_display}")),
        "expected /status to show native-relative and environment-native foreign paths, got: {rendered}"
    );
    assert!(
        !rendered.contains("Agents.md  <none>"),
        "expected /status to avoid stale <none> when app-server provided instruction sources, got: {rendered}"
    );
}

#[tokio::test]
async fn status_command_overlapping_refreshes_update_matching_cells_only() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);
    match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(_)) => {}
        other => panic!("expected first status output, got {other:?}"),
    }
    let first_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
        }) => request_id,
        other => panic!("expected first refresh request, got {other:?}"),
    };

    chat.dispatch_command(SlashCommand::Status);
    let second_rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected second status output, got {other:?}"),
    };
    let second_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
        }) => request_id,
        other => panic!("expected second refresh request, got {other:?}"),
    };

    assert_ne!(first_request_id, second_request_id);
    assert!(
        !second_rendered.contains("refreshing limits"),
        "expected /status to avoid transient refresh text in terminal history, got: {second_rendered}"
    );

    chat.finish_status_rate_limit_refresh(first_request_id, Vec::new());
    pretty_assertions::assert_eq!(chat.refreshing_status_outputs.len(), 1);

    chat.finish_status_rate_limit_refresh(second_request_id, vec![snapshot(/*percent*/ 92.0)]);
    assert!(chat.refreshing_status_outputs.is_empty());
}

#[tokio::test]
async fn account_update_rejects_stale_status_rate_limit_snapshots() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.dispatch_command(SlashCommand::Status);
    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits {
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
        }) => request_id,
        other => panic!("expected status refresh request, got {other:?}"),
    };

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    chat.finish_status_rate_limit_refresh(request_id, vec![snapshot(/*percent*/ 92.0)]);

    assert!(chat.rate_limit_snapshots_by_limit_id.is_empty());
}

#[tokio::test]
async fn status_command_renders_immediately_and_updates_first_card_without_status_line() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let thread_id =
        ThreadId::from_string("019fc8ab-1fb2-7000-8000-000000000001").expect("valid thread id");
    chat.thread_id = Some(thread_id);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Status);

    let first_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected immediate /status output, got {event:?}"),
    };
    let initial = lines_to_single_string(&first_cell.display_lines(/*width*/ 90));
    assert!(!initial.contains("Thread usage"));

    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage {
            thread_id: requested_thread_id,
            request_id,
        }) => {
            assert_eq!(requested_thread_id, thread_id);
            request_id
        }
        event => panic!("expected /status to request thread usage, got {event:?}"),
    };
    drain_insert_history(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: vec![
                ThreadUsageBreakdownGroup {
                    model: Some("gpt-5.4".to_string()),
                    reasoning_effort: Some("high".to_string()),
                    speed: Some("fast".to_string()),
                    estimated_usage_credits_micros: 40_000_000,
                    net_new_input_tokens: Some(80),
                    cached_input_tokens: Some(20),
                    input_tokens: Some(100),
                    output_tokens: Some(40),
                    total_tokens: Some(140),
                },
                ThreadUsageBreakdownGroup {
                    model: Some("gpt-5-mini".to_string()),
                    reasoning_effort: Some("medium".to_string()),
                    speed: Some("standard".to_string()),
                    estimated_usage_credits_micros: 10_000_000,
                    net_new_input_tokens: Some(15),
                    cached_input_tokens: Some(5),
                    input_tokens: Some(20),
                    output_tokens: Some(10),
                    total_tokens: Some(30),
                },
            ],
        })),
    ));

    let settled = lines_to_single_string(&first_cell.display_lines(/*width*/ 90));
    assert!(settled.contains("50 credits · ~$1.82"));
    assert!(settled.contains("GPT-5.4 80%, GPT-5 Mini 20%"));
    assert!(drain_insert_history(&mut rx).is_empty());
    chat.dispatch_command(SlashCommand::Status);
    let cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected /status output with cached thread usage, got {event:?}"),
    };
    let rendered =
        normalize_snapshot_paths(lines_to_single_string(&cell.display_lines(/*width*/ 90)));
    assert!(rendered.contains("50 credits · ~$1.82"));
    assert!(rendered.contains("GPT-5.4 80%, GPT-5 Mini 20%"));
    assert!(rendered.contains("Medium 20%, High 80%"));
    assert!(rendered.contains("Fast mode 80%, Standard 20%"));
    assert!(rendered.contains("120 input (25 cached) + 50 output"));
    assert!(
        rendered.find("Limits:") < rendered.find("Thread usage:"),
        "account limits should appear above the thread usage group: {rendered}"
    );
    let thread_usage = rendered
        .lines()
        .skip_while(|line| !line.contains("Thread usage:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!("status_command_estimated_thread_usage", thread_usage);
}

#[tokio::test]
async fn status_command_omits_thread_usage_for_non_cbp_enterprise_plans() {
    for plan_type in [
        Some(PlanType::Enterprise),
        Some(PlanType::Ent26),
        Some(PlanType::Pro),
        Some(PlanType::Edu),
        Some(PlanType::SelfServeBusinessUsageBased),
        Some(PlanType::Unknown),
        None,
    ] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        set_chatgpt_auth(&mut chat);
        chat.thread_id = Some(ThreadId::new());
        chat.plan_type = plan_type;

        chat.dispatch_command(SlashCommand::Status);

        let rendered = match rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => {
                lines_to_single_string(&cell.display_lines(/*width*/ 90))
            }
            event => panic!("expected immediate /status output, got {event:?}"),
        };
        assert!(!rendered.contains("Thread usage"));
        assert!(
            !std::iter::from_fn(|| rx.try_recv().ok())
                .any(|event| matches!(event, AppEvent::RefreshThreadUsage { .. })),
            "unsupported plan {plan_type:?} must not request thread usage",
        );
    }
}

#[tokio::test]
async fn status_command_stays_visible_and_updates_after_thread_usage_retry() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Status);
    let cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected immediate /status output, got {event:?}"),
    };
    assert!(!lines_to_single_string(&cell.display_lines(/*width*/ 90)).contains("Thread usage"));
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected thread usage request, got {event:?}"),
    };
    while rx.try_recv().is_ok() {}
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Err("thread usage temporarily unavailable".to_string()),
    ));
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.dispatch_command(SlashCommand::Status);
    let retry_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected another immediate /status output, got {event:?}"),
    };
    let retry_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected thread usage retry, got {event:?}"),
    };
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        retry_request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));

    for status_cell in [&cell, &retry_cell] {
        assert!(
            lines_to_single_string(&status_cell.display_lines(/*width*/ 90))
                .contains("50 credits · ~$1.82")
        );
    }
}

#[tokio::test]
async fn status_command_renders_credits_and_breakdowns_without_usd_estimate() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Status);
    let first_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        event => panic!("expected immediate /status output, got {event:?}"),
    };
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected thread usage request, got {event:?}"),
    };
    drain_insert_history(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: None,
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
        })),
    ));

    let first_rendered = lines_to_single_string(&first_cell.display_lines(/*width*/ 90));
    assert!(first_rendered.contains("50 credits"));
    assert!(first_rendered.contains("GPT-5.4 100%"));
    assert!(!first_rendered.contains("~$"));
    drain_insert_history(&mut rx);
    chat.dispatch_command(SlashCommand::Status);
    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 90))
        }
        event => panic!("expected cached credits-only /status output, got {event:?}"),
    };
    assert!(rendered.contains("50 credits"));
    assert!(rendered.contains("GPT-5.4 100%"));
    assert!(rendered.contains("High 100%"));
    assert!(rendered.contains("Fast mode 100%"));
    assert!(rendered.contains("100 input (20 cached) + 40 output"));
    assert!(!rendered.contains("~$"));
    let thread_usage = rendered
        .lines()
        .skip_while(|line| !line.contains("Thread usage:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!(
        "status_command_estimated_thread_usage_without_usd",
        thread_usage
    );
}

#[tokio::test]
async fn status_command_drops_stale_usd_when_updated_usage_has_only_credits() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Status);
    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected initial thread usage request, got {event:?}"),
    };
    drain_insert_history(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 40_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));

    drain_insert_history(&mut rx);
    chat.dispatch_command(SlashCommand::Status);
    let cached = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 90))
        }
        event => panic!("expected cached /status output, got {event:?}"),
    };
    assert!(cached.contains("40 credits · ~$1.82"));
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected refreshed thread usage request, got {event:?}"),
    };
    drain_insert_history(&mut rx);
    assert!(chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: None,
            groups: Vec::new(),
        })),
    ));

    drain_insert_history(&mut rx);
    chat.dispatch_command(SlashCommand::Status);
    let refreshed = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 90))
        }
        event => panic!("expected credits-only /status output, got {event:?}"),
    };
    assert!(refreshed.contains("50 credits"));
    assert!(!refreshed.contains("~$1.82"));
}

#[tokio::test]
async fn status_command_requests_thread_usage_for_remote_connection_metadata() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.update_account_state(
        /*status_account_display*/ None,
        Some(PlanType::Business),
        /*has_chatgpt_account*/ false,
        /*has_codex_backend_auth*/ true,
    );
    chat.remote_connection = Some(crate::status::remote_connection::RemoteConnectionStatus {
        address: "wss://remote.example.com".to_string(),
        version: "v1.0.0".to_string(),
    });

    chat.dispatch_command(SlashCommand::Status);

    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::RefreshThreadUsage { .. }));
}

#[tokio::test]
async fn status_command_requests_thread_usage_with_backend_only_authentication() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.update_account_state(
        /*status_account_display*/ None,
        Some(PlanType::Business),
        /*has_chatgpt_account*/ false,
        /*has_codex_backend_auth*/ true,
    );
    assert!(!chat.has_chatgpt_account());
    assert!(chat.has_codex_backend_auth());
    chat.dispatch_command(SlashCommand::Status);

    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::RefreshThreadUsage { .. }));
}

#[tokio::test]
async fn status_command_remains_visible_when_account_changes_during_usage_refresh() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Status);
    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected thread usage request, got {event:?}"),
    };
    drain_insert_history(&mut rx);

    chat.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ false, /*has_codex_backend_auth*/ false,
    );

    assert!(!chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));
    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn status_command_remains_visible_when_thread_changes_during_usage_refresh() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.plan_type = Some(PlanType::Business);

    chat.dispatch_command(SlashCommand::Status);
    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage { request_id, .. }) => request_id,
        event => panic!("expected thread usage request, got {event:?}"),
    };
    drain_insert_history(&mut rx);

    chat.thread_id = Some(ThreadId::new());
    chat.clear_thread_usage_state();

    assert!(!chat.finish_thread_usage_refresh(
        thread_id,
        request_id,
        Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    ));
    assert!(drain_insert_history(&mut rx).is_empty());
}
