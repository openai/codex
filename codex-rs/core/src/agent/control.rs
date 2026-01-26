use crate::agent::AgentStatus;
use crate::agent::guards::Guards;
use crate::agent::status::is_final;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::thread_manager::ThreadManagerState;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use tokio::sync::watch;
use uuid::Uuid;

const SYNTHETIC_WAIT_TIMEOUT_MS: i64 = 300_000;

#[derive(Default)]
struct TurnCollabLedger {
    spawned: HashSet<ThreadId>,
    acknowledged: HashSet<ThreadId>,
    in_flight_waits: HashMap<ThreadId, usize>,
}

impl TurnCollabLedger {
    fn is_idle(&self) -> bool {
        self.in_flight_waits.is_empty()
            && self
                .spawned
                .iter()
                .all(|agent_id| self.acknowledged.contains(agent_id))
    }
}

#[derive(Default)]
struct CollabLedger {
    turns: HashMap<String, TurnCollabLedger>,
}

/// RAII guard that marks a set of agents as being explicitly waited on.
pub(crate) struct WaitGuard {
    collab: Arc<Mutex<CollabLedger>>,
    turn_id: String,
    agent_ids: Vec<ThreadId>,
}

impl WaitGuard {
    fn new(collab: Arc<Mutex<CollabLedger>>, turn_id: &str, agent_ids: &[ThreadId]) -> Self {
        Self {
            collab,
            turn_id: turn_id.to_string(),
            agent_ids: agent_ids.to_vec(),
        }
    }
}

impl Drop for WaitGuard {
    fn drop(&mut self) {
        let mut collab = self
            .collab
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(turn) = collab.turns.get_mut(&self.turn_id) {
            for agent_id in &self.agent_ids {
                if let Some(count) = turn.in_flight_waits.get_mut(agent_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        turn.in_flight_waits.remove(agent_id);
                    }
                }
            }
            if turn.is_idle() {
                collab.turns.remove(&self.turn_id);
            }
        }
    }
}

/// Control-plane handle for multi-agent operations.
/// `AgentControl` is held by each session (via `SessionServices`). It provides capability to
/// spawn new agents and the inter-agent communication layer.
/// An `AgentControl` instance is shared per "user session" which means the same `AgentControl`
/// is used for every sub-agent spawned by Codex. By doing so, we make sure the guards are
/// scoped to a user session.
#[derive(Clone, Default)]
pub(crate) struct AgentControl {
    /// Weak handle back to the global thread registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ThreadManagerState -> CodexThread -> Session -> SessionServices -> ThreadManagerState`.
    manager: Weak<ThreadManagerState>,
    state: Arc<Guards>,
    collab: Arc<Mutex<CollabLedger>>,
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ThreadManagerState>) -> Self {
        Self {
            manager,
            ..Default::default()
        }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    pub(crate) async fn spawn_agent(
        &self,
        config: crate::config::Config,
        prompt: String,
        session_source: Option<codex_protocol::protocol::SessionSource>,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self.state.reserve_spawn_slot(config.agent_max_threads)?;

        // The same `AgentControl` is sent to spawn the thread.
        let new_thread = match session_source {
            Some(session_source) => {
                state
                    .spawn_new_thread_with_source(config, self.clone(), session_source)
                    .await?
            }
            None => state.spawn_new_thread(config, self.clone()).await?,
        };
        reservation.commit(new_thread.thread_id);

        // Notify a new thread has been created. This notification will be processed by clients
        // to subscribe or drain this newly created thread.
        // TODO(jif) add helper for drain
        state.notify_thread_created(new_thread.thread_id);

        self.send_prompt(new_thread.thread_id, prompt).await?;

        Ok(new_thread.thread_id)
    }

    /// Send a `user` prompt to an existing agent thread.
    pub(crate) async fn send_prompt(
        &self,
        agent_id: ThreadId,
        prompt: String,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        let result = state
            .send_op(
                agent_id,
                Op::UserInput {
                    items: vec![UserInput::Text {
                        text: prompt,
                        // Agent control prompts are plain text with no UI text elements.
                        text_elements: Vec::new(),
                    }],
                    final_output_json_schema: None,
                },
            )
            .await;
        if matches!(result, Err(CodexErr::InternalAgentDied)) {
            let _ = state.remove_thread(&agent_id).await;
            self.state.release_spawned_thread(agent_id);
        }
        result
    }

    /// Interrupt the current task for an existing agent thread.
    pub(crate) async fn interrupt_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        state.send_op(agent_id, Op::Interrupt).await
    }

    /// Submit a shutdown request to an existing agent thread.
    pub(crate) async fn shutdown_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        let result = state.send_op(agent_id, Op::Shutdown {}).await;
        let _ = state.remove_thread(&agent_id).await;
        self.state.release_spawned_thread(agent_id);
        result
    }

    /// Fetch the last known status for `agent_id`, returning `NotFound` when unavailable.
    pub(crate) async fn get_status(&self, agent_id: ThreadId) -> AgentStatus {
        let Ok(state) = self.upgrade() else {
            // No agent available if upgrade fails.
            return AgentStatus::NotFound;
        };
        let Ok(thread) = state.get_thread(agent_id).await else {
            return AgentStatus::NotFound;
        };
        thread.agent_status().await
    }

    /// Subscribe to status updates for `agent_id`, yielding the latest value and changes.
    pub(crate) async fn subscribe_status(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<watch::Receiver<AgentStatus>> {
        let state = self.upgrade()?;
        let thread = state.get_thread(agent_id).await?;
        Ok(thread.subscribe_status())
    }

    pub(crate) fn register_spawn(&self, turn_id: &str, agent_id: ThreadId) {
        let mut collab = self.lock_collab();
        let turn = collab.turns.entry(turn_id.to_string()).or_default();
        turn.spawned.insert(agent_id);
        turn.acknowledged.remove(&agent_id);
    }

    pub(crate) fn wait_guard(&self, turn_id: &str, agent_ids: &[ThreadId]) -> WaitGuard {
        let mut collab = self.lock_collab();
        let turn = collab.turns.entry(turn_id.to_string()).or_default();
        for agent_id in agent_ids {
            *turn.in_flight_waits.entry(*agent_id).or_default() += 1;
        }
        WaitGuard::new(Arc::clone(&self.collab), turn_id, agent_ids)
    }

    pub(crate) fn acknowledge(&self, turn_id: &str, agent_ids: &[ThreadId]) {
        let mut collab = self.lock_collab();
        if let Some(turn) = collab.turns.get_mut(turn_id) {
            turn.acknowledged.extend(agent_ids.iter().copied());
            if turn.is_idle() {
                collab.turns.remove(turn_id);
            }
        }
    }

    pub(crate) fn clear_turn(&self, turn_id: &str) {
        let mut collab = self.lock_collab();
        collab.turns.remove(turn_id);
    }

    pub(crate) async fn collect_unacknowledged_finals(
        &self,
        turn_id: &str,
    ) -> Vec<(ThreadId, AgentStatus)> {
        let mut candidates = {
            let collab = self.lock_collab();
            collab
                .turns
                .get(turn_id)
                .map(|turn| {
                    turn.spawned
                        .iter()
                        .copied()
                        .filter(|agent_id| {
                            !turn.acknowledged.contains(agent_id)
                                && !turn.in_flight_waits.contains_key(agent_id)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        if candidates.is_empty() {
            return Vec::new();
        }
        candidates.sort_by_key(std::string::ToString::to_string);

        let mut finals = Vec::new();
        for agent_id in candidates {
            let status = self.get_status(agent_id).await;
            if is_final(&status) {
                finals.push((agent_id, status));
            }
        }
        if finals.is_empty() {
            return Vec::new();
        }

        let mut ready = Vec::new();
        let mut collab = self.lock_collab();
        if let Some(turn) = collab.turns.get_mut(turn_id) {
            for (agent_id, status) in finals {
                if turn.acknowledged.contains(&agent_id)
                    || turn.in_flight_waits.contains_key(&agent_id)
                {
                    continue;
                }
                turn.acknowledged.insert(agent_id);
                ready.push((agent_id, status));
            }
            if turn.is_idle() {
                collab.turns.remove(turn_id);
            }
        }
        ready
    }

    pub(crate) fn synthetic_wait_items(statuses: &[(ThreadId, AgentStatus)]) -> Vec<ResponseItem> {
        if statuses.is_empty() {
            return Vec::new();
        }
        let call_id = format!("synthetic-wait-{}", Uuid::new_v4());

        let mut ids = statuses
            .iter()
            .map(|(agent_id, _)| agent_id.to_string())
            .collect::<Vec<_>>();
        ids.sort();
        let arguments = json!({
            "ids": ids,
            "timeout_ms": SYNTHETIC_WAIT_TIMEOUT_MS,
        })
        .to_string();

        let status_map = statuses
            .iter()
            .map(|(agent_id, status)| (agent_id.to_string(), status.clone()))
            .collect::<BTreeMap<_, _>>();
        let output = json!({
            "status": status_map,
            "timed_out": false,
        })
        .to_string();

        let call = ResponseItem::FunctionCall {
            id: None,
            name: "wait".to_string(),
            arguments,
            call_id: call_id.clone(),
        };
        let output = ResponseItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: output,
                ..Default::default()
            },
        };

        vec![call, output]
    }

    fn upgrade(&self) -> CodexResult<Arc<ThreadManagerState>> {
        self.manager
            .upgrade()
            .ok_or_else(|| CodexErr::UnsupportedOperation("thread manager dropped".to_string()))
    }

    fn lock_collab(&self) -> std::sync::MutexGuard<'_, CollabLedger> {
        self.collab
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodexAuth;
    use crate::CodexThread;
    use crate::ThreadManager;
    use crate::agent::agent_status_from_event;
    use crate::config::Config;
    use crate::config::ConfigBuilder;
    use assert_matches::assert_matches;
    use codex_protocol::protocol::ErrorEvent;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use toml::Value as TomlValue;

    async fn test_config_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> (TempDir, Config) {
        let home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .cli_overrides(cli_overrides)
            .build()
            .await
            .expect("load default test config");
        (home, config)
    }

    async fn test_config() -> (TempDir, Config) {
        test_config_with_cli_overrides(Vec::new()).await
    }

    struct AgentControlHarness {
        _home: TempDir,
        config: Config,
        manager: ThreadManager,
        control: AgentControl,
    }

    impl AgentControlHarness {
        async fn new() -> Self {
            let (home, config) = test_config().await;
            let manager = ThreadManager::with_models_provider_and_home(
                CodexAuth::from_api_key("dummy"),
                config.model_provider.clone(),
                config.codex_home.clone(),
            );
            let control = manager.agent_control();
            Self {
                _home: home,
                config,
                manager,
                control,
            }
        }

        async fn start_thread(&self) -> (ThreadId, Arc<CodexThread>) {
            let new_thread = self
                .manager
                .start_thread(self.config.clone())
                .await
                .expect("start thread");
            (new_thread.thread_id, new_thread.thread)
        }
    }

    #[tokio::test]
    async fn send_prompt_errors_when_manager_dropped() {
        let control = AgentControl::default();
        let err = control
            .send_prompt(ThreadId::new(), "hello".to_string())
            .await
            .expect_err("send_prompt should fail without a manager");
        assert_eq!(
            err.to_string(),
            "unsupported operation: thread manager dropped"
        );
    }

    #[tokio::test]
    async fn get_status_returns_not_found_without_manager() {
        let control = AgentControl::default();
        let got = control.get_status(ThreadId::new()).await;
        assert_eq!(got, AgentStatus::NotFound);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_started() {
        let status = agent_status_from_event(&EventMsg::TurnStarted(TurnStartedEvent {
            model_context_window: None,
        }));
        assert_eq!(status, Some(AgentStatus::Running));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_complete() {
        let status = agent_status_from_event(&EventMsg::TurnComplete(TurnCompleteEvent {
            last_agent_message: Some("done".to_string()),
        }));
        let expected = AgentStatus::Completed(Some("done".to_string()));
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_error() {
        let status = agent_status_from_event(&EventMsg::Error(ErrorEvent {
            message: "boom".to_string(),
            codex_error_info: None,
        }));

        let expected = AgentStatus::Errored("boom".to_string());
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_turn_aborted() {
        let status = agent_status_from_event(&EventMsg::TurnAborted(TurnAbortedEvent {
            reason: TurnAbortReason::Interrupted,
        }));

        let expected = AgentStatus::Errored("Interrupted".to_string());
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_shutdown_complete() {
        let status = agent_status_from_event(&EventMsg::ShutdownComplete);
        assert_eq!(status, Some(AgentStatus::Shutdown));
    }

    #[tokio::test]
    async fn spawn_agent_errors_when_manager_dropped() {
        let control = AgentControl::default();
        let (_home, config) = test_config().await;
        let err = control
            .spawn_agent(config, "hello".to_string(), None)
            .await
            .expect_err("spawn_agent should fail without a manager");
        assert_eq!(
            err.to_string(),
            "unsupported operation: thread manager dropped"
        );
    }

    #[tokio::test]
    async fn send_prompt_errors_when_thread_missing() {
        let harness = AgentControlHarness::new().await;
        let thread_id = ThreadId::new();
        let err = harness
            .control
            .send_prompt(thread_id, "hello".to_string())
            .await
            .expect_err("send_prompt should fail for missing thread");
        assert_matches!(err, CodexErr::ThreadNotFound(id) if id == thread_id);
    }

    #[tokio::test]
    async fn get_status_returns_not_found_for_missing_thread() {
        let harness = AgentControlHarness::new().await;
        let status = harness.control.get_status(ThreadId::new()).await;
        assert_eq!(status, AgentStatus::NotFound);
    }

    #[tokio::test]
    async fn get_status_returns_pending_init_for_new_thread() {
        let harness = AgentControlHarness::new().await;
        let (thread_id, _) = harness.start_thread().await;
        let status = harness.control.get_status(thread_id).await;
        assert_eq!(status, AgentStatus::PendingInit);
    }

    #[tokio::test]
    async fn subscribe_status_errors_for_missing_thread() {
        let harness = AgentControlHarness::new().await;
        let thread_id = ThreadId::new();
        let err = harness
            .control
            .subscribe_status(thread_id)
            .await
            .expect_err("subscribe_status should fail for missing thread");
        assert_matches!(err, CodexErr::ThreadNotFound(id) if id == thread_id);
    }

    #[tokio::test]
    async fn subscribe_status_updates_on_shutdown() {
        let harness = AgentControlHarness::new().await;
        let (thread_id, thread) = harness.start_thread().await;
        let mut status_rx = harness
            .control
            .subscribe_status(thread_id)
            .await
            .expect("subscribe_status should succeed");
        assert_eq!(status_rx.borrow().clone(), AgentStatus::PendingInit);

        let _ = thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");

        let _ = status_rx.changed().await;
        assert_eq!(status_rx.borrow().clone(), AgentStatus::Shutdown);
    }

    #[tokio::test]
    async fn send_prompt_submits_user_message() {
        let harness = AgentControlHarness::new().await;
        let (thread_id, _thread) = harness.start_thread().await;

        let submission_id = harness
            .control
            .send_prompt(thread_id, "hello from tests".to_string())
            .await
            .expect("send_prompt should succeed");
        assert!(!submission_id.is_empty());
        let expected = (
            thread_id,
            Op::UserInput {
                items: vec![UserInput::Text {
                    text: "hello from tests".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            },
        );
        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|entry| *entry == expected);
        assert_eq!(captured, Some(expected));
    }

    #[tokio::test]
    async fn spawn_agent_creates_thread_and_sends_prompt() {
        let harness = AgentControlHarness::new().await;
        let thread_id = harness
            .control
            .spawn_agent(harness.config.clone(), "spawned".to_string(), None)
            .await
            .expect("spawn_agent should succeed");
        let _thread = harness
            .manager
            .get_thread(thread_id)
            .await
            .expect("thread should be registered");
        let expected = (
            thread_id,
            Op::UserInput {
                items: vec![UserInput::Text {
                    text: "spawned".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            },
        );
        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|entry| *entry == expected);
        assert_eq!(captured, Some(expected));
    }

    #[tokio::test]
    async fn spawn_agent_respects_max_threads_limit() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();

        let _ = manager
            .start_thread(config.clone())
            .await
            .expect("start thread");

        let first_agent_id = control
            .spawn_agent(config.clone(), "hello".to_string(), None)
            .await
            .expect("spawn_agent should succeed");

        let err = control
            .spawn_agent(config, "hello again".to_string(), None)
            .await
            .expect_err("spawn_agent should respect max threads");
        let CodexErr::AgentLimitReached {
            max_threads: seen_max_threads,
        } = err
        else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(seen_max_threads, max_threads);

        let _ = control
            .shutdown_agent(first_agent_id)
            .await
            .expect("shutdown agent");
    }

    #[tokio::test]
    async fn spawn_agent_releases_slot_after_shutdown() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();

        let first_agent_id = control
            .spawn_agent(config.clone(), "hello".to_string(), None)
            .await
            .expect("spawn_agent should succeed");
        let _ = control
            .shutdown_agent(first_agent_id)
            .await
            .expect("shutdown agent");

        let second_agent_id = control
            .spawn_agent(config.clone(), "hello again".to_string(), None)
            .await
            .expect("spawn_agent should succeed after shutdown");
        let _ = control
            .shutdown_agent(second_agent_id)
            .await
            .expect("shutdown agent");
    }

    #[tokio::test]
    async fn spawn_agent_limit_shared_across_clones() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();
        let cloned = control.clone();

        let first_agent_id = cloned
            .spawn_agent(config.clone(), "hello".to_string(), None)
            .await
            .expect("spawn_agent should succeed");

        let err = control
            .spawn_agent(config, "hello again".to_string(), None)
            .await
            .expect_err("spawn_agent should respect shared guard");
        let CodexErr::AgentLimitReached { max_threads } = err else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(max_threads, 1);

        let _ = control
            .shutdown_agent(first_agent_id)
            .await
            .expect("shutdown agent");
    }
}
