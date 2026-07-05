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
use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;
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

    fn selection_contribution(
        &self,
        width: u16,
        mode: crate::history_cell::HistoryRenderMode,
    ) -> crate::history_cell::SelectionContribution {
        crate::history_cell::selection_contribution_from_display_lines(
            self.display_lines_for_mode(width, mode),
            width,
        )
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

    fn selection_contribution(
        &self,
        width: u16,
        mode: crate::history_cell::HistoryRenderMode,
    ) -> crate::history_cell::SelectionContribution {
        crate::history_cell::selection_contribution_from_display_lines(
            self.display_lines_for_mode(width, mode),
            width,
        )
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
    let split_preference = app.chat_widget.owned_screen_split_preference();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|frame| {
            let layout = OwnedScreenLayout::new(frame.area(), has_side, focused, split_preference);
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

fn primary_event(kind: MousePrimaryEventKind, column: u16, row: u16) -> MousePrimaryEvent {
    MousePrimaryEvent { kind, column, row }
}

fn primary_press(column: u16, row: u16) -> MousePrimaryEvent {
    primary_event(MousePrimaryEventKind::Press, column, row)
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
async fn primary_press_focuses_closed_parent_without_enabling_input() {
    let mut app = app_with_owned_side().await;
    seed_pane(&mut app, PaneSlot::Parent, "", &["parent transcript"]);
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .mark_thread_closed();
    assert!(app.chat_widget.focus(PaneSlot::Side));
    let _terminal = render_app(&mut app, /*width*/ 83, /*height*/ 12);

    let parent_area = app
        .chat_widget
        .by_slot(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("parent screen")
        .last_pane_area;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    assert!(
        app.handle_owned_screen_mouse_primary(
            &mut tui,
            primary_press(parent_area.x, parent_area.y),
        )
    );
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    let split_preference = app.chat_widget.owned_screen_split_preference();
    let mut rendered_cursor = None;
    let mut terminal =
        Terminal::new(TestBackend::new(/*width*/ 83, /*height*/ 12)).expect("create terminal");
    terminal
        .draw(|frame| {
            rendered_cursor = Some(
                render_layout(
                    &mut app.chat_widget,
                    OwnedScreenLayout::new(
                        frame.area(),
                        /*has_side*/ true,
                        PaneSlot::Parent,
                        split_preference,
                    ),
                    PaneSlot::Parent,
                    frame.buffer_mut(),
                )
                .expect("render closed parent")
                .cursor,
            );
        })
        .expect("render owned panes");
    assert_eq!(rendered_cursor, Some(None));
    assert_eq!(
        app.chat_widget
            .by_slot(PaneSlot::Side)
            .expect("side pane")
            .composer_text_with_pending(),
        "side draft"
    );
}

#[tokio::test]
async fn primary_drag_selects_text_in_a_single_owned_pane() {
    let mut app = app_with_owned_parent().await;
    seed_pane(&mut app, PaneSlot::Parent, "", &["selectable"]);
    let _terminal = render_app(&mut app, /*width*/ 40, /*height*/ 10);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    assert!(
        app.handle_owned_screen_mouse_primary(&mut tui, primary_press(/*column*/ 0, /*row*/ 0),)
    );
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(
            MousePrimaryEventKind::Drag,
            /*column*/ 4,
            /*row*/ 0,
        ),
    ));
    assert!(
        app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );

    let selected = render_app(&mut app, /*width*/ 40, /*height*/ 10);
    for column in 0..=4 {
        assert!(
            selected.backend().buffer()[(column, 0)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED),
            "column {column} should be highlighted"
        );
    }
}

#[tokio::test]
async fn primary_drag_can_start_in_pet_reserved_right_padding() {
    let mut app = app_with_owned_parent().await;
    app.chat_widget
        .set_pet_image_support_for_tests(crate::pets::PetImageSupport::Supported(
            crate::pets::ImageProtocol::Kitty,
        ));
    app.chat_widget
        .install_test_ambient_pet_for_tests(/*animations_enabled*/ false);
    seed_pane(&mut app, PaneSlot::Parent, "", &["selectable"]);
    let _terminal = render_app(&mut app, /*width*/ 40, /*height*/ 10);
    let (pane_area, conversation_area) = {
        let screen = app
            .chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen");
        (screen.last_pane_area, screen.last_conversation_area)
    };
    assert!(conversation_area.right() < pane_area.right());
    let padding_column = pane_area.right().saturating_sub(/*rhs*/ 1);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(padding_column, conversation_area.y),
    ));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(
            MousePrimaryEventKind::Drag,
            conversation_area.x,
            conversation_area.y,
        ),
    ));
    assert!(
        app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );

    app.cancel_owned_screen_selection();
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(padding_column, conversation_area.bottom()),
    ));
    assert!(
        !app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );
}

#[tokio::test]
async fn click_release_clears_selection_without_copying() {
    let mut app = app_with_owned_parent().await;
    seed_pane(&mut app, PaneSlot::Parent, "", &["selectable"]);
    let _terminal = render_app(&mut app, /*width*/ 40, /*height*/ 10);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    assert!(
        app.handle_owned_screen_mouse_primary(&mut tui, primary_press(/*column*/ 0, /*row*/ 0),)
    );
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(
            MousePrimaryEventKind::Release,
            /*column*/ 0,
            /*row*/ 0,
        ),
    ));
    assert!(
        !app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );
}

#[tokio::test]
async fn text_drag_crosses_divider_but_divider_press_takes_priority() {
    let mut app = app_with_owned_side().await;
    seed_pane(&mut app, PaneSlot::Parent, "", &["parent selectable"]);
    seed_pane(&mut app, PaneSlot::Side, "", &["side selectable"]);
    let _terminal = render_app(&mut app, /*width*/ 120, /*height*/ 12);
    let parent = app
        .chat_widget
        .by_slot(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("parent screen");
    let conversation = parent.last_conversation_area;
    let divider_column = parent.last_pane_area.right();
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(conversation.x, conversation.y),
    ));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(
            MousePrimaryEventKind::Drag,
            divider_column.saturating_add(/*rhs*/ 8),
            conversation.y,
        ),
    ));
    assert!(!app.chat_widget.owned_screen_split_is_dragging());
    assert!(
        app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(divider_column, conversation.y),
    ));
    assert!(app.chat_widget.owned_screen_split_is_dragging());
    assert!(
        !app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(
            MousePrimaryEventKind::Release,
            divider_column,
            conversation.y,
        ),
    ));
}

#[tokio::test]
async fn focused_only_layout_cancels_selection_in_the_hidden_pane() {
    let mut app = app_with_owned_side().await;
    seed_pane(&mut app, PaneSlot::Parent, "", &["parent selectable"]);
    seed_pane(&mut app, PaneSlot::Side, "", &["side selectable"]);
    let _wide = render_app(&mut app, /*width*/ 83, /*height*/ 12);
    let conversation = app
        .chat_widget
        .by_slot(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("parent screen")
        .last_conversation_area;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(conversation.x, conversation.y),
    ));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(
            MousePrimaryEventKind::Drag,
            conversation.x.saturating_add(/*rhs*/ 4),
            conversation.y,
        ),
    ));

    assert!(app.chat_widget.focus(PaneSlot::Side));
    let _narrow = render_app(&mut app, /*width*/ 82, /*height*/ 12);

    assert!(
        !app.chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("parent screen")
            .selection_is_active()
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
    let _wide = render_app(&mut app, /*width*/ 83, /*height*/ 14);
    assert!(app.chat_widget.focus(PaneSlot::Side));

    let terminal = render_app(&mut app, /*width*/ 82, /*height*/ 14);

    assert_snapshot!("owned_screen_narrow_side_focused", terminal.backend());
    assert_eq!(
        app.chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .map(|screen| (screen.last_pane_area, screen.last_conversation_area)),
        Some((Rect::default(), Rect::default()))
    );

    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    assert!(
        app.handle_owned_screen_mouse_primary(&mut tui, primary_press(/*column*/ 2, /*row*/ 2),)
    );
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Side);
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
async fn primary_press_focuses_visible_pane_regions() {
    let mut app = app_with_owned_side().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "parent draft",
        &["parent transcript"],
    );
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);
    let _terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);
    let (parent_area, parent_conversation_area, side_area, side_conversation_area) = {
        let parent = app
            .chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("parent screen");
        let side = app
            .chat_widget
            .by_slot(PaneSlot::Side)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("side screen");
        (
            parent.last_pane_area,
            parent.last_conversation_area,
            side.last_pane_area,
            side.last_conversation_area,
        )
    };
    let region_pairs = [
        ((parent_area.x, parent_area.y), (side_area.x, side_area.y)),
        (
            (parent_conversation_area.x, parent_conversation_area.y),
            (side_conversation_area.x, side_conversation_area.y),
        ),
        (
            (parent_area.x, parent_conversation_area.bottom()),
            (side_area.x, side_conversation_area.bottom()),
        ),
        (
            (parent_area.x, parent_area.bottom() - 1),
            (side_area.x, side_area.bottom() - 1),
        ),
    ];
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    for (parent_position, side_position) in region_pairs {
        app.backtrack.primed = true;
        assert!(app.handle_owned_screen_mouse_primary(
            &mut tui,
            primary_press(side_position.0, side_position.1),
        ));
        assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Side);
        assert!(!app.backtrack.primed);
        assert!(app.handle_owned_screen_mouse_primary(
            &mut tui,
            primary_press(parent_position.0, parent_position.1),
        ));
        assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    }

    app.backtrack.primed = true;
    assert!(
        app.handle_owned_screen_mouse_primary(
            &mut tui,
            primary_press(parent_area.x, parent_area.y),
        )
    );
    assert!(!app.backtrack.primed);
    assert!(!app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(parent_area.right(), parent_area.y),
    ));
    assert!(!app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(side_area.right(), side_area.y),
    ));
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
}

#[tokio::test]
async fn primary_press_preserves_pane_drafts_and_cursors() {
    let mut app = app_with_owned_side().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "parent draft",
        &["parent transcript"],
    );
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);
    for (slot, cursor_moves) in [(PaneSlot::Parent, 1), (PaneSlot::Side, 2)] {
        for _ in 0..cursor_moves {
            app.chat_widget
                .by_slot_mut(slot)
                .expect("installed pane")
                .handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        }
    }

    let mut parent_terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);
    let parent_cursor = parent_terminal
        .get_cursor_position()
        .expect("parent cursor");
    let (parent_area, parent_conversation_area, side_area, side_conversation_area) = {
        let parent = app
            .chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("parent screen");
        let side = app
            .chat_widget
            .by_slot(PaneSlot::Side)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("side screen");
        (
            parent.last_pane_area,
            parent.last_conversation_area,
            side.last_pane_area,
            side.last_conversation_area,
        )
    };
    assert!(app.chat_widget.focus(PaneSlot::Side));
    let mut side_terminal_before_click =
        render_app(&mut app, /*width*/ 83, /*height*/ 16);
    let side_cursor_before_click = side_terminal_before_click
        .get_cursor_position()
        .expect("side cursor before click");
    assert!(app.chat_widget.focus(PaneSlot::Parent));
    let _parent_terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(side_area.x, side_conversation_area.bottom()),
    ));
    let mut side_terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);
    let side_cursor = side_terminal.get_cursor_position().expect("side cursor");
    assert_eq!(side_cursor, side_cursor_before_click);
    assert_snapshot!(
        "owned_screen_wide_split_side_focused_by_click",
        side_terminal.backend()
    );
    let buffer = side_terminal.backend().buffer();
    assert!(
        buffer[(parent_area.x + 1, parent_area.y)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::DIM)
    );
    assert!(
        buffer[(side_area.x + 1, side_area.y)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert_eq!(
        (
            app.chat_widget
                .by_slot(PaneSlot::Parent)
                .expect("parent pane")
                .composer_text_with_pending(),
            app.chat_widget
                .by_slot(PaneSlot::Side)
                .expect("side pane")
                .composer_text_with_pending(),
        ),
        ("parent draft".to_string(), "side draft".to_string())
    );

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(parent_area.x, parent_conversation_area.bottom()),
    ));
    let mut parent_terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);
    assert_eq!(
        parent_terminal
            .get_cursor_position()
            .expect("restored parent cursor"),
        parent_cursor
    );
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(side_area.x, side_conversation_area.bottom()),
    ));
    let mut side_terminal = render_app(&mut app, /*width*/ 83, /*height*/ 16);
    assert_eq!(
        side_terminal
            .get_cursor_position()
            .expect("restored side cursor"),
        side_cursor
    );
}

#[tokio::test]
async fn primary_press_does_not_switch_behind_overlay_or_popup() {
    let mut app = app_with_owned_side().await;
    let _terminal = render_app(&mut app, /*width*/ 83, /*height*/ 12);
    let side_area = app
        .chat_widget
        .by_slot(PaneSlot::Side)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("side screen")
        .last_pane_area;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    app.overlay = Some(Overlay::new_transcript(
        Vec::new(),
        app.keymap.pager.clone(),
    ));
    assert!(
        !app.handle_owned_screen_mouse_primary(&mut tui, primary_press(side_area.x, side_area.y),)
    );
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);

    app.overlay = None;
    let keymap = app.keymap.clone();
    app.chat_widget.open_keymap_debug(&keymap);
    assert!(
        !app.handle_owned_screen_mouse_primary(&mut tui, primary_press(side_area.x, side_area.y),)
    );
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
}

#[tokio::test]
async fn primary_drag_resizes_panes_without_changing_pane_state() {
    let mut app = app_with_owned_side().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "parent draft",
        &["parent transcript"],
    );
    seed_pane(&mut app, PaneSlot::Side, "side draft", &["side transcript"]);
    let _initial = render_app(&mut app, /*width*/ 120, /*height*/ 12);
    let initial_parent = app
        .chat_widget
        .by_slot(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("parent screen")
        .last_pane_area;
    let initial_side = app
        .chat_widget
        .by_slot(PaneSlot::Side)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("side screen")
        .last_pane_area;
    assert_eq!((initial_parent.width, initial_side.width), (60, 59));

    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    app.backtrack.primed = true;
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(
            initial_parent.right(),
            initial_parent.y.saturating_add(/*rhs*/ 2)
        ),
    ));
    assert!(app.chat_widget.owned_screen_split_is_dragging());
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(MousePrimaryEventKind::Drag, /*column*/ 70, u16::MAX,),
    ));

    let active = render_app(&mut app, /*width*/ 120, /*height*/ 12);
    let parent = app
        .chat_widget
        .by_slot(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("parent screen")
        .last_pane_area;
    let side = app
        .chat_widget
        .by_slot(PaneSlot::Side)
        .and_then(|pane| pane.owned_screen.as_ref())
        .expect("side screen")
        .last_pane_area;
    assert_eq!((parent.width, side.width), (70, 49));
    assert_eq!(active.backend().buffer()[(parent.right(), 2)].symbol(), "┃");
    assert!(
        active.backend().buffer()[(parent.right(), 2)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert_snapshot!(
        "owned_screen_resized_parent_wide_dragging",
        active.backend()
    );
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    assert!(app.backtrack.primed);
    assert_eq!(
        (
            app.chat_widget
                .by_slot(PaneSlot::Parent)
                .expect("parent pane")
                .composer_text_with_pending(),
            app.chat_widget
                .by_slot(PaneSlot::Side)
                .expect("side pane")
                .composer_text_with_pending(),
        ),
        ("parent draft".to_string(), "side draft".to_string())
    );

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(MousePrimaryEventKind::Release, /*column*/ 70, u16::MAX,),
    ));
    assert!(!app.chat_widget.owned_screen_split_is_dragging());
    let settled = render_app(&mut app, /*width*/ 120, /*height*/ 12);
    assert_eq!(
        settled.backend().buffer()[(parent.right(), 2)].symbol(),
        "│"
    );
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
async fn mouse_wheel_routes_to_the_pane_with_an_active_selection() {
    let mut app = app_with_owned_side().await;
    let cells = [
        "one", "two", "three", "four", "five", "six", "seven", "eight",
    ];
    seed_pane(&mut app, PaneSlot::Parent, "", &cells);
    seed_pane(&mut app, PaneSlot::Side, "", &cells);
    let _terminal = render_app(&mut app, /*width*/ 83, /*height*/ 8);
    let (parent_area, side_area) = {
        let parent = app
            .chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("parent screen")
            .last_conversation_area;
        let side = app
            .chat_widget
            .by_slot(PaneSlot::Side)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("side screen")
            .last_conversation_area;
        (parent, side)
    };
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(
            parent_area.x,
            parent_area.bottom().saturating_sub(/*rhs*/ 1),
        ),
    ));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(MousePrimaryEventKind::Drag, parent_area.x, parent_area.y,),
    ));

    assert!(app.handle_owned_screen_mouse_scroll(
        &mut tui,
        MouseScrollEvent {
            direction: MouseScrollDirection::Up,
            column: side_area.x,
            row: side_area.y,
        },
    ));

    assert!(!is_following_bottom(&app, PaneSlot::Parent));
    assert!(is_following_bottom(&app, PaneSlot::Side));
    assert!(
        app.chat_widget
            .by_slot(PaneSlot::Parent)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("parent screen")
            .selection_is_active()
    );
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
async fn navigation_preserves_composer_keys_and_draft_input() {
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
async fn updated_pager_keymap_reaches_both_owned_panes() {
    let mut app = app_with_owned_side().await;
    app.keymap.pager.page_up = vec![crate::key_hint::ctrl(KeyCode::Char('g'))];
    app.sync_owned_screen_keymap();
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test TUI");

    for slot in [PaneSlot::Parent, PaneSlot::Side] {
        assert!(app.chat_widget.focus(slot));
        assert!(app.handle_owned_screen_navigation_key(
            &mut tui,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        ));
        assert!(!app.handle_owned_screen_navigation_key(
            &mut tui,
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        ));
    }
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
