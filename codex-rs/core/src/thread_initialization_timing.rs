use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::time::Duration;
use std::time::Instant;

use codex_analytics::CompletedThreadInitialization;
use codex_analytics::ThreadInitializationMode;
use codex_analytics::ThreadInitializationProfile;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionDependencyBranch {
    ThreadPersistence,
    StateDbLoading,
    AuthAndMcpDiscovery,
    PluginAndSkillWarmup,
}

tokio::task_local! {
    static ACTIVE_THREAD_INITIALIZATION_TIMING: ThreadInitializationTiming;
}

/// Request-scoped monotonic timing for app-server thread initialization.
#[derive(Clone, Debug)]
pub struct ThreadInitializationTiming(Arc<Mutex<ThreadInitializationTimingState>>);

#[derive(Debug)]
struct ThreadInitializationTimingState {
    request_started_at: Instant,
    last_transition_at: Option<Instant>,
    active_phase: Option<ThreadInitializationPhase>,
    initialization_mode: Option<ThreadInitializationMode>,
    profile: ThreadInitializationProfile,
    core_complete: bool,
}

#[must_use]
pub(crate) struct SessionDependencyTimingGuard {
    timing: ThreadInitializationTiming,
    branch: SessionDependencyBranch,
    started_at: Instant,
}

impl ThreadInitializationTiming {
    pub fn begin_request() -> Self {
        Self::new_at(Instant::now())
    }

    pub async fn scope<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        ACTIVE_THREAD_INITIALIZATION_TIMING
            .scope(self.clone(), future)
            .await
    }

    pub fn resume_lookup_started() {
        Self::start_current(
            ThreadInitializationMode::Resumed,
            ThreadInitializationPhase::ExistingThreadLookup,
        );
    }

    pub fn core_completed() {
        Self::with_current(|timing| timing.complete_core_at(Instant::now()));
    }

    pub fn complete_request(&self) -> Option<CompletedThreadInitialization> {
        self.complete_request_at(Instant::now())
    }

    pub(crate) fn new_thread_started() {
        Self::start_configuration_resolution(ThreadInitializationMode::New);
    }

    pub(crate) fn cleared_thread_started() {
        Self::start_configuration_resolution(ThreadInitializationMode::Cleared);
    }

    pub(crate) fn forked_thread_started() {
        Self::start_configuration_resolution(ThreadInitializationMode::Forked);
    }

    pub(crate) fn resumed_thread_started() {
        Self::start_configuration_resolution(ThreadInitializationMode::Resumed);
    }

    pub(crate) fn configuration_resolution_started() {
        Self::transition_to(ThreadInitializationPhase::ConfigurationResolution);
    }

    pub(crate) fn session_dependency_loading_started() {
        Self::transition_to(ThreadInitializationPhase::SessionDependencyLoading);
    }

    pub(crate) fn session_construction_started() {
        Self::transition_to(ThreadInitializationPhase::SessionConstruction);
    }

    pub(crate) fn mcp_startup_started() {
        Self::transition_to(ThreadInitializationPhase::McpStartup);
    }

    pub(crate) fn session_activation_started() {
        Self::transition_to(ThreadInitializationPhase::SessionActivation);
    }

    pub(crate) fn thread_registration_started() {
        Self::transition_to(ThreadInitializationPhase::ThreadRegistration);
    }

    pub(crate) fn begin_thread_persistence() -> Option<SessionDependencyTimingGuard> {
        Self::begin_session_dependency(SessionDependencyBranch::ThreadPersistence)
    }

    pub(crate) fn begin_state_db_loading() -> Option<SessionDependencyTimingGuard> {
        Self::begin_session_dependency(SessionDependencyBranch::StateDbLoading)
    }

    pub(crate) fn begin_auth_and_mcp_discovery() -> Option<SessionDependencyTimingGuard> {
        Self::begin_session_dependency(SessionDependencyBranch::AuthAndMcpDiscovery)
    }

    pub(crate) fn begin_plugin_and_skill_warmup() -> Option<SessionDependencyTimingGuard> {
        Self::begin_session_dependency(SessionDependencyBranch::PluginAndSkillWarmup)
    }

    fn transition_to(phase: ThreadInitializationPhase) {
        Self::with_current(|timing| timing.transition_at(Instant::now(), phase));
    }

    fn start_configuration_resolution(mode: ThreadInitializationMode) {
        Self::start_current(mode, ThreadInitializationPhase::ConfigurationResolution);
    }

    fn begin_session_dependency(
        branch: SessionDependencyBranch,
    ) -> Option<SessionDependencyTimingGuard> {
        ACTIVE_THREAD_INITIALIZATION_TIMING
            .try_with(|timing| SessionDependencyTimingGuard {
                timing: timing.clone(),
                branch,
                started_at: Instant::now(),
            })
            .ok()
    }

    fn start_current(mode: ThreadInitializationMode, phase: ThreadInitializationPhase) {
        Self::with_current(|timing| timing.start_core_at(Instant::now(), mode, phase));
    }

    fn with_current(f: impl FnOnce(&Self)) {
        let _ = ACTIVE_THREAD_INITIALIZATION_TIMING.try_with(f);
    }

    fn new_at(request_started_at: Instant) -> Self {
        Self(Arc::new(Mutex::new(ThreadInitializationTimingState {
            request_started_at,
            last_transition_at: None,
            active_phase: None,
            initialization_mode: None,
            profile: ThreadInitializationProfile::default(),
            core_complete: false,
        })))
    }

    fn start_core_at(
        &self,
        now: Instant,
        mode: ThreadInitializationMode,
        phase: ThreadInitializationPhase,
    ) {
        let mut state = self.state();
        if state.last_transition_at.is_some() || state.core_complete {
            return;
        }
        state.last_transition_at = Some(now);
        state.active_phase = Some(phase);
        state.initialization_mode = Some(mode);
    }

    fn transition_at(&self, now: Instant, phase: ThreadInitializationPhase) {
        let mut state = self.state();
        if state.core_complete || state.last_transition_at.is_none() {
            return;
        }
        state.advance(now);
        state.active_phase = Some(phase);
    }

    fn complete_core_at(&self, now: Instant) {
        let mut state = self.state();
        if state.core_complete || state.last_transition_at.is_none() {
            return;
        }
        state.advance(now);
        state.profile.core_duration_ms = core_duration_ms(&state.profile);
        state.active_phase = None;
        state.core_complete = true;
    }

    fn complete_request_at(&self, now: Instant) -> Option<CompletedThreadInitialization> {
        let state = self.state();
        if !state.core_complete {
            return None;
        }
        let mut profile = state.profile;
        profile.duration_ms =
            duration_to_u64_ms(now.saturating_duration_since(state.request_started_at));
        profile.app_server_duration_ms =
            profile.duration_ms.saturating_sub(profile.core_duration_ms);
        Some(CompletedThreadInitialization {
            initialization_mode: state.initialization_mode?,
            profile,
        })
    }

    fn record_dependency(&self, branch: SessionDependencyBranch, duration: Duration) {
        let mut state = self.state();
        if state.core_complete || state.last_transition_at.is_none() {
            return;
        }
        let duration_ms = duration_to_u64_ms(duration);
        *dependency_duration_mut(&mut state.profile, branch) = duration_ms;
    }

    fn state(&self) -> MutexGuard<'_, ThreadInitializationTimingState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for SessionDependencyTimingGuard {
    fn drop(&mut self) {
        self.timing.record_dependency(
            self.branch,
            Instant::now().saturating_duration_since(self.started_at),
        );
    }
}

impl ThreadInitializationTimingState {
    fn advance(&mut self, now: Instant) {
        let (Some(previous), Some(phase)) =
            (self.last_transition_at.replace(now), self.active_phase)
        else {
            return;
        };
        let duration_ms = duration_to_u64_ms(now.saturating_duration_since(previous));
        let phase_duration = phase_duration_mut(&mut self.profile, phase);
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

fn dependency_duration_mut(
    profile: &mut ThreadInitializationProfile,
    branch: SessionDependencyBranch,
) -> &mut u64 {
    match branch {
        SessionDependencyBranch::ThreadPersistence => &mut profile.thread_persistence_ms,
        SessionDependencyBranch::StateDbLoading => &mut profile.state_db_loading_ms,
        SessionDependencyBranch::AuthAndMcpDiscovery => &mut profile.auth_and_mcp_discovery_ms,
        SessionDependencyBranch::PluginAndSkillWarmup => &mut profile.plugin_and_skill_warmup_ms,
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
