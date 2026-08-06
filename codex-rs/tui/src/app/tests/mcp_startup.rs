use super::*;
use pretty_assertions::assert_eq;

fn configure_mcp_servers(app: &mut App) {
    let config: codex_config::types::McpServerConfig =
        toml::from_str::<toml::Value>("command = 'true'")
            .expect("test MCP config should parse")
            .try_into()
            .expect("test MCP config should deserialize");
    app.config
        .mcp_servers
        .set(HashMap::from([
            ("eager".to_string(), config.clone()),
            ("deferred".to_string(), config),
        ]))
        .expect("test MCP servers should accept any configuration");
}

#[tokio::test]
async fn subagent_mcp_startup_settles_while_cached_servers_remain_deferred() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    configure_mcp_servers(&mut app);
    let app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let root_thread_id = ThreadId::new();
    let subagent_thread_id = ThreadId::new();
    app.primary_thread_id = Some(root_thread_id);
    app.upsert_agent_picker_thread(
        subagent_thread_id,
        /*agent_nickname*/ None,
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.ensure_thread_channel(subagent_thread_id);
    app.activate_thread_channel(subagent_thread_id).await;
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: Some(test_thread_session(
                subagent_thread_id,
                test_path_buf("/tmp/subagent"),
            )),
            turns: Vec::new(),
            events: Vec::new(),
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );

    let mut visible_startup_states = Vec::new();
    for (name, status, task_running) in [
        ("eager", McpServerStartupState::Starting, true),
        ("eager", McpServerStartupState::Ready, false),
        ("deferred", McpServerStartupState::Starting, true),
        ("deferred", McpServerStartupState::Ready, false),
    ] {
        app.handle_app_server_event(
            &app_server,
            codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                    thread_id: Some(subagent_thread_id.to_string()),
                    name: name.to_string(),
                    status,
                    error: None,
                    failure_reason: None,
                }),
            )),
        )
        .await;
        let event = app
            .active_thread_rx
            .as_mut()
            .expect("subagent receiver should be active")
            .try_recv()
            .expect("MCP startup update should reach the active subagent");
        app.handle_thread_event_now(event);

        assert_eq!(app.chat_widget.is_task_running_for_test(), task_running);
        let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80);
        let visible_status = rendered
            .lines()
            .find(|line| line.contains("Booting MCP server:"))
            .and_then(|line| line.split_once(" ("))
            .map_or("idle", |(status, _)| status);
        visible_startup_states.push(format!("{name}: {visible_status}"));
    }

    insta::assert_snapshot!(visible_startup_states.join("\n"), @r"
    eager: • Booting MCP server: eager
    eager: idle
    deferred: • Booting MCP server: deferred
    deferred: idle
    ");
}

#[tokio::test]
async fn resumed_subagent_mcp_startup_settles_while_cached_servers_remain_deferred() {
    let mut app = make_test_app().await;
    configure_mcp_servers(&mut app);
    let subagent_thread_id = ThreadId::new();
    app.primary_thread_id = Some(subagent_thread_id);
    app.active_thread_id = Some(subagent_thread_id);
    app.agent_navigation.mark_parent_owned(subagent_thread_id);
    app.refresh_mcp_startup_expected_servers_from_config();

    for status in [
        McpServerStartupState::Starting,
        McpServerStartupState::Ready,
    ] {
        app.chat_widget.handle_server_notification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                thread_id: Some(subagent_thread_id.to_string()),
                name: "eager".to_string(),
                status,
                error: None,
                failure_reason: None,
            }),
            /*replay_kind*/ None,
        );
    }

    assert!(!app.chat_widget.is_task_running_for_test());
}

#[tokio::test]
async fn side_conversations_wait_for_every_configured_mcp_server() {
    let mut app = make_test_app().await;
    configure_mcp_servers(&mut app);
    let root_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(root_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(root_thread_id));
    app.active_thread_id = Some(side_thread_id);
    app.refresh_mcp_startup_expected_servers_from_config();

    for status in [
        McpServerStartupState::Starting,
        McpServerStartupState::Ready,
    ] {
        app.chat_widget.handle_server_notification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                thread_id: Some(side_thread_id.to_string()),
                name: "eager".to_string(),
                status,
                error: None,
                failure_reason: None,
            }),
            /*replay_kind*/ None,
        );
    }

    assert!(app.chat_widget.is_task_running_for_test());
}
