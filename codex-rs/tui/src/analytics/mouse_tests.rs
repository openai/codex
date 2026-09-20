//! Mouse routing, painted control bounds, and scrolling across retained report surfaces.

use super::*;
use crate::analytics::controls::Control;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;

fn mouse_at(view: &mut AnalyticsView, kind: MouseEventKind, area: Rect) {
    view.handle_mouse(MouseEvent {
        kind,
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
}

#[tokio::test]
async fn mouse_routing_keeps_clipped_tabs_and_refresh_targets_account_scoped() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    screen(&mut view, /*width*/ 100, /*height*/ 24);
    let (_, tab) = view
        .tab_hits
        .iter()
        .find(|(section, _)| *section == Section::Plugins)
        .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui().unwrap();
    view.handle_event(
        &mut tui,
        TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: tab.x,
            row: tab.y,
            modifiers: KeyModifiers::NONE,
        }),
    )
    .unwrap();
    assert_eq!(view.section, Section::Plugins);
    screen(&mut view, /*width*/ 20, /*height*/ 12);
    let (_, tab) = view.tab_hits[0];
    mouse_at(&mut view, MouseEventKind::Down(MouseButton::Left), tab);
    assert_eq!(view.section, Section::Plugins);

    screen(&mut view, /*width*/ 100, /*height*/ 24);
    let targets = view.control_hits.clone();
    assert!(!targets.is_empty());
    let before = (view.section, view.ranges);
    press(&mut view, KeyCode::Char('R'));
    for (_, rect) in targets {
        mouse_at(&mut view, MouseEventKind::Down(MouseButton::Left), rect);
    }
    assert_eq!((view.section, view.ranges), before);
    assert!(matches!(view.account, Load::Unavailable));
}

#[test]
fn clipped_controls_ignore_remapped_keys_and_obsolete_frames() {
    let mut config = TuiKeymap::default();
    config.list.cancel = Some(KeybindingsSpec::One(KeybindingSpec("r".into())));
    config.list.accept = Some(KeybindingsSpec::Many(Vec::new()));
    config.list.move_left = Some(KeybindingsSpec::Many(Vec::new()));
    config.list.move_right = Some(KeybindingsSpec::Many(Vec::new()));
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.keymap = RuntimeKeymap::from_config(&config).unwrap().list;
    screen(&mut view, /*width*/ 100, /*height*/ 24);
    let (_, range) = *view
        .control_hits
        .iter()
        .find(|(control, _)| *control == Control::Range)
        .unwrap();
    mouse_at(&mut view, MouseEventKind::Down(MouseButton::Left), range);
    assert_eq!(
        (
            view.ranges[view.range_group(Section::Usage) as usize],
            view.is_done
        ),
        (1, false)
    );

    press(&mut view, KeyCode::Char('z'));
    screen(&mut view, /*width*/ 100, /*height*/ 24);
    let (_, dashboard) = *view
        .control_hits
        .iter()
        .find(|(control, _)| *control == Control::Dashboard)
        .unwrap();
    mouse_at(
        &mut view,
        MouseEventKind::Down(MouseButton::Left),
        dashboard,
    );
    assert!(view.zoomed);

    view.plan.enabled = true;
    view.select_section(Section::Plan);
    view.plan.cursor = [2, 4];
    let retained = view.plan.cursor;
    for width in [58, 14] {
        view.plan.window = 0;
        screen(&mut view, width, /*height*/ 20);
        let (_, hit) = *view
            .control_hits
            .iter()
            .find(|(control, _)| *control == Control::PlanWindow)
            .unwrap();
        mouse_at(
            &mut view,
            MouseEventKind::Down(MouseButton::Left),
            Rect {
                x: hit.right(),
                ..hit
            },
        );
        assert_eq!(view.plan.window, 0);
        let last_painted = Rect {
            x: hit.right() - 1,
            ..hit
        };
        mouse_at(
            &mut view,
            MouseEventKind::Down(MouseButton::Left),
            last_painted,
        );
        assert_eq!(
            (view.plan.window, view.plan.cursor, view.is_done),
            (1, retained, false)
        );
        mouse_at(
            &mut view,
            MouseEventKind::Down(MouseButton::Left),
            last_painted,
        );
        assert_eq!(view.plan.window, 1);
    }
}

#[test]
fn wheel_scrolls_only_the_painted_body_within_its_bounds() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.follow_selection = false;
    screen(&mut view, /*width*/ 58, /*height*/ 12);
    assert!(view.max_scroll > 3);
    let body = view.body_area;
    let tab = view.tab_hits[0].1;
    mouse_at(&mut view, MouseEventKind::ScrollDown, tab);
    assert_eq!(view.scroll_offset(), 0);
    mouse_at(&mut view, MouseEventKind::ScrollDown, body);
    assert_eq!(view.scroll_offset(), 3);
    *view.scroll_offset_mut() = view.max_scroll;
    mouse_at(&mut view, MouseEventKind::ScrollDown, body);
    assert_eq!(view.scroll_offset(), view.max_scroll);
    mouse_at(&mut view, MouseEventKind::ScrollUp, body);
    assert_eq!(view.scroll_offset(), view.max_scroll - 3);
    assert!(!view.follow_selection);
    let reading = view.scroll_offset();

    press(&mut view, KeyCode::Char('?'));
    mouse_at(&mut view, MouseEventKind::ScrollDown, body);
    assert_eq!(view.scroll_offset(), 0);
    screen(&mut view, /*width*/ 58, /*height*/ 12);
    let help_body = view.body_area;
    mouse_at(&mut view, MouseEventKind::ScrollDown, help_body);
    assert_eq!(view.scroll_offset(), 3);
    press(&mut view, KeyCode::Esc);
    assert_eq!(view.scroll_offset(), reading);
}
