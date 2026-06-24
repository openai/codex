use std::sync::Arc;

use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

use super::*;

fn thread_id(suffix: u128) -> ThreadId {
    ThreadId::from_string(&format!("00000000-0000-0000-0000-{suffix:012}"))
        .expect("valid thread id")
}

#[tokio::test]
async fn shared_store_upserts_reparents_and_updates_status() {
    let store_id = "agent-graph-store-upsert-test";
    InMemoryAgentGraphStore::remove_id(store_id);
    let store = InMemoryAgentGraphStore::for_id(store_id);
    let shared_store = InMemoryAgentGraphStore::for_id(store_id);
    assert!(Arc::ptr_eq(&store, &shared_store));

    let first_parent = thread_id(/*suffix*/ 1);
    let second_parent = thread_id(/*suffix*/ 2);
    let child = thread_id(/*suffix*/ 3);
    store
        .upsert_thread_spawn_edge(first_parent, child, ThreadSpawnEdgeStatus::Open)
        .await
        .expect("edge should insert");
    store
        .upsert_thread_spawn_edge(second_parent, child, ThreadSpawnEdgeStatus::Closed)
        .await
        .expect("edge should reparent");

    assert_eq!(
        store
            .list_thread_spawn_children(first_parent, /*status_filter*/ None)
            .await
            .expect("first parent children should load"),
        Vec::<ThreadId>::new()
    );
    assert_eq!(
        shared_store
            .list_thread_spawn_children(second_parent, Some(ThreadSpawnEdgeStatus::Closed),)
            .await
            .expect("second parent children should load"),
        vec![child]
    );

    store
        .set_thread_spawn_edge_status(child, ThreadSpawnEdgeStatus::Open)
        .await
        .expect("edge should reopen");
    store
        .set_thread_spawn_edge_status(thread_id(/*suffix*/ 4), ThreadSpawnEdgeStatus::Closed)
        .await
        .expect("missing edge update should be a no-op");
    assert_eq!(
        store
            .list_thread_spawn_children(second_parent, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("open children should load"),
        vec![child]
    );

    InMemoryAgentGraphStore::remove_id(store_id);
}

#[tokio::test]
async fn descendants_are_breadth_first_sorted_filtered_and_cycle_safe() {
    let store = InMemoryAgentGraphStore::default();
    let root = thread_id(/*suffix*/ 10);
    let first_child = thread_id(/*suffix*/ 11);
    let second_child = thread_id(/*suffix*/ 12);
    let closed_grandchild = thread_id(/*suffix*/ 13);
    let open_grandchild = thread_id(/*suffix*/ 14);

    for (parent, child, status) in [
        (root, second_child, ThreadSpawnEdgeStatus::Open),
        (root, first_child, ThreadSpawnEdgeStatus::Open),
        (
            first_child,
            closed_grandchild,
            ThreadSpawnEdgeStatus::Closed,
        ),
        (second_child, open_grandchild, ThreadSpawnEdgeStatus::Open),
        (open_grandchild, root, ThreadSpawnEdgeStatus::Open),
    ] {
        store
            .upsert_thread_spawn_edge(parent, child, status)
            .await
            .expect("edge should insert");
    }

    assert_eq!(
        store
            .list_thread_spawn_descendants(root, /*status_filter*/ None)
            .await
            .expect("all descendants should load"),
        vec![
            first_child,
            second_child,
            closed_grandchild,
            open_grandchild,
        ]
    );
    assert_eq!(
        store
            .list_thread_spawn_descendants(root, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("open descendants should load"),
        vec![first_child, second_child, open_grandchild]
    );
}
