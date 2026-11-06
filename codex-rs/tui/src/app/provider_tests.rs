//! Provider browsing through loading, startup attachment, nested views, and dismissal.
#![cfg(test)]

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::app::tests::test_thread_session;
use crate::chatwidget::tests::render_bottom_popup;
use crate::config_panel::ConfigEditor;
use crate::config_panel::ConfigEditorHandoff;
use crate::config_panel::ResumeCurrentAvailability;
use crate::model_catalog::ModelCatalog;
use crate::tui::TuiEvent;
use crate::tui::test_support::make_test_tui;
use codex_app_server_client::TypedRequestError;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::mpsc::UnboundedReceiver;

async fn next_provider_event(rx: &mut UnboundedReceiver<AppEvent>) -> Event {
    loop {
        if let AppEvent::Provider(event) = rx.recv().await.expect("open event channel") {
            return event;
        }
    }
}

#[tokio::test]
async fn loading_survives_initial_attachment_and_refreshes_under_an_overlay() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = make_test_tui()?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    app.chat_widget.open_config_panel();
    let handoff = ConfigEditorHandoff::new(
        ConfigEditor::ModelProviders,
        crate::key_hint::plain(KeyCode::Enter),
    );
    handoff.mark_activation_inactive();
    app.chat_widget
        .open_config_editor(handoff, ResumeCurrentAvailability::Available);
    assert!(app.chat_widget.thread_id().is_none());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    let loaded = next_provider_event(&mut events).await;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.chat_widget.handle_thread_session_quiet(session);
    app.chat_widget.show_selection_view(SelectionViewParams {
        view_id: Some("test_overlay"),
        items: vec![back()],
        ..Default::default()
    });
    handle(&mut app, &mut tui, &server, loaded);
    assert!(app.chat_widget.active_view_is("test_overlay"));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    assert!(app.chat_widget.active_view_is(LIST_VIEW));
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    assert!(matches!(app.overlay, Some(Overlay::Static(_))));
    app.handle_backtrack_overlay_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Char('q').into()),
    )
    .await?;
    assert!(app.overlay.is_none());
    assert!(app.chat_widget.active_view_is(LIST_VIEW));

    // A late result cannot replace a new loading popup after Back and reopen.
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    let (view, load) = loading();
    app.chat_widget.show_selection_view(view);
    handle(&mut app, &mut tui, &server, load);
    let stale = next_provider_event(&mut events).await;
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    let (view, _load) = loading();
    app.chat_widget.show_selection_view(view);
    handle(&mut app, &mut tui, &server, stale);
    assert!(app.chat_widget.active_view_is(LOADING_VIEW));
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn provider_views_show_reported_endpoints_and_ids_outside_the_catalog() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = make_test_tui()?;
    let server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let provider = ModelProviderSummary {
        id: "openai".into(),
        name: "OpenAI".into(),
        can_edit: false,
        base_url: Some(
            "https://example.test/organization/deployment/region/service/openai/v1".into(),
        ),
        settings: ProviderSettings::OpenAi,
    };
    let page = ModelProviderListResponse {
        data: vec![provider],
        default_provider_id: "lmstudio".into(),
        can_change_default: true,
    };
    app.chat_widget.show_selection_view(list(
        Some("ollama"),
        Arc::new(Catalog {
            backend: server.backend_id(),
            page,
        }),
        /*client_override*/ None,
        &app.keymap.list,
        /*notice*/ None,
    ));
    insta::assert_snapshot!(
        "provider_list",
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );

    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    let Some(Overlay::Static(pager)) = &mut app.overlay else {
        panic!("expected provider details pager");
    };
    let mut terminal = Terminal::new(TestBackend::new(/*width*/ 48, /*height*/ 9))?;
    terminal.draw(|frame| pager.render(frame.area(), frame.buffer_mut()))?;
    let first_page = terminal.backend().to_string();
    pager.handle_event(&mut tui, TuiEvent::Key(KeyCode::PageDown.into()))?;
    terminal.draw(|frame| pager.render(frame.area(), frame.buffer_mut()))?;
    insta::assert_snapshot!(
        "openai_provider_details",
        format!("{first_page}\n{}", terminal.backend())
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn space_saves_once_retries_and_preserves_the_current_chat() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = make_test_tui()?;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        r#"model_providers.gateway = { name = "Example Gateway", base_url = "http://127.0.0.1:9/v1" }"#,
    )?;
    let server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    let original = session.model_provider_id.clone();
    app.chat_widget.handle_thread_session_quiet(session);
    let (view, load) = loading();
    app.chat_widget.show_selection_view(view);
    handle(&mut app, &mut tui, &server, load);
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );

    // Disconnection after the picker opened must release its write guard for retry.
    app.chat_widget.handle_key_event(KeyCode::Home.into());
    app.reconnect.offline = true;
    app.chat_widget.handle_key_event(KeyCode::Char(' ').into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    app.reconnect.offline = false;
    while !app.chat_widget.active_view_is(LIST_VIEW) {
        handle(
            &mut app,
            &mut tui,
            &server,
            next_provider_event(&mut events).await,
        );
    }
    let retried = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(retried.contains("Could not save provider: Reconnect"));
    assert!(!app.model_catalog.provider_may_have_changed());

    app.chat_widget.handle_key_event(KeyCode::Home.into());
    app.chat_widget.handle_key_event(KeyCode::Char(' ').into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    // The spawned RPC cannot run until this current-thread test yields.
    write::Write::Default("openai".into()).start(&mut app, &server);
    assert!(std::iter::from_fn(|| events.try_recv().ok()).any(|event| {
        matches!(event, AppEvent::InsertHistoryCell(cell) if cell.display_lines(/*width*/ 100)[0]
            .to_string().contains("A provider change is already saving."))
    }));

    // Wait for the RPC without dispatching its completion on the original event channel.
    let backend = server.backend_id();
    assert!(*app.provider_writes[&backend].lock().await);
    let other =
        ModelCatalog::new(Vec::new()).for_backend(BackendId::new(), &mut app.provider_writes);
    assert!(!other.provider_may_have_changed());
    let reconnected = ModelCatalog::new(Vec::new()).for_backend(backend, &mut app.provider_writes);
    assert!(reconnected.provider_may_have_changed());
    app.chat_widget.show_selection_view(SelectionViewParams {
        view_id: Some("test_overlay"),
        items: vec![back()],
        ..Default::default()
    });
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    assert!(app.chat_widget.model_picker_unavailable().is_some());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    assert!(app.chat_widget.active_view_is("test_overlay"));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.handle_key_event(KeyCode::Home.into());
    assert_eq!(app.chat_widget.config_ref().model_provider_id, original);
    insta::assert_snapshot!(
        "saved_default_provider",
        render_bottom_popup(&app.chat_widget, /*width*/ 100)
    );

    // Switch back, then dismiss the picker before the save result is delivered.
    app.chat_widget.handle_key_event(KeyCode::End.into());
    app.chat_widget.handle_key_event(KeyCode::Up.into());
    app.chat_widget.handle_key_event(KeyCode::Char(' ').into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    let cell = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .last()
        .expect("restart hint in history");
    insta::assert_snapshot!(cell.display_lines(/*width*/ 160)[0].to_string(), @"• /new now uses openai. Restart the app server and TUI to refresh /model.");
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn only_ambiguous_write_failures_invalidate_the_model_catalog() {
    use codex_app_server_client::AppServerClient;
    use codex_app_server_client::CloudClient;
    use codex_app_server_protocol::JSONRPCErrorError;

    let rejected = Ok(Err(JSONRPCErrorError {
        code: -32602,
        message: "Rejected".into(),
        data: None,
    }));
    let overridden = Ok(Ok(serde_json::json!({
        "status": "okOverridden", "version": "1", "filePath": std::env::temp_dir().join("config.toml")
    })));
    let disconnected = Err(std::io::Error::other("connection lost"));
    let unreadable = Ok(Ok(serde_json::Value::Null));
    for (reply, stale) in [
        (rejected, false),
        (overridden, false),
        (disconnected, true),
        (unreadable, true),
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let (client, mut commands, _server_events) = CloudClient::channel(/*capacity*/ 1);
        let backend = BackendId::new();
        let server = AppServerSession::new(
            backend,
            AppServerClient::Cloud(client),
            crate::app_server_session::ThreadParamsMode::Remote,
        );
        app.chat_widget.show_selection_view(SelectionViewParams {
            view_id: Some(LIST_VIEW),
            items: vec![back()],
            ..Default::default()
        });
        write::Write::Default("gateway".into()).start(&mut app, &server);
        let command = commands.recv().await.expect("config write");
        command
            .response
            .send(reply)
            .expect("write is awaiting a response");
        assert!(matches!(
            next_provider_event(&mut events).await,
            Event::Written(_)
        ));
        let reconnected =
            ModelCatalog::new(Vec::new()).for_backend(backend, &mut app.provider_writes);
        assert_eq!(reconnected.provider_may_have_changed(), stale);
    }
}

#[test]
fn unsupported_list_method_explains_server_compatibility() {
    for (code, message) in [
        (-32601, "Method not found"),
        (
            -32600,
            "Invalid request: unknown variant `modelProvider/list`",
        ),
    ] {
        let error = TypedRequestError::Server {
            method: "modelProvider/list".into(),
            source: codex_app_server_protocol::JSONRPCErrorError {
                code,
                message: message.into(),
                data: None,
            },
        };
        assert_eq!(
            rpc::request_error(error),
            "Update the connected app server to browse model providers."
        );
    }
}

#[tokio::test]
async fn list_opened_before_first_attachment_shows_the_attached_provider() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = make_test_tui()?;
    let server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let (view, event) = loading();
    app.chat_widget.show_selection_view(view);
    handle(&mut app, &mut tui, &server, event);
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 100)
            .contains("Current chat: No active chat")
    );

    let mut session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    session.model_provider_id = "ollama".into();
    app.chat_widget.handle_thread_session_quiet(session);
    assert!(app.chat_widget.active_view_is(LOADING_VIEW));
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("Current chat: ollama"));
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn adding_a_custom_provider_saves_to_the_server_and_preserves_the_default() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = make_test_tui()?;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        r#"model_providers.seed = { name = "Existing", env_key = "CATALOG_KEY" }"#,
    )?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let (view, load) = loading();
    app.chat_widget.show_selection_view(view);
    handle(&mut app, &mut tui, &server, load);
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    // Open the existing gateway's scrollable details and enter its editor.
    app.chat_widget.handle_key_event(KeyCode::End.into());
    app.chat_widget.handle_key_event(KeyCode::Up.into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    let Some(Overlay::Static(pager)) = &mut app.overlay else {
        panic!("expected provider details pager");
    };
    let mut terminal = Terminal::new(TestBackend::new(/*width*/ 72, /*height*/ 12))?;
    terminal.draw(|frame| pager.render(frame.area(), frame.buffer_mut()))?;
    insta::assert_snapshot!("editable_provider_details", terminal.backend());
    app.handle_backtrack_overlay_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into()))
        .await?;
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    assert!(app.chat_widget.active_view_is(editor::VIEW_ID));
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 72).contains("Existing"));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());

    app.chat_widget.handle_key_event(KeyCode::End.into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    handle(
        &mut app,
        &mut tui,
        &server,
        next_provider_event(&mut events).await,
    );
    let chat = &mut app.chat_widget;
    insta::assert_snapshot!(render_bottom_popup(chat, /*width*/ 72), @r"
    Add custom provider

    › Name*  Team gateway
      Base URL*  https://gateway.example/v1
      API key variable  $TEAM_API_KEY
      Save

    * Required
    Enter the environment variable name on the app server.
    tab move · enter next/save · esc back
    ");
    for value in ["Team gateway", "https://gateway.example.test/v1"] {
        chat.handle_paste(value.into());
        chat.handle_key_event(KeyCode::Enter.into());
    }
    chat.handle_paste("CAT".into());
    chat.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL));
    assert!(render_bottom_popup(chat, /*width*/ 19).contains("$CATALOG_KEY"));
    chat.handle_key_event(KeyCode::Tab.into());
    chat.handle_key_event(KeyCode::Enter.into());
    // Process the write, its completion, and the subsequent catalog reload.
    for _ in 0..4 {
        handle(
            &mut app,
            &mut tui,
            &server,
            next_provider_event(&mut events).await,
        );
    }
    let list = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(list.contains("Team gateway"));
    assert!(list.contains("Default for new chats: openai"));
    server.shutdown().await?;
    Ok(())
}
