use super::*;
use crate::app::test_support::make_test_app;
use crate::app_event::PaneSlot;

#[test]
fn terminal_browser_requests_require_the_displayed_thread() {
    let displayed = ThreadId::new();
    let inactive = ThreadId::new();

    assert!(terminal_browser_request_matches_thread(
        Some(displayed),
        &displayed.to_string(),
    ));
    assert!(!terminal_browser_request_matches_thread(
        Some(displayed),
        &inactive.to_string(),
    ));
    assert!(!terminal_browser_request_matches_thread(
        /*active_thread_id*/ None,
        &displayed.to_string(),
    ));
}

#[tokio::test]
async fn profile_approval_expires_when_the_browser_generation_changes() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    app.terminal_browser = Some(Arc::new(TerminalBrowser::discover()));
    app.terminal_browser_owner_thread_id = Some(thread_id);
    app.terminal_browser_generation = 3;
    let approval = app
        .terminal_browser_profile_approval(TerminalBrowserProfileCommand::Ephemeral)
        .expect("current browser should produce an approval token");

    assert!(app.terminal_browser_profile_approval_is_current(&approval));
    app.terminal_browser_generation += 1;
    assert!(!app.terminal_browser_profile_approval_is_current(&approval));
}
