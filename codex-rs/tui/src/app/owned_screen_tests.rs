use codex_terminal_browser::BrowserCell;
use codex_terminal_browser::BrowserMouseButton;
use codex_terminal_browser::BrowserMouseInput;
use codex_terminal_browser::BrowserMouseKind;
use codex_terminal_browser::BrowserScreen;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::BrowserView;
use codex_terminal_browser::TerminalBrowser;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;

use super::super::conversation_panes::ConversationPaneInit;
use super::*;
use crate::app_event::OwnedScreenPanelPreference;
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

async fn app_with_owned_parent_and_events() -> (App, tokio::sync::mpsc::UnboundedReceiver<AppEvent>)
{
    let (mut app, app_event_rx, _op_rx) =
        super::super::test_support::make_test_app_with_channels().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    (app, app_event_rx)
}

async fn install_owned_side(app: &mut App) {
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
}

async fn app_with_owned_side() -> App {
    let mut app = app_with_owned_parent().await;
    install_owned_side(&mut app).await;
    app
}

async fn app_with_owned_side_and_events() -> (App, tokio::sync::mpsc::UnboundedReceiver<AppEvent>) {
    let (mut app, app_event_rx) = app_with_owned_parent_and_events().await;
    install_owned_side(&mut app).await;
    (app, app_event_rx)
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

fn render_frame_app(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    render_frame_app_with_browser(app, width, height, /*browser_view*/ None)
}

fn render_frame_app_with_browser(
    app: &mut App,
    width: u16,
    height: u16,
    browser_view: Option<&BrowserView>,
) -> Terminal<TestBackend> {
    let frame_overlays_enabled = app.chat_widget.no_modal_or_popup_active();
    let mut browser_chrome = crate::terminal_browser::BrowserChromeState::default();
    browser_chrome.sync_url(browser_view.and_then(|view| view.url.as_deref()));
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|frame| {
            let browser = browser_view.map(|view| OwnedScreenBrowser {
                runtime: None,
                view,
                chrome: &browser_chrome,
            });
            let rendered = render_owned_screen_contents(
                &mut app.chat_widget,
                &mut app.owned_screen_frame,
                frame.area(),
                frame.buffer_mut(),
                frame_overlays_enabled,
                browser,
            );
            if let Some(rendered) = rendered {
                if browser_view.is_some_and(|view| view.human_control)
                    && let Some((x, y)) = rendered.browser_cursor
                {
                    frame.set_cursor_position((x, y));
                } else if app.owned_screen_frame.focus() == OwnedScreenFrameFocus::Conversation
                    && let Some((x, y)) = rendered.cursor
                {
                    frame.set_cursor_position((x, y));
                }
            }
        })
        .expect("render owned frame");
    terminal
}

fn browser_view(human_control: bool, cursor: Option<(u16, u16)>) -> BrowserView {
    let rows = 3;
    let cols = 24;
    let mut cells = vec![BrowserCell::default(); usize::from(rows * cols)];
    for (row, text) in ["Codex browser panel", "interactive page", "ready"]
        .into_iter()
        .enumerate()
    {
        for (col, character) in text.chars().enumerate() {
            cells[row * usize::from(cols) + col].text = character.to_string();
        }
    }
    BrowserView {
        status: BrowserStatus::Running,
        title: Some("Example".to_string()),
        url: Some("https://example.com".to_string()),
        visible: true,
        human_control,
        screen: BrowserScreen {
            rows,
            cols,
            cells,
            cursor,
        },
    }
}

fn attach_visible_browser(app: &mut App) -> Arc<TerminalBrowser> {
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let browser = Arc::new(TerminalBrowser::discover());
    browser.set_visibility(/*visible*/ true);
    app.terminal_browser = Some(Arc::clone(&browser));
    app.terminal_browser_owner_thread_id = Some(thread_id);
    browser
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
    MousePrimaryEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
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
async fn owned_screen_auto_frame_wide_renders_both_rails() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "draft sentinel",
        &["committed response"],
    );

    let terminal = render_frame_app(&mut app, /*width*/ 144, /*height*/ 12);

    assert_snapshot!(
        "owned_screen_auto_frame_wide_renders_both_rails",
        terminal.backend()
    );
}

#[tokio::test]
async fn owned_screen_explicit_summary_renders_as_narrow_overlay() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "draft sentinel",
        &["committed response"],
    );
    app.owned_screen_frame
        .set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);

    let terminal = render_frame_app(&mut app, /*width*/ 75, /*height*/ 12);

    assert_snapshot!(
        "owned_screen_explicit_summary_renders_as_narrow_overlay",
        terminal.backend()
    );
}

#[tokio::test]
async fn owned_screen_browser_renders_in_wide_right_rail() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "draft sentinel",
        &["committed response"],
    );
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let browser = browser_view(/*human_control*/ false, Some((1, 2)));

    let terminal = render_frame_app_with_browser(
        &mut app,
        /*width*/ 144,
        /*height*/ 12,
        Some(&browser),
    );

    assert_snapshot!(
        "owned_screen_browser_renders_in_wide_right_rail",
        terminal.backend()
    );
}

#[tokio::test]
async fn owned_screen_browser_preserves_narrow_overlay_chrome() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "draft sentinel",
        &["committed response"],
    );
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let browser = browser_view(/*human_control*/ false, Some((1, 2)));

    let terminal = render_frame_app_with_browser(
        &mut app,
        /*width*/ 75,
        /*height*/ 12,
        Some(&browser),
    );

    assert_snapshot!(
        "owned_screen_browser_preserves_narrow_overlay_chrome",
        terminal.backend()
    );
}

#[tokio::test]
async fn terminal_browser_completed_click_preserves_input_and_rejects_non_clicks() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let browser_view = browser_view(/*human_control*/ false, Some((1, 2)));
    let _terminal = render_frame_app_with_browser(
        &mut app,
        /*width*/ 144,
        /*height*/ 12,
        Some(&browser_view),
    );
    let body = app
        .owned_screen_frame
        .panel_body(OwnedScreenPanel::Summary)
        .expect("browser panel body");
    let viewport = browser_viewport(body);
    let press = primary_event(
        MousePrimaryEventKind::Press,
        viewport.x.saturating_add(/*rhs*/ 2),
        viewport.y.saturating_add(/*rhs*/ 1),
    );
    let release = primary_event(
        MousePrimaryEventKind::Release,
        viewport.x.saturating_add(/*rhs*/ 3),
        viewport.y.saturating_add(/*rhs*/ 2),
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;

    assert!(app.handle_owned_screen_mouse_primary(&mut tui, press));
    let inputs = app
        .terminal_browser_control_inputs(release, &browser_view)
        .expect("completed browser viewport click should request control");
    let expected_press = BrowserMouseInput {
        kind: BrowserMouseKind::Down,
        button: BrowserMouseButton::Left,
        column: 2,
        row: 1,
        viewport_cols: viewport.width,
        viewport_rows: viewport.height,
        modifiers: Default::default(),
    };

    assert_eq!(
        inputs,
        [
            expected_press,
            BrowserMouseInput {
                kind: BrowserMouseKind::Up,
                column: 3,
                row: 2,
                ..expected_press
            },
        ]
    );
    let idle_view = BrowserView {
        status: BrowserStatus::Idle,
        ..browser_view.clone()
    };
    assert!(
        app.terminal_browser_control_inputs(release, &idle_view)
            .is_none(),
        "an idle browser should ignore click-to-control"
    );
    assert!(app.handle_owned_screen_mouse_primary(&mut tui, release));
    assert!(!app.owned_screen_frame.is_interacting());

    assert!(app.handle_owned_screen_mouse_primary(&mut tui, press));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        MousePrimaryEvent {
            kind: MousePrimaryEventKind::Drag,
            ..release
        },
    ));
    assert!(
        app.terminal_browser_control_inputs(release, &browser_view)
            .is_none(),
        "dragging within the browser should not request control"
    );
    assert!(app.handle_owned_screen_mouse_primary(&mut tui, release));
    assert!(!app.owned_screen_frame.is_interacting());
    Ok(())
}

#[tokio::test]
async fn browser_cursor_wins_over_composer_during_human_control() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "draft sentinel",
        &["committed response"],
    );
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    app.owned_screen_frame.focus_conversation();
    let browser = browser_view(/*human_control*/ true, Some((1, 2)));

    let mut terminal = render_frame_app_with_browser(
        &mut app,
        /*width*/ 144,
        /*height*/ 12,
        Some(&browser),
    );
    let body = app
        .owned_screen_frame
        .panel_body(OwnedScreenPanel::Summary)
        .expect("browser panel body");
    let viewport = browser_viewport(body);

    assert_eq!(
        terminal.get_cursor_position().expect("browser cursor"),
        Position::new(
            viewport.x.saturating_add(/*rhs*/ 2),
            viewport.y.saturating_add(/*rhs*/ 1),
        )
    );
}

#[tokio::test]
async fn escape_hides_a_docked_browser_without_hiding_the_shared_rail() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    let browser = attach_visible_browser(&mut app);
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    app.owned_screen_frame.layout(
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 144, /*height*/ 12,
        ),
        /*has_side*/ false,
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;

    assert!(app.handle_owned_screen_navigation_key(
        &mut tui,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    ));

    assert!(!browser.view().visible);
    assert_eq!(
        app.owned_screen_frame.right_rail_content(),
        OwnedScreenRightRailContent::Summary
    );
    assert_eq!(
        app.owned_screen_frame.preference(OwnedScreenPanel::Summary),
        OwnedScreenPanelPreference::Shown
    );
    Ok(())
}

#[tokio::test]
async fn outside_click_hides_a_browser_overlay() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    let browser = attach_visible_browser(&mut app);
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    app.owned_screen_frame.layout(
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 75, /*height*/ 12,
        ),
        /*has_side*/ false,
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;

    assert!(
        app.handle_owned_screen_mouse_primary(&mut tui, primary_press(/*column*/ 0, /*row*/ 0),)
    );

    assert!(!browser.view().visible);
    assert_eq!(
        app.owned_screen_frame.right_rail_content(),
        OwnedScreenRightRailContent::Summary
    );
    assert_eq!(
        app.owned_screen_frame.preference(OwnedScreenPanel::Summary),
        OwnedScreenPanelPreference::Hidden
    );
    Ok(())
}

#[tokio::test]
async fn escape_closes_focused_frame_overlay_before_backtrack() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    app.owned_screen_frame
        .set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let _terminal = render_frame_app(&mut app, /*width*/ 75, /*height*/ 30);
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Summary
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    )
    .await;

    assert_eq!(
        app.owned_screen_frame.preference(OwnedScreenPanel::Summary),
        OwnedScreenPanelPreference::Hidden
    );
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Conversation
    );
    assert!(!app.backtrack.primed);
    Ok(())
}

#[tokio::test]
async fn remapped_panel_shortcuts_toggle_frame_preferences() -> Result<()> {
    let (mut app, mut app_event_rx) = app_with_owned_parent_and_events().await;
    app.keymap.app.toggle_sidebar = vec![crate::key_hint::plain(KeyCode::F(11))];
    app.keymap.app.toggle_summary = vec![crate::key_hint::plain(KeyCode::F(12))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    for (key, panel) in [
        (KeyCode::F(11), OwnedScreenPanel::Sidebar),
        (KeyCode::F(12), OwnedScreenPanel::Summary),
    ] {
        app.handle_key_event(
            &mut tui,
            &mut app_server,
            KeyEvent::new(key, KeyModifiers::NONE),
        )
        .await;
        let event = app_event_rx
            .try_recv()
            .expect("panel shortcut should emit an app event");
        assert!(matches!(
            &event,
            AppEvent::SetOwnedScreenPanel {
                panel: emitted_panel,
                preference: None,
            } if *emitted_panel == panel
        ));
        app.handle_event(&mut tui, &mut app_server, event).await?;
        assert_eq!(
            app.owned_screen_frame.preference(panel),
            OwnedScreenPanelPreference::Shown
        );
    }
    Ok(())
}

#[tokio::test]
async fn remapped_panel_shortcut_precedes_fixed_pane_focus() -> Result<()> {
    let (mut app, mut app_event_rx) = app_with_owned_side_and_events().await;
    app.keymap.app.toggle_summary = vec![crate::key_hint::alt(KeyCode::Char('2'))];
    let _terminal = render_frame_app(&mut app, /*width*/ 144, /*height*/ 12);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT),
    )
    .await;

    assert!(matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::SetOwnedScreenPanel {
            panel: OwnedScreenPanel::Summary,
            preference: None,
        })
    ));
    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    Ok(())
}

#[tokio::test]
async fn frame_overlay_traps_fixed_pane_focus_shortcut() -> Result<()> {
    let mut app = app_with_owned_side().await;
    app.owned_screen_frame
        .set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let _terminal = render_frame_app(&mut app, /*width*/ 75, /*height*/ 12);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT),
    )
    .await;

    assert_eq!(app.chat_widget.focused_slot(), PaneSlot::Parent);
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Summary
    );
    Ok(())
}

#[tokio::test]
async fn remapped_copy_page_up_beats_focused_frame_navigation() -> Result<()> {
    let (mut app, mut app_event_rx) = app_with_owned_parent_and_events().await;
    let mut keymap_config = app.config.tui_keymap.clone();
    keymap_config.global.copy = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("page-up".to_string()),
    ));
    let runtime_keymap =
        crate::keymap::RuntimeKeymap::from_config(&keymap_config).expect("valid copy remap");
    app.chat_widget
        .apply_keymap_update(keymap_config, &runtime_keymap);
    app.keymap = runtime_keymap;
    let _terminal = render_frame_app(&mut app, /*width*/ 144, /*height*/ 12);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
    )
    .await;
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Sidebar
    );

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
    )
    .await;

    let event = app_event_rx
        .try_recv()
        .expect("copy shortcut should emit an info cell");
    let AppEvent::InsertHistoryCell(cell) = event else {
        panic!("expected copy info cell");
    };
    assert!(
        cell.display_lines(/*width*/ 120)
            .iter()
            .any(|line| line.to_string().contains("No agent response to copy"))
    );
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Sidebar
    );
    Ok(())
}

#[tokio::test]
async fn active_transcript_selection_owns_drag_across_rail() {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "",
        &["selectable transcript text"],
    );
    let _terminal = render_frame_app(&mut app, /*width*/ 144, /*height*/ 12);
    let conversation = app
        .chat_widget
        .owned_screen
        .as_ref()
        .expect("owned screen")
        .last_conversation_area;
    let frame_layout = app.owned_screen_frame.layout(
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 144, /*height*/ 12,
        ),
        /*has_side*/ false,
    );
    let sidebar = frame_layout.sidebar.expect("sidebar").area;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create input test TUI");

    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_press(conversation.x, conversation.y),
    ));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut tui,
        primary_event(MousePrimaryEventKind::Drag, sidebar.x, sidebar.y),
    ));

    assert!(
        app.chat_widget
            .owned_screen
            .as_ref()
            .expect("owned screen")
            .selection_is_active()
    );
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Conversation
    );
}

#[tokio::test]
async fn typing_with_a_draft_returns_frame_focus_to_the_composer() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    seed_pane(&mut app, PaneSlot::Parent, "draft", &[]);
    let _terminal = render_frame_app(&mut app, /*width*/ 144, /*height*/ 12);
    let sidebar = app
        .owned_screen_frame
        .layout(
            Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 144, /*height*/ 12,
            ),
            /*has_side*/ false,
        )
        .sidebar
        .expect("sidebar")
        .area;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.backtrack.primed = true;
    assert!(app.handle_owned_screen_mouse_primary(&mut tui, primary_press(sidebar.x, sidebar.y),));
    assert!(!app.backtrack.primed);
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Sidebar
    );
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )
    .await;

    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Conversation
    );
    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
    )
    .await;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "draftx".to_string()
    );
    Ok(())
}

#[tokio::test]
async fn modal_scroll_bypasses_background_frame_panel() {
    let mut app = app_with_owned_parent().await;
    app.owned_screen_frame
        .set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let _terminal = render_frame_app(&mut app, /*width*/ 75, /*height*/ 12);
    let summary = app
        .owned_screen_frame
        .layout(
            Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 75, /*height*/ 30,
            ),
            /*has_side*/ false,
        )
        .summary
        .expect("summary overlay")
        .area;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create input test TUI");
    assert!(app.handle_owned_screen_mouse_primary(&mut tui, primary_press(summary.x, summary.y),));
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Summary
    );
    app.chat_widget.open_approvals_popup();
    let terminal = render_frame_app(&mut app, /*width*/ 75, /*height*/ 30);
    let rendered = terminal.backend().to_string();
    assert!(
        rendered.contains("Update Model Permissions"),
        "expected approval popup, got:\n{rendered}"
    );
    assert!(!rendered.contains("┌ Summary"));

    assert!(!app.handle_owned_screen_mouse_scroll(
        &mut tui,
        MouseScrollEvent {
            direction: MouseScrollDirection::Down,
            column: summary.x,
            row: summary.y,
            modifiers: KeyModifiers::NONE,
        },
    ));
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Conversation
    );
}

#[tokio::test]
async fn frame_overlay_traps_paste_before_hidden_composer() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    app.chat_widget
        .set_composer_text("draft".to_string(), Vec::new(), Vec::new());
    app.owned_screen_frame
        .set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let _terminal = render_frame_app(&mut app, /*width*/ 75, /*height*/ 12);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Paste("hidden paste".to_string()),
    )
    .await?;

    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
    assert_eq!(
        app.owned_screen_frame.focus(),
        OwnedScreenFrameFocus::Summary
    );
    Ok(())
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
        primary_event(MousePrimaryEventKind::Drag, u16::MAX, u16::MAX),
    ));
    assert!(!app.chat_widget.owned_screen_split_is_dragging());
    assert!(
        !app.chat_widget
            .by_slot(PaneSlot::Side)
            .and_then(|pane| pane.owned_screen.as_ref())
            .expect("side owned screen")
            .selection_is_active()
    );
    let selected = app
        .chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .and_then(|pane| pane.owned_screen.as_mut())
        .expect("parent owned screen")
        .finish_selection(Position::new(/*x*/ u16::MAX, /*y*/ u16::MAX));
    assert_eq!(selected, Some("parent selectable".to_string()));

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
async fn edge_selection_schedules_frames_and_survives_resize_events() -> Result<()> {
    let mut app = app_with_owned_parent().await;
    seed_pane(
        &mut app,
        PaneSlot::Parent,
        "",
        &[
            "zero", "one", "two", "three", "four", "five", "six", "seven",
        ],
    );
    let _terminal = render_app(&mut app, /*width*/ 40, /*height*/ 8);
    let area = app
        .chat_widget
        .owned_screen
        .as_ref()
        .expect("parent owned screen")
        .last_conversation_area;
    let mut input_tui = crate::tui::test_support::make_test_tui().expect("create input test TUI");
    assert!(app.handle_owned_screen_mouse_primary(
        &mut input_tui,
        primary_press(area.x, area.bottom().saturating_sub(/*rhs*/ 1),),
    ));
    assert!(app.handle_owned_screen_mouse_primary(
        &mut input_tui,
        primary_event(MousePrimaryEventKind::Drag, area.x, area.y),
    ));
    let mut tui = crate::tui::test_support::make_test_tui().expect("create render test TUI");
    let mut draw_rx = tui.subscribe_draws_for_test();

    app.render_owned_screen_frame(&mut tui)
        .expect("render owned screen");

    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), draw_rx.recv())
        .await
        .expect("timed out waiting for autoscroll frame")
        .expect("draw channel closed");
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Resize)
        .await?;
    assert!(
        app.chat_widget
            .owned_screen
            .as_ref()
            .expect("parent owned screen")
            .selection_is_active()
    );
    Ok(())
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
            modifiers: KeyModifiers::NONE,
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
            modifiers: KeyModifiers::NONE,
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
            modifiers: KeyModifiers::NONE,
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
            modifiers: KeyModifiers::NONE,
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
        modifiers: KeyModifiers::NONE,
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
        modifiers: KeyModifiers::NONE,
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
        modifiers: KeyModifiers::NONE,
    }));

    assert!(screen.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Down,
        column: 2,
        row: 2,
        modifiers: KeyModifiers::NONE,
    }));
    terminal
        .draw(|frame| {
            screen.render(&chat_widget, frame.area(), frame.buffer_mut());
        })
        .expect("render restored bottom");
    assert!(screen.viewport.is_following_bottom());
}
