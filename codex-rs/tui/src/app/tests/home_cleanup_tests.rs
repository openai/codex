//! Verifies that app fixtures own their Codex homes across widget replacement.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn app_home_survives_widget_replacement_until_app_drop() {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let home = app.config.codex_home.clone();
    std::fs::write(home.join("test.log"), "app fixture").expect("app fixture file");
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );

    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    assert_eq!(
        std::fs::read_to_string(home.join("test.log")).expect("app fixture remains"),
        "app fixture"
    );

    drop(app);
    assert!(!home.exists(), "app fixture should be removed");
}
