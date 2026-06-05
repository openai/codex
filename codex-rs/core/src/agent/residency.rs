use codex_protocol::ThreadId;
use std::collections::VecDeque;

pub(crate) const MIN_RESIDENT_SUBAGENTS: usize = 64;

#[derive(Default)]
pub(crate) struct ResidentAgents {
    lru: VecDeque<ThreadId>,
}

impl ResidentAgents {
    pub(crate) fn len(&self) -> usize {
        self.lru.len()
    }

    pub(crate) fn contains(&self, thread_id: ThreadId) -> bool {
        self.lru.contains(&thread_id)
    }

    pub(crate) fn touch(&mut self, thread_id: ThreadId) {
        self.remove(thread_id);
        self.lru.push_back(thread_id);
    }

    pub(crate) fn remove(&mut self, thread_id: ThreadId) {
        self.lru.retain(|resident_id| *resident_id != thread_id);
    }

    pub(crate) fn snapshot(&self) -> Vec<ThreadId> {
        self.lru.iter().copied().collect()
    }
}
