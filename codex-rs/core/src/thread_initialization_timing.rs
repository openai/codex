use std::time::Duration;
use std::time::Instant;

use codex_analytics::CompletedCoreThreadInitialization;
use codex_analytics::ThreadInitializationMode;
use codex_analytics::ThreadInitializationProfile;

use codex_protocol::protocol::InitialHistory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThreadInitializationPhase {
    ExistingThreadLookup,
    ConfigurationResolution,
    SessionDependencyLoading,
    SessionConstruction,
    McpStartup,
    SessionActivation,
    ThreadRegistration,
}

/// Monotonic timing state owned by one thread initialization operation.
#[derive(Debug)]
pub struct ThreadInitializationTiming {
    last_transition_at: Instant,
    active_phase: ThreadInitializationPhase,
    initialization_mode: ThreadInitializationMode,
    profile: ThreadInitializationProfile,
}

impl ThreadInitializationTiming {
    pub(crate) fn begin(initial_history: &InitialHistory) -> Self {
        let (initialization_mode, phase) = match initial_history {
            InitialHistory::New => (
                ThreadInitializationMode::New,
                ThreadInitializationPhase::ConfigurationResolution,
            ),
            InitialHistory::Cleared => (
                ThreadInitializationMode::Cleared,
                ThreadInitializationPhase::ConfigurationResolution,
            ),
            InitialHistory::Forked(_) => (
                ThreadInitializationMode::Forked,
                ThreadInitializationPhase::ConfigurationResolution,
            ),
            InitialHistory::Resumed(_) => (
                ThreadInitializationMode::Resumed,
                ThreadInitializationPhase::ExistingThreadLookup,
            ),
        };
        Self::new_at(Instant::now(), initialization_mode, phase)
    }

    pub fn begin_resumed_lookup() -> Self {
        Self::new_at(
            Instant::now(),
            ThreadInitializationMode::Resumed,
            ThreadInitializationPhase::ExistingThreadLookup,
        )
    }

    pub(crate) fn begin_forked() -> Self {
        Self::new_at(
            Instant::now(),
            ThreadInitializationMode::Forked,
            ThreadInitializationPhase::ConfigurationResolution,
        )
    }

    pub(crate) fn existing_thread_lookup_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::ExistingThreadLookup);
    }

    pub(crate) fn configuration_resolution_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::ConfigurationResolution);
    }

    pub(crate) fn session_dependency_loading_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::SessionDependencyLoading);
    }

    pub(crate) fn session_construction_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::SessionConstruction);
    }

    pub(crate) fn mcp_startup_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::McpStartup);
    }

    pub(crate) fn session_activation_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::SessionActivation);
    }

    pub(crate) fn thread_registration_started(&mut self) {
        self.transition_to(ThreadInitializationPhase::ThreadRegistration);
    }

    pub fn complete(self) -> CompletedCoreThreadInitialization {
        self.complete_at(Instant::now())
    }

    fn transition_to(&mut self, phase: ThreadInitializationPhase) {
        self.transition_at(Instant::now(), phase);
    }

    fn new_at(
        started_at: Instant,
        initialization_mode: ThreadInitializationMode,
        active_phase: ThreadInitializationPhase,
    ) -> Self {
        Self {
            last_transition_at: started_at,
            active_phase,
            initialization_mode,
            profile: ThreadInitializationProfile::default(),
        }
    }

    fn transition_at(&mut self, now: Instant, phase: ThreadInitializationPhase) {
        self.advance(now);
        self.active_phase = phase;
    }

    fn complete_at(mut self, now: Instant) -> CompletedCoreThreadInitialization {
        self.advance(now);
        self.profile.core_duration_ms = core_duration_ms(&self.profile);
        CompletedCoreThreadInitialization {
            initialization_mode: self.initialization_mode,
            profile: self.profile,
        }
    }

    fn advance(&mut self, now: Instant) {
        let duration_ms =
            duration_to_u64_ms(now.saturating_duration_since(self.last_transition_at));
        self.last_transition_at = now;
        let phase_duration = phase_duration_mut(&mut self.profile, self.active_phase);
        *phase_duration = phase_duration.saturating_add(duration_ms);
    }
}

fn phase_duration_mut(
    profile: &mut ThreadInitializationProfile,
    phase: ThreadInitializationPhase,
) -> &mut u64 {
    match phase {
        ThreadInitializationPhase::ExistingThreadLookup => &mut profile.existing_thread_lookup_ms,
        ThreadInitializationPhase::ConfigurationResolution => {
            &mut profile.configuration_resolution_ms
        }
        ThreadInitializationPhase::SessionDependencyLoading => {
            &mut profile.session_dependency_loading_ms
        }
        ThreadInitializationPhase::SessionConstruction => &mut profile.session_construction_ms,
        ThreadInitializationPhase::McpStartup => &mut profile.mcp_startup_ms,
        ThreadInitializationPhase::SessionActivation => &mut profile.session_activation_ms,
        ThreadInitializationPhase::ThreadRegistration => &mut profile.thread_registration_ms,
    }
}

fn core_duration_ms(profile: &ThreadInitializationProfile) -> u64 {
    profile
        .existing_thread_lookup_ms
        .saturating_add(profile.configuration_resolution_ms)
        .saturating_add(profile.session_dependency_loading_ms)
        .saturating_add(profile.session_construction_ms)
        .saturating_add(profile.mcp_startup_ms)
        .saturating_add(profile.session_activation_ms)
        .saturating_add(profile.thread_registration_ms)
}

fn duration_to_u64_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "thread_initialization_timing_tests.rs"]
mod tests;
