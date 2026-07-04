use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;

use super::super::conversation_panes::ConversationPaneInit;
use super::*;
use crate::app_event::PaneSlot;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane_with_sender;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::file_search::FileSearchManager;
use crate::tui::MouseScrollDirection;
use crate::tui::MouseScrollEvent;
use codex_app_server_protocol::ConfigWarningNotification;

#[derive(Debug)]
struct TestCell(&'static str);

impl HistoryCell for TestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![self.0.into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![self.0.into()]
    }
}

#[derive(Debug)]
struct RenderModeCell {
    display: &'static str,
    raw: &'static str,
}

impl HistoryCell for RenderModeCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![self.display.into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![self.raw.into()]
    }
}

async fn app_with_owned_parent() -> App {
    let mut app = super::super::test_support::make_test_app().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    app
}

async fn app_with_owned_side() -> App {
    let mut app = app_with_owned_parent().await;
    let (side_widget, _side_rx) = make_chatwidget_for_pane(PaneSlot::Side).await;
    let file_search = FileSearchManager::new(
        side_widget.config_ref().cwd.to_path_buf(),
        side_widget.conversation_event_sender(),
    );
    let owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &side_widget,
        app.keymap.pager.clone(),
    );
    let result = app.chat_widget.install_side(ConversationPaneInit {
        chat_widget: side_widget,
        file_search,
        owned_screen,
    });
    assert!(result.is_ok(), "side pane should install");
    app
}

fn seed_pane(app: &mut App, slot: PaneSlot, draft: &str, cells: &[&'static str]) {
    let pane = app.chat_widget.by_slot_mut(slot).expect("installed pane");
    pane.chat_widget
        .set_composer_text(draft.to_string(), Vec::new(), Vec::new());
    let screen = pane.owned_screen.as_mut().expect("owned screen");
    for text in cells {
        screen.viewport.push_cell(Arc::new(TestCell(text)));
    }
}

fn render_app(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let focused = app.chat_widget.focused_slot();
    let has_side = app.chat_widget.has_side();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|frame| {
            let layout = OwnedScreenLayout::new(frame.area(), has_side, focused);
            if let Some(rendered) =
                render_layout(&mut app.chat_widget, layout, focused, frame.buffer_mut())
                && let Some((x, y)) = rendered.cursor
            {
                frame.set_cursor_position((x, y));
            }
        })
        .expect("render owned panes");
    terminal
}

fn is_following_bottom(app: &App, slot: PaneSlot) -> bool {
    app.chat_widget
        .by_slot(slot)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("owned screen")
        .viewport
        .is_following_bottom()
}

#[test]
fn responsive_layout_uses_expected_threshold_and_parent_bias() {
    let area_narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 82, /*height*/ 20,
    );
    let narrow = OwnedScreenLayout::new(area_narrow, /*has_side*/ true, PaneSlot::Side);
    assert!(matches!(
        narrow,
        OwnedScreenLayout::Single {
            slot: PaneSlot::Side,
            show_header: true,
            ..
        }
    ));
    assert_eq!(
        OwnedScreenLayout::new(area_narrow, /*has_side*/ false, PaneSlot::Side),
        OwnedScreenLayout::Single {
            slot: PaneSlot::Parent,
            area: area_narrow,
            show_header: false,
        }
    );
    for (width, expected_parent_width) in [(83, 41), (84, 42)] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 20);
        let OwnedScreenLayout::Split {
            parent,
            divider,
            side,
            ..
        } = OwnedScreenLayout::new(area, /*has_side*/ true, PaneSlot::Parent)
        else {
            panic!("width {width} should split");
        };
        assert_eq!(parent.width, expected_parent_width);
        assert_eq!(
            divider,
            Rect::new(
                parent.right(),
                /*y*/ 0,
                /*width*/ 1,
                /*height*/ 20
            )
        );
        assert_eq!(side.width, 41);
    }
}

#[tokio::test]
async fn single_pane_app_layout_preserves_existing_owned_render() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "draft sentinel",
        &["committed response"],
    );

    let terminal = render_app(&mut app, /*width*/ 50, /*height*/ 10);

    assert_snapshot!(terminal.backend(), @r###"
"committed response                                "
"                                                  "
"                                                  "
"                                                  "
"                                                  "
"                                                  "
"                                                  "
"› draft sentinel                                  "
"                                                  "
"  gpt-5.5 default · /tmp/project                  "
"###);
}

#[tokio::test]
async fn renders_wide_parent_left_and_side_right() {
    let mut app = app_with_owned_side().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "parent draft",
        &["parent transcript"],
    );
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);

    let terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);

    assert_snapshot!("owned_screen_wide_split_parent_focused", terminal.backend());
    let buffer = terminal.backend().buffer();
    assert!(
        buffer[(1, 0)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert!(
        buffer[(43, 0)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::DIM)
    );
}

#[tokio::test]
async fn raw_output_mode_fans_out_without_changing_focus_or_drafts() {
    let mut app = app_with_owned_side().await;
    for (slot, draft, display, raw) in [
        (
            PaneSlot::Parent,
            "parent draft",
            "parent rich transcript",
            "parent raw transcript",
        ),
        (
            PaneSlot::Side,
            "side draft",
            "side rich transcript",
            "side raw transcript",
        ),
    ] {
        let pane = app.chat_widget.by_slot_mut(slot).expect("installed pane");
        pane.chat_widget
            .set_composer_text(draft.to_string(), Vec::new(), Vec::new());
        pane.owned_screen
            .as_mut()
            .expect("owned screen")
            .viewport
            .push_cell(Arc::new(RenderModeCell { display, raw }));
    }
    assert!(app.chat_widget.focus(PaneSlot::Side));
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    app.apply_raw_output_mode(&mut tui, /*enabled*/ true, /*notify*/ false);

    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Side);
    for (slot, draft) in [
        (PaneSlot::Parent, "parent draft"),
        (PaneSlot::Side, "side draft"),
    ] {
        let pane = app.chat_widget.by_slot(slot).expect("installed pane");
        assert!(pane.chat_widget.raw_output_mode());
        assert_eq!(pane.chat_widget.composer_text_with_pending(), draft);
    }
    let terminal = render_app(&mut app, /*width*/ 83, /*height*/ 12);
    assert_snapshot!("owned_screen_raw_output_mode_fans_out", terminal.backend());
}

#[tokio::test]
async fn global_warning_renders_in_both_owned_panes() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) =
        super::super::test_support::make_test_app_with_channels().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let side_widget =
        make_chatwidget_for_pane_with_sender(PaneSlot::Side, app.app_event_tx.clone()).await;
    let file_search = FileSearchManager::new(
        side_widget.config_ref().cwd.to_path_buf(),
        side_widget.conversation_event_sender(),
    );
    let owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &side_widget,
        app.keymap.pager.clone(),
    );
    assert!(
        app.chat_widget
            .install_side(ConversationPaneInit {
                chat_widget: side_widget,
                file_search,
                owned_screen,
            })
            .is_ok(),
        "side pane should install"
    );
    for (slot, draft) in [
        (PaneSlot::Parent, "parent draft"),
        (PaneSlot::Side, "side draft"),
    ] {
        app.chat_widget
            .by_slot_mut(slot)
            .expect("installed pane")
            .set_composer_text(draft.to_string(), Vec::new(), Vec::new());
    }
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(
            ServerNotification::ConfigWarning(ConfigWarningNotification {
                summary: "Shared configuration warning".to_string(),
                details: None,
                path: None,
                range: None,
            }),
        ),
    )
    .await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while let Ok(event) = app_event_rx.try_recv() {
        app.handle_event(&mut tui, &mut app_server, event).await?;
    }
    assert_eq!(
        app.chat_widget
            .by_slot(PaneSlot::Parent)
            .expect("parent pane")
            .transcript_cells
            .len(),
        1
    );
    assert_eq!(
        app.chat_widget
            .by_slot(PaneSlot::Side)
            .expect("side pane")
            .transcript_cells
            .len(),
        1
    );

    let terminal = render_app(&mut app, /*width*/ 83, /*height*/ 18);

    assert_snapshot!("owned_screen_global_warning_fans_out", terminal.backend());
    Ok(())
}

#[tokio::test]
async fn renders_closed_parent_read_only_while_side_remains_focused() {
    let mut app = app_with_owned_side().await;
    seed_pane(&mut app, PaneSlot::Parent, "", &["parent transcript"]);
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .mark_thread_closed();
    assert!(app.chat_widget.focus(PaneSlot::Side));

    let terminal = render_app(&mut app, /*width*/ 83, /*height*/ 12);

    assert_snapshot!(
        "owned_screen_closed_parent_side_focused",
        terminal.backend()
    );
}

#[tokio::test]
async fn narrow_layout_renders_only_the_focused_side() {
    let mut app = app_with_owned_side().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "parent draft",
        &["PARENT MUST BE HIDDEN"],
    );
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);
    assert!(app.chat_widget.focus(PaneSlot::Side));

    let terminal = render_app(&mut app, /*width*/ 82, /*height*/ 14);

    assert_snapshot!("owned_screen_narrow_side_focused", terminal.backend());
    assert_eq!(
        app.chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .map(|screen| screen.last_conversation_area),
        Some(Rect::default())
    );
}

#[tokio::test]
async fn terminal_cursor_tracks_only_the_focused_pane() {
    let mut app = app_with_owned_side().await;
    seed_pane(&mut app, PaneSlot::Parent, "parent", &[]);
    seed_pane(&mut app, PaneSlot::Side, "side", &[]);

    let mut terminal = render_app(&mut app, /*width*/ 83, /*height*/ 8);
    let parent_cursor = terminal.get_cursor_position().expect("parent cursor");
    assert!(parent_cursor.x < 41);

    assert!(app.chat_widget.focus(PaneSlot::Side));
    terminal = render_app(&mut app, /*width*/ 83, /*height*/ 8);
    let side_cursor = terminal.get_cursor_position().expect("side cursor");
    assert!(side_cursor.x > 41);
}

#[tokio::test]
async fn mouse_wheel_routes_by_pointer_without_changing_focus() {
    let mut app = app_with_owned_side().await;
    let cells = [
        "one", "two", "three", "four", "five", "six", "seven", "eight",
    ];
    seed_pane(&mut app, PaneSlot::Parent, "", &cells);
    seed_pane(&mut app, PaneSlot::Side, "", &cells);
    let _terminal = render_app(&mut app, /*width*/ 83, /*height*/ 8);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    assert!(app.handle_owned_screen_mouse_scroll(
        &mut tui,
        MouseScrollEvent {
            direction: MouseScrollDirection::Up,
            column: 2,
            row: 2,
        },
    ));
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    assert!(!is_following_bottom(&app, PaneSlot::Parent));
    assert!(is_following_bottom(&app, PaneSlot::Side));

    assert!(app.handle_owned_screen_mouse_scroll(
        &mut tui,
        MouseScrollEvent {
            direction: MouseScrollDirection::Up,
            column: 44,
            row: 2,
        },
    ));
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    assert!(!is_following_bottom(&app, PaneSlot::Side));
}

#[tokio::test]
async fn narrow_side_clears_parent_hit_area_before_wheel_routing() {
    let mut app = app_with_owned_side().await;
    let cells = ["one", "two", "three", "four", "five", "six"];
    seed_pane(&mut app, PaneSlot::Parent, "", &cells);
    seed_pane(&mut app, PaneSlot::Side, "", &cells);
    let _wide = render_app(&mut app, /*width*/ 83, /*height*/ 7);
    assert!(app.chat_widget.focus(PaneSlot::Side));
    let _narrow = render_app(&mut app, /*width*/ 82, /*height*/ 7);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    let side_area = app
        .chat_widget
        .by_slot(PaneSlot::Side)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("side screen")
        .last_conversation_area;
    assert!(side_area.height > 0);

    assert!(app.handle_owned_screen_mouse_scroll(
        &mut tui,
        MouseScrollEvent {
            direction: MouseScrollDirection::Up,
            column: side_area.x,
            row: side_area.y,
        },
    ));
    assert!(is_following_bottom(&app, PaneSlot::Parent));
    assert!(!is_following_bottom(&app, PaneSlot::Side));
}

#[tokio::test]
async fn resizing_between_split_and_focused_only_preserves_pane_state() {
    let mut app = app_with_owned_side().await;
    let cells = [
        "one", "two", "three", "four", "five", "six", "seven", "eight",
    ];
    seed_pane(&mut app, PaneSlot::Parent, "parent draft", &cells);
    seed_pane(&mut app, PaneSlot::Side, "side draft", &cells);
    let _wide = render_app(&mut app, /*width*/ 83, /*height*/ 8);
    let parent_screen = app
        .chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_mut())
        .expect("parent screen");
    assert!(parent_screen.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Up,
        column: 2,
        row: 2,
    }));
    assert!(app.chat_widget.focus(PaneSlot::Side));

    let _narrow = render_app(&mut app, /*width*/ 82, /*height*/ 8);
    let _wide_again = render_app(&mut app, /*width*/ 83, /*height*/ 8);

    assert_eq!(
        app.chat_widget
            .by_slot(PaneSlot::Parent)
            .expect("parent pane")
            .composer_text_with_pending(),
        "parent draft"
    );
    assert_eq!(app.chat_widget.composer_text_with_pending(), "side draft");
    assert!(!is_following_bottom(&app, PaneSlot::Parent));
    assert!(is_following_bottom(&app, PaneSlot::Side));
}

#[tokio::test]
async fn renders_committed_conversation_above_fixed_composer() {
    let (mut chat_widget, _app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    chat_widget.set_composer_text("draft sentinel".to_string(), Vec::new(), Vec::new());
    let mut screen = OwnedScreen::new(&chat_widget, crate::keymap::RuntimeKeymap::defaults().pager);
    screen
        .viewport
        .push_cell(Arc::new(TestCell("committed response")));
    let mut terminal =
        Terminal::new(TestBackend::new(/*width*/ 50, /*height*/ 10)).expect("create terminal");

    terminal
        .draw(|frame| {
            screen.render(&chat_widget, frame.area(), frame.buffer_mut());
        })
        .expect("render owned screen");

    assert_snapshot!(terminal.backend(), @r###"
"committed response                                "
"                                                  "
"                                                  "
"                                                  "
"                                                  "
"                                                  "
"                                                  "
"› draft sentinel                                  "
"                                                  "
"  gpt-5.5 default · /tmp/project                  "
"###);
}

#[tokio::test]
async fn committed_cell_updates_viewport_without_queuing_terminal_history() {
    let mut app = super::super::test_support::make_test_app().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    app.insert_history_cell(&mut tui, Box::new(TestCell("retained")));

    let screen = app.chat_widget.owned_screen.as_ref().expect("owned screen");
    assert_eq!(screen.viewport.committed_cell_count(), 1);
    assert_eq!(app.chat_widget.transcript_cells.len(), 1);
    assert!(!app.has_emitted_history_lines);
    assert!(!tui.has_pending_history_lines());
}

#[tokio::test]
async fn replay_retains_cells_while_draw_scheduling_is_deferred() {
    let mut app = super::super::test_support::make_test_app().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    let mut draw_rx = tui.subscribe_draws_for_test();

    app.begin_initial_history_replay_buffer();
    app.insert_history_cell(&mut tui, Box::new(TestCell("first")));
    app.insert_history_cell(&mut tui, Box::new(TestCell("second")));

    tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await;
    assert!(matches!(draw_rx.try_recv(), Err(TryRecvError::Empty)));

    assert!(app.owned_screen_replay_in_progress());
    assert_eq!(
        app.chat_widget
            .owned_screen
            .as_ref()
            .expect("owned screen")
            .viewport
            .committed_cell_count(),
        2
    );

    app.finish_initial_history_replay_buffer(&mut tui);

    assert!(!app.owned_screen_replay_in_progress());
    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), draw_rx.recv())
        .await
        .expect("timed out waiting for replay completion draw")
        .expect("draw channel closed");
}

#[tokio::test]
async fn navigation_does_not_steal_printable_or_draft_input() {
    let mut app = super::super::test_support::make_test_app().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    let cases = [
        (KeyCode::Char('k'), false),
        (KeyCode::Up, false),
        (KeyCode::Down, false),
        (KeyCode::Home, false),
        (KeyCode::End, false),
        (KeyCode::PageUp, true),
        (KeyCode::PageDown, true),
    ];
    for (code, expected) in cases {
        assert_eq!(
            app.handle_owned_screen_navigation_key(
                &mut tui,
                KeyEvent::new(code, KeyModifiers::NONE),
            ),
            expected,
        );
    }

    app.chat_widget
        .set_composer_text("draft".to_string(), Vec::new(), Vec::new());
    assert!(!app.handle_owned_screen_navigation_key(
        &mut tui,
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
    ));
}

#[tokio::test]
async fn mouse_wheel_scrolls_transcript_without_changing_draft() {
    let (mut chat_widget, _app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    chat_widget.set_composer_text("draft sentinel".to_string(), Vec::new(), Vec::new());
    let mut screen = OwnedScreen::new(&chat_widget, crate::keymap::RuntimeKeymap::defaults().pager);
    for text in ["oldest", "older", "middle", "newer", "LATEST"] {
        screen.viewport.push_cell(Arc::new(TestCell(text)));
    }
    let mut terminal =
        Terminal::new(TestBackend::new(/*width*/ 40, /*height*/ 8)).expect("create terminal");
    terminal
        .draw(|frame| {
            screen.render(&chat_widget, frame.area(), frame.buffer_mut());
        })
        .expect("render bottom");

    assert!(screen.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Up,
        column: 2,
        row: 2,
    }));
    terminal
        .draw(|frame| {
            screen.render(&chat_widget, frame.area(), frame.buffer_mut());
        })
        .expect("render scrolled");

    assert_snapshot!(terminal.backend(), @r###"
"                                        "
"middle                                  "
"                                        "
"                                        "
"                                        "
"› draft sentinel                        "
"                                        "
"  gpt-5.5 default · /tmp/project        "
"###);
    assert!(!screen.viewport.is_following_bottom());
    assert!(!screen.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Up,
        column: 2,
        row: 7,
    }));

    assert!(screen.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Down,
        column: 2,
        row: 2,
    }));
    terminal
        .draw(|frame| {
            screen.render(&chat_widget, frame.area(), frame.buffer_mut());
        })
        .expect("render restored bottom");
    assert!(screen.viewport.is_following_bottom());
}
