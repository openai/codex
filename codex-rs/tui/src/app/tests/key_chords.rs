use super::App;
use super::AppServerSession;
use super::Result;
use super::RuntimeKeymap;
use super::TuiEvent;
use super::make_test_app;
use super::start_config_write_test_app_server;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::keymap::KeymapContext;
use crate::test_support::test_path_display;
use crate::tui::Tui;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

async fn chord_app() -> Result<(App, Tui, AppServerSession)> {
    let mut app = make_test_app().await;
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x ctrl-t".to_string(),
    )));
    config.chat.interrupt_turn = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x ctrl-u".to_string(),
    )));
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x j".to_string())));
    config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x enter".to_string(),
    )));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;

    let app_server = start_config_write_test_app_server(&app).await?;
    let tui = crate::tui::test_support::make_test_tui()?;
    Ok((app, tui, app_server))
}

async fn press(
    app: &mut App,
    tui: &mut Tui,
    app_server: &mut AppServerSession,
    key: KeyEvent,
) -> Result<()> {
    app.handle_tui_event(tui, app_server, TuiEvent::Key(key))
        .await?;
    Ok(())
}

fn ctrl(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
}

#[tokio::test]
async fn completed_global_chord_reuses_the_existing_action_handler() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_none());
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .replace(&test_path_display("/tmp/project"), "/tmp/project"),
        @r"
        › Ask Codex to do anything

           ctrl + x … waiting for next key    esc cancel
        "
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('t')).await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_some());
    Ok(())
}

#[tokio::test]
async fn wrong_second_stroke_passes_through_but_escape_is_consumed() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    let wrong_second = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(
        app.route_key_chord_event(&mut tui, wrong_second),
        Some(wrong_second)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.route_key_chord_event(&mut tui, escape), None);
    assert!(!app.key_chord_matcher.is_pending());
    assert!(!app.backtrack.primed);
    Ok(())
}

#[tokio::test]
async fn physical_dispatch_band_events_are_dropped() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let (code, modifiers) = app
        .keymap
        .app
        .open_transcript
        .last()
        .expect("configured chord appends a dispatch token")
        .parts();

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(code, modifiers),
    )
    .await?;

    assert!(app.overlay.is_none());
    Ok(())
}

#[tokio::test]
async fn physical_chords_route_list_and_mixed_request_input_modals() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.show_selection_view(SelectionViewParams {
        view_id: Some("list"),
        items: ["First", "Second"]
            .into_iter()
            .map(|name| SelectionItem {
                name: name.to_string(),
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(0)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(0)
    );
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(1)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        None
    );

    app.chat_widget
        .handle_request_user_input_now(ToolRequestUserInputParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "call-1".to_string(),
            questions: vec![ToolRequestUserInputQuestion {
                id: "choice".to_string(),
                header: "Pick one".to_string(),
                question: "Choose an option.".to_string(),
                is_other: false,
                is_secret: false,
                options: Some(
                    ["First", "Second"]
                        .into_iter()
                        .map(|label| ToolRequestUserInputOption {
                            label: label.to_string(),
                            description: label.to_string(),
                        })
                        .collect(),
                ),
            }],
            is_blocking: true,
            auto_resolution_ms: None,
        });
    let contexts = app.chat_widget.keymap_contexts();
    assert!(contexts.contains(KeymapContext::Chat));
    assert!(contexts.contains(KeymapContext::List));
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("› 1. First"));

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("› 2. Second"));

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(&mut app, &mut tui, &mut app_server, ctrl('u')).await?;
    assert!(app.chat_widget.can_launch_external_editor());
    Ok(())
}
