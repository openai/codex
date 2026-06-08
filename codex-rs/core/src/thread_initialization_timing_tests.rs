use std::time::Duration;
use std::time::Instant;

use codex_analytics::ThreadInitializationProfile;
use pretty_assertions::assert_eq;

use super::SessionDependencyBranch;
use super::ThreadInitializationPhase;
use super::ThreadInitializationTiming;

#[test]
fn thread_initialization_profile_breaks_down_core_phases() {
    let started_at = Instant::now();
    let timing = ThreadInitializationTiming::start_at(started_at);

    timing.transition_at(
        started_at + Duration::from_millis(10),
        ThreadInitializationPhase::ConfigurationResolution,
    );
    timing.transition_at(
        started_at + Duration::from_millis(30),
        ThreadInitializationPhase::SessionDependencyLoading,
    );
    timing.record_session_dependency(
        SessionDependencyBranch::ThreadPersistence,
        Duration::from_millis(12),
    );
    timing.record_session_dependency(
        SessionDependencyBranch::StateDbLoading,
        Duration::from_millis(8),
    );
    timing.record_session_dependency(
        SessionDependencyBranch::AuthAndMcpDiscovery,
        Duration::from_millis(15),
    );
    timing.record_session_dependency(
        SessionDependencyBranch::PluginAndSkillWarmup,
        Duration::from_millis(18),
    );
    timing.transition_at(
        started_at + Duration::from_millis(50),
        ThreadInitializationPhase::SessionConstruction,
    );
    timing.transition_at(
        started_at + Duration::from_millis(80),
        ThreadInitializationPhase::McpStartup,
    );
    timing.transition_at(
        started_at + Duration::from_millis(120),
        ThreadInitializationPhase::SessionActivation,
    );
    timing.transition_at(
        started_at + Duration::from_millis(170),
        ThreadInitializationPhase::ThreadRegistration,
    );

    assert_eq!(
        timing.complete_at(started_at + Duration::from_millis(230)),
        ThreadInitializationProfile {
            duration_ms: 230,
            app_server_duration_ms: 0,
            core_duration_ms: 230,
            existing_thread_lookup_ms: 10,
            configuration_resolution_ms: 20,
            session_dependency_loading_ms: 20,
            session_construction_ms: 30,
            mcp_startup_ms: 40,
            session_activation_ms: 50,
            thread_registration_ms: 60,
            thread_persistence_ms: 12,
            state_db_loading_ms: 8,
            auth_and_mcp_discovery_ms: 15,
            plugin_and_skill_warmup_ms: 18,
        }
    );
}

#[test]
fn loaded_resume_records_only_existing_thread_lookup() {
    let started_at = Instant::now();
    let timing = ThreadInitializationTiming::start_at(started_at);

    assert_eq!(
        timing.complete_at(started_at + Duration::from_millis(17)),
        ThreadInitializationProfile {
            duration_ms: 17,
            core_duration_ms: 17,
            existing_thread_lookup_ms: 17,
            ..Default::default()
        }
    );
}
