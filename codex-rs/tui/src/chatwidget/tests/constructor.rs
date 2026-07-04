use super::*;

async fn test_init() -> (
    ChatWidgetInit,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) {
    let config = test_config().await;
    let model = get_model_offline_for_tests(config.model.as_deref());
    let session_telemetry = test_session_telemetry(&config, model.as_str());
    let model_catalog = test_model_catalog(&config);
    let (tx, rx) = unbounded_channel();

    (
        ChatWidgetInit {
            config,
            frame_requester: FrameRequester::test_dummy(),
            app_event_tx: AppEventSender::new(tx),
            workspace_command_runner: None,
            initial_user_message: None,
            enhanced_keys_supported: false,
            has_chatgpt_account: false,
            has_codex_backend_auth: false,
            model_catalog,
            feedback: codex_feedback::CodexFeedback::new(),
            is_first_run: false,
            status_account_display: None,
            runtime_model_provider_base_url: None,
            initial_plan_type: None,
            model: Some(model),
            startup_tooltip_override: None,
            status_line_invalid_items_warned: Arc::new(AtomicBool::new(/*v*/ false)),
            terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(/*v*/ false)),
            session_telemetry,
        },
        rx,
    )
}

pub(crate) async fn make_chatwidget_for_pane(
    pane: PaneSlot,
) -> (ChatWidget, tokio::sync::mpsc::UnboundedReceiver<AppEvent>) {
    let (init, rx) = test_init().await;
    (ChatWidget::new_with_app_event_for_pane(init, pane), rx)
}

pub(crate) async fn make_chatwidget_for_pane_with_sender(
    pane: PaneSlot,
    app_event_tx: AppEventSender,
) -> ChatWidget {
    let (mut init, _rx) = test_init().await;
    init.app_event_tx = app_event_tx;
    ChatWidget::new_with_app_event_for_pane(init, pane)
}

#[tokio::test]
async fn app_event_constructor_defaults_to_parent_pane() {
    let (init, _rx) = test_init().await;
    let widget = ChatWidget::new_with_app_event(init);

    assert_eq!(
        widget.conversation_origin().map(|origin| origin.pane),
        Some(PaneSlot::Parent)
    );
}

#[tokio::test]
async fn app_event_constructor_uses_requested_pane_and_fresh_generation() {
    let (first, _first_rx) = make_chatwidget_for_pane(PaneSlot::Side).await;
    let (second, _second_rx) = make_chatwidget_for_pane(PaneSlot::Side).await;
    let first_origin = first.conversation_origin().expect("scoped first widget");
    let second_origin = second.conversation_origin().expect("scoped second widget");

    assert_eq!(first_origin.pane, PaneSlot::Side);
    assert_eq!(second_origin.pane, PaneSlot::Side);
    assert_ne!(first_origin.generation, second_origin.generation);
}
