//! Exercise the rendered-cell adapter; gesture and clipboard state machines have shared tests.

use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseEventKind::Down;
use crossterm::event::MouseEventKind::Drag;
use crossterm::event::MouseEventKind::Up;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use ratatui::widgets::Widget;

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> TuiEvent {
    TuiEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn rendered_selection_preserves_unicode_links_and_the_selected_revision() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 2, /*width*/ 30, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    Line::from(vec![
        "  ".into(),
        "café 界 👩‍💻".cyan().underlined(),
        "  ·  20%…".dim(),
    ])
    .render(area, &mut buffer);
    crate::terminal_hyperlinks::mark_underlined_hyperlink(&mut buffer, area, "https://example.com");
    let original = buffer.clone();
    let mut selection = RenderedSelection::default();
    selection.areas.clear();
    selection.register(area, /*invalidate*/ false);
    selection.render(&mut buffer);
    for event in [
        mouse(Down(Left), /*column*/ 2, /*row*/ 2),
        mouse(Drag(Left), /*column*/ 12, /*row*/ 2),
        mouse(Up(Left), /*column*/ 12, /*row*/ 2),
    ] {
        selection.handle_event(&event, &buffer);
    }
    assert_eq!(
        selection.view.selected_text(&selection.cells).as_deref(),
        Some("café 界 👩‍💻")
    );
    buffer.reset();
    Line::from("  changed · 40%").render(area, &mut buffer);
    selection.areas.clear();
    selection.register(area, /*invalidate*/ false);
    selection.render(&mut buffer);
    for position in area.positions() {
        assert_eq!(buffer[position].symbol(), original[position].symbol());
    }
    insta::assert_snapshot!(format!("{buffer:?}"));
    // A new warning must replace the held revision even while the turn keeps running.
    selection.register(area, /*invalidate*/ true);
    assert!(!selection.view.has_selection_range());
}

#[test]
fn release_copy_and_cross_surface_drags_use_only_visible_text() {
    let status = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 7, /*height*/ 1,
    );
    let footer = Rect { y: 1, ..status };
    let mut buffer = Buffer::empty(status.union(footer));
    "branch…".render(status, &mut buffer);
    "? shortcuts".render(footer, &mut buffer);
    let mut selection = RenderedSelection::default();
    selection.view.copy_on_select = true;
    selection.register(status, /*invalidate*/ false);
    selection.register(footer, /*invalidate*/ false);
    // Registration retains geometry only: select the final painted cells, not earlier content.
    "commit…".render(status, &mut buffer);
    selection.handle_event(&mouse(Down(Left), /*column*/ 0, /*row*/ 0), &buffer);
    selection.handle_event(&mouse(Drag(Left), /*column*/ 19, /*row*/ 1), &buffer);
    let Some(RegionAction::Selection(ViewAction::CopyOnSelect(text))) =
        selection.handle_event(&mouse(Up(Left), /*column*/ 19, /*row*/ 1), &buffer)
    else {
        panic!("release must copy the selected status row");
    };
    assert_eq!(text, "commit…");
    assert!(selection.view.has_selection_range());
    // Collapsing the range keeps keyboard ownership, including Enter, with the footer.
    for _ in text.chars() {
        selection.handle_event(&TuiEvent::Key(KeyCode::Left.into()), &buffer);
    }
    assert!(!selection.view.has_selection_range());
    assert!(matches!(
        selection.handle_event(&TuiEvent::Key(KeyCode::Enter.into()), &buffer),
        Some(RegionAction::Selection(ViewAction::Changed))
    ));
    selection.handle_event(&TuiEvent::Key(KeyCode::Right.into()), &buffer);
    assert_eq!(
        selection.view.selected_text(&selection.cells).as_deref(),
        Some("c")
    );
    // Invalidating another visible region must not release this selection.
    selection.register(footer, /*invalidate*/ true);
    assert!(selection.view.has_active_interaction());
    // Find belongs to the app, not a hidden search inside the footer.
    assert!(
        selection
            .handle_event(
                &TuiEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)),
                &buffer
            )
            .is_none()
    );
    assert!(!selection.view.has_active_interaction());
    // Resize invalidates both the highlight and its pointer anchor.
    selection.handle_event(&mouse(Down(Left), /*column*/ 0, /*row*/ 1), &buffer);
    selection.handle_event(&mouse(Drag(Left), /*column*/ 3, /*row*/ 1), &buffer);
    selection.handle_event(
        &TuiEvent::Resize(ratatui::layout::Size::new(
            /*width*/ 10, /*height*/ 2,
        )),
        &buffer,
    );
    assert!(!selection.view.has_active_interaction());
}

#[test]
fn stationary_clicks_are_deferred_but_dragging_back_does_not_activate() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    "⚠ 2 warnings".render(area, &mut buffer);
    let mut selection = RenderedSelection::default();
    selection.register(area, /*invalidate*/ false);
    assert!(matches!(
        selection.handle_event(&mouse(Down(Left), /*column*/ 2, /*row*/ 0), &buffer),
        Some(RegionAction::Selection(_))
    ));
    assert!(matches!(
        selection.handle_event(&mouse(Up(Left), /*column*/ 2, /*row*/ 0), &buffer),
        Some(RegionAction::Click(_))
    ));
    selection.handle_event(&mouse(Down(Left), /*column*/ 5, /*row*/ 0), &buffer);
    selection.handle_event(&mouse(Drag(Left), /*column*/ 7, /*row*/ 0), &buffer);
    assert!(!matches!(
        selection.handle_event(&mouse(Up(Left), /*column*/ 5, /*row*/ 0), &buffer),
        Some(RegionAction::Click(_))
    ));
    // A stationary click after a selection releases the frozen revision.
    for event in [
        mouse(Down(Left), /*column*/ 0, /*row*/ 0),
        mouse(Drag(Left), /*column*/ 3, /*row*/ 0),
        mouse(Up(Left), /*column*/ 3, /*row*/ 0),
    ] {
        selection.handle_event(&event, &buffer);
    }
    assert!(selection.view.has_selection_range());
    selection.handle_event(&mouse(Down(Left), /*column*/ 10, /*row*/ 0), &buffer);
    assert!(matches!(
        selection.handle_event(&mouse(Up(Left), /*column*/ 10, /*row*/ 0), &buffer),
        Some(RegionAction::Click(_))
    ));
    assert!(!selection.view.has_active_interaction());
}
