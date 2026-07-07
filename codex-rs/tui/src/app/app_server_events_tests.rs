use super::*;
use crate::app::conversation_panes::ConversationPaneInit;
use crate::app::test_support::make_test_app_with_channels;
use crate::app_event::ManagedNetworkChoice;
use crate::app_event::PaneSlot;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane_with_sender;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::file_search::FileSearchManager;
use crate::legacy_core::config::PermissionProfileSnapshot;
use crate::test_support::PathBufExt;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_config::types::ApprovalsReviewer;
use codex_features::Feature;
use codex_protocol::account::PlanType;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

async fn install_test_side(app: &mut App) {
    let side_widget =
        make_chatwidget_for_pane_with_sender(PaneSlot::Side, app.app_event_tx.clone()).await;
    let file_search = FileSearchManager::new(
        side_widget.config_ref().cwd.to_path_buf(),
        side_widget.conversation_event_sender(),
    );
    assert!(
        app.chat_widget
            .install_side(ConversationPaneInit {
                chat_widget: side_widget,
                file_search,
                owned_screen: None,
            })
            .is_ok(),
        "side pane should install"
    );
}

fn take_history_messages(events: &mut UnboundedReceiver<AppEvent>) -> Vec<(PaneSlot, String)> {
    let mut messages = Vec::new();
    while let Ok(event) = events.try_recv() {
        let (pane, event) = match event {
            AppEvent::FromConversation { target, event } => (target.pane, *event),
            event @ AppEvent::InsertHistoryCell(_) => (PaneSlot::Parent, event),
            _ => continue,
        };
        let AppEvent::InsertHistoryCell(cell) = event else {
            continue;
        };
        let text = cell
            .display_lines(/*width*/ 80)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        messages.push((pane, text));
    }
    messages
}

#[tokio::test]
async fn account_and_global_notifications_reach_both_installed_panes() {
    let (mut app, mut app_events, _op_rx) = make_test_app_with_channels().await;
    install_test_side(&mut app).await;
    let app_server = crate::start_embedded_app_server_for_picker(&app.config)
        .await
        .expect("embedded app server should start");

    app.handle_server_notification_event(
        &app_server,
        ServerNotification::AccountUpdated(AccountUpdatedNotification {
            auth_mode: Some(AuthMode::Chatgpt),
            plan_type: Some(PlanType::Plus),
        }),
    )
    .await;

    for slot in [PaneSlot::Parent, PaneSlot::Side] {
        let pane = app.chat_widget.by_slot(slot).expect("installed pane");
        assert_eq!(pane.current_plan_type(), Some(PlanType::Plus));
        assert!(pane.has_chatgpt_account());
        assert!(pane.has_codex_backend_auth());
    }

    while app_events.try_recv().is_ok() {}
    app.handle_server_notification_event(
        &app_server,
        ServerNotification::ConfigWarning(ConfigWarningNotification {
            summary: "shared warning".to_string(),
            details: None,
            path: None,
            range: None,
        }),
    )
    .await;

    assert_eq!(
        take_history_messages(&mut app_events),
        vec![
            (PaneSlot::Parent, "⚠ shared warning".to_string()),
            (PaneSlot::Side, "⚠ shared warning".to_string()),
        ]
    );
}

#[tokio::test]
async fn lag_finishes_mcp_startup_in_both_installed_panes() {
    let (mut app, mut app_events, _op_rx) = make_test_app_with_channels().await;
    install_test_side(&mut app).await;
    let sentry_config = toml::from_str::<toml::Value>("command = 'true'")
        .expect("test MCP config should parse")
        .try_into()
        .expect("test MCP config should deserialize");
    app.config
        .mcp_servers
        .set(std::collections::HashMap::from([(
            "sentry".to_string(),
            sentry_config,
        )]))
        .expect("test MCP servers should accept any configuration");
    let app_server = crate::start_embedded_app_server_for_picker(&app.config)
        .await
        .expect("embedded app server should start");
    let starting = ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
        thread_id: None,
        name: "alpha".to_string(),
        status: McpServerStartupState::Starting,
        error: None,
        failure_reason: None,
    });
    app.chat_widget.for_each_installed_mut(|pane| {
        pane.handle_server_notification(starting.clone(), /*replay_kind*/ None);
    });
    while app_events.try_recv().is_ok() {}

    app.handle_app_server_event(&app_server, AppServerEvent::Lagged { skipped: 1 })
        .await;

    let warnings = take_history_messages(&mut app_events);
    assert_eq!(warnings.len(), 2);
    assert_eq!(warnings[0].0, PaneSlot::Parent);
    assert_eq!(warnings[1].0, PaneSlot::Side);
    assert!(
        warnings
            .iter()
            .all(|(_, text)| text.contains("alpha") && text.contains("sentry")),
        "each pane should report both observed and configured MCP servers: {warnings:?}"
    );
}

#[tokio::test]
async fn terminal_browser_restricted_open_waits_for_managed_network_confirmation()
-> color_eyre::eyre::Result<()> {
    let (mut app, mut app_events, _op_rx) = make_test_app_with_channels().await;
    let config_dir = tempfile::tempdir()?;
    let user_config_path = config_dir.path().join("config.toml").abs();
    let system_config_path = config_dir.path().join("system-config.toml");
    std::fs::write(
        &system_config_path,
        r#"
sandbox_mode = "workspace-write"
approvals_reviewer = "auto_review"

[sandbox_workspace_write]
network_access = true

[features]
terminal_browser = true
"#,
    )?;
    std::fs::write(
        user_config_path.as_path(),
        r#"
sandbox_mode = "workspace-write"
approvals_reviewer = "auto_review"

[sandbox_workspace_write]
network_access = false

[features]
terminal_browser = true
"#,
    )?;
    app.config.codex_home = config_dir.path().to_path_buf().abs();
    app.loader_overrides.user_config_path = Some(user_config_path);
    app.loader_overrides.system_config_path = Some(system_config_path);
    let rebuilt = app
        .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
        .await?;
    app.config = rebuilt.clone();
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::AutoReview);
    app.chat_widget
        .set_permission_profile_from_session_snapshot(
            PermissionProfileSnapshot::from_session_snapshot(
                rebuilt.permissions.permission_profile().clone(),
                rebuilt.permissions.active_permission_profile(),
            ),
        )?;
    app.chat_widget
        .set_permission_network(rebuilt.permissions.network.clone());
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_server_request_event(
        &app_server,
        ServerRequest::DynamicToolCall {
            request_id: AppServerRequestId::Integer(42),
            params: DynamicToolCallParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                call_id: "browser-open-1".to_string(),
                namespace: Some(TERMINAL_BROWSER_NAMESPACE.to_string()),
                tool: "open".to_string(),
                arguments: serde_json::json!({
                    "url": "file:///tmp/not-allowed",
                    "visible": true,
                }),
            },
        },
    )
    .await;

    assert!(app.has_pending_terminal_browser_open());
    let popup = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(popup.contains("Enable managed network access?"), "{popup}");
    assert!(app_events.try_recv().is_err());

    app.chat_widget
        .handle_key_event(KeyEvent::from(KeyCode::Enter));
    let (selection, network_choice) = std::iter::from_fn(|| app_events.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::ApplyAutoReviewPreset {
                selection,
                network_choice,
            } => Some((selection, network_choice)),
            _ => None,
        })
        .expect("approval should resolve the pending browser permission decision");
    assert_eq!(network_choice, ManagedNetworkChoice::RestoreManaged);
    assert!(
        app.apply_auto_review_preset(&mut app_server, selection, network_choice)
            .await
    );
    assert!(app.config.permissions.network_sandbox_policy().is_enabled());
    app.resolve_pending_terminal_browser_open(/*allow*/ true)
        .await;
    let response = tokio::time::timeout(Duration::from_secs(/*secs*/ 1), async {
        loop {
            if let Some(AppEvent::TerminalBrowserToolCompleted { response, .. }) =
                app_events.recv().await
            {
                break response;
            }
        }
    })
    .await?;
    assert!(!response.success);
    app_server.shutdown().await?;
    Ok(())
}
