use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

use super::*;

#[test]
fn conversation_sender_captures_bound_thread_for_codex_ops() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation(PaneSlot::Parent);
    let cloned_sender = sender.clone();
    let thread_id = ThreadId::new();
    sender.bind_conversation_thread(thread_id);

    cloned_sender.send(AppEvent::CodexOp(AppCommand::compact()));

    let event = rx.try_recv().expect("conversation op should be sent");
    let AppEvent::ConversationOp { target, op } = event else {
        panic!("expected conversation-scoped op");
    };
    assert_eq!(target.pane, PaneSlot::Parent);
    assert_eq!(target.thread_id, thread_id);
    assert!(matches!(op, AppCommand::Compact { .. }));
}

#[test]
fn independent_conversation_senders_do_not_share_targets() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx);
    let first_sender = sender.scoped_to_conversation(PaneSlot::Parent);
    let second_sender = sender.scoped_to_conversation(PaneSlot::Side);
    let first_thread_id = ThreadId::new();
    let second_thread_id = ThreadId::new();
    first_sender.bind_conversation_thread(first_thread_id);
    second_sender.bind_conversation_thread(second_thread_id);

    first_sender.compact();
    second_sender.compact();

    let targets = [rx.try_recv(), rx.try_recv()].map(|event| match event {
        Ok(AppEvent::ConversationOp { target, .. }) => target,
        _ => panic!("expected conversation-scoped op"),
    });
    assert_eq!(targets[0].pane, PaneSlot::Parent);
    assert_eq!(targets[0].thread_id, first_thread_id);
    assert_eq!(targets[1].pane, PaneSlot::Side);
    assert_eq!(targets[1].thread_id, second_thread_id);
    assert_ne!(targets[0].generation, targets[1].generation);
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
fn unscoped_sender_removes_a_conversation_envelope() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx)
        .scoped_to_conversation(PaneSlot::Parent)
        .unscoped();

    sender.send(AppEvent::TerminalBrowserUpdated);

    assert!(matches!(
        rx.try_recv(),
        Ok(AppEvent::TerminalBrowserUpdated)
    ));
}

#[test]
fn unbound_conversation_sender_does_not_fall_back_to_active_thread() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation(PaneSlot::Parent);

    sender.compact();

    assert!(rx.try_recv().is_err());
}

#[test]
fn conversation_sender_envelopes_non_operations_before_thread_binding() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation(PaneSlot::Parent);

    sender.send(AppEvent::NewSession);

    let event = rx.try_recv().expect("conversation event should be sent");
    let AppEvent::FromConversation { target, event } = event else {
        panic!("expected conversation envelope");
    };
    assert_eq!(target.pane, PaneSlot::Parent);
    assert!(matches!(*event, AppEvent::NewSession));
}

#[test]
fn replacement_sender_has_a_new_generation_for_the_same_pane() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx);
    let stale_sender = sender.scoped_to_conversation(PaneSlot::Parent);
    let replacement_sender = sender.scoped_to_conversation(PaneSlot::Parent);

    stale_sender.send(AppEvent::ClearUi);
    replacement_sender.send(AppEvent::ClearUi);

    let targets = [rx.try_recv(), rx.try_recv()].map(|event| match event {
        Ok(AppEvent::FromConversation { target, event }) => {
            assert!(matches!(*event, AppEvent::ClearUi));
            target
        }
        _ => panic!("expected conversation envelope"),
    });
    assert_eq!(targets[0].pane, PaneSlot::Parent);
    assert_eq!(targets[1].pane, PaneSlot::Parent);
    assert_ne!(targets[0].generation, targets[1].generation);
}

#[test]
fn explicit_thread_operation_is_not_enveloped() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation(PaneSlot::Parent);
    let thread_id = ThreadId::new();

    sender.send(AppEvent::SubmitThreadOp {
        thread_id,
        op: AppCommand::compact(),
    });

    assert!(matches!(
        rx.try_recv(),
        Ok(AppEvent::SubmitThreadOp {
            thread_id: event_thread_id,
            op: AppCommand::Compact { .. },
        }) if event_thread_id == thread_id
    ));
}

#[test]
fn widget_local_thread_transport_is_enveloped() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx).scoped_to_conversation(PaneSlot::Parent);
    let thread_id = ThreadId::new();

    sender.send(AppEvent::LookupMessageHistoryEntry {
        thread_id,
        offset: 3,
        log_id: 7,
    });

    let event = rx.try_recv().expect("conversation event should be sent");
    let AppEvent::FromConversation { event, .. } = event else {
        panic!("expected conversation envelope");
    };
    assert!(matches!(
        *event,
        AppEvent::LookupMessageHistoryEntry {
            thread_id: event_thread_id,
            offset: 3,
            log_id: 7,
        } if event_thread_id == thread_id
    ));
}

#[test]
fn global_sender_preserves_non_operation_events() {
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx);

    sender.send(AppEvent::NewSession);

    assert!(matches!(rx.try_recv(), Ok(AppEvent::NewSession)));
}
