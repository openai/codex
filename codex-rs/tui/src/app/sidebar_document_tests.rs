#![cfg(test)]
#![cfg(not(windows))]

use super::*;
use crate::app::owned_transcript::tests::attach_thread;
use crate::app::test_support::enable_side_retention;
use crate::app::test_support::side_frame;
use crate::app::tests::make_test_app_with_channels;
use crate::history_cell::AgentMarkdownCell;
use crate::keymap::KeymapContext;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use crossterm::event::MouseEvent;
use pretty_assertions::assert_eq;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use unicode_width::UnicodeWidthStr;

#[test]
fn focused_markdown_link_opens_read_only_preview_only_with_workspace() -> Result<()> {
    // The reader and embedded app server load full configurations on this thread.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(Box::pin(document_preview_scenario()))
        })?
        .join()
        .expect("document preview test thread panicked")
}

async fn document_preview_scenario() -> Result<()> {
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
    let link_press = |screen: &str, label: &str| {
        let (column, row) = screen
            .lines()
            .enumerate()
            .find_map(|(row, line)| {
                let byte = line.find(label)?;
                Some((line[..byte].width() as u16, row as u16))
            })
            .expect("the local link is visible");
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    };
    let screen = side_frame(&mut app, &mut tui, size)?;
    let press = link_press(&screen, "guide.md");
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
    app.local_settings.tui.keymap.global.select_transcript =
        Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x [".into())));
    app.local_settings.tui.keymap.list.move_down =
        Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x j".into())));
    app.local_settings.tui.keymap.list.cancel =
        Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x q".into())));
    enable_side_retention(&mut app)?;
    crate::markdown_render::preferences::set_document_links(/*enabled*/ true);
    let enabled = side_frame(&mut app, &mut tui, size)?;
    assert!(!enabled.contains("guide.md"));
    let press = link_press(&enabled, "Guide");
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
    let error = side_frame(&mut app, &mut tui, Size::new(size.width, /*height*/ 24))?;
    let pane_text: String = error
        .lines()
        .filter_map(|line| line.split_once('│'))
        .map(|(_, pane)| pane.trim())
        .collect();
    let directory = dir.path().canonicalize()?;
    assert!(
        pane_text.contains(&format!("File not foundguide.mdin {}", directory.display())),
        "{error}"
    );

    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    insta::assert_snapshot!(visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 1), @"esc close · c copy path · p show path");

    Box::pin(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Char('p').into()),
    ))
    .await?;
    let revealed = side_frame(&mut app, &mut tui, size)?;
    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    let rows = (sidebar.y + 1..sidebar.bottom())
        .map(|row| visible_sidebar_row(&tui, sidebar, row))
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        rows[1..rows.len() - 1].join(""),
        directory.join("guide.md").to_string_lossy(),
        "{revealed}"
    );
    insta::assert_snapshot!(format!("{}\n{}", rows[0], rows.last().unwrap()), @"
    File path
    p/esc hide path · c copy path
    ");
    let prefix = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(app.route_key_chord_event(&mut tui, prefix), None);
    let scroll = app
        .route_key_chord_event(&mut tui, KeyCode::Char('j').into())
        .and_then(|key| app.keymap.list.action_for(key));
    assert_eq!(scroll, Some(crate::keymap::ListAction::MoveDown));
    for key in [prefix, KeyCode::Char('q').into()] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))).await?;
    }
    assert!(app.active_keymap_contexts().contains(KeymapContext::Pager));
    let dismissed = side_frame(&mut app, &mut tui, size)?;
    assert!(dismissed.contains("File not found"), "{dismissed}");
    assert_eq!(app.sidebar().unwrap().documents.len(), 1);

    std::fs::write(
        dir.path().join("guide.md"),
        format!(
            "# Guide\n\nFirst section.\n\n{}Last section.\n\n[Next](./notes.md)",
            "Middle.\n\n".repeat(12)
        ),
    )?;
    let path = FileLink {
        path: "guide.md".into(),
        location: None,
    };
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
    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    assert!(!screen.contains("Back to bottom"), "{screen}");
    let gap = visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 3);
    let hints = visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 2);
    let actions = visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 1);
    insta::assert_snapshot!(format!("{gap:?}\n{hints}\n{actions}"), @r#"
    ""
    esc close · c copy path · p show path · ↑/↓ read · f3 find · Tab link
    ⌃x [ select
    "#);
    let scroll_up = std::mem::replace(
        &mut app.keymap.pager.scroll_up,
        vec![crate::key_hint::plain(KeyCode::Char('p'))],
    );
    let remapped = side_frame(&mut app, &mut tui, size)?;
    assert!(remapped.contains("c copy path"), "{remapped}");
    assert!(!remapped.contains("p show path"), "{remapped}");
    app.keymap.pager.scroll_up = scroll_up;
    let original_scroll = app.keymap.pager.scroll_down.clone();
    for key in [
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
    ] {
        app.keymap.pager.scroll_down = vec![KeyBinding::from_event(key)];
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))).await?;
        let scrolled = side_frame(&mut app, &mut tui, size)?;
        assert_eq!(scrolled.contains("Tab link"), key.code != KeyCode::Tab);
        assert!(!scrolled.contains("# Guide"), "{scrolled}");
        assert!(!scrolled.contains("Back to bottom"), "{scrolled}");
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Home.into())))
            .await?;
        side_frame(&mut app, &mut tui, size)?;
    }
    app.keymap.pager.scroll_down = original_scroll;
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Down.into())))
        .await?;
    let before_jump = side_frame(&mut app, &mut tui, size)?;
    let jump_top = std::mem::take(&mut app.keymap.pager.jump_top);
    let jump_bottom = std::mem::take(&mut app.keymap.pager.jump_bottom);
    for key in [
        KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL),
    ] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))).await?;
        assert_eq!(side_frame(&mut app, &mut tui, size)?, before_jump);
    }
    app.keymap.pager.jump_top = jump_top;
    app.keymap.pager.jump_bottom = jump_bottom;
    for key in [
        KeyCode::F(3),
        KeyCode::Char('e'),
        KeyCode::Char('g'),
        KeyCode::Char('g'),
    ] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into()))).await?;
    }
    let find = side_frame(&mut app, &mut tui, size)?;
    let footer = find
        .lines()
        .find_map(|line| line.find("Find: ").map(|start| line[start..].trim_end()))
        .expect("document Find editor");
    insta::assert_snapshot!(footer, @"Find: egg");
    app.focus_workspace_composer(&mut tui);
    assert_eq!(app.panel_focus(), Some(PaneFocus::Main));
    let parked = side_frame(&mut app, &mut tui, size)?;
    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    insta::assert_snapshot!(visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 1), @"Find: egg");
    assert!(!parked.contains("esc cancel"), "{parked}");
    let pane = app.sidebar_mut();
    pane.show(pane.visible_content().unwrap(), PaneFocus::Sidebar);
    assert!(side_frame(&mut app, &mut tui, size)?.contains("Find: egg"));

    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    let select = crossterm::event::KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE);
    app.local_settings.tui.keymap.global.select_transcript =
        Some(KeybindingsSpec::One(KeybindingSpec("f8".into())));
    enable_side_retention(&mut app)?;
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(select))).await?;
    side_frame(&mut app, &mut tui, size)?;
    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    let footer = visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 1);
    insta::assert_snapshot!(footer, @"m mark paragraph · q close");
    for key in [KeyCode::Char('v'), KeyCode::Right] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into()))).await?;
    }
    app.focus_workspace_composer(&mut tui);
    let parked = side_frame(&mut app, &mut tui, size)?;
    assert!(!parked.contains("q close"), "{parked}");
    let pane = app.sidebar_mut();
    pane.show(pane.visible_content().unwrap(), PaneFocus::Sidebar);
    assert!(side_frame(&mut app, &mut tui, size)?.contains("q close"));
    app.reconnect.offline = true;
    let copy = crossterm::event::KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(matches!(
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(copy))).await?,
        AppRunControl::Continue
    ));
    assert!(tui.clipboard.is_busy());
    let pane = app.sidebar_mut();
    let document = match pane.visible_content().unwrap() {
        sidebar::SidebarContent::Document(id) => id,
        _ => unreachable!(),
    };
    pane.documents
        .get_mut(&document)
        .unwrap()
        .reader()
        .unwrap()
        .0
        .publish_primary(&mut tui, "visible preview");
    pane.show(
        sidebar::SidebarContent::Document(document),
        sidebar::PaneFocus::Main,
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if tui.clipboard.poll().is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("active copy completes");
    tui.clipboard.advance(tui.frame_requester());
    assert!(tui.clipboard.is_busy(), "visible preview publishes PRIMARY");
    let pane = app.sidebar_mut();
    pane.documents
        .get_mut(&document)
        .unwrap()
        .reader()
        .unwrap()
        .0
        .publish_primary(&mut tui, "hidden preview");
    pane.show(
        sidebar::SidebarContent::Chooser,
        sidebar::PaneFocus::Sidebar,
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while tui.clipboard.is_busy() {
            tui.clipboard.poll();
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("PRIMARY publication completes");
    tui.clipboard.advance(tui.frame_requester());
    assert!(
        !tui.clipboard.is_busy(),
        "hidden preview must not publish PRIMARY"
    );
    app.sidebar_mut().show(
        sidebar::SidebarContent::Document(document),
        sidebar::PaneFocus::Sidebar,
    );
    app.reconnect.offline = false;
    Box::pin(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Char('q').into()),
    ))
    .await?;
    assert!(app.transcript_view.begin_select_mode(&app.transcript_cells));
    for key in [
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyCode::End.into(),
    ] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))).await?;
    }
    let end = side_frame(&mut app, &mut tui, size)?;
    assert!(end.contains("Last section."), "{end}");

    std::fs::write(dir.path().join("notes.md"), "# Notes")?;
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Tab.into())))
        .await?;
    side_frame(&mut app, &mut tui, size)?;
    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    let buffer = tui.terminal.previous_buffer();
    let highlighted = (sidebar.y + 1..sidebar.bottom() - 1)
        .flat_map(|y| (sidebar.x..sidebar.right()).map(move |x| &buffer[(x, y)]))
        .filter(|cell| cell.modifier.contains(Modifier::REVERSED))
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    insta::assert_snapshot!(highlighted, @"Next");
    let line = visible_sidebar_row(&tui, sidebar, sidebar.bottom() - 1);
    assert!(line.starts_with("esc clear link"), "{line}");
    for key in [KeyCode::Esc, KeyCode::Tab] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into()))).await?;
        assert_eq!(app.sidebar().unwrap().documents.len(), 1);
    }

    for key in [KeyCode::Home, KeyCode::Enter] {
        assert!(
            Box::pin(app.handle_document_input(&mut tui, &mut server, &TuiEvent::Key(key.into())))
                .await
        );
    }
    assert_eq!(app.sidebar().unwrap().documents.len(), 1);
    let document = app.sidebar_mut().active_document().unwrap();
    let (view, _) = document.reader().unwrap();
    view.jump_to_latest();
    side_frame(&mut app, &mut tui, size)?;
    assert!(
        Box::pin(app.handle_document_input(
            &mut tui,
            &mut server,
            &TuiEvent::Key(KeyCode::Tab.into())
        ))
        .await
    );
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into())))
        .await?;
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
        FileLink {
            path: "closed.md".into(),
            location: None,
        },
    )
    .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    // A closed tab can cancel its read. Hold an already completed result to test late dispatch.
    let closed_document_result = next_document_event(false).await;
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
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
    Box::pin(app.handle_event(&mut tui, &mut server, closed_document_result)).await?;
    assert_eq!(app.sidebar().unwrap().documents.len(), 1);
    std::fs::write(
        dir.path().join("main.rs"),
        "// first\r\n// second\r\nlet value = 3;\r\n",
    )?;
    for location in [None, Some(SourceLocation { line: 3, column: 5 })] {
        app.open_document(
            &mut tui,
            &mut server,
            parent,
            FileLink {
                path: "./main.rs".into(),
                location,
            },
        )
        .await?;
        Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
        if location.is_none() {
            Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await))
                .await?;
            let Some(SidebarContent::Document(id)) = app.sidebar().unwrap().visible_content()
            else {
                panic!("the source tab should be visible");
            };
            app.sidebar_mut()
                .documents
                .get_mut(&id)
                .unwrap()
                .reader()
                .unwrap()
                .0
                .begin_search();
        }
    }
    let source = side_frame(&mut app, &mut tui, Size::new(200, 15))?;
    let preview = source
        .lines()
        .filter_map(|line| line.rsplit_once('│'))
        .map(|(_, sidebar)| sidebar.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(preview, @"
    guide.md       ×   main.rs        ×                           +
    3 let value = 3;                                               ↑
    esc clear highlight · c copy path · p show path · ↑/↓ read
    f3 find · Tab link · f8 select
    ");
    let panel = app.conversation_frame.panel_area().unwrap();
    let buffer = tui.terminal.previous_buffer();
    let target_cell = &buffer[(panel.x + 7, panel.y + 1)];
    let adjacent = &buffer[(panel.x + 8, panel.y + 1)];
    assert_eq!(target_cell.symbol(), "v");
    assert_ne!(
        target_cell
            .modifier
            .contains(ratatui::style::Modifier::REVERSED),
        adjacent
            .modifier
            .contains(ratatui::style::Modifier::REVERSED),
    );
    assert_eq!(app.sidebar().unwrap().documents.len(), 2);
    let Some(SidebarContent::Document(id)) = app.sidebar().unwrap().visible_content() else {
        panic!("the source tab should be visible");
    };
    let (view, cells) = app
        .sidebar_mut()
        .documents
        .get_mut(&id)
        .unwrap()
        .reader()
        .unwrap();
    assert!(view.begin_select_mode(cells));
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    let State::Ready { view, target, .. } = &app.sidebar().unwrap().documents[&id].state else {
        panic!("the source tab should be ready");
    };
    assert!(!view.is_select_mode());
    assert!(target.is_some());
    assert!(!view.has_active_interaction());
    for remaining in [2, 1] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
            .await?;
        assert_eq!(app.sidebar().unwrap().documents.len(), remaining);
    }
    app.chat_widget
        .set_pet_image_support_for_tests(crate::pets::PetImageSupport::Unsupported(
            crate::pets::PetImageUnsupportedReason::Terminal,
        ));
    image::RgbImage::new(24, 16).save(dir.path().join("figure.png"))?;
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        FileLink {
            path: "figure.png".into(),
            location: None,
        },
    )
    .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    let image = side_frame(&mut app, &mut tui, Size::new(160, 15))?;
    let image_rows = image
        .lines()
        .map(|row| row.rsplit('│').next().unwrap_or(row).trim())
        .filter(|row| {
            row.contains("PNG ·")
                || row.contains("Image preview unavailable")
                || row.contains("←/→ tabs")
                || row.contains("c copy path")
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(image_rows, @"
    PNG · 24 × 16
    Image preview unavailable in this terminal
    esc close · c copy path · p show path · ←/→ tabs
    ");
    assert_eq!(app.sidebar().unwrap().documents.len(), 2);
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        FileLink {
            path: "notes.md".into(),
            location: None,
        },
    )
    .await?;
    app.overlay = Some(crate::pager_overlay::Overlay::Transcript(
        crate::pager_overlay::TranscriptOverlay::new(Vec::new(), app.keymap.pager.clone()),
    ));
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    assert_eq!(app.sidebar().unwrap().documents.len(), 3);
    assert!(app.overlay.take().is_some());
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    let after_overlay = side_frame(&mut app, &mut tui, size)?;
    assert!(after_overlay.contains("PNG · 24 × 16"), "{after_overlay}");
    std::fs::write(
        dir.path().join("pointer.png"),
        "version https://git-lfs.github.com/spec/v1
size 123
",
    )?;
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        FileLink {
            path: "pointer.png".into(),
            location: None,
        },
    )
    .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    let pointer = side_frame(&mut app, &mut tui, Size::new(200, 15))?;
    insta::assert_snapshot!(pointer.lines().filter_map(|row| row.split_once('│')).map(|(_, row)| row.trim()).filter_map(|row| row.split_once(' ').map(|(_, line)| line.trim())).filter(|row| row.starts_with("version ") || row.starts_with("size ")).collect::<Vec<_>>().join("
"), @"
    version https://git-lfs.github.com/spec/v1
    size 123
    ");
    let Some(SidebarContent::Document(pointer_id)) = app.sidebar().unwrap().visible_content()
    else {
        panic!("the pointer preview should be visible");
    };
    app.sidebar_mut().remove_document(pointer_id);

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
    for directory in ["featureA", "featureB"] {
        std::fs::create_dir_all(dir.path().join(directory).join("shared"))?;
        std::fs::write(
            dir.path().join(directory).join("shared/guide.md"),
            "# Guide",
        )?;
        Box::pin(app.open_document(
            &mut tui,
            &mut server,
            parent,
            FileLink {
                path: format!("{directory}/shared/guide.md"),
                location: None,
            },
        ))
        .await?;
        Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
        Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    }
    let _ = side_frame(&mut app, &mut tui, Size::new(76, 12))?;
    let strip = app
        .conversation_frame
        .sidebar_tab_strip()
        .expect("tab strip");
    let buffer = tui.terminal.previous_buffer();
    let labels = (strip.x..strip.right())
        .map(|x| buffer[(x, strip.y)].symbol())
        .collect::<String>();
    let duplicate_labels = labels
        .split('×')
        .map(|label| label.trim_start_matches('‹').trim())
        .filter(|label| label.starts_with("featureA/") || label.starts_with("featureB/"))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(duplicate_labels, @"
    featureA/guide.md
    featureB/guide.md
    ");
    side_frame(&mut app, &mut tui, Size::new(/*width*/ 33, /*height*/ 12))?;
    let strip = app
        .conversation_frame
        .sidebar_tab_strip()
        .expect("tab strip");
    let narrow_labels = visible_sidebar_row(&tui, strip, strip.y);
    insta::assert_snapshot!(narrow_labels, @"‹  f…eA/g…d ×   f…eB/g…d ×    +");
    let long_file = "Before the fold.\n\n".repeat(24);
    for directory in ["feature-alpha-build", "feature-beta-build"] {
        std::fs::create_dir_all(dir.path().join(directory))?;
        std::fs::write(
            dir.path().join(directory).join("guide.md"),
            format!("{long_file}Original ending."),
        )?;
        Box::pin(app.open_document(
            &mut tui,
            &mut server,
            parent,
            FileLink {
                path: format!("{directory}/guide.md"),
                location: None,
            },
        ))
        .await?;
        Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
        Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(false).await)).await?;
    }
    side_frame(&mut app, &mut tui, Size::new(/*width*/ 33, /*height*/ 12))?;
    let strip = app
        .conversation_frame
        .sidebar_tab_strip()
        .expect("tab strip");
    let narrow_labels = visible_sidebar_row(&tui, strip, strip.y);
    insta::assert_snapshot!(narrow_labels, @"‹  1:f…d/…d ×   2:f…d/…d ×    +");
    // If resolution itself fails, there is no new tab in which to show the path.
    let tab_count = app.sidebar().unwrap().documents.len();
    app.open_document(
        &mut tui,
        &mut server,
        parent,
        FileLink {
            path: "~/notes.md".into(),
            location: None,
        },
    )
    .await?;
    Box::pin(app.handle_event(&mut tui, &mut server, next_document_event(true).await)).await?;
    assert_eq!(app.sidebar().unwrap().documents.len(), tab_count);
    assert!(std::iter::from_fn(|| events.try_recv().ok()).any(|event| {
        matches!(event, AppEvent::InsertHistoryCell(cell) if cell.display_lines(/*width*/ 120).iter()
            .any(|line| line.to_string().contains("Could not open ~/notes.md:")))
    }));

    // Replace the file like an editor doing an atomic save. The new last line should
    // appear in the still-open tab without reopening the file or jumping to the top.
    app.sidebar_mut()
        .active_document()
        .unwrap()
        .reader()
        .unwrap()
        .0
        .jump_to_latest();
    assert!(side_frame(&mut app, &mut tui, size)?.contains("Original ending."));
    let mut wait_for_preview =
        async |app: &mut App, tui: &mut tui::Tui, server: &mut AppServerSession, text: &str| {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    tokio::select! {
                        Some(event) = server.next_event() => {
                            Box::pin(app.handle_app_server_event(server, event.event)).await;
                        }
                        Some(event @ AppEvent::SidebarDocumentLoaded { .. }) = events.recv() => {
                            Box::pin(app.handle_event(tui, server, event)).await?;
                            if side_frame(app, tui, size)?.contains(text) {
                                return Ok::<_, color_eyre::Report>(());
                            }
                        }
                    }
                }
            })
            .await
            .unwrap_or_else(|_| panic!("open preview did not show {text:?}"))
        };
    let directory = dir.path().join("feature-beta-build");
    let saved = tempfile::NamedTempFile::new_in(dir.path())?;
    std::fs::write(saved.path(), format!("{long_file}Updated ending."))?;
    saved.persist(directory.join("guide.md"))?;
    wait_for_preview(&mut app, &mut tui, &mut server, "Updated ending.").await?;

    // Moving the whole folder makes the file unreadable; bringing it back must
    // restore the scrolled reader, even after the intervening error frame.
    let moved = dir.path().join("moved");
    std::fs::rename(&directory, &moved)?;
    wait_for_preview(&mut app, &mut tui, &mut server, "File not found").await?;
    std::fs::rename(moved, directory)?;
    wait_for_preview(&mut app, &mut tui, &mut server, "Updated ending.").await?;
    let sidebar = app.conversation_frame.panel_area().expect("document panel");
    let visible = (sidebar.y..sidebar.bottom())
        .map(|row| visible_sidebar_row(&tui, sidebar, row))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("refreshed_sidebar_at_reading_position", visible);
    server.shutdown().await?;
    Ok(())
}

fn visible_sidebar_row(tui: &crate::tui::Tui, sidebar: Rect, row: u16) -> String {
    let buffer = tui.terminal.previous_buffer();
    (sidebar.x..sidebar.right())
        .map(|column| buffer[(column, row)].symbol())
        .collect::<String>()
        .trim()
        .to_owned()
}

#[test]
fn preview_error_keeps_the_filename_visible_in_a_narrow_pane() -> Result<()> {
    let path = PathUri::parse("file:///archive/worktrees/docs/2026-09-11-tmux-attach-freeze.md")?;
    let message = |method: &str, reason: &str| {
        let error = codex_app_server_client::TypedRequestError::Server {
            method: method.into(),
            source: codex_app_server_protocol::JSONRPCErrorError {
                code: -32603,
                data: None,
                message: reason.into(),
            },
        };
        open_error::lines(&path, &error.into(), /*width*/ 38)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    insta::assert_snapshot!(message("fs/readFile", "No such file or directory (os error 2)"), @"
    File not found

    2026-09-11-tmux-attach-freeze.md
    in /archive/worktrees/docs/

    This file may have moved since the
    link was added.
    ");

    let denied = message("fs/readFile", "Permission denied (os error 13)");
    assert_eq!(denied.lines().next(), Some("Permission denied"));
    let thread_error = message("thread/read", "No such file or directory (os error 2)");
    assert_eq!(thread_error.lines().next(), Some("Could not open file"));
    assert!(message("fs/readFile", "Storage offline").ends_with("Storage offline"));
    Ok(())
}
