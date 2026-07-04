use super::*;
use crate::app_event::HistoryLookupResponse;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane;
use pretty_assertions::assert_eq;

async fn pane_init(
    slot: PaneSlot,
) -> (
    ConversationPaneInit,
    tokio::sync::mpsc::UnboundedReceiver<crate::app_event::AppEvent>,
) {
    let (chat_widget, rx) = make_chatwidget_for_pane(slot).await;
    let file_search = FileSearchManager::new(
        chat_widget.config_ref().cwd.to_path_buf(),
        chat_widget.conversation_event_sender(),
    );
    (
        ConversationPaneInit {
            chat_widget,
            file_search,
            owned_screen: None,
        },
        rx,
    )
}

fn buffered_event(offset: usize) -> ThreadBufferedEvent {
    ThreadBufferedEvent::HistoryEntryResponse(HistoryLookupResponse {
        offset,
        log_id: offset as u64,
        entry: Some(format!("entry {offset}")),
    })
}

fn event_offset(event: ThreadBufferedEvent) -> usize {
    let ThreadBufferedEvent::HistoryEntryResponse(response) = event else {
        panic!("expected history entry response");
    };
    response.offset
}

#[tokio::test]
async fn focus_and_dispatch_select_different_panes() {
    let (parent, _parent_rx) = pane_init(PaneSlot::Parent).await;
    let (side, _side_rx) = pane_init(PaneSlot::Side).await;
    let Ok(mut panes) = ConversationPanes::new_parent(parent) else {
        panic!("parent pane should install");
    };
    assert!(matches!(panes.install_side(side), Ok(None)));
    let parent_origin = panes
        .by_slot(PaneSlot::Parent)
        .and_then(ConversationPane::origin)
        .expect("parent origin");
    let side_origin = panes
        .by_slot(PaneSlot::Side)
        .and_then(ConversationPane::origin)
        .expect("side origin");

    assert!(panes.focus(PaneSlot::Side));
    assert_eq!(panes.focused_slot(), PaneSlot::Side);
    assert_eq!(panes.conversation_origin(), Some(side_origin));
    assert!(panes.dispatch_to(parent_origin));
    assert_eq!(panes.conversation_origin(), Some(parent_origin));
    assert_eq!(panes.clear_dispatch(), Some(PaneSlot::Parent));
    assert_eq!(panes.conversation_origin(), Some(side_origin));
}

#[tokio::test]
async fn taking_focused_side_restores_parent_selection() {
    let (parent, _parent_rx) = pane_init(PaneSlot::Parent).await;
    let (side, _side_rx) = pane_init(PaneSlot::Side).await;
    let Ok(mut panes) = ConversationPanes::new_parent(parent) else {
        panic!("parent pane should install");
    };
    assert!(panes.install_side(side).is_ok());
    assert!(panes.focus(PaneSlot::Side));
    let side_origin = panes.conversation_origin().expect("side origin");
    panes
        .by_slot(PaneSlot::Side)
        .expect("side pane")
        .commit_anim_running
        .store(/*val*/ true, Ordering::Release);
    assert!(panes.dispatch_to(side_origin));

    let removed = panes.take_side().expect("installed side pane");

    assert_eq!(removed.origin(), Some(side_origin));
    assert!(!removed.commit_anim_running.load(Ordering::Acquire));
    assert_eq!(panes.focused_slot(), PaneSlot::Parent);
    assert_eq!(panes.clear_dispatch(), None);
    assert!(panes.finish_dispatch(side_origin));
    assert!(panes.by_slot(PaneSlot::Side).is_none());
    assert_eq!(
        panes.conversation_origin().map(|origin| origin.pane),
        Some(PaneSlot::Parent)
    );
}

#[tokio::test]
async fn installed_thread_ids_are_parent_first_and_unique() {
    let (parent, _parent_rx) = pane_init(PaneSlot::Parent).await;
    let (side, _side_rx) = pane_init(PaneSlot::Side).await;
    let Ok(mut panes) = ConversationPanes::new_parent(parent) else {
        panic!("parent pane should install");
    };
    assert!(panes.install_side(side).is_ok());
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    for (slot, thread_id) in [
        (PaneSlot::Parent, parent_thread_id),
        (PaneSlot::Side, side_thread_id),
    ] {
        panes
            .by_slot_mut(slot)
            .expect("installed pane")
            .attach_thread(thread_id, /*receiver*/ None);
    }
    assert_eq!(
        panes.installed_thread_ids(),
        vec![parent_thread_id, side_thread_id]
    );

    panes
        .by_slot_mut(PaneSlot::Side)
        .expect("side pane")
        .attach_thread(parent_thread_id, /*receiver*/ None);
    assert_eq!(panes.installed_thread_ids(), vec![parent_thread_id]);
}

#[tokio::test]
async fn a_closed_receiver_does_not_starve_the_other_pane() {
    let (parent, _parent_app_rx) = pane_init(PaneSlot::Parent).await;
    let (side, _side_app_rx) = pane_init(PaneSlot::Side).await;
    let Ok(mut panes) = ConversationPanes::new_parent(parent) else {
        panic!("parent pane should install");
    };
    assert!(panes.install_side(side).is_ok());
    let (parent_tx, parent_rx) = mpsc::channel(/*buffer*/ 1);
    let (side_tx, side_rx) = mpsc::channel(/*buffer*/ 1);
    panes
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(ThreadId::new(), Some(parent_rx));
    panes
        .by_slot_mut(PaneSlot::Side)
        .expect("side pane")
        .attach_thread(ThreadId::new(), Some(side_rx));

    drop(parent_tx);
    let (slot, event) = panes.recv_thread_event().await;
    assert_eq!(slot, PaneSlot::Parent);
    assert!(event.is_none());
    panes
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .clear_thread();
    side_tx
        .send(buffered_event(/*offset*/ 3))
        .await
        .expect("side receiver");

    let (slot, event) = panes.recv_thread_event().await;
    assert_eq!(slot, PaneSlot::Side);
    assert_eq!(event_offset(event.expect("side event")), 3);
}

#[tokio::test]
async fn both_pane_receivers_are_serviced() {
    let (parent, _parent_app_rx) = pane_init(PaneSlot::Parent).await;
    let (side, _side_app_rx) = pane_init(PaneSlot::Side).await;
    let Ok(mut panes) = ConversationPanes::new_parent(parent) else {
        panic!("parent pane should install");
    };
    assert!(panes.install_side(side).is_ok());
    let (parent_tx, parent_rx) = mpsc::channel(/*buffer*/ 1);
    let (side_tx, side_rx) = mpsc::channel(/*buffer*/ 1);
    let parent_thread = ThreadId::new();
    let side_thread = ThreadId::new();
    panes
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(parent_thread, Some(parent_rx));
    panes
        .by_slot_mut(PaneSlot::Side)
        .expect("side pane")
        .attach_thread(side_thread, Some(side_rx));
    parent_tx
        .send(buffered_event(/*offset*/ 1))
        .await
        .expect("parent receiver");
    side_tx
        .send(buffered_event(/*offset*/ 2))
        .await
        .expect("side receiver");

    let first = panes.recv_thread_event().await;
    let second = panes.recv_thread_event().await;
    let mut received = [
        (first.0, event_offset(first.1.expect("first event"))),
        (second.0, event_offset(second.1.expect("second event"))),
    ];
    received.sort_by_key(|(slot, _)| match slot {
        PaneSlot::Parent => 0,
        PaneSlot::Side => 1,
    });

    assert_eq!(received, [(PaneSlot::Parent, 1), (PaneSlot::Side, 2)]);
    assert!(panes.contains_thread(parent_thread));
    assert!(panes.contains_thread(side_thread));
    assert!(panes.has_thread_event_receiver());
}
