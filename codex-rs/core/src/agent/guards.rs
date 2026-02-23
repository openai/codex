use crate::error::CodexErr;
use crate::error::Result;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// This structure is used to add some limits on the multi-agent capabilities for Codex. In
/// the current implementation, it limits:
/// * Total number of sub-agents (i.e. threads) per user session
///
/// This structure is shared by all agents in the same user session (because the `AgentControl`
/// is).
#[derive(Default)]
pub(crate) struct Guards {
    active_agents: Mutex<ActiveAgents>,
    total_count: AtomicUsize,
}

#[derive(Default)]
struct ActiveAgents {
    threads_set: HashSet<ThreadId>,
    thread_agent_nicknames: HashMap<ThreadId, String>,
    used_agent_nicknames: HashSet<String>,
    nickname_reset_count: usize,
}

fn session_depth(session_source: &SessionSource) -> i32 {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { depth, .. }) => *depth,
        SessionSource::SubAgent(_) => 0,
        _ => 0,
    }
}

pub(crate) fn next_thread_spawn_depth(session_source: &SessionSource) -> i32 {
    session_depth(session_source).saturating_add(1)
}

pub(crate) fn exceeds_thread_spawn_depth_limit(depth: i32, max_depth: i32) -> bool {
    depth > max_depth
}

impl Guards {
    pub(crate) fn reserve_spawn_slot(
        self: &Arc<Self>,
        max_threads: Option<usize>,
    ) -> Result<SpawnReservation> {
        if let Some(max_threads) = max_threads {
            if !self.try_increment_spawned(max_threads) {
                return Err(CodexErr::AgentLimitReached { max_threads });
            }
        } else {
            self.total_count.fetch_add(1, Ordering::AcqRel);
        }
        Ok(SpawnReservation {
            state: Arc::clone(self),
            active: true,
            reserved_agent_nickname: None,
        })
    }

    pub(crate) fn release_spawned_thread(&self, thread_id: ThreadId) {
        let removed = {
            let mut active_agents = self
                .active_agents
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let removed = active_agents.threads_set.remove(&thread_id);
            active_agents.thread_agent_nicknames.remove(&thread_id);
            removed
        };
        if removed {
            self.total_count.fetch_sub(1, Ordering::AcqRel);
        }
    }

    fn register_spawned_thread(&self, thread_id: ThreadId, agent_nickname: Option<String>) {
        let mut active_agents = self
            .active_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active_agents.threads_set.insert(thread_id);
        if let Some(agent_nickname) = agent_nickname {
            active_agents
                .used_agent_nicknames
                .insert(agent_nickname.clone());
            active_agents
                .thread_agent_nicknames
                .insert(thread_id, agent_nickname);
        }
    }

    fn reserve_agent_nickname(&self, names: &[&str], preferred: Option<&str>) -> Option<String> {
        let mut active_agents = self
            .active_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let agent_nickname = if let Some(preferred) = preferred {
            preferred.to_string()
        } else {
            if names.is_empty() {
                return None;
            }
            let available_names: Vec<&str> = names
                .iter()
                .copied()
                .filter(|name| !active_agents.used_agent_nicknames.contains(*name))
                .collect();
            if let Some(name) = available_names.choose(&mut rand::rng()) {
                (*name).to_string()
            } else {
                active_agents.used_agent_nicknames.clear();
                active_agents.nickname_reset_count += 1;
                names.choose(&mut rand::rng())?.to_string()
            }
        };
        active_agents
            .used_agent_nicknames
            .insert(agent_nickname.clone());
        Some(agent_nickname)
    }

    fn try_increment_spawned(&self, max_threads: usize) -> bool {
        let mut current = self.total_count.load(Ordering::Acquire);
        loop {
            if current >= max_threads {
                return false;
            }
            match self.total_count.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(updated) => current = updated,
            }
        }
    }
}

pub(crate) struct SpawnReservation {
    state: Arc<Guards>,
    active: bool,
    reserved_agent_nickname: Option<String>,
}

impl SpawnReservation {
    pub(crate) fn reserve_agent_nickname(&mut self, names: &[&str]) -> Result<String> {
        self.reserve_agent_nickname_with_preference(names, None)
    }

    pub(crate) fn reserve_agent_nickname_with_preference(
        &mut self,
        names: &[&str],
        preferred: Option<&str>,
    ) -> Result<String> {
        let agent_nickname = self
            .state
            .reserve_agent_nickname(names, preferred)
            .ok_or_else(|| {
                CodexErr::UnsupportedOperation("no available agent nicknames".to_string())
            })?;
        self.reserved_agent_nickname = Some(agent_nickname.clone());
        Ok(agent_nickname)
    }

    pub(crate) fn commit(self, thread_id: ThreadId) {
        self.commit_with_agent_nickname(thread_id, None);
    }

    pub(crate) fn commit_with_agent_nickname(
        mut self,
        thread_id: ThreadId,
        agent_nickname: Option<String>,
    ) {
        let agent_nickname = self.reserved_agent_nickname.take().or(agent_nickname);
        self.state
            .register_spawned_thread(thread_id, agent_nickname);
        self.active = false;
    }
}

impl Drop for SpawnReservation {
    fn drop(&mut self) {
        if self.active {
            self.state.total_count.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn session_depth_defaults_to_zero_for_root_sources() {
        assert_eq!(session_depth(&SessionSource::Cli), 0);
    }

    #[test]
    fn thread_spawn_depth_increments_and_enforces_limit() {
        let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: ThreadId::new(),
            depth: 1,
            agent_nickname: None,
            agent_role: None,
        });
        let child_depth = next_thread_spawn_depth(&session_source);
        assert_eq!(child_depth, 2);
        assert!(exceeds_thread_spawn_depth_limit(child_depth, 1));
    }

    #[test]
    fn non_thread_spawn_subagents_default_to_depth_zero() {
        let session_source = SessionSource::SubAgent(SubAgentSource::Review);
        assert_eq!(session_depth(&session_source), 0);
        assert_eq!(next_thread_spawn_depth(&session_source), 1);
        assert!(!exceeds_thread_spawn_depth_limit(1, 1));
    }

    #[test]
    fn reservation_drop_releases_slot() {
        let guards = Arc::new(Guards::default());
        let reservation = guards.reserve_spawn_slot(Some(1)).expect("reserve slot");
        drop(reservation);

        let reservation = guards.reserve_spawn_slot(Some(1)).expect("slot released");
        drop(reservation);
    }

    #[test]
    fn commit_holds_slot_until_release() {
        let guards = Arc::new(Guards::default());
        let reservation = guards.reserve_spawn_slot(Some(1)).expect("reserve slot");
        let thread_id = ThreadId::new();
        reservation.commit(thread_id);

        let err = match guards.reserve_spawn_slot(Some(1)) {
            Ok(_) => panic!("limit should be enforced"),
            Err(err) => err,
        };
        let CodexErr::AgentLimitReached { max_threads } = err else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(max_threads, 1);

        guards.release_spawned_thread(thread_id);
        let reservation = guards
            .reserve_spawn_slot(Some(1))
            .expect("slot released after thread removal");
        drop(reservation);
    }

    #[test]
    fn release_ignores_unknown_thread_id() {
        let guards = Arc::new(Guards::default());
        let reservation = guards.reserve_spawn_slot(Some(1)).expect("reserve slot");
        let thread_id = ThreadId::new();
        reservation.commit(thread_id);

        guards.release_spawned_thread(ThreadId::new());

        let err = match guards.reserve_spawn_slot(Some(1)) {
            Ok(_) => panic!("limit should still be enforced"),
            Err(err) => err,
        };
        let CodexErr::AgentLimitReached { max_threads } = err else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(max_threads, 1);

        guards.release_spawned_thread(thread_id);
        let reservation = guards
            .reserve_spawn_slot(Some(1))
            .expect("slot released after real thread removal");
        drop(reservation);
    }

    #[test]
    fn release_is_idempotent_for_registered_threads() {
        let guards = Arc::new(Guards::default());
        let reservation = guards.reserve_spawn_slot(Some(1)).expect("reserve slot");
        let first_id = ThreadId::new();
        reservation.commit(first_id);

        guards.release_spawned_thread(first_id);

        let reservation = guards.reserve_spawn_slot(Some(1)).expect("slot reused");
        let second_id = ThreadId::new();
        reservation.commit(second_id);

        guards.release_spawned_thread(first_id);

        let err = match guards.reserve_spawn_slot(Some(1)) {
            Ok(_) => panic!("limit should still be enforced"),
            Err(err) => err,
        };
        let CodexErr::AgentLimitReached { max_threads } = err else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(max_threads, 1);

        guards.release_spawned_thread(second_id);
        let reservation = guards
            .reserve_spawn_slot(Some(1))
            .expect("slot released after second thread removal");
        drop(reservation);
    }

    #[test]
    fn failed_spawn_keeps_nickname_marked_used() {
        let guards = Arc::new(Guards::default());
        let mut reservation = guards.reserve_spawn_slot(None).expect("reserve slot");
        let agent_nickname = reservation
            .reserve_agent_nickname(&["alpha"])
            .expect("reserve agent name");
        assert_eq!(agent_nickname, "alpha");
        drop(reservation);

        let mut reservation = guards.reserve_spawn_slot(None).expect("reserve slot");
        let agent_nickname = reservation
            .reserve_agent_nickname(&["alpha", "beta"])
            .expect("unused name should still be preferred");
        assert_eq!(agent_nickname, "beta");
    }

    #[test]
    fn agent_nickname_resets_used_pool_when_exhausted() {
        let guards = Arc::new(Guards::default());
        let mut first = guards.reserve_spawn_slot(None).expect("reserve first slot");
        let first_name = first
            .reserve_agent_nickname(&["alpha"])
            .expect("reserve first agent name");
        let first_id = ThreadId::new();
        first.commit(first_id);
        assert_eq!(first_name, "alpha");

        let mut second = guards
            .reserve_spawn_slot(None)
            .expect("reserve second slot");
        let second_name = second
            .reserve_agent_nickname(&["alpha"])
            .expect("name should be reused after pool reset");
        assert_eq!(second_name, "alpha");
        let active_agents = guards
            .active_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(active_agents.nickname_reset_count, 1);
    }

    #[test]
    fn released_nickname_stays_used_until_pool_reset() {
        let guards = Arc::new(Guards::default());

        let mut first = guards.reserve_spawn_slot(None).expect("reserve first slot");
        let first_name = first
            .reserve_agent_nickname(&["alpha"])
            .expect("reserve first agent name");
        let first_id = ThreadId::new();
        first.commit(first_id);
        assert_eq!(first_name, "alpha");

        guards.release_spawned_thread(first_id);

        let mut second = guards
            .reserve_spawn_slot(None)
            .expect("reserve second slot");
        let second_name = second
            .reserve_agent_nickname(&["alpha", "beta"])
            .expect("released name should still be marked used");
        assert_eq!(second_name, "beta");
        let second_id = ThreadId::new();
        second.commit(second_id);
        guards.release_spawned_thread(second_id);

        let mut third = guards.reserve_spawn_slot(None).expect("reserve third slot");
        let third_name = third
            .reserve_agent_nickname(&["alpha", "beta"])
            .expect("pool reset should permit a duplicate");
        assert!(third_name == "alpha" || third_name == "beta");
        let active_agents = guards
            .active_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(active_agents.nickname_reset_count, 1);
    }
}
