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
