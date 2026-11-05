//! Provider browsing through loading, startup attachment, nested views, and dismissal.
#![cfg(test)]

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::app::tests::test_thread_session;
use crate::chatwidget::tests::render_bottom_popup;
use crate::config_panel::ConfigEditor;
use crate::config_panel::ConfigEditorHandoff;
use crate::config_panel::ResumeCurrentAvailability;
use crate::tui::TuiEvent;
use crate::tui::test_support::make_test_tui;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
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
    app.chat_widget
        .show_selection_view(list(Some("ollama"), server.backend_id(), page));
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
            request_error(error),
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
