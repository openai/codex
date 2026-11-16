#![cfg(test)]
#![cfg(not(windows))]

use super::*;
use crate::app::owned_transcript::tests::attach_thread;
use crate::app::test_support::enable_side_retention;
use crate::app::test_support::side_frame;
use crate::app::tests::make_test_app_with_channels;
use crate::history_cell::AgentMarkdownCell;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use crossterm::event::MouseEvent;
use pretty_assertions::assert_eq;
use ratatui::layout::Size;
use unicode_width::UnicodeWidthStr;

#[tokio::test]
async fn focused_markdown_link_opens_read_only_preview_only_with_workspace() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.local_settings.transcript_mode = crate::transcript_mode::TranscriptMode::Owned;
    app.config.features.disable(Feature::TuiWorkspace)?;
    app.local_settings.tui.animations = false;
    let dir = tempfile::tempdir()?;
    app.config.cwd =
        codex_utils_absolute_path::AbsolutePathBuf::try_from(dir.path().canonicalize()?)?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let parent = server.start_thread(&app.config).await?.session.thread_id;
    attach_thread(&mut app, parent);
    app.transcript_cells = vec![Arc::new(AgentMarkdownCell::new(
        "[Guide](./guide.md)".into(),
        dir.path(),
    ))];

    let size = Size::new(/*width*/ 120, /*height*/ 12);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    tui.terminal = crate::tui::Terminal::with_screen_size_and_cursor_position_for_test(
        ratatui::backend::CrosstermBackend::new(std::io::stdout()),
        size,
        Default::default(),
    );
    let screen = side_frame(&mut app, &mut tui, size)?;
    let (column, row) = screen
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            let byte = line.find("guide.md")?;
            Some((line[..byte].width() as u16, row as u16))
        })
        .expect("the local link's path is visible");
    let press = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    let modified_press = MouseEvent {
        modifiers: KeyModifiers::CONTROL,
        ..press
    };
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Mouse(modified_press))).await?;
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .all(|event| !matches!(event, AppEvent::OpenSidebarDocument { .. }))
    );
    assert_eq!(app.panel_focus(), None);

    app.local_settings.tui.keymap.pager.jump_bottom =
        Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x end".into())));
    enable_side_retention(&mut app)?;
    let enabled = side_frame(&mut app, &mut tui, size)?;
    assert_eq!(
        enabled.lines().nth(row as usize),
        screen.lines().nth(row as usize)
    );
    let release = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        ..press
    };
    for mouse in [press, release] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Mouse(mouse))).await?;
    }
    let open = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::OpenSidebarDocument { .. }))
        .expect("plain click queues a document from the focused transcript");
    app.zoomed_thread = Some(parent);
    Box::pin(app.handle_event(&mut tui, &mut server, open)).await?;
    let mut next_document_event = async |resolved: bool| loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
            .await
            .expect("document event arrives")
            .expect("document event sender");
        if (resolved && matches!(event, AppEvent::SidebarDocumentResolved { .. }))
            || (!resolved && matches!(event, AppEvent::SidebarDocumentLoaded { .. }))
        {
            break event;
        }
    };
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    let loading = side_frame(&mut app, &mut tui, size)?;
    assert!(loading.contains("Opening…"), "{loading}");

    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    let error = side_frame(&mut app, &mut tui, size)?;
    assert!(error.contains("Could not open file"), "{error}");

    std::fs::write(
        dir.path().join("guide.md"),
        format!(
            "# Guide\n\nFirst section.\n\n{}Last section.\n\n[Next](./notes.md)",
            "Middle.\n\n".repeat(12)
        ),
    )?;
    let path = AppServerPath::from_app_server("guide.md");
    app.open_document(&mut tui, &mut server, parent, path.clone())
        .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    let mut other_backend =
        Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    assert_ne!(other_backend.backend_id(), server.backend_id());
    Box::pin(app.handle_event(
        &mut tui,
        &mut other_backend,
        next_document_event(false).await,
    ))
    .await?;
    other_backend.shutdown().await?;
    let screen = side_frame(&mut app, &mut tui, Size::new(/*width*/ 76, /*height*/ 12))?;
    let strip = app
        .conversation_frame
        .sidebar_tab_strip()
        .expect("tab strip");
    let buffer = tui.terminal.previous_buffer();
    let strip_text = (strip.x..strip.right())
        .map(|x| buffer[(x, strip.y)].symbol())
        .collect::<String>();
    let tab = strip_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let visible = std::iter::once(tab.as_str())
        .chain(
            screen
                .lines()
                .map(str::trim)
                .filter(|row| row.ends_with("# Guide") || row.ends_with("First section."))
                .map(|row| row.rsplit('│').next().unwrap_or(row).trim()),
        )
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(visible, @"
    guide.md × +
    # Guide
    First section.
    ");
    for key in [
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyCode::End.into(),
    ] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))).await?;
    }
    let end = side_frame(&mut app, &mut tui, size)?;
    assert!(end.contains("Last section."), "{end}");

    std::fs::write(dir.path().join("notes.md"), "# Notes")?;
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        AppServerPath::from_app_server("notes.md"),
    )
    .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    let two = side_frame(&mut app, &mut tui, size)?;
    assert!(
        two.lines()
            .any(|line| line.contains("guide.md") && line.contains("notes.md")),
        "{two}"
    );
    assert!(two.contains("# Notes"), "{two}");
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    let returned = side_frame(&mut app, &mut tui, size)?;
    assert!(returned.contains("Last section."), "{returned}");
    let repeat = crossterm::event::KeyEvent::new_with_kind(
        KeyCode::Esc,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    );
    assert!(app.route_key_chord_event(&mut tui, repeat).is_none());
    assert_eq!(app.sidebar().unwrap().documents.len(), 1);
    Box::pin(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
    ))
    .await?;
    app.open_document(&mut tui, &mut server, parent, path)
        .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    assert_eq!(app.sidebar().unwrap().documents.len(), 1);
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        AppServerPath::from_app_server("closed.md"),
    )
    .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    assert_eq!(app.sidebar().unwrap().documents.len(), 1);
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        AppServerPath::from_app_server("notes.md"),
    )
    .await?;
    app.overlay = Some(crate::pager_overlay::Overlay::Transcript(
        crate::pager_overlay::TranscriptOverlay::new(Vec::new(), app.keymap.pager.clone()),
    ));
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    assert_eq!(app.sidebar().unwrap().documents.len(), 2);
    assert!(app.overlay.take().is_some());
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    let after_overlay = side_frame(&mut app, &mut tui, size)?;
    assert!(after_overlay.contains("Last section."), "{after_overlay}");
    let child = ThreadId::new();
    app.side_threads.insert(child, SideThreadState::new(parent));
    let (stream, display) = app.chat_widget.take_side_display(/*width*/ 120);
    assert!(app.transcript_view.document_links);
    Box::pin(app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::ParkSideCompanion {
            pair: child,
            thread_id: parent,
            stream,
            display,
        },
    ))
    .await?;
    assert!(!app.side_companions[&parent].view.document_links);
    server.shutdown().await?;
    Ok(())
}
