use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;

use codex_protocol::ThreadId;

use crate::AgentGraphStore;
use crate::AgentGraphStoreFuture;
use crate::ThreadSpawnEdgeStatus;

static IN_MEMORY_AGENT_GRAPH_STORES: OnceLock<
    Mutex<HashMap<String, Arc<InMemoryAgentGraphStore>>>,
> = OnceLock::new();

fn stores() -> &'static Mutex<HashMap<String, Arc<InMemoryAgentGraphStore>>> {
    IN_MEMORY_AGENT_GRAPH_STORES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stores_guard() -> MutexGuard<'static, HashMap<String, Arc<InMemoryAgentGraphStore>>> {
    match stores().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Process-local [`AgentGraphStore`] paired with an in-memory thread store.
#[derive(Debug, Default)]
pub struct InMemoryAgentGraphStore {
    edges_by_child: tokio::sync::Mutex<HashMap<ThreadId, ThreadSpawnEdge>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ThreadSpawnEdge {
    parent_thread_id: ThreadId,
    status: ThreadSpawnEdgeStatus,
}

impl InMemoryAgentGraphStore {
    /// Returns the graph store associated with `id`, creating it if needed.
    pub fn for_id(id: impl Into<String>) -> Arc<Self> {
        let id = id.into();
        let mut stores = stores_guard();
        stores
            .entry(id)
            .or_insert_with(|| Arc::new(Self::default()))
            .clone()
    }

    /// Removes a shared in-memory graph store for `id`.
    pub fn remove_id(id: &str) -> Option<Arc<Self>> {
        stores_guard().remove(id)
    }
}

impl AgentGraphStore for InMemoryAgentGraphStore {
    fn upsert_thread_spawn_edge(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreFuture<'_, ()> {
        Box::pin(async move {
            self.edges_by_child.lock().await.insert(
                child_thread_id,
                ThreadSpawnEdge {
                    parent_thread_id,
                    status,
                },
            );
            Ok(())
        })
    }

    fn set_thread_spawn_edge_status(
        &self,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreFuture<'_, ()> {
        Box::pin(async move {
            if let Some(edge) = self.edges_by_child.lock().await.get_mut(&child_thread_id) {
                edge.status = status;
            }
            Ok(())
        })
    }

    fn list_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreFuture<'_, Vec<ThreadId>> {
        Box::pin(async move {
            let edges_by_child = self.edges_by_child.lock().await;
            let mut children = edges_by_child
                .iter()
                .filter_map(|(child_thread_id, edge)| {
                    (edge.parent_thread_id == parent_thread_id
                        && status_filter.is_none_or(|status| edge.status == status))
                    .then_some(*child_thread_id)
                })
                .collect::<Vec<_>>();
            children.sort_unstable_by_key(ThreadId::to_string);
            Ok(children)
        })
    }

    fn list_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreFuture<'_, Vec<ThreadId>> {
        Box::pin(async move {
            let edges_by_child = self.edges_by_child.lock().await;
            let mut descendants = Vec::new();
            let mut visited = HashSet::from([root_thread_id]);
            let mut frontier = HashSet::from([root_thread_id]);

            while !frontier.is_empty() {
                let mut next_frontier = edges_by_child
                    .iter()
                    .filter_map(|(child_thread_id, edge)| {
                        (frontier.contains(&edge.parent_thread_id)
                            && status_filter.is_none_or(|status| edge.status == status)
                            && !visited.contains(child_thread_id))
                        .then_some(*child_thread_id)
                    })
                    .collect::<Vec<_>>();
                next_frontier.sort_unstable_by_key(ThreadId::to_string);
                next_frontier.dedup();
                visited.extend(next_frontier.iter().copied());
                descendants.extend(next_frontier.iter().copied());
                frontier = next_frontier.into_iter().collect();
            }

            Ok(descendants)
        })
    }
}

#[cfg(test)]
#[path = "in_memory_tests.rs"]
mod tests;
