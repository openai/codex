use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

use super::*;

#[test]
fn conversation_sender_captures_bound_thread_for_codex_ops() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation();
    let cloned_sender = sender.clone();
    let thread_id = ThreadId::new();
    sender.bind_conversation_thread(thread_id);

    cloned_sender.send(AppEvent::CodexOp(AppCommand::compact()));

    let event = rx.try_recv().expect("conversation op should be sent");
    let AppEvent::ConversationOp { target, op } = event else {
        panic!("expected conversation-scoped op");
    };
    assert_eq!(target.thread_id, thread_id);
    assert!(matches!(op, AppCommand::Compact { .. }));
}

#[test]
fn independent_conversation_senders_do_not_share_targets() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx);
    let first_sender = sender.scoped_to_conversation();
    let second_sender = sender.scoped_to_conversation();
    let first_thread_id = ThreadId::new();
    let second_thread_id = ThreadId::new();
    first_sender.bind_conversation_thread(first_thread_id);
    second_sender.bind_conversation_thread(second_thread_id);

    first_sender.compact();
    second_sender.compact();

    let targets = [rx.try_recv(), rx.try_recv()].map(|event| match event {
        Ok(AppEvent::ConversationOp { target, .. }) => target.thread_id,
        _ => panic!("expected conversation-scoped op"),
    });
    assert_eq!(targets, [first_thread_id, second_thread_id]);
}

#[test]
fn global_sender_preserves_active_thread_codex_ops() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx);

    sender.compact();

    assert!(matches!(
        rx.try_recv(),
        Ok(AppEvent::CodexOp(AppCommand::Compact { .. }))
    ));
}

#[test]
fn unbound_conversation_sender_does_not_fall_back_to_active_thread() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation();

    sender.compact();

    assert!(rx.try_recv().is_err());
}
