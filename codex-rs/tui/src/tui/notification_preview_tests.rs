//! Focus policy and backend state survive a notification preview.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn notification_preview_preserves_disabled_backend_and_focus_policy() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    tui.notification_backend = None;
    tui.set_notification_condition(NotificationCondition::Unfocused);

    assert!(!tui.notify_test("preview"));
    assert_eq!(tui.notification_condition, NotificationCondition::Unfocused);
    assert!(tui.notification_backend.is_none());
}
