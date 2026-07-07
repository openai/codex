use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
pub(crate) struct ThreadActivityGate {
    state: Mutex<ThreadActivityState>,
}

#[derive(Default)]
struct ThreadActivityState {
    next_generation: u64,
    nodes: HashMap<ThreadId, ThreadActivityNode>,
}

struct ThreadActivityNode {
    generation: u64,
    // A lineage edge follows the stable logical thread ID across runtime generations.
    parent_thread_id: Option<ThreadId>,
    active: usize,
    closing: bool,
    handle_live: bool,
    initializing: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("thread or ancestor is shutting down")]
pub(crate) struct ThreadActivityRegistrationError;

pub(crate) struct ThreadActivityHandle {
    gate: Arc<ThreadActivityGate>,
    thread_id: ThreadId,
    generation: u64,
}

impl ThreadActivityGate {
    pub(crate) fn register(
        self: &Arc<Self>,
        thread_id: ThreadId,
        parent_thread_id: Option<ThreadId>,
    ) -> Result<ThreadActivityHandle, ThreadActivityRegistrationError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if state
            .nodes
            .get(&thread_id)
            .is_some_and(|node| !node.closing || node.active > 0)
            || parent_thread_id.is_some_and(|parent_thread_id| {
                Self::ancestor_blocks_registration(&state, parent_thread_id, thread_id)
            })
            || Self::has_active_descendant(&state, thread_id)
        {
            return Err(ThreadActivityRegistrationError);
        }

        state.next_generation = state.next_generation.wrapping_add(1);
        let generation = state.next_generation;
        state.nodes.insert(
            thread_id,
            ThreadActivityNode {
                generation,
                parent_thread_id,
                active: 1,
                closing: false,
                handle_live: true,
                initializing: true,
            },
        );
        Ok(ThreadActivityHandle {
            gate: Arc::clone(self),
            thread_id,
            generation,
        })
    }

    fn ancestor_blocks_registration(
        state: &ThreadActivityState,
        mut ancestor_id: ThreadId,
        descendant_id: ThreadId,
    ) -> bool {
        let mut visited = HashSet::new();
        while visited.insert(ancestor_id) {
            if ancestor_id == descendant_id {
                return true;
            }
            let Some(node) = state.nodes.get(&ancestor_id) else {
                return false;
            };
            if node.closing {
                return true;
            }
            let Some(parent_thread_id) = node.parent_thread_id else {
                return false;
            };
            ancestor_id = parent_thread_id;
        }
        true
    }

    fn has_active_descendant(state: &ThreadActivityState, ancestor_id: ThreadId) -> bool {
        state.nodes.iter().any(|(thread_id, node)| {
            if *thread_id == ancestor_id || node.active == 0 {
                return false;
            }
            let mut parent_thread_id = node.parent_thread_id;
            let mut visited = HashSet::new();
            while let Some(parent_id) = parent_thread_id
                && visited.insert(parent_id)
            {
                if parent_id == ancestor_id {
                    return true;
                }
                parent_thread_id = state
                    .nodes
                    .get(&parent_id)
                    .and_then(|node| node.parent_thread_id);
            }
            false
        })
    }

    fn mark_closed(&self, thread_id: ThreadId, generation: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return;
        };
        if node.generation == generation {
            node.closing = true;
        }
    }

    fn mark_initialized(&self, thread_id: ThreadId, generation: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return;
        };
        if node.generation == generation && node.initializing {
            node.initializing = false;
            node.active = node.active.saturating_sub(1);
        }
    }

    fn prune_inactive_orphaned(
        state: &mut ThreadActivityState,
        mut thread_id: ThreadId,
        mut generation: u64,
    ) {
        loop {
            let has_children = state
                .nodes
                .values()
                .any(|node| node.parent_thread_id == Some(thread_id));
            let Some(node) = state.nodes.get(&thread_id) else {
                return;
            };
            if node.generation != generation || node.handle_live || node.active != 0 || has_children
            {
                return;
            }
            let parent_thread_id = node.parent_thread_id;
            state.nodes.remove(&thread_id);
            let Some(parent_id) = parent_thread_id else {
                return;
            };
            let Some(parent) = state.nodes.get(&parent_id) else {
                return;
            };
            thread_id = parent_id;
            generation = parent.generation;
        }
    }
}

impl ThreadActivityHandle {
    pub(crate) fn mark_initialized(&self) {
        self.gate.mark_initialized(self.thread_id, self.generation);
    }

    pub(crate) fn mark_closed(&self) {
        self.gate.mark_closed(self.thread_id, self.generation);
    }
}

impl Drop for ThreadActivityHandle {
    fn drop(&mut self) {
        let mut state = self
            .gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&self.thread_id) else {
            return;
        };
        if node.generation == self.generation {
            node.handle_live = false;
            node.active = 0;
            node.closing = true;
            node.initializing = false;
            ThreadActivityGate::prune_inactive_orphaned(
                &mut state,
                self.thread_id,
                self.generation,
            );
        }
    }
}

#[cfg(test)]
#[path = "thread_activity_tests.rs"]
mod tests;
