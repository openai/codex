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
    committed: usize,
    closing: bool,
    closed: bool,
    unpublished: bool,
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

pub(crate) struct ThreadActivityReservation {
    gate: Arc<ThreadActivityGate>,
    thread_id: ThreadId,
    generation: u64,
    clear_closing_on_drop: bool,
    delivery_prepared: bool,
    active: bool,
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
                committed: 0,
                closing: false,
                closed: false,
                unpublished: false,
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

    fn validate_ancestors(
        state: &ThreadActivityState,
        mut parent_thread_id: Option<ThreadId>,
        allow_closing: bool,
    ) -> bool {
        let mut visited = HashSet::new();
        while let Some(parent_id) = parent_thread_id
            && visited.insert(parent_id)
        {
            let Some(node) = state.nodes.get(&parent_id) else {
                return true;
            };
            if (node.closing || node.initializing) && !allow_closing {
                return false;
            }
            parent_thread_id = node.parent_thread_id;
        }
        parent_thread_id.is_none()
    }

    fn try_reserve(
        self: &Arc<Self>,
        thread_id: ThreadId,
        generation: u64,
        close: bool,
    ) -> Option<ThreadActivityReservation> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let node = state.nodes.get(&thread_id)?;
        if node.generation != generation
            || node.closed
            || node.closing
            || (node.initializing && !close)
            || !Self::validate_ancestors(&state, node.parent_thread_id, close)
        {
            return None;
        }
        let node = state.nodes.get_mut(&thread_id)?;
        node.active = node.active.checked_add(1)?;
        node.closing = close;
        Some(ThreadActivityReservation {
            gate: Arc::clone(self),
            thread_id,
            generation,
            clear_closing_on_drop: close,
            delivery_prepared: false,
            active: true,
        })
    }

    fn try_reserve_idle_shutdown(
        self: &Arc<Self>,
        thread_id: ThreadId,
        generation: u64,
    ) -> Option<ThreadActivityReservation> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let node = state.nodes.get(&thread_id)?;
        if node.generation != generation
            || node.closed
            || node.closing
            || !Self::validate_ancestors(&state, node.parent_thread_id, /*allow_closing*/ true)
            || node.active != 0
            || Self::has_active_descendant(&state, thread_id)
        {
            return None;
        }
        let node = state.nodes.get_mut(&thread_id)?;
        node.active = 1;
        node.closing = true;
        Some(ThreadActivityReservation {
            gate: Arc::clone(self),
            thread_id,
            generation,
            clear_closing_on_drop: true,
            delivery_prepared: false,
            active: true,
        })
    }

    fn release(&self, thread_id: ThreadId, generation: u64, clear_closing: bool) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return;
        };
        if node.generation != generation || node.active == 0 {
            return;
        }
        node.active -= 1;
        if clear_closing && !node.closed {
            node.closing = false;
        }
        if node.active == 0 && !node.handle_live {
            Self::prune_inactive_orphaned(&mut state, thread_id, generation);
        }
    }

    fn prepare_delivery(&self, thread_id: ThreadId, generation: u64) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return false;
        };
        if node.generation != generation || node.closed {
            return false;
        }
        let Some(committed) = node.committed.checked_add(1) else {
            return false;
        };
        if committed > node.active {
            return false;
        }
        node.committed = committed;
        true
    }

    fn rollback_delivery(&self, thread_id: ThreadId, generation: u64, clear_closing: bool) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return;
        };
        if node.generation != generation || node.committed == 0 || node.active == 0 {
            return;
        }
        node.committed -= 1;
        node.active -= 1;
        if clear_closing && !node.closed {
            node.closing = false;
        }
        if node.active == 0 && !node.handle_live {
            Self::prune_inactive_orphaned(&mut state, thread_id, generation);
        }
    }

    fn finish_submission(&self, thread_id: ThreadId, generation: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return;
        };
        if node.generation != generation || node.committed == 0 {
            return;
        }
        node.committed -= 1;
        node.active = node.active.saturating_sub(1);
        if node.active == 0 && !node.handle_live {
            Self::prune_inactive_orphaned(&mut state, thread_id, generation);
        }
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
            node.closed = true;
            if node.unpublished {
                node.unpublished = false;
                node.active = node.active.saturating_sub(1);
            }
        }
    }

    fn mark_unpublished(&self, thread_id: ThreadId, generation: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return;
        };
        if node.generation != generation || node.closed || node.unpublished {
            return;
        }
        let Some(active) = node.active.checked_add(1) else {
            return;
        };
        node.active = active;
        node.unpublished = true;
    }

    fn prepare_idle_shutdown_delivery(&self, thread_id: ThreadId, generation: u64) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(node) = state.nodes.get(&thread_id) else {
            return false;
        };
        if node.generation != generation
            || !node.closing
            || node.closed
            || node.active != 1
            || Self::has_active_descendant(&state, thread_id)
        {
            return false;
        }
        let Some(node) = state.nodes.get_mut(&thread_id) else {
            return false;
        };
        node.committed = 1;
        true
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

    pub(crate) fn try_reserve(&self, close: bool) -> Option<ThreadActivityReservation> {
        self.gate
            .try_reserve(self.thread_id, self.generation, close)
    }

    pub(crate) fn try_reserve_idle_shutdown(&self) -> Option<ThreadActivityReservation> {
        self.gate
            .try_reserve_idle_shutdown(self.thread_id, self.generation)
    }

    pub(crate) fn finish_submission(&self) {
        self.gate.finish_submission(self.thread_id, self.generation);
    }

    pub(crate) fn mark_closed(&self) {
        self.gate.mark_closed(self.thread_id, self.generation);
    }

    pub(crate) fn mark_unpublished(&self) {
        self.gate.mark_unpublished(self.thread_id, self.generation);
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
            node.closing = true;
            node.closed = true;
            // The loop cannot finish committed submissions after its session handle disappears.
            // Live RAII reservations (notably parent completion delivery) must drain themselves.
            node.active = node.active.saturating_sub(node.committed);
            node.committed = 0;
            if node.initializing {
                node.active = node.active.saturating_sub(1);
                node.initializing = false;
            }
            if node.unpublished {
                node.active = node.active.saturating_sub(1);
                node.unpublished = false;
            }
            ThreadActivityGate::prune_inactive_orphaned(
                &mut state,
                self.thread_id,
                self.generation,
            );
        }
    }
}

impl ThreadActivityReservation {
    pub(crate) fn prepare_delivery(&mut self) -> bool {
        if !self.active || self.delivery_prepared {
            return false;
        }
        if self.gate.prepare_delivery(self.thread_id, self.generation) {
            self.delivery_prepared = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn prepare_idle_shutdown_delivery(&mut self) -> bool {
        if !self.active || self.delivery_prepared {
            return false;
        }
        if self
            .gate
            .prepare_idle_shutdown_delivery(self.thread_id, self.generation)
        {
            self.delivery_prepared = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn commit(mut self) {
        if self.delivery_prepared {
            self.active = false;
        }
    }

    pub(crate) fn release_after_failed_delivery(mut self) {
        self.rollback(/*clear_closing*/ false);
    }

    fn rollback(&mut self, clear_closing: bool) {
        if !self.active {
            return;
        }
        if self.delivery_prepared {
            self.gate
                .rollback_delivery(self.thread_id, self.generation, clear_closing);
        } else {
            self.gate
                .release(self.thread_id, self.generation, clear_closing);
        }
        self.active = false;
    }
}

impl Drop for ThreadActivityReservation {
    fn drop(&mut self) {
        self.rollback(self.clear_closing_on_drop);
    }
}

#[cfg(test)]
#[path = "thread_activity_tests.rs"]
mod tests;
