use super::*;
use crate::app::conversation_panes::ConversationPaneInit;
use crate::app::test_support::make_test_app;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

async fn install_test_side(app: &mut App) {
    let (side_widget, _side_rx) = make_chatwidget_for_pane(PaneSlot::Side).await;
    let file_search = FileSearchManager::new(
        side_widget.config_ref().cwd.to_path_buf(),
        side_widget.conversation_event_sender(),
    );
    let installed = app.chat_widget.install_side(ConversationPaneInit {
        chat_widget: side_widget,
        file_search,
        owned_screen: None,
    });
    assert!(installed.is_ok(), "side pane should install");
}

#[tokio::test]
async fn selected_pet_updates_both_installed_widget_configs() -> Result<()> {
    let mut app = make_test_app().await;
    install_test_side(&mut app).await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let request_id = app.chat_widget.show_pet_selection_loading_popup();
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.handle_pet_selection_loaded(
        &mut tui,
        request_id,
        "chefito".to_string(),
        /*result*/ Ok(None),
    )
    .await?;

    assert_eq!(app.config.tui_pet.as_deref(), Some("chefito"));
    for slot in [PaneSlot::Parent, PaneSlot::Side] {
        assert_eq!(
            app.chat_widget
                .by_slot(slot)
                .expect("installed pane")
                .config_ref()
                .tui_pet
                .as_deref(),
            Some("chefito")
        );
    }
    Ok(())
}
