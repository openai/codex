use super::control::AgentControl;
use super::guards::Guards;
use super::guards::MAX_THREAD_SPAWN_DEPTH;
use super::guards::exceeds_thread_spawn_depth_limit;
use super::status::is_final;
use crate::codex::load_watchdog_prompt;
use crate::config::Config;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::thread_manager::ThreadManagerState;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use std::collections::HashMap;
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

pub(crate) const DEFAULT_WATCHDOG_INTERVAL_S: i64 = 60;

// Tick frequently enough to respect short intervals (e.g., 1s demos).
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
    guards: Arc<Guards>,
    registrations: Mutex<HashMap<ThreadId, WatchdogEntry>>,
    started: AtomicBool,
    next_generation: AtomicI64,
}

impl WatchdogManager {
    pub(crate) fn new(manager: Weak<ThreadManagerState>, guards: Arc<Guards>) -> Arc<Self> {
        Arc::new(Self {
            manager,
            guards,
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
    ) -> CodexResult<Vec<ThreadId>> {
        if exceeds_thread_spawn_depth_limit(registration.child_depth) {
            return Err(CodexErr::UnsupportedOperation(format!(
                "agent depth limit reached: max depth is {MAX_THREAD_SPAWN_DEPTH}"
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
        for superseded_target in &superseded_targets {
            registrations.remove(superseded_target);
        }
        registrations.insert(entry.registration.target_thread_id, entry);
        Ok(superseded_targets)
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

        // The owner thread is what we actually monitor for idle time and context.
        let owner_thread = manager_state.get_thread(snapshot.owner_thread_id).await;
        let owner_status = match owner_thread.as_ref() {
            Ok(thread) => thread.agent_status().await,
            Err(_) => AgentStatus::NotFound,
        };
        if is_watchdog_terminated(&owner_status) {
            self.remove_if_generation(target_thread_id, generation)
                .await;
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

        let control_for_spawn = AgentControl::from_parts(
            self.manager.clone(),
            Arc::clone(&self.guards),
            Arc::clone(self),
        );

        if let Some(helper_id) = snapshot.active_helper_id {
            let helper_status = get_status(manager_state, helper_id).await;
            if !is_final(&helper_status) {
                return;
            }

            let helper_sent_input = manager_state
                .get_thread(helper_id)
                .await
                .map(|thread| thread.last_completed_turn_used_collab_send_input())
                .unwrap_or(false);
            if let AgentStatus::Completed(Some(message)) = &helper_status
                && !message.trim().is_empty()
                && !helper_sent_input
            {
                if let Err(err) = control_for_spawn
                    .send_collab_message(snapshot.owner_thread_id, helper_id, message.clone())
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
            // Helpers finish quickly and we don't explicitly shut them down,
            // so we must release their spawn slot here to avoid exhausting
            // the agent thread limit over time.
            self.guards.release_spawned_thread(helper_id);
            self.update_after_spawn(target_thread_id, generation, now, None)
                .await;
            return;
        }

        if now.duration_since(snapshot.last_trigger) < snapshot.interval {
            return;
        }

        let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: snapshot.owner_thread_id,
            depth: snapshot.child_depth,
        });
        let mut helper_config = snapshot.config.clone();
        // Watchdog helpers are short-lived check-ins. Keep their state in-memory and
        // avoid writing full forked rollout history to disk for each helper run.
        helper_config.ephemeral = true;
        let watchdog_instructions =
            watchdog_developer_instructions(&helper_config, snapshot.owner_thread_id).await;
        helper_config.developer_instructions = Some(match helper_config.developer_instructions {
            Some(existing) if !existing.trim().is_empty() => {
                format!("{watchdog_instructions}\n\n{existing}")
            }
            _ => watchdog_instructions,
        });
        let spawn_result = control_for_spawn
            .fork_agent(
                helper_config,
                snapshot.prompt.clone(),
                snapshot.owner_thread_id,
                // Keep full history for helper forks to maximize model-side caching efficiency.
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
                self.update_after_spawn(target_thread_id, generation, now, None)
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

    async fn remove_if_generation(&self, target_thread_id: ThreadId, generation: i64) {
        let mut registrations = self.registrations.lock().await;
        let Some(entry) = registrations.get(&target_thread_id) else {
            return;
        };
        if entry.generation == generation {
            registrations.remove(&target_thread_id);
        }
    }

    pub(crate) async fn unregister(&self, target_thread_id: ThreadId) {
        let mut registrations = self.registrations.lock().await;
        registrations.remove(&target_thread_id);
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

    pub(crate) async fn take_for_owner(&self, owner_thread_id: ThreadId) -> Vec<ThreadId> {
        let mut registrations = self.registrations.lock().await;
        let removed_targets: Vec<ThreadId> = registrations
            .iter()
            .filter_map(|(target_thread_id, entry)| {
                (entry.registration.owner_thread_id == owner_thread_id).then_some(*target_thread_id)
            })
            .collect();
        for removed_target in &removed_targets {
            registrations.remove(removed_target);
        }
        removed_targets
    }

    #[cfg(any(test, feature = "test-support"))]
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

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) async fn force_due_for_tests(&self, target_thread_id: ThreadId) {
        let mut registrations = self.registrations.lock().await;
        let Some(entry) = registrations.get_mut(&target_thread_id) else {
            return;
        };
        let due_at = Instant::now() - entry.interval;
        entry.last_trigger = due_at;
        entry.owner_idle_since = Some(due_at);
        entry.owner_was_running = false;
        entry.force_due_once = true;
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

async fn get_status(
    manager_state: &Arc<ThreadManagerState>,
    thread_id: ThreadId,
) -> codex_protocol::protocol::AgentStatus {
    let Ok(thread) = manager_state.get_thread(thread_id).await else {
        return codex_protocol::protocol::AgentStatus::NotFound;
    };
    thread.agent_status().await
}

fn is_running(status: &codex_protocol::protocol::AgentStatus) -> bool {
    matches!(
        status,
        codex_protocol::protocol::AgentStatus::PendingInit
            | codex_protocol::protocol::AgentStatus::Running
    )
}

fn is_watchdog_terminated(status: &codex_protocol::protocol::AgentStatus) -> bool {
    matches!(
        status,
        codex_protocol::protocol::AgentStatus::Shutdown
            | codex_protocol::protocol::AgentStatus::NotFound
    )
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

async fn watchdog_developer_instructions(config: &Config, target_thread_id: ThreadId) -> String {
    let watchdog_prompt = load_watchdog_prompt(&config.codex_home).await;
    format!("{watchdog_prompt}\n\nTarget agent id: {target_thread_id}")
}
