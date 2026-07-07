use super::*;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use std::path::Path;
use std::sync::Arc;

fn register(
    gate: &Arc<ThreadActivityGate>,
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
) -> ThreadActivityHandle {
    let handle = gate
        .register(thread_id, parent_thread_id)
        .expect("register thread");
    handle.mark_initialized();
    handle
}

#[test]
fn registration_rejects_parent_cycles() {
    let gate = Arc::new(ThreadActivityGate::default());
    let first_id = ThreadId::new();
    let missing_parent_id = ThreadId::new();
    let _first = register(&gate, first_id, Some(missing_parent_id));

    assert!(gate.register(missing_parent_id, Some(first_id)).is_err());
    let self_parent_id = ThreadId::new();
    assert!(gate.register(self_parent_id, Some(self_parent_id)).is_err());
}

#[test]
fn registration_and_publication_preserve_thread_incarnations() {
    let gate = Arc::new(ThreadActivityGate::default());
    let parent_id = ThreadId::new();
    let child_id = ThreadId::new();
    let parent = register(&gate, parent_id, /*parent_thread_id*/ None);
    let child = gate
        .register(child_id, Some(parent_id))
        .expect("register initializing child");

    parent.mark_closed();
    assert!(gate.register(parent_id, /*parent_thread_id*/ None).is_err());
    assert!(gate.register(ThreadId::new(), Some(parent_id)).is_err());

    child.mark_initialized();
    let replacement = gate
        .register(parent_id, /*parent_thread_id*/ None)
        .expect("register replacement parent");
    replacement.mark_initialized();
    drop(parent);
    assert!(gate.register(parent_id, /*parent_thread_id*/ None).is_err());
    let _grandchild = register(&gate, ThreadId::new(), Some(child_id));
}

#[test]
fn registration_error_maps_to_invalid_request() {
    let error = anyhow::Error::new(ThreadActivityRegistrationError);
    let mapped = crate::session_rollout_init_error::map_session_init_error(&error, Path::new(""));
    let CodexErr::InvalidRequest(message) = mapped else {
        panic!("expected invalid request");
    };
    assert_eq!(message, error.to_string());
}

#[test]
fn dropped_tree_handles_are_pruned_leaf_to_root() {
    let gate = Arc::new(ThreadActivityGate::default());
    let parent_id = ThreadId::new();
    let child_id = ThreadId::new();
    let parent = register(&gate, parent_id, /*parent_thread_id*/ None);
    let child = register(&gate, child_id, Some(parent_id));

    parent.mark_closed();
    child.mark_closed();
    drop(parent);
    assert_eq!(gate.state.lock().expect("gate state").nodes.len(), 2);
    drop(child);
    assert!(gate.state.lock().expect("gate state").nodes.is_empty());
}

#[test]
fn unpublication_holds_activity_through_a_provisional_close() {
    let gate = Arc::new(ThreadActivityGate::default());
    let parent_id = ThreadId::new();
    let child_id = ThreadId::new();
    let parent = register(&gate, parent_id, /*parent_thread_id*/ None);
    let child = register(&gate, child_id, Some(parent_id));

    let provisional_close = child
        .try_reserve_idle_shutdown()
        .expect("reserve provisional child shutdown");
    child.mark_unpublished();
    drop(provisional_close);

    assert!(parent.try_reserve_idle_shutdown().is_none());

    child.mark_closed();
    assert!(parent.try_reserve_idle_shutdown().is_some());
    assert!(child.try_reserve(/*close*/ false).is_none());

    let child = register(&gate, ThreadId::new(), Some(parent_id));
    let mut parent_close = parent
        .try_reserve_idle_shutdown()
        .expect("reserve provisional parent shutdown");
    child.mark_unpublished();
    assert!(!parent_close.prepare_idle_shutdown_delivery());
    drop(parent_close);
    assert!(parent.try_reserve_idle_shutdown().is_none());
}

#[test]
fn activity_reservations_are_atomic_and_survive_session_teardown() {
    let gate = Arc::new(ThreadActivityGate::default());
    let parent_id = ThreadId::new();
    let child_id = ThreadId::new();
    let parent = register(&gate, parent_id, /*parent_thread_id*/ None);
    let child = register(&gate, child_id, Some(parent_id));

    let child_activity = child
        .try_reserve(/*close*/ false)
        .expect("reserve child activity");
    assert!(parent.try_reserve_idle_shutdown().is_none());
    drop(child_activity);
    let parent_activity = parent
        .try_reserve(/*close*/ false)
        .expect("reserve parent activity");
    assert!(parent.try_reserve_idle_shutdown().is_none());
    drop(parent);
    assert!(gate.register(parent_id, /*parent_thread_id*/ None).is_err());
    drop(parent_activity);
    let parent = register(&gate, parent_id, /*parent_thread_id*/ None);
    let shutdown = parent
        .try_reserve_idle_shutdown()
        .expect("reserve idle shutdown");
    assert!(child.try_reserve(/*close*/ false).is_none());
    drop(shutdown);

    let mut failed_shutdown = parent
        .try_reserve(/*close*/ true)
        .expect("reserve explicit shutdown");
    assert!(failed_shutdown.prepare_delivery());
    failed_shutdown.release_after_failed_delivery();
    {
        let state = gate.state.lock().expect("gate state");
        let node = state.nodes.get(&parent_id).expect("parent node");
        assert_eq!((node.active, node.committed, node.closing), (0, 0, true));
    }
}

#[test]
fn receiver_can_finish_before_sender_observes_delivery() {
    let gate = Arc::new(ThreadActivityGate::default());
    let thread_id = ThreadId::new();
    let thread = register(&gate, thread_id, /*parent_thread_id*/ None);
    let mut delivery = thread
        .try_reserve(/*close*/ false)
        .expect("reserve submission delivery");
    assert!(delivery.prepare_delivery());

    thread.finish_submission();
    delivery.commit();

    let state = gate.state.lock().expect("gate state");
    let node = state.nodes.get(&thread_id).expect("thread node");
    assert_eq!((node.active, node.committed), (0, 0));
}
