use super::*;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

#[test]
fn format_agent_nickname_adds_ordinals_after_reset() {
    assert_eq!(format_agent_nickname("Plato", 0), "Plato");
    assert_eq!(format_agent_nickname("Plato", 1), "Plato the 2nd");
    assert_eq!(format_agent_nickname("Plato", 2), "Plato the 3rd");
    assert_eq!(format_agent_nickname("Plato", 10), "Plato the 11th");
    assert_eq!(format_agent_nickname("Plato", 20), "Plato the 21st");
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
    assert_eq!(second_name, "alpha the 2nd");
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
    let expected_names = HashSet::from(["alpha the 2nd".to_string(), "beta the 2nd".to_string()]);
    assert!(expected_names.contains(&third_name));
    let active_agents = guards
        .active_agents
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(active_agents.nickname_reset_count, 1);
}

#[test]
fn repeated_resets_advance_the_ordinal_suffix() {
    let guards = Arc::new(Guards::default());

    let mut first = guards.reserve_spawn_slot(None).expect("reserve first slot");
    let first_name = first
        .reserve_agent_nickname(&["Plato"])
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    first.commit(first_id);
    assert_eq!(first_name, "Plato");
    guards.release_spawned_thread(first_id);

    let mut second = guards
        .reserve_spawn_slot(None)
        .expect("reserve second slot");
    let second_name = second
        .reserve_agent_nickname(&["Plato"])
        .expect("reserve second agent name");
    let second_id = ThreadId::new();
    second.commit(second_id);
    assert_eq!(second_name, "Plato the 2nd");
    guards.release_spawned_thread(second_id);

    let mut third = guards.reserve_spawn_slot(None).expect("reserve third slot");
    let third_name = third
        .reserve_agent_nickname(&["Plato"])
        .expect("reserve third agent name");
    assert_eq!(third_name, "Plato the 3rd");
    let active_agents = guards
        .active_agents
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(active_agents.nickname_reset_count, 2);
}
