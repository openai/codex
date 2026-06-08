use std::time::Duration;
use std::time::Instant;

use codex_analytics::ThreadInitializationFact;
use codex_analytics::ThreadInitializationMode;
use codex_analytics::ThreadInitializationProfile;
use pretty_assertions::assert_eq;

use super::SessionDependencyBranch;
use super::ThreadInitializationPhase;
use super::ThreadInitializationTiming;

#[test]
fn thread_initialization_profile_breaks_down_request_and_core_time() {
    let request_started_at = Instant::now();
    let core_started_at = request_started_at + Duration::from_millis(20);
    let timing = ThreadInitializationTiming::new_at(request_started_at);

    timing.start_core_at(
        core_started_at,
        ThreadInitializationMode::Cleared,
        ThreadInitializationPhase::ConfigurationResolution,
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(10),
        ThreadInitializationPhase::ExistingThreadLookup,
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(15),
        ThreadInitializationPhase::ConfigurationResolution,
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(30),
        ThreadInitializationPhase::SessionDependencyLoading,
    );
    timing.record_dependency(
        SessionDependencyBranch::ThreadPersistence,
        Duration::from_millis(12),
    );
    timing.record_dependency(
        SessionDependencyBranch::StateDbLoading,
        Duration::from_millis(8),
    );
    timing.record_dependency(
        SessionDependencyBranch::AuthAndMcpDiscovery,
        Duration::from_millis(15),
    );
    timing.record_dependency(
        SessionDependencyBranch::PluginAndSkillWarmup,
        Duration::from_millis(18),
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(50),
        ThreadInitializationPhase::SessionConstruction,
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(80),
        ThreadInitializationPhase::McpStartup,
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(120),
        ThreadInitializationPhase::SessionActivation,
    );
    timing.transition_at(
        core_started_at + Duration::from_millis(170),
        ThreadInitializationPhase::ThreadRegistration,
    );
    timing.complete_core_at(core_started_at + Duration::from_millis(230));
    assert_eq!(
        timing.complete_request_at(request_started_at + Duration::from_millis(275)),
        Some(ThreadInitializationFact {
            initialization_mode: ThreadInitializationMode::Cleared,
            profile: ThreadInitializationProfile {
                duration_ms: 275,
                app_server_duration_ms: 45,
                core_duration_ms: 230,
                existing_thread_lookup_ms: 5,
                configuration_resolution_ms: 25,
                session_dependency_loading_ms: 20,
                session_construction_ms: 30,
                mcp_startup_ms: 40,
                session_activation_ms: 50,
                thread_registration_ms: 60,
                thread_persistence_ms: 12,
                state_db_loading_ms: 8,
                auth_and_mcp_discovery_ms: 15,
                plugin_and_skill_warmup_ms: 18,
            },
        })
    );
}

#[test]
fn loaded_resume_records_only_existing_thread_lookup() {
    let request_started_at = Instant::now();
    let core_started_at = request_started_at + Duration::from_millis(3);
    let timing = ThreadInitializationTiming::new_at(request_started_at);
    timing.start_core_at(
        core_started_at,
        ThreadInitializationMode::Resumed,
        ThreadInitializationPhase::ExistingThreadLookup,
    );
    timing.complete_core_at(core_started_at + Duration::from_millis(17));

    assert_eq!(
        timing.complete_request_at(request_started_at + Duration::from_millis(25)),
        Some(ThreadInitializationFact {
            initialization_mode: ThreadInitializationMode::Resumed,
            profile: ThreadInitializationProfile {
                duration_ms: 25,
                app_server_duration_ms: 8,
                core_duration_ms: 17,
                existing_thread_lookup_ms: 17,
                ..Default::default()
            },
        })
    );
}
