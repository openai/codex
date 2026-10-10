use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn pet_load_without_runtime_sends_completion_event() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let app_event_tx = AppEventSender::new(tx);

    spawn_pet_load(
        async { Ok::<Option<crate::pets::AmbientPet>, String>(None) },
        app_event_tx,
        |result| AppEvent::ConfiguredPetLoaded {
            generation: next_configured_pet_load_generation(),
            pet_id: crate::pets::DEFAULT_PET_ID.to_string(),
            result,
        },
    );

    match rx.blocking_recv().expect("pet load completion event") {
        AppEvent::ConfiguredPetLoaded { pet_id, result, .. } => {
            assert_eq!(pet_id, crate::pets::DEFAULT_PET_ID);
            assert!(result.expect("successful pet load").is_none());
        }
        event => panic!("expected configured pet completion, got {event:?}"),
    }
}

#[tokio::test]
async fn configured_pet_loads_are_scoped_across_widget_replacements() {
    let (mut replaced_chat, _replaced_tx, mut replaced_rx, _replaced_op_rx) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let (mut chat, _tx, mut rx, _op_rx) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let codex_home = tempfile::tempdir().unwrap();
    replaced_chat.local_settings.codex_home =
        AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute temporary path");
    chat.local_settings.codex_home =
        AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute temporary path");
    let pet_id = "missing-custom-pet".to_string();

    replaced_chat.set_tui_pet(/*pet*/ Some(pet_id.clone()));
    let replaced_event = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 5),
        replaced_rx.recv(),
    )
    .await
    .expect("replaced widget pet load completion")
    .expect("replaced widget pet load event");
    let AppEvent::ConfiguredPetLoaded {
        generation: replaced_generation,
        ..
    } = replaced_event
    else {
        panic!("expected configured pet completion");
    };

    chat.set_tui_pet(/*pet*/ Some(pet_id.clone()));
    let event = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), rx.recv())
        .await
        .expect("configured pet load completion")
        .expect("configured pet load event");
    let AppEvent::ConfiguredPetLoaded {
        generation,
        pet_id: loaded_pet_id,
        ..
    } = event
    else {
        panic!("expected configured pet completion");
    };
    assert_eq!(loaded_pet_id, pet_id);

    assert!(!chat.finish_configured_pet_load(
        replaced_generation,
        pet_id.clone(),
        /*result*/ Ok(None),
    ));
    assert!(chat.finish_configured_pet_load(
        generation,
        pet_id,
        /*result*/
        Ok(Some(crate::pets::test_ambient_pet(
            chat.frame_requester.clone(),
            /*animations_enabled*/ false,
        ))),
    ));
    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 24,
    );
    assert!(chat.ambient_pet_draw(area, area.bottom()).is_some());
}

#[tokio::test]
async fn shared_pet_load_uses_cached_builtin_assets() {
    let (chat, _tx, _rx, _op_rx) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let codex_home = tempfile::tempdir().unwrap();
    crate::pets::write_test_pack(codex_home.path());

    crate::pets::load_pet_with_assets(
        crate::pets::DEFAULT_PET_ID.to_string(),
        AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute temporary path"),
        chat.frame_requester.clone(),
        /*animations_enabled*/ false,
        &chat.pet_http_client,
    )
    .await
    .expect("load cached built-in pet");

    assert!(
        codex_home
            .path()
            .join("cache")
            .join("tui-pets")
            .join("frame-cache")
            .join(crate::pets::DEFAULT_PET_ID)
            .is_dir()
    );
}

#[tokio::test]
async fn stale_pet_preview_completion_keeps_current_preview() {
    let (mut chat, _tx, _rx, _op_rx) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    chat.pet_picker_preview_request_id = 2;
    chat.pet_picker_preview_pet = Some(crate::pets::test_ambient_pet(
        chat.frame_requester.clone(),
        /*animations_enabled*/ false,
    ));

    chat.finish_pet_picker_preview_load(/*request_id*/ 1, Err("stale preview".to_string()));

    assert!(chat.pet_picker_preview_pet.is_some());
    assert_eq!(chat.pet_picker_preview_request_id, 2);
}

#[tokio::test]
async fn stale_pet_selection_completion_keeps_current_loading_popup() {
    let (mut chat, _tx, _rx, _op_rx) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let current_request_id = chat.show_pet_selection_loading_popup();
    let stale_request_id = current_request_id.wrapping_sub(/*rhs*/ 1);

    assert!(!chat.finish_pet_selection_loading_popup(stale_request_id));
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::PET_SELECTION_LOADING_VIEW_ID)
    );
    assert!(chat.finish_pet_selection_loading_popup(current_request_id));
    assert_eq!(chat.bottom_pane.active_view_id(), None);
}
