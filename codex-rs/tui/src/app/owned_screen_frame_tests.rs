use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;

use super::*;
use crate::app::owned_screen_resize::OwnedScreenLayout;

fn area(width: u16) -> Rect {
    Rect::new(/*x*/ 7, /*y*/ 3, width, /*height*/ 12)
}

#[test]
fn auto_layout_uses_width_derived_breakpoints_for_one_pane() {
    let mut state = OwnedScreenFrameState::default();

    let narrow = state.layout(area(/*width*/ 108), /*has_side*/ false);
    assert_eq!(narrow.center, area(/*width*/ 108));
    assert_eq!(narrow.sidebar, None);
    assert_eq!(narrow.summary, None);

    let medium = state.layout(area(/*width*/ 109), /*has_side*/ false);
    assert_eq!(medium.sidebar.unwrap().area.width, SIDEBAR_DEFAULT_WIDTH);
    assert_eq!(medium.summary, None);
    assert_eq!(medium.center.width, PREFERRED_CENTER_WIDTH);

    let wide = state.layout(area(/*width*/ 144), /*has_side*/ false);
    assert_eq!(wide.sidebar.unwrap().area.width, SIDEBAR_DEFAULT_WIDTH);
    assert_eq!(wide.summary.unwrap().area.width, SUMMARY_DEFAULT_WIDTH);
    assert_eq!(wide.center.width, PREFERRED_CENTER_WIDTH);
}

#[test]
fn auto_layout_preserves_split_pane_minimums() {
    let mut state = OwnedScreenFrameState::default();

    let narrow = state.layout(area(/*width*/ 111), /*has_side*/ true);
    assert_eq!(narrow.sidebar, None);
    assert_eq!(narrow.summary, None);

    let medium = state.layout(area(/*width*/ 112), /*has_side*/ true);
    assert!(medium.sidebar.is_some());
    assert_eq!(medium.summary, None);
    assert_eq!(
        medium.center.width,
        OwnedScreenLayout::minimum_width(/*has_side*/ true)
    );

    let wide = state.layout(area(/*width*/ 147), /*has_side*/ true);
    assert!(wide.sidebar.is_some());
    assert!(wide.summary.is_some());
    assert_eq!(
        wide.center.width,
        OwnedScreenLayout::minimum_width(/*has_side*/ true)
    );
}

#[test]
fn shown_panel_docks_to_hard_minimum_then_becomes_overlay() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);

    let docked = state.layout(area(/*width*/ 76), /*has_side*/ false);
    assert_eq!(
        docked.summary.unwrap().presentation,
        OwnedScreenPanelPresentation::Docked
    );
    assert_eq!(
        docked.center.width,
        OwnedScreenLayout::minimum_width(/*has_side*/ false)
    );

    let overlay = state.layout(area(/*width*/ 75), /*has_side*/ false);
    assert_eq!(overlay.center, area(/*width*/ 75));
    assert_eq!(
        overlay.summary.unwrap().presentation,
        OwnedScreenPanelPresentation::Overlay
    );
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);
}

#[test]
fn selecting_browser_reuses_and_focuses_the_summary_rail() {
    let mut state = OwnedScreenFrameState::default();
    let initial = state.layout(area(/*width*/ 144), /*has_side*/ false);
    let initial_summary = initial.summary.expect("summary rail").area;
    assert_eq!(
        state.right_rail_content(),
        OwnedScreenRightRailContent::Summary
    );

    state.select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let selected = state.layout(area(/*width*/ 144), /*has_side*/ false);

    assert_eq!(
        state.right_rail_content(),
        OwnedScreenRightRailContent::Browser
    );
    assert_eq!(
        selected.summary.expect("browser rail").area,
        initial_summary
    );
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);
    assert_eq!(
        state.preference(OwnedScreenPanel::Summary),
        OwnedScreenPanelPreference::Shown
    );
}

#[test]
fn panel_body_excludes_docked_header_and_overlay_border() {
    let mut docked_state = OwnedScreenFrameState::default();
    let docked = docked_state.layout(area(/*width*/ 144), /*has_side*/ false);
    let docked_area = docked.summary.expect("docked summary").area;
    assert_eq!(
        docked_state.panel_body(OwnedScreenPanel::Summary),
        Some(Rect::new(
            docked_area.x,
            docked_area.y.saturating_add(/*rhs*/ 1),
            docked_area.width,
            docked_area.height.saturating_sub(/*rhs*/ 1),
        ))
    );

    let mut overlay_state = OwnedScreenFrameState::default();
    overlay_state.select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let overlay = overlay_state.layout(area(/*width*/ 75), /*has_side*/ false);
    let overlay_area = overlay.summary.expect("browser overlay").area;
    assert_eq!(
        overlay_state.panel_body(OwnedScreenPanel::Summary),
        Some(Rect::new(
            overlay_area.x.saturating_add(/*rhs*/ 1),
            overlay_area.y.saturating_add(/*rhs*/ 1),
            overlay_area.width.saturating_sub(/*rhs*/ 2),
            overlay_area.height.saturating_sub(/*rhs*/ 2),
        ))
    );
}

#[test]
fn right_rail_chrome_highlights_the_selected_tab_when_focused() {
    let mut state = OwnedScreenFrameState::default();
    state.select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let layout = state.layout(area(/*width*/ 144), /*has_side*/ false);
    let rail = layout.summary.expect("browser rail").area;
    let mut buffer = Buffer::empty(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 180, /*height*/ 30,
    ));

    assert_eq!(
        state.render_panel_chrome(OwnedScreenPanel::Summary, "Summary", &mut buffer),
        state.panel_body(OwnedScreenPanel::Summary)
    );
    let title = (rail.x..rail.x.saturating_add(/*rhs*/ 19))
        .map(|x| buffer[(x, rail.y)].symbol())
        .collect::<String>();
    assert_eq!(title, " Summary | Browser ");
    assert_eq!(
        buffer[(rail.x.saturating_add(/*rhs*/ 1), rail.y)].style(),
        Style::default()
            .fg(Color::Reset)
            .bg(Color::Reset)
            .underline_color(Color::Reset)
            .dim()
    );
    assert_eq!(
        buffer[(rail.x.saturating_add(/*rhs*/ 11), rail.y)].style(),
        Style::default()
            .fg(Color::Cyan)
            .bg(Color::Reset)
            .underline_color(Color::Reset)
            .bold()
    );
}

#[test]
fn last_explicit_panel_gets_docking_priority() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Sidebar, OwnedScreenPanelPreference::Shown);
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);

    let layout = state.layout(area(/*width*/ 84), /*has_side*/ false);
    assert_eq!(
        layout.summary.unwrap().presentation,
        OwnedScreenPanelPresentation::Docked
    );
    assert_eq!(
        layout.sidebar.unwrap().presentation,
        OwnedScreenPanelPresentation::Overlay
    );
}

#[test]
fn divider_drag_resizes_only_the_target_panel() {
    let mut state = OwnedScreenFrameState::default();
    let initial = state.layout(area(/*width*/ 144), /*has_side*/ false);
    let divider = initial.sidebar_divider.expect("sidebar divider");
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: divider.x,
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Drag,
        column: area(/*width*/ 144).x + 35,
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Release,
        column: area(/*width*/ 144).x + 35,
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));

    let resized = state.layout(area(/*width*/ 151), /*has_side*/ false);
    assert_eq!(resized.sidebar.unwrap().area.width, 35);
    assert_eq!(resized.summary.unwrap().area.width, SUMMARY_DEFAULT_WIDTH);
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Shown
    );
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);
}

#[test]
fn resizing_auto_panel_keeps_it_visible_at_the_auto_breakpoint() {
    let mut state = OwnedScreenFrameState::default();
    let initial = state.layout(area(/*width*/ 109), /*has_side*/ false);
    let divider = initial.sidebar_divider.expect("sidebar divider");
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: divider.x,
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Drag,
        column: divider.x.saturating_add(/*rhs*/ 1),
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));

    let resized = state.layout(area(/*width*/ 109), /*has_side*/ false);
    assert_eq!(resized.sidebar.expect("sidebar").area.width, 29);
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Shown
    );
}

#[test]
fn clicking_divider_without_dragging_preserves_auto_preference() {
    let mut state = OwnedScreenFrameState::default();
    let initial = state.layout(area(/*width*/ 109), /*has_side*/ false);
    let divider = initial.sidebar_divider.expect("sidebar divider");
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: divider.x,
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Release,
        column: divider.x,
        row: divider.y,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Auto
    );
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);
}

#[test]
fn panel_scroll_is_independent_and_clamped() {
    let mut state = OwnedScreenFrameState::default();
    let layout = state.layout(area(/*width*/ 144), /*has_side*/ false);
    let mut buffer = Buffer::empty(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 180, /*height*/ 30,
    ));
    let lines = (0..30)
        .map(|index| Line::from(format!("row {index}")))
        .collect::<Vec<_>>();
    state.render_panel(OwnedScreenPanel::Sidebar, "Tasks", &lines, &mut buffer);
    state.render_panel(OwnedScreenPanel::Summary, "Summary", &lines, &mut buffer);
    let sidebar = layout.sidebar.expect("sidebar").area;
    assert!(state.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Down,
        column: sidebar.x,
        row: sidebar.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(state.sidebar.scroll, PANEL_SCROLL_ROWS);
    assert_eq!(state.summary.scroll, 0);
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);

    state.handle_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    for _ in 0..20 {
        state.handle_navigation_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    }
    assert_eq!(state.sidebar.scroll, state.sidebar.max_scroll());
}

#[test]
fn overlay_consumes_wheel_outside_its_boundary() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let layout = state.layout(area(/*width*/ 75), /*has_side*/ false);
    let overlay = layout.summary.expect("summary overlay").area;
    let mut buffer = Buffer::empty(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 100, /*height*/ 30,
    ));
    let lines = (0..30)
        .map(|index| Line::from(format!("row {index}")))
        .collect::<Vec<_>>();
    state.render_panel(OwnedScreenPanel::Summary, "Summary", &lines, &mut buffer);

    assert!(state.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Down,
        column: layout.center.x,
        row: layout.center.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(state.summary.scroll, 0);

    assert!(state.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Down,
        column: overlay.x,
        row: overlay.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(state.summary.scroll, PANEL_SCROLL_ROWS);
}

#[test]
fn hiding_overlay_invalidates_its_input_boundary_immediately() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let layout = state.layout(area(/*width*/ 75), /*has_side*/ false);
    assert!(state.traps_background_input());

    state.set_preference(
        OwnedScreenPanel::Summary,
        OwnedScreenPanelPreference::Hidden,
    );

    assert!(!state.traps_background_input());
    assert!(!state.handle_mouse_scroll(MouseScrollEvent {
        direction: MouseScrollDirection::Down,
        column: layout.center.x,
        row: layout.center.y,
        modifiers: KeyModifiers::NONE,
    }));
}

#[test]
fn outside_press_dismisses_overlay_without_leaking_to_conversation() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Sidebar, OwnedScreenPanelPreference::Shown);
    let layout = state.layout(area(/*width*/ 60), /*has_side*/ false);
    let overlay = layout.sidebar.expect("overlay").area;
    let outside = Position::new(overlay.right(), overlay.y);

    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: outside.x,
        row: outside.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Hidden
    );
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);
}

#[test]
fn conversation_press_does_not_hide_a_docked_explicit_panel() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Sidebar, OwnedScreenPanelPreference::Shown);
    let layout = state.layout(area(/*width*/ 109), /*has_side*/ false);
    assert_eq!(
        layout.sidebar.expect("sidebar").presentation,
        OwnedScreenPanelPresentation::Docked
    );

    assert!(!state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: layout.center.x,
        row: layout.center.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Shown
    );
}

#[test]
fn fallback_overlay_is_dismissed_when_last_explicit_panel_docks() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Sidebar, OwnedScreenPanelPreference::Shown);
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    let layout = state.layout(area(/*width*/ 84), /*has_side*/ false);
    let sidebar = layout.sidebar.expect("sidebar overlay");
    assert_eq!(sidebar.presentation, OwnedScreenPanelPresentation::Overlay);
    assert_eq!(
        layout.summary.expect("summary rail").presentation,
        OwnedScreenPanelPresentation::Docked
    );

    assert!(state.handle_mouse_primary(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: sidebar.area.right(),
        row: sidebar.area.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Hidden
    );
    assert_eq!(
        state.preference(OwnedScreenPanel::Summary),
        OwnedScreenPanelPreference::Shown
    );
}

#[test]
fn escape_dismisses_overlay_even_when_another_panel_has_focus() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Sidebar, OwnedScreenPanelPreference::Shown);
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    state.layout(area(/*width*/ 84), /*has_side*/ false);
    state.focus = OwnedScreenFrameFocus::Summary;
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);

    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,)));
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Hidden
    );
    assert_eq!(
        state.preference(OwnedScreenPanel::Summary),
        OwnedScreenPanelPreference::Shown
    );
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);
}

#[test]
fn panel_pointer_captures_drag_until_release() {
    let mut state = OwnedScreenFrameState::default();
    let layout = state.layout(area(/*width*/ 144), /*has_side*/ false);
    let sidebar = layout.sidebar.expect("sidebar").area;
    let outside = area(/*width*/ 144).right().saturating_add(4);

    for kind in [
        MousePrimaryEventKind::Press,
        MousePrimaryEventKind::Drag,
        MousePrimaryEventKind::Release,
    ] {
        assert!(state.handle_mouse_primary(MousePrimaryEvent {
            kind,
            column: if kind == MousePrimaryEventKind::Press {
                sidebar.x
            } else {
                outside
            },
            row: sidebar.y,
            modifiers: KeyModifiers::NONE,
        }));
    }
    assert!(!state.is_interacting());
}

#[test]
fn tab_cycles_only_visible_frame_regions() {
    let mut state = OwnedScreenFrameState::default();
    state.layout(area(/*width*/ 144), /*has_side*/ false);

    assert!(!state.handle_navigation_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);
    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Sidebar);
    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);
    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Sidebar);
}

#[test]
fn tab_skips_auto_hidden_regions_at_medium_width() {
    let mut state = OwnedScreenFrameState::default();
    let layout = state.layout(area(/*width*/ 109), /*has_side*/ false);
    assert!(layout.sidebar.is_some());
    assert!(layout.summary.is_none());

    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Sidebar);
    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Conversation);
}

#[test]
fn repeated_toggle_uses_preference_before_stale_layout() {
    let mut state = OwnedScreenFrameState::default();
    state.layout(area(/*width*/ 144), /*has_side*/ false);

    state.toggle(OwnedScreenPanel::Sidebar);
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Hidden
    );
    state.toggle(OwnedScreenPanel::Sidebar);
    assert_eq!(
        state.preference(OwnedScreenPanel::Sidebar),
        OwnedScreenPanelPreference::Shown
    );
}

#[test]
fn overlay_keeps_modal_focus_across_tab_character_and_render() {
    let mut state = OwnedScreenFrameState::default();
    state.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
    state.layout(area(/*width*/ 75), /*has_side*/ false);
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);

    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE,)));
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);
    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE,)));
    assert!(state.handle_navigation_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL,)));
    state.layout(area(/*width*/ 75), /*has_side*/ false);
    assert_eq!(state.focus(), OwnedScreenFrameFocus::Summary);
}
