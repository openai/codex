use std::time::Duration;
use std::time::Instant;

use codex_analytics::CompletedCoreThreadInitialization;
use codex_analytics::ThreadInitializationMode;
use codex_analytics::ThreadInitializationProfile;
use pretty_assertions::assert_eq;

use super::ThreadInitializationPhase;
use super::ThreadInitializationTiming;

#[test]
fn thread_initialization_profile_breaks_down_core_time() {
    let core_started_at = Instant::now();
    let mut timing = ThreadInitializationTiming::new_at(
        core_started_at,
        ThreadInitializationMode::Cleared,
        ThreadInitializationPhase::ConfigurationResolution,
    );
    for (elapsed_ms, phase) in [
        (10, ThreadInitializationPhase::ExistingThreadLookup),
        (15, ThreadInitializationPhase::ConfigurationResolution),
        (30, ThreadInitializationPhase::SessionDependencyLoading),
        (50, ThreadInitializationPhase::SessionConstruction),
        (80, ThreadInitializationPhase::McpStartup),
        (120, ThreadInitializationPhase::SessionActivation),
        (170, ThreadInitializationPhase::ThreadRegistration),
    ] {
        timing.transition_at(core_started_at + Duration::from_millis(elapsed_ms), phase);
    }
    assert_eq!(
        timing.complete_at(core_started_at + Duration::from_millis(230)),
        CompletedCoreThreadInitialization {
            initialization_mode: ThreadInitializationMode::Cleared,
            profile: ThreadInitializationProfile {
                core_duration_ms: 230,
                existing_thread_lookup_ms: 5,
                configuration_resolution_ms: 25,
                session_dependency_loading_ms: 20,
                session_construction_ms: 30,
                mcp_startup_ms: 40,
                session_activation_ms: 50,
                thread_registration_ms: 60,
                ..Default::default()
            },
        }
    );
}

#[test]
fn loaded_resume_records_only_existing_thread_lookup() {
    let core_started_at = Instant::now();
    let timing = ThreadInitializationTiming::new_at(
        core_started_at,
        ThreadInitializationMode::Resumed,
        ThreadInitializationPhase::ExistingThreadLookup,
    );

    assert_eq!(
        timing.complete_at(core_started_at + Duration::from_millis(17)),
        CompletedCoreThreadInitialization {
            initialization_mode: ThreadInitializationMode::Resumed,
            profile: ThreadInitializationProfile {
                core_duration_ms: 17,
                existing_thread_lookup_ms: 17,
                ..Default::default()
            },
        }
    );
}
