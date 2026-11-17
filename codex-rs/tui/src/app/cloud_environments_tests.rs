#![cfg(test)]

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::chatwidget::tests::render_bottom_popup;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn cloud_environment_picker_shows_default_and_saved_choice() {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    let saved = SavedEnvironment {
        id: "ws~asenvcfg_saved".into(),
        name: "Team Rust".into(),
        workspace_shared: true,
    };
    app.chat_widget
        .show_selection_view(cloud_environment_picker(Ok(vec![saved.clone()]), source));

    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 110);
    let rows = ["Cloud environment", "Default", "Team Rust"].map(|label| {
        let line = rendered.lines().find(|line| line.contains(label)).unwrap();
        line[line.find(label).unwrap()..]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    });
    insta::assert_snapshot!(rows.join("\n"), @"
    Cloud environment
    Default Use the default environment with the current GitHub project, if available
    Team Rust Workspace environment
    ");

    for key in "rust".chars().map(KeyCode::Char) {
        app.chat_widget.handle_key_event(key.into());
    }
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    let selection = std::iter::from_fn(|| events.try_recv().ok()).find_map(|event| match event {
        AppEvent::StartCloudConversation {
            source,
            saved_environment,
        } => Some((source, saved_environment)),
        _ => None,
    });
    assert_eq!(selection, Some((source, Some(saved))));
}

#[tokio::test]
async fn dismissing_cloud_environment_loading_keeps_the_draft() {
    let (mut app, _, _) = make_test_app_with_channels().await;
    app.chat_widget.apply_external_edit("draft".into());
    app.chat_widget.show_selection_view(loading_view());
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    let view = cloud_environment_picker(Ok(Vec::new()), ThreadId::new());
    assert!(
        !app.chat_widget
            .replace_selection_view_if_present(LOADING_VIEW_ID, view)
    );
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
}
