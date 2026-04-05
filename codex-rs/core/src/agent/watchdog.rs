use super::control::AgentControl;
use super::registry::AgentRegistry;
use super::registry::exceeds_thread_spawn_depth_limit;
use super::status::is_final;
use crate::codex::load_watchdog_prompt;
use crate::config::Config;
use crate::thread_manager::ThreadManagerState;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio::time::Instant;
use tracing::info;
use tracing::warn;

const WATCHDOG_TICK_SECONDS: i64 = 1;

#[derive(Clone)]
pub(crate) struct WatchdogRegistration {
    pub(crate) owner_thread_id: ThreadId,
    pub(crate) target_thread_id: ThreadId,
    pub(crate) child_depth: i32,
    pub(crate) interval_s: i64,
    pub(crate) prompt: String,
    pub(crate) config: Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RemovedWatchdog {
    pub(crate) target_thread_id: ThreadId,
    pub(crate) active_helper_id: Option<ThreadId>,
}

struct WatchdogEntry {
    registration: WatchdogRegistration,
    interval: Duration,
    last_trigger: Instant,
    active_helper_id: Option<ThreadId>,
    owner_idle_since: Option<Instant>,
    owner_was_running: bool,
    force_due_once: bool,
    generation: i64,
}

pub(crate) struct WatchdogManager {
    manager: Weak<ThreadManagerState>,
    state: Arc<AgentRegistry>,
    registrations: Mutex<HashMap<ThreadId, WatchdogEntry>>,
    started: AtomicBool,
    next_generation: AtomicI64,
}

impl WatchdogManager {
    pub(crate) fn new(manager: Weak<ThreadManagerState>, state: Arc<AgentRegistry>) -> Arc<Self> {
        Arc::new(Self {
            manager,
            state,
            registrations: Mutex::new(HashMap::new()),
            started: AtomicBool::new(false),
            next_generation: AtomicI64::new(1),
        })
    }

    pub(crate) fn start(self: &Arc<Self>) {
        if self
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.run_loop().await;
        });
    }

    pub(crate) async fn register(
        self: &Arc<Self>,
        registration: WatchdogRegistration,
    ) -> CodexResult<Vec<RemovedWatchdog>> {
        if exceeds_thread_spawn_depth_limit(
            registration.child_depth,
            registration.config.agent_max_depth,
        ) {
            let max_depth = registration.config.agent_max_depth;
            return Err(CodexErr::UnsupportedOperation(format!(
                "agent depth limit reached: max depth is {max_depth}"
            )));
        }
        let interval = interval_duration(registration.interval_s)?;
        let generation = self.next_generation.fetch_add(1, Ordering::AcqRel);
        let now = Instant::now();
        let entry = WatchdogEntry {
            registration,
            interval,
            last_trigger: now,
            active_helper_id: None,
            owner_idle_since: Some(now),
            owner_was_running: false,
            force_due_once: false,
            generation,
        };

        let mut registrations = self.registrations.lock().await;
        let superseded_targets: Vec<ThreadId> = registrations
            .iter()
            .filter_map(|(target_thread_id, existing_entry)| {
                (existing_entry.registration.owner_thread_id == entry.registration.owner_thread_id
                    && *target_thread_id != entry.registration.target_thread_id)
                    .then_some(*target_thread_id)
            })
            .collect();
        let mut superseded = Vec::new();
        for superseded_target in superseded_targets {
            if let Some(removed) = registrations.remove(&superseded_target) {
                superseded.push(RemovedWatchdog {
                    target_thread_id: superseded_target,
                    active_helper_id: removed.active_helper_id,
                });
            }
        }
        registrations.insert(entry.registration.target_thread_id, entry);
        Ok(superseded)
    }

    async fn run_loop(self: Arc<Self>) {
        let tick = tick_duration();
        loop {
            self.run_once().await;
            if self.manager.upgrade().is_none() {
                break;
            }
            tokio::time::sleep(tick).await;
        }
    }

    pub(crate) async fn run_once(self: &Arc<Self>) {
        let Some(manager_state) = self.manager.upgrade() else {
            self.registrations.lock().await.clear();
            return;
        };

        let snapshots: Vec<(ThreadId, i64)> = {
            let registrations = self.registrations.lock().await;
            registrations
                .iter()
                .map(|(target_id, entry)| (*target_id, entry.generation))
                .collect()
        };
        let now = Instant::now();

        for (target_id, generation) in snapshots {
            self.evaluate(&manager_state, target_id, generation, now)
                .await;
        }
    }

    async fn evaluate(
        self: &Arc<Self>,
        manager_state: &Arc<ThreadManagerState>,
        target_thread_id: ThreadId,
        generation: i64,
        now: Instant,
    ) {
        let Some(snapshot) = self.snapshot(target_thread_id, generation).await else {
            return;
        };

        let owner_thread = manager_state.get_thread(snapshot.owner_thread_id).await;
        let owner_status = match owner_thread.as_ref() {
            Ok(thread) => thread.agent_status().await,
            Err(_) => AgentStatus::NotFound,
        };
        let control_for_spawn = AgentControl::from_parts(
            self.manager.clone(),
            Arc::clone(&self.state),
            Arc::clone(self),
        );
        if is_watchdog_terminated(&owner_status) {
            match control_for_spawn.shutdown_agent(target_thread_id).await {
                Ok(_) | Err(CodexErr::ThreadNotFound(_)) | Err(CodexErr::InternalAgentDied) => {}
                Err(err) => {
                    warn!(
                        owner_thread_id = %snapshot.owner_thread_id,
                        target_thread_id = %target_thread_id,
                        "watchdog owner termination cleanup failed: {err}"
                    );
                }
            }
            return;
        }
        let force_due = self
            .take_force_due_if_generation(target_thread_id, generation)
            .await;
        let owner_has_active_turn = match owner_thread {
            Ok(thread) => thread.has_active_turn().await,
            Err(_) => false,
        };
        let owner_running = (is_running(&owner_status) || owner_has_active_turn) && !force_due;
        let owner_idle_since = self
            .update_owner_idle_state_if_generation(
                target_thread_id,
                generation,
                owner_running,
                now,
                force_due,
            )
            .await;
        if owner_running {
            return;
        }
        let owner_idle_since = owner_idle_since.or(snapshot.owner_idle_since);
        let Some(owner_idle_since) = owner_idle_since else {
            return;
        };
        if now.duration_since(owner_idle_since) < snapshot.interval {
            return;
        }

        if let Some(helper_id) = snapshot.active_helper_id {
            let helper_status = get_status(manager_state, helper_id).await;
            if !is_final(&helper_status) {
                return;
            }

            let helper_sent_input = manager_state
                .get_thread(helper_id)
                .await
                .map(|thread| thread.last_completed_turn_used_agent_send_input())
                .unwrap_or(false);
            // A watchdog helper should wake the owner through `send_input` or a
            // final assistant report. If neither exists, emit nothing here and
            // let the next scheduled check-in try again.
            //
            // Preferred path: the helper explicitly calls `send_input`.
            // Fallback: if the helper reaches a terminal state without using
            // `send_input`, forward only a real final assistant message. If
            // there is no report body, emit nothing and schedule the next
            // helper normally.
            if !helper_sent_input {
                let fallback_message = match &helper_status {
                    AgentStatus::Completed(Some(message)) if !message.trim().is_empty() => {
                        Some(message.clone())
                    }
                    AgentStatus::Completed(_)
                    | AgentStatus::Errored(_)
                    | AgentStatus::Interrupted
                    | AgentStatus::Shutdown
                    | AgentStatus::NotFound
                    | AgentStatus::PendingInit
                    | AgentStatus::Running => None,
                };

                if let Some(message) = fallback_message {
                    if let Err(err) = control_for_spawn
                        .send_watchdog_wakeup(snapshot.owner_thread_id, helper_id, message)
                        .await
                    {
                        warn!(
                            helper_id = %helper_id,
                            owner_thread_id = %snapshot.owner_thread_id,
                            "watchdog helper forward failed: {err}"
                        );
                    } else {
                        info!(
                            helper_id = %helper_id,
                            owner_thread_id = %snapshot.owner_thread_id,
                            "watchdog forwarded helper completion to owner"
                        );
                    }
                }
            }
            if let Err(err) = control_for_spawn.shutdown_agent(helper_id).await {
                warn!(
                    helper_id = %helper_id,
                    owner_thread_id = %snapshot.owner_thread_id,
                    "watchdog helper cleanup failed: {err}"
                );
            }
            self.update_after_spawn(
                target_thread_id,
                generation,
                now,
                /*active_helper_id*/ None,
            )
            .await;
            return;
        }

        if now.duration_since(snapshot.last_trigger) < snapshot.interval {
            return;
        }

        let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: snapshot.owner_thread_id,
            depth: snapshot.child_depth,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        });
        let mut helper_config = snapshot.config.clone();
        helper_config.ephemeral = true;
        if helper_config
            .features
            .enabled(Feature::AgentPromptInjection)
        {
            let watchdog_prompt = load_watchdog_prompt(&helper_config.codex_home).await;
            helper_config.developer_instructions = match (
                watchdog_prompt.trim().is_empty(),
                helper_config.developer_instructions,
            ) {
                (true, existing) => existing,
                (false, Some(existing)) if !existing.trim().is_empty() => {
                    Some(format!("{existing}\n\n{watchdog_prompt}"))
                }
                (false, _) => Some(watchdog_prompt),
            };
        }
        let helper_prompt =
            watchdog_helper_prompt(&helper_config, snapshot.owner_thread_id, &snapshot.prompt)
                .await;
        // Watchdog check-ins must fork a distinct helper thread. If this path ever resumes
        // the owner thread instead, the owner can self-wake and rapidly duplicate session
        // state in memory.
        let spawn_result = control_for_spawn
            .fork_agent(
                helper_config,
                vec![UserInput::Text {
                    text: helper_prompt,
                    text_elements: Vec::new(),
                }],
                snapshot.owner_thread_id,
                usize::MAX,
                session_source,
            )
            .await;

        match spawn_result {
            Ok(helper_id) => {
                info!("watchdog spawned helper {helper_id} for target {target_thread_id}");
                self.update_after_spawn(target_thread_id, generation, now, Some(helper_id))
                    .await;
            }
            Err(err) => {
                warn!("watchdog spawn failed for target {target_thread_id}: {err}");
                self.update_after_spawn(
                    target_thread_id,
                    generation,
                    now,
                    /*active_helper_id*/ None,
                )
                .await;
            }
        }
    }

    async fn snapshot(
        &self,
        target_thread_id: ThreadId,
        generation: i64,
    ) -> Option<WatchdogSnapshot> {
        let registrations = self.registrations.lock().await;
        let entry = registrations.get(&target_thread_id)?;
        if entry.generation != generation {
            return None;
        }
        Some(WatchdogSnapshot {
            owner_thread_id: entry.registration.owner_thread_id,
            child_depth: entry.registration.child_depth,
            prompt: entry.registration.prompt.clone(),
            config: entry.registration.config.clone(),
            interval: entry.interval,
            last_trigger: entry.last_trigger,
            active_helper_id: entry.active_helper_id,
            owner_idle_since: entry.owner_idle_since,
        })
    }

    async fn update_owner_idle_state_if_generation(
        &self,
        target_thread_id: ThreadId,
        generation: i64,
        owner_running: bool,
        now: Instant,
        force_due: bool,
    ) -> Option<Instant> {
        let mut registrations = self.registrations.lock().await;
        let entry = registrations.get_mut(&target_thread_id)?;
        if entry.generation != generation {
            return None;
        }

        if force_due {
            return entry.owner_idle_since;
        }

        if owner_running {
            entry.owner_idle_since = None;
            entry.owner_was_running = true;
            return None;
        }

        if entry.owner_was_running || entry.owner_idle_since.is_none() {
            entry.owner_idle_since = Some(now);
        }
        entry.owner_was_running = false;
        entry.owner_idle_since
    }

    async fn take_force_due_if_generation(
        &self,
        target_thread_id: ThreadId,
        generation: i64,
    ) -> bool {
        let mut registrations = self.registrations.lock().await;
        let Some(entry) = registrations.get_mut(&target_thread_id) else {
            return false;
        };
        if entry.generation != generation || !entry.force_due_once {
            return false;
        }
        entry.force_due_once = false;
        true
    }

    #[cfg(test)]
    pub(crate) async fn force_due_for_tests(&self, target_thread_id: ThreadId) {
        let mut registrations = self.registrations.lock().await;
        if let Some(entry) = registrations.get_mut(&target_thread_id) {
            entry.force_due_once = true;
        }
    }

    async fn update_after_spawn(
        &self,
        target_thread_id: ThreadId,
        generation: i64,
        now: Instant,
        active_helper_id: Option<ThreadId>,
    ) {
        let mut registrations = self.registrations.lock().await;
        let Some(entry) = registrations.get_mut(&target_thread_id) else {
            return;
        };
        if entry.generation != generation {
            return;
        }
        entry.last_trigger = now;
        entry.active_helper_id = active_helper_id;
    }

    pub(crate) async fn unregister(&self, target_thread_id: ThreadId) -> Option<RemovedWatchdog> {
        let mut registrations = self.registrations.lock().await;
        registrations
            .remove(&target_thread_id)
            .map(|removed| RemovedWatchdog {
                target_thread_id,
                active_helper_id: removed.active_helper_id,
            })
    }

    pub(crate) async fn owner_for_active_helper(
        &self,
        helper_thread_id: ThreadId,
    ) -> Option<ThreadId> {
        let registrations = self.registrations.lock().await;
        registrations.values().find_map(|entry| {
            (entry.active_helper_id == Some(helper_thread_id))
                .then_some(entry.registration.owner_thread_id)
        })
    }

    pub(crate) async fn registered_targets(&self, candidate_ids: &[ThreadId]) -> HashSet<ThreadId> {
        let registrations = self.registrations.lock().await;
        candidate_ids
            .iter()
            .copied()
            .filter(|candidate_id| registrations.contains_key(candidate_id))
            .collect()
    }

    pub(crate) async fn take_for_owner(&self, owner_thread_id: ThreadId) -> Vec<RemovedWatchdog> {
        let mut registrations = self.registrations.lock().await;
        let removed_targets: Vec<ThreadId> = registrations
            .iter()
            .filter_map(|(target_thread_id, entry)| {
                (entry.registration.owner_thread_id == owner_thread_id).then_some(*target_thread_id)
            })
            .collect();
        let mut removed = Vec::new();
        for removed_target in removed_targets {
            if let Some(entry) = registrations.remove(&removed_target) {
                removed.push(RemovedWatchdog {
                    target_thread_id: removed_target,
                    active_helper_id: entry.active_helper_id,
                });
            }
        }
        removed
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) async fn set_active_helper_for_tests(
        &self,
        target_thread_id: ThreadId,
        helper_thread_id: ThreadId,
    ) {
        let mut registrations = self.registrations.lock().await;
        let Some(entry) = registrations.get_mut(&target_thread_id) else {
            return;
        };
        let due_at = Instant::now() - entry.interval;
        entry.last_trigger = due_at;
        entry.owner_idle_since = Some(due_at);
        entry.owner_was_running = false;
        entry.active_helper_id = Some(helper_thread_id);
    }
}

#[derive(Clone)]
struct WatchdogSnapshot {
    owner_thread_id: ThreadId,
    child_depth: i32,
    prompt: String,
    config: Config,
    interval: Duration,
    last_trigger: Instant,
    active_helper_id: Option<ThreadId>,
    owner_idle_since: Option<Instant>,
}

async fn get_status(manager_state: &Arc<ThreadManagerState>, thread_id: ThreadId) -> AgentStatus {
    let Ok(thread) = manager_state.get_thread(thread_id).await else {
        return AgentStatus::NotFound;
    };
    thread.agent_status().await
}

fn is_running(status: &AgentStatus) -> bool {
    matches!(status, AgentStatus::PendingInit | AgentStatus::Running)
}

fn is_watchdog_terminated(status: &AgentStatus) -> bool {
    matches!(status, AgentStatus::Shutdown | AgentStatus::NotFound)
}

fn interval_duration(interval_s: i64) -> CodexResult<Duration> {
    if interval_s <= 0 {
        return Err(CodexErr::UnsupportedOperation(
            "interval_s must be greater than zero".to_string(),
        ));
    }
    let seconds = u64::try_from(interval_s).map_err(|_| {
        CodexErr::UnsupportedOperation(format!("interval_s out of range: {interval_s}"))
    })?;
    Ok(Duration::from_secs(seconds))
}

fn tick_duration() -> Duration {
    let seconds = u64::try_from(WATCHDOG_TICK_SECONDS).unwrap_or(5);
    Duration::from_secs(seconds)
}

async fn watchdog_helper_prompt(
    config: &Config,
    target_thread_id: ThreadId,
    prompt: &str,
) -> String {
    if !config.features.enabled(Feature::AgentPromptInjection) {
        if prompt.trim().is_empty() {
            return format!("Target agent id: {target_thread_id}");
        }
        return format!("Target agent id: {target_thread_id}\n\n{prompt}");
    }
    let watchdog_prompt = load_watchdog_prompt(&config.codex_home).await;
    if prompt.trim().is_empty() {
        format!("{watchdog_prompt}\n\nTarget agent id: {target_thread_id}")
    } else {
        format!("{watchdog_prompt}\n\nTarget agent id: {target_thread_id}\n\n{prompt}")
    }
}

#[cfg(test)]
mod tests {
    use super::watchdog_helper_prompt;
    use crate::config::ConfigBuilder;
    use codex_features::Feature;
    use codex_protocol::ThreadId;

    #[tokio::test]
    async fn watchdog_helper_prompt_is_minimal_when_agent_prompt_injection_is_disabled() {
        let codex_home = tempfile::tempdir().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("load config");
        let target_thread_id = ThreadId::default();
        let prompt = watchdog_helper_prompt(&config, target_thread_id, "ping").await;
        assert_eq!(
            prompt,
            format!("Target agent id: {target_thread_id}\n\nping")
        );
    }

    #[tokio::test]
    async fn watchdog_helper_prompt_includes_watchdog_instructions_when_enabled() {
        let codex_home = tempfile::tempdir().expect("create temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("load config");
        let _ = config.features.enable(Feature::AgentPromptInjection);

        let prompt = watchdog_helper_prompt(&config, ThreadId::default(), "ping").await;
        assert!(prompt.contains("# You are a Subagent"));
        assert!(prompt.contains("Call `send_input`"));
        assert!(prompt.contains("Target agent id:"));
        assert!(prompt.ends_with("\n\nping"));
    }
}
