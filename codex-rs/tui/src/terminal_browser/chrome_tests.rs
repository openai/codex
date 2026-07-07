use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn long_location_keeps_the_cursor_visible_at_both_ends() {
    let mut state = BrowserChromeState::default();
    state.sync_url(Some("https://example.com/a/very/long/location"));
    assert_eq!(state.location_view(/*width*/ 12).text, "https://exam");
    state.focus = BrowserChromeFocus::Location;

    let end = state.location_view(/*width*/ 12);
    assert_eq!(end.text, "ng/location");
    assert_eq!(end.cursor_col, 11);

    state.cursor = 0;
    let start = state.location_view(/*width*/ 12);
    assert_eq!(start.text, "https://exam");
    assert_eq!(start.cursor_col, 0);
}

#[test]
fn location_editor_replaces_selected_url_and_normalizes_hostnames() {
    let mut state = BrowserChromeState::default();
    state.sync_url(Some("https://old.example"));

    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
        BrowserChromeKeyResult::Consumed
    );
    assert!(state.handle_paste("openai.com"));
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        BrowserChromeKeyResult::Navigate(HumanNavigationAction::Goto(
            "https://openai.com".to_string()
        ))
    );
    assert!(!state.is_location_focused());
    let mut state = BrowserChromeState::default();
    state.sync_url(Some("https://example.com/café"));
    state.focus = BrowserChromeFocus::Location;
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        BrowserChromeKeyResult::Consumed
    );
    assert_eq!(state.draft, "https://example.com/caf");
}

#[test]
fn navigation_controls_have_distinct_click_targets() {
    let mut state = BrowserChromeState::default();
    let header = Rect::new(
        /*x*/ 20, /*y*/ 10, /*width*/ 40, /*height*/ 2,
    );
    let click = |column| MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };

    for (column, action) in [
        (22, HumanNavigationAction::Back),
        (26, HumanNavigationAction::Forward),
        (30, HumanNavigationAction::Reload),
    ] {
        assert_eq!(
            state.handle_mouse_primary(click(column), header),
            BrowserChromeMouseResult::Consumed(Some(action))
        );
    }
}
