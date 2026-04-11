use super::*;
use codex_protocol::AgentPath;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

fn agent_path(path: &str) -> AgentPath {
    AgentPath::try_from(path).expect("valid agent path")
}

fn agent_metadata(thread_id: ThreadId) -> AgentMetadata {
    AgentMetadata {
        agent_id: Some(thread_id),
        ..Default::default()
    }
}

#[test]
fn format_agent_nickname_adds_ordinals_after_reset() {
    assert_eq!(
        format_agent_nickname("Plato", /*nickname_reset_count*/ 0),
        "Plato"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*nickname_reset_count*/ 1),
        "Plato the 2nd"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*nickname_reset_count*/ 2),
        "Plato the 3rd"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*nickname_reset_count*/ 10),
        "Plato the 11th"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*nickname_reset_count*/ 20),
        "Plato the 21st"
    );
}

#[test]
fn session_depth_defaults_to_zero_for_root_sources() {
    assert_eq!(session_depth(&SessionSource::Cli), 0);
}

#[test]
fn thread_spawn_depth_increments_and_enforces_limit() {
    let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: ThreadId::new(),
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });
    let child_depth = next_thread_spawn_depth(&session_source);
    assert_eq!(child_depth, 2);
    assert!(exceeds_thread_spawn_depth_limit(
        child_depth,
        /*max_depth*/ 1
    ));
}

#[test]
fn non_thread_spawn_subagents_default_to_depth_zero() {
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);
    assert_eq!(session_depth(&session_source), 0);
    assert_eq!(next_thread_spawn_depth(&session_source), 1);
    assert!(!exceeds_thread_spawn_depth_limit(
        /*depth*/ 1, /*max_depth*/ 1
    ));
}

#[test]
fn reservation_drop_releases_slot() {
    let registry = Arc::new(AgentRegistry::default());
    let reservation = registry.reserve_spawn_slot(Some(1)).expect("reserve slot");
    drop(reservation);

    let reservation = registry.reserve_spawn_slot(Some(1)).expect("slot released");
    drop(reservation);
}

#[test]
fn commit_holds_slot_until_release() {
    let registry = Arc::new(AgentRegistry::default());
    let reservation = registry.reserve_spawn_slot(Some(1)).expect("reserve slot");
    let thread_id = ThreadId::new();
    reservation.commit(agent_metadata(thread_id));

    let err = match registry.reserve_spawn_slot(Some(1)) {
        Ok(_) => panic!("limit should be enforced"),
        Err(err) => err,
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    registry.release_spawned_thread(thread_id);
    let reservation = registry
        .reserve_spawn_slot(Some(1))
        .expect("slot released after thread removal");
    drop(reservation);
}

#[test]
fn release_ignores_unknown_thread_id() {
    let registry = Arc::new(AgentRegistry::default());
    let reservation = registry.reserve_spawn_slot(Some(1)).expect("reserve slot");
    let thread_id = ThreadId::new();
    reservation.commit(agent_metadata(thread_id));

    registry.release_spawned_thread(ThreadId::new());

    let err = match registry.reserve_spawn_slot(Some(1)) {
        Ok(_) => panic!("limit should still be enforced"),
        Err(err) => err,
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    registry.release_spawned_thread(thread_id);
    let reservation = registry
        .reserve_spawn_slot(Some(1))
        .expect("slot released after real thread removal");
    drop(reservation);
}

#[test]
fn release_is_idempotent_for_registered_threads() {
    let registry = Arc::new(AgentRegistry::default());
    let reservation = registry.reserve_spawn_slot(Some(1)).expect("reserve slot");
    let first_id = ThreadId::new();
    reservation.commit(agent_metadata(first_id));

    registry.release_spawned_thread(first_id);

    let reservation = registry.reserve_spawn_slot(Some(1)).expect("slot reused");
    let second_id = ThreadId::new();
    reservation.commit(agent_metadata(second_id));

    registry.release_spawned_thread(first_id);

    let err = match registry.reserve_spawn_slot(Some(1)) {
        Ok(_) => panic!("limit should still be enforced"),
        Err(err) => err,
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    registry.release_spawned_thread(second_id);
    let reservation = registry
        .reserve_spawn_slot(Some(1))
        .expect("slot released after second thread removal");
    drop(reservation);
}

#[test]
fn failed_spawn_keeps_nickname_marked_used() {
    let registry = Arc::new(AgentRegistry::default());
    let mut reservation = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve slot");
    let agent_nickname = reservation
        .reserve_agent_nickname_with_preference(&["alpha"], /*preferred*/ None)
        .expect("reserve agent name");
    assert_eq!(agent_nickname, "alpha");
    drop(reservation);

    let mut reservation = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve slot");
    let agent_nickname = reservation
        .reserve_agent_nickname_with_preference(&["alpha", "beta"], /*preferred*/ None)
        .expect("unused name should still be preferred");
    assert_eq!(agent_nickname, "beta");
}

#[test]
fn agent_nickname_resets_used_pool_when_exhausted() {
    let registry = Arc::new(AgentRegistry::default());
    let mut first = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve first slot");
    let first_name = first
        .reserve_agent_nickname_with_preference(&["alpha"], /*preferred*/ None)
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    first.commit(agent_metadata(first_id));
    assert_eq!(first_name, "alpha");

    let mut second = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve second slot");
    let second_name = second
        .reserve_agent_nickname_with_preference(&["alpha"], /*preferred*/ None)
        .expect("name should be reused after pool reset");
    assert_eq!(second_name, "alpha the 2nd");
    let active_agents = registry
        .active_agents
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(active_agents.nickname_reset_count, 1);
}

#[test]
fn released_nickname_stays_used_until_pool_reset() {
    let registry = Arc::new(AgentRegistry::default());

    let mut first = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve first slot");
    let first_name = first
        .reserve_agent_nickname_with_preference(&["alpha"], /*preferred*/ None)
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    first.commit(agent_metadata(first_id));
    assert_eq!(first_name, "alpha");

    registry.release_spawned_thread(first_id);

    let mut second = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve second slot");
    let second_name = second
        .reserve_agent_nickname_with_preference(&["alpha", "beta"], /*preferred*/ None)
        .expect("released name should still be marked used");
    assert_eq!(second_name, "beta");
    let second_id = ThreadId::new();
    second.commit(agent_metadata(second_id));
    registry.release_spawned_thread(second_id);

    let mut third = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve third slot");
    let third_name = third
        .reserve_agent_nickname_with_preference(&["alpha", "beta"], /*preferred*/ None)
        .expect("pool reset should permit a duplicate");
    let expected_names = HashSet::from(["alpha the 2nd".to_string(), "beta the 2nd".to_string()]);
    assert!(expected_names.contains(&third_name));
    let active_agents = registry
        .active_agents
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(active_agents.nickname_reset_count, 1);
}

#[test]
fn repeated_resets_advance_the_ordinal_suffix() {
    let registry = Arc::new(AgentRegistry::default());

    let mut first = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve first slot");
    let first_name = first
        .reserve_agent_nickname_with_preference(&["Plato"], /*preferred*/ None)
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    first.commit(agent_metadata(first_id));
    assert_eq!(first_name, "Plato");
    registry.release_spawned_thread(first_id);

    let mut second = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve second slot");
    let second_name = second
        .reserve_agent_nickname_with_preference(&["Plato"], /*preferred*/ None)
        .expect("reserve second agent name");
    let second_id = ThreadId::new();
    second.commit(agent_metadata(second_id));
    assert_eq!(second_name, "Plato the 2nd");
    registry.release_spawned_thread(second_id);

    let mut third = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve third slot");
    let third_name = third
        .reserve_agent_nickname_with_preference(&["Plato"], /*preferred*/ None)
        .expect("reserve third agent name");
    assert_eq!(third_name, "Plato the 3rd");
    let active_agents = registry
        .active_agents
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(active_agents.nickname_reset_count, 2);
}

#[test]
fn register_root_thread_indexes_root_path() {
    let registry = Arc::new(AgentRegistry::default());
    let root_thread_id = ThreadId::new();

    registry.register_root_thread(root_thread_id);

    assert_eq!(
        registry.agent_id_for_path(&AgentPath::root()),
        Some(root_thread_id)
    );
}

#[test]
fn reserved_agent_path_is_released_when_spawn_fails() {
    let registry = Arc::new(AgentRegistry::default());
    let mut first = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve first slot");
    first
        .reserve_agent_path(&agent_path("/root/researcher"))
        .expect("reserve first path");
    drop(first);

    let mut second = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve second slot");
    second
        .reserve_agent_path(&agent_path("/root/researcher"))
        .expect("dropped reservation should free the path");
}

#[test]
fn committed_agent_path_is_indexed_until_release() {
    let registry = Arc::new(AgentRegistry::default());
    let thread_id = ThreadId::new();
    let mut reservation = registry
        .reserve_spawn_slot(/*max_threads*/ None)
        .expect("reserve slot");
    reservation
        .reserve_agent_path(&agent_path("/root/researcher"))
        .expect("reserve path");
    reservation.commit(AgentMetadata {
        agent_id: Some(thread_id),
        agent_path: Some(agent_path("/root/researcher")),
        ..Default::default()
    });

    assert_eq!(
        registry.agent_id_for_path(&agent_path("/root/researcher")),
        Some(thread_id)
    );

    registry.release_spawned_thread(thread_id);
    assert_eq!(
        registry.agent_id_for_path(&agent_path("/root/researcher")),
        None
    );
}
