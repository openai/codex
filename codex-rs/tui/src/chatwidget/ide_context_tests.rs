use crate::chatwidget::UserMessage;
use crate::chatwidget::tests::helpers::make_chatwidget_manual_with_sender;
use codex_features::Feature;
use codex_protocol::ThreadId;

#[tokio::test]
async fn ide_context_keeps_follow_ups_in_the_local_queue() {
    let (mut chat, _app_event_tx, _rx, mut op_rx) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::UserMessageQueue, /*enabled*/ true);
    chat.ide_context.enable();
    chat.input_queue.user_turn_pending_start = true;

    chat.queue_user_message(UserMessage::from("use fresh IDE context"));

    assert!(op_rx.try_recv().is_err());
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["use fresh IDE context"]
    );
}
