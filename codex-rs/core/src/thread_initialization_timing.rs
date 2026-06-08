use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_analytics::ThreadInitializationProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum ThreadInitializationPhase {
    ExistingThreadLookup,
    ConfigurationResolution,
    SessionDependencyLoading,
    SessionConstruction,
    McpStartup,
    SessionActivation,
    ThreadRegistration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum SessionDependencyBranch {
    ThreadPersistence,
    StateDbLoading,
    AuthAndMcpDiscovery,
    PluginAndSkillWarmup,
}

const PHASE_COUNT: usize = 7;
const DEPENDENCY_BRANCH_COUNT: usize = 4;

/// Monotonic timing state for a thread initialization operation.
#[derive(Clone, Debug)]
pub struct ThreadInitializationTiming {
    state: Arc<Mutex<ThreadInitializationTimingState>>,
}

#[derive(Debug)]
struct ThreadInitializationTimingState {
    started_at: Instant,
    last_transition_at: Instant,
    active_phase: ThreadInitializationPhase,
    phase_durations: [Duration; PHASE_COUNT],
    dependency_branch_durations: [Duration; DEPENDENCY_BRANCH_COUNT],
    completed_profile: Option<ThreadInitializationProfile>,
}

#[must_use]
pub(crate) struct SessionDependencyTimingGuard {
    timing: ThreadInitializationTiming,
    branch: SessionDependencyBranch,
    started_at: Instant,
}

impl ThreadInitializationTiming {
    /// Starts timing in the existing-thread lookup phase at the current monotonic instant.
    pub fn start() -> Self {
        Self::start_at(Instant::now())
    }

    pub(crate) fn start_configuration_resolution() -> Self {
        let timing = Self::start();
        timing.transition_to(ThreadInitializationPhase::ConfigurationResolution);
        timing
    }

    /// Completes a lookup-only initialization, such as resuming an already loaded thread.
    pub fn complete_existing_thread_lookup(&self) -> ThreadInitializationProfile {
        self.complete_at(Instant::now())
    }

    pub(crate) fn transition_to(&self, phase: ThreadInitializationPhase) {
        self.transition_at(Instant::now(), phase);
    }

    pub(crate) fn begin_session_dependency(
        &self,
        branch: SessionDependencyBranch,
    ) -> SessionDependencyTimingGuard {
        SessionDependencyTimingGuard {
            timing: self.clone(),
            branch,
            started_at: Instant::now(),
        }
    }

    pub(crate) fn complete(&self) -> ThreadInitializationProfile {
        self.complete_at(Instant::now())
    }

    fn start_at(started_at: Instant) -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreadInitializationTimingState {
                started_at,
                last_transition_at: started_at,
                active_phase: ThreadInitializationPhase::ExistingThreadLookup,
                phase_durations: [Duration::ZERO; PHASE_COUNT],
                dependency_branch_durations: [Duration::ZERO; DEPENDENCY_BRANCH_COUNT],
                completed_profile: None,
            })),
        }
    }

    fn transition_at(&self, now: Instant, phase: ThreadInitializationPhase) {
        let mut state = self.state();
        if state.completed_profile.is_some() {
            return;
        }
        state.advance(now);
        state.active_phase = phase;
    }

    fn record_session_dependency(&self, branch: SessionDependencyBranch, duration: Duration) {
        let mut state = self.state();
        if state.completed_profile.is_some() {
            return;
        }
        let branch_duration = &mut state.dependency_branch_durations[branch as usize];
        *branch_duration = branch_duration.saturating_add(duration);
    }

    fn complete_at(&self, now: Instant) -> ThreadInitializationProfile {
        let mut state = self.state();
        state.complete(now)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ThreadInitializationTimingState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Drop for SessionDependencyTimingGuard {
    fn drop(&mut self) {
        self.timing.record_session_dependency(
            self.branch,
            Instant::now().saturating_duration_since(self.started_at),
        );
    }
}

impl ThreadInitializationTimingState {
    fn advance(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_transition_at);
        self.last_transition_at = now;
        let phase_duration = &mut self.phase_durations[self.active_phase as usize];
        *phase_duration = phase_duration.saturating_add(elapsed);
    }

    fn complete(&mut self, now: Instant) -> ThreadInitializationProfile {
        if let Some(profile) = self.completed_profile.as_ref() {
            return *profile;
        }

        self.advance(now);
        let mut phase_durations_ms = self.phase_durations.map(duration_to_u64_ms);
        let elapsed_ms = duration_to_u64_ms(now.saturating_duration_since(self.started_at));
        let classified_ms = phase_durations_ms
            .iter()
            .copied()
            .fold(0_u64, u64::saturating_add);
        let active_phase_ms = &mut phase_durations_ms[self.active_phase as usize];
        *active_phase_ms = active_phase_ms.saturating_add(elapsed_ms.saturating_sub(classified_ms));
        let dependency_branch_durations_ms =
            self.dependency_branch_durations.map(duration_to_u64_ms);
        let profile = profile_from_durations(phase_durations_ms, dependency_branch_durations_ms);
        self.completed_profile = Some(profile);
        profile
    }
}

fn profile_from_durations(
    phase_durations_ms: [u64; PHASE_COUNT],
    dependency_branch_durations_ms: [u64; DEPENDENCY_BRANCH_COUNT],
) -> ThreadInitializationProfile {
    let [
        existing_thread_lookup_ms,
        configuration_resolution_ms,
        session_dependency_loading_ms,
        session_construction_ms,
        mcp_startup_ms,
        session_activation_ms,
        thread_registration_ms,
    ] = phase_durations_ms;
    let [
        thread_persistence_ms,
        state_db_loading_ms,
        auth_and_mcp_discovery_ms,
        plugin_and_skill_warmup_ms,
    ] = dependency_branch_durations_ms;
    let core_duration_ms = phase_durations_ms
        .iter()
        .copied()
        .fold(0_u64, u64::saturating_add);
    ThreadInitializationProfile {
        duration_ms: core_duration_ms,
        core_duration_ms,
        existing_thread_lookup_ms,
        configuration_resolution_ms,
        session_dependency_loading_ms,
        session_construction_ms,
        mcp_startup_ms,
        session_activation_ms,
        thread_registration_ms,
        thread_persistence_ms,
        state_db_loading_ms,
        auth_and_mcp_discovery_ms,
        plugin_and_skill_warmup_ms,
        ..Default::default()
    }
}

fn duration_to_u64_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "thread_initialization_timing_tests.rs"]
mod tests;
