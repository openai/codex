use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn background_event_updates_status_header() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "bg-1".into(),
        msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
            message: "Waiting for `vim`".to_string(),
        }),
    });

    assert!(chat.bottom_pane.status_indicator_visible());
    assert_eq!(chat.current_status.header, "Waiting for `vim`");
    assert!(drain_insert_history(&mut rx).is_empty());
}
