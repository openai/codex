use super::*;
use crate::app::conversation_panes::ConversationPaneInit;
use crate::app::test_support::make_test_app_with_channels;
use crate::app_event::PaneSlot;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane_with_sender;
use crate::file_search::FileSearchManager;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
use codex_protocol::account::PlanType;
use pretty_assertions::assert_eq;
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
