use std::time::Duration;

use pretty_assertions::assert_eq;
use ratatui::layout::Size;

use crate::tui::TuiEvent;

#[tokio::test]
async fn draw_size_policy_refreshes_only_after_resume() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let cached = Size::new(/*width*/ 120, /*height*/ 40);
    let resumed = tui.terminal.size().expect("backend size");
    tui.terminal.last_known_screen_size = cached;
    let resized = Size::new(/*width*/ 100, /*height*/ 30);
    for (event, expected) in [
        (TuiEvent::Draw, cached),
        (TuiEvent::Resume, resumed),
        (TuiEvent::Resize(resized), resized),
        (TuiEvent::Paste(String::new()), cached),
        (TuiEvent::Draw, resized),
        (TuiEvent::Paste(String::new()), cached),
    ] {
        assert_eq!(tui.screen_size_for_event(&event).expect("size"), expected);
        if matches!(event, TuiEvent::Resize(_)) {
            tui.defer_screen_size(resized);
        }
    }
    assert_eq!(tui.take_event_screen_size().expect("size"), resumed);
    assert!(tui.screen_size.pending_recheck_at.is_none());
}

#[tokio::test]
async fn standalone_resize_draw_rechecks_settled_screen_size_once() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let resize_size = Size::new(/*width*/ 120, /*height*/ 40);

    tui.screen_size_for_event(&TuiEvent::Resize(resize_size))
        .expect("resolve resize");
    tui.terminal.resize(resize_size).expect("apply resize");
    assert_eq!(
        tui.screen_size_for_event(&TuiEvent::Draw)
            .expect("resolve early draw"),
        resize_size
    );

    tui.schedule_screen_size_recheck(Duration::ZERO);
    assert_eq!(
        tui.screen_size_for_event(&TuiEvent::Draw)
            .expect("resolve settled draw"),
        tui.terminal.size().expect("terminal size")
    );
    assert!(tui.screen_size.pending_recheck_at.is_none());
}

#[tokio::test]
async fn resize_reflow_draw_consumes_reserved_screen_size() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let backend_size = tui.terminal.size().expect("backend size");
    let resize_size = Size::new(backend_size.width, backend_size.height + 13);
    let size = tui
        .screen_size_for_event(&TuiEvent::Resize(resize_size))
        .expect("resolve resize");

    tui.draw_with_resize_reflow(/*height*/ 3, size, /*hud*/ None, |_| {})
        .expect("draw inline frame");

    assert_eq!(
        tui.prepare_draw_size().expect("next draw size"),
        backend_size
    );
}

#[tokio::test]
async fn early_paint_preserves_settled_size_recheck() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let backend_size = tui.terminal.size().expect("backend size");
    tui.schedule_screen_size_recheck(Duration::from_secs(1));
    let deadline = tui.screen_size.pending_recheck_at;
    let size = tui.prepare_draw_size().expect("early paint size");
    tui.draw_with_resize_reflow(/*height*/ 3, size, /*hud*/ None, |_| {})
        .expect("early inline paint");
    assert_eq!(tui.screen_size.pending_recheck_at, deadline);
    tui.screen_size.pending_recheck_at = Some(std::time::Instant::now());
    tui.terminal
        .resize(Size::new(backend_size.width + 16, backend_size.height))
        .expect("intermediate cached size");

    assert_eq!(
        tui.screen_size_for_event(&TuiEvent::Draw)
            .expect("settled size"),
        backend_size
    );
}

#[tokio::test]
async fn entering_alternate_screen_updates_cached_screen_size() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let screen_size = tui.terminal.size().expect("terminal size");
    tui.terminal.last_known_screen_size = Size::new(/*width*/ 120, /*height*/ 40);

    tui.enter_alt_screen().expect("enter alternate screen");

    assert_eq!(tui.terminal.last_known_screen_size, screen_size);
    tui.leave_alt_screen().expect("leave alternate screen");
}
