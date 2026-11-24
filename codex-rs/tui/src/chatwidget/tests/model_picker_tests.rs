//! Model-family presentation at constrained sizes and with configured list bindings.

use super::app_server::configured_thread_session;
use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn model_picker_compact_hint_and_actions_follow_configured_bindings() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.5")).await;
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.list.accept = vec![key_hint::plain(KeyCode::F(3))];
    keymap.list.cancel = vec![key_hint::plain(KeyCode::F(2))];
    chat.bottom_pane.set_keymap_bindings(&keymap);
    chat.open_all_models_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 40);
    assert_eq!(popup.lines().last().unwrap().trim(), "f3 select · f2 back");
    chat.handle_key_event(KeyEvent::from(KeyCode::F(3)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenReasoningPopup { .. }));
    chat.handle_key_event(KeyEvent::from(KeyCode::F(2)));
    assert!(chat.no_modal_or_popup_active());
}

#[tokio::test]
async fn model_picker_opens_after_discovery_populates_an_empty_catalog() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.5")).await;
    chat.thread_id = Some(ThreadId::new());
    let presets = chat.model_catalog.models().to_vec();
    Arc::make_mut(&mut chat.model_catalog).set_models(Vec::new());
    chat.open_model_popup();
    let loading = render_bottom_popup(&chat, /*width*/ 80);
    let loading_title = loading.lines().find(|line| line.contains("Loading models"));
    insta::assert_snapshot!(loading_title.unwrap().trim(), @"Loading models…");
    let obsolete = chat.model_popup_request_id.unwrap();
    chat.handle_key_event(KeyCode::Esc.into());
    assert!(chat.no_modal_or_popup_active());
    chat.open_model_popup();
    assert!(!chat.on_models_loaded(obsolete, Ok(presets.clone())));
    let request_id = chat.model_popup_request_id.unwrap();
    assert!(chat.on_models_loaded(request_id, Ok(presets)));
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("model_selection_popup", popup);
}

#[tokio::test]
async fn model_picker_restores_provider_models_and_ignores_late_replies() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    let mut session = configured_thread_session(ThreadId::new());
    session.model_provider_id = "openai".into();
    let defaults = chat.model_catalog.models().to_vec();
    let mut alternate = defaults[0].clone();
    alternate.model = "alternate-only".into();
    let alternate = vec![alternate];

    chat.handle_thread_session(session.clone());
    chat.open_model_popup();
    let stale = chat.model_popup_request_id.unwrap();
    let mut other = session.clone();
    other.thread_id = ThreadId::new();
    other.model_provider_id = "alternate".into();
    chat.handle_thread_session(other.clone());
    assert!(chat.model_popup_request_id.is_some());
    chat.open_model_popup();
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Loading models"));
    assert_eq!(chat.model_catalog.provider(), Some("alternate"));
    let current = chat.model_popup_request_id.unwrap();
    assert!(chat.on_models_loaded(current, Ok(alternate.clone())));
    assert!(!chat.model_catalog.is_default_provider());
    assert_eq!(chat.model_catalog.default_models(), defaults);

    while events.try_recv().is_ok() {}
    chat.handle_thread_session(session.clone());
    assert!(chat.model_popup_request_id.is_none());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::SyncModelCatalog))
    );
    assert!(!chat.on_models_loaded(stale, Ok(alternate.clone())));
    assert_eq!(chat.model_catalog.models(), defaults);
    chat.handle_thread_session(other);
    assert_eq!(chat.model_catalog.models(), alternate);
    assert!(chat.model_popup_request_id.is_none());
    chat.open_model_popup();
    assert!(chat.model_popup_request_id.is_some());

    chat.clear_account_models();
    assert!(chat.model_catalog.models().is_empty());
    chat.handle_thread_session(session);
    assert!(chat.model_catalog.models().is_empty());
}

#[tokio::test]
async fn legacy_remote_picker_keeps_the_server_catalog_unscoped() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    chat.thread_params_mode = crate::ThreadParamsMode::Remote;
    let preset = get_available_model(&chat, "gpt-5.5");
    chat.model_catalog = Arc::new(ModelCatalog::new(
        vec![preset.clone()],
        /*provider*/ None,
    ));
    let mut session = configured_thread_session(ThreadId::new());
    session.model_provider_id = "alternate".into();
    chat.handle_thread_session(session);
    chat.open_model_popup();
    assert_eq!(chat.model_catalog.provider(), None);
    assert_eq!(chat.model_catalog.models(), &[preset]);
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("unscoped_remote_model_picker", popup);
}
