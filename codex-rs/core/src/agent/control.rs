use super::watchdog::WatchdogManager;
use super::watchdog::WatchdogRegistration;
use crate::agent::AgentStatus;
use crate::agent::guards::Guards;
use crate::config::Config;
use crate::config::types::CollabInboxDeliveryRole;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::thread_manager::ThreadManagerState;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::COLLAB_INBOX_KIND;
use codex_protocol::protocol::COLLAB_INBOX_MESSAGE_PREFIX;
use codex_protocol::protocol::CollabInboxPayload;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::watch;
use uuid::Uuid;

/// Control-plane handle for multi-agent operations.
/// `AgentControl` is held by each session (via `SessionServices`). It provides capability to
/// spawn new agents and the inter-agent communication layer.
/// An `AgentControl` instance is shared per "user session" which means the same `AgentControl`
/// is used for every sub-agent spawned by Codex. By doing so, we make sure the guards are
/// scoped to a user session.
#[derive(Clone)]
pub(crate) struct AgentControl {
    /// Weak handle back to the global thread registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ThreadManagerState -> CodexThread -> Session -> SessionServices -> ThreadManagerState`.
    manager: Weak<ThreadManagerState>,
    guards: Arc<Guards>,
    watchdogs: Arc<WatchdogManager>,
    watchdog_compactions_in_progress: Arc<Mutex<HashSet<ThreadId>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentListing {
    pub(crate) thread_id: ThreadId,
    pub(crate) parent_thread_id: Option<ThreadId>,
    pub(crate) status: AgentStatus,
    pub(crate) depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WatchdogParentCompactionResult {
    NotWatchdogHelper,
    ParentBusy {
        parent_thread_id: ThreadId,
    },
    AlreadyInProgress {
        parent_thread_id: ThreadId,
    },
    Submitted {
        parent_thread_id: ThreadId,
        submission_id: String,
    },
}

impl Default for AgentControl {
    fn default() -> Self {
        let manager = Weak::new();
        let guards = Arc::new(Guards::default());
        let watchdogs = WatchdogManager::new(manager.clone(), Arc::clone(&guards));
        Self::from_parts(manager, guards, watchdogs)
    }
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ThreadManagerState>) -> Self {
        let guards = Arc::new(Guards::default());
        let watchdogs = WatchdogManager::new(manager.clone(), Arc::clone(&guards));
        watchdogs.start();
        Self::from_parts(manager, guards, watchdogs)
    }

    pub(crate) fn from_parts(
        manager: Weak<ThreadManagerState>,
        guards: Arc<Guards>,
        watchdogs: Arc<WatchdogManager>,
    ) -> Self {
        Self {
            manager,
            guards,
            watchdogs,
            watchdog_compactions_in_progress: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    pub(crate) async fn spawn_agent(
        &self,
        config: Config,
        prompt: String,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self.guards.reserve_spawn_slot(config.agent_max_threads)?;

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

    /// Spawn a new agent thread but do not submit an initial prompt.
    ///
    /// This is used for watchdog handles, which should not run a model turn on
    /// their own. The watchdog manager will fork helpers from the owner thread
    /// when the owner becomes idle.
    pub(crate) async fn spawn_agent_handle(
        &self,
        config: Config,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self.guards.reserve_spawn_slot(config.agent_max_threads)?;

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
        state.notify_thread_created(new_thread.thread_id);

        Ok(new_thread.thread_id)
    }

    /// Fork an existing agent thread and submit a prompt to the fork.
    pub(crate) async fn fork_agent(
        &self,
        config: Config,
        prompt: String,
        parent_thread_id: ThreadId,
        nth_user_message: usize,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self.guards.reserve_spawn_slot(config.agent_max_threads)?;

        let parent_thread = state.get_thread(parent_thread_id).await?;
        parent_thread.flush_rollout().await;
        let rollout_path = parent_thread.rollout_path().ok_or_else(|| {
            CodexErr::UnsupportedOperation(format!(
                "rollout history unavailable for thread {parent_thread_id}"
            ))
        })?;

        let new_thread = state
            .fork_thread_with_source(
                nth_user_message,
                config,
                self.clone(),
                rollout_path,
                session_source,
            )
            .await?;
        reservation.commit(new_thread.thread_id);
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
            self.guards.release_spawned_thread(agent_id);
        }
        result
    }

    /// Send a prompt to an existing agent thread using the configured collab inbox delivery role.
    pub(crate) async fn send_collab_message(
        &self,
        agent_id: ThreadId,
        sender_thread_id: ThreadId,
        message: String,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        let thread = state.get_thread(agent_id).await?;
        let snapshot = thread.config_snapshot().await;

        if matches!(snapshot.session_source, SessionSource::SubAgent(_)) {
            return self.send_prompt(agent_id, message).await;
        }

        let prepend_turn_start_user_message = !thread.has_active_turn().await;
        let items = build_collab_inbox_items(
            snapshot.collab_inbox_delivery_role,
            sender_thread_id,
            message,
            prepend_turn_start_user_message,
        )?;
        state
            .send_op(agent_id, Op::InjectResponseItems { items })
            .await
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
        self.guards.release_spawned_thread(agent_id);
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

    pub(crate) async fn register_watchdog(
        &self,
        registration: WatchdogRegistration,
    ) -> CodexResult<Vec<ThreadId>> {
        self.watchdogs.register(registration).await
    }

    pub(crate) async fn unregister_watchdog(&self, target_thread_id: ThreadId) {
        self.watchdogs.unregister(target_thread_id).await;
    }

    pub(crate) async fn unregister_watchdogs_for_owner(
        &self,
        owner_thread_id: ThreadId,
    ) -> Vec<ThreadId> {
        self.watchdogs.take_for_owner(owner_thread_id).await
    }

    pub(crate) async fn compact_parent_for_watchdog_helper(
        &self,
        helper_thread_id: ThreadId,
    ) -> CodexResult<WatchdogParentCompactionResult> {
        let Some(parent_thread_id) = self
            .watchdogs
            .owner_for_active_helper(helper_thread_id)
            .await
        else {
            return Ok(WatchdogParentCompactionResult::NotWatchdogHelper);
        };
        let state = self.upgrade()?;
        let parent_thread = state.get_thread(parent_thread_id).await?;
        let parent_has_active_turn = parent_thread.has_active_turn().await;

        {
            let mut compacting = self.watchdog_compactions_in_progress.lock().await;
            if compacting.contains(&parent_thread_id) {
                if parent_has_active_turn {
                    return Ok(WatchdogParentCompactionResult::AlreadyInProgress {
                        parent_thread_id,
                    });
                }
                // Clear stale marker when the parent is no longer actively compacting.
                compacting.remove(&parent_thread_id);
            }
            if parent_has_active_turn {
                return Ok(WatchdogParentCompactionResult::ParentBusy { parent_thread_id });
            }
            compacting.insert(parent_thread_id);
        }

        match state.send_op(parent_thread_id, Op::Compact).await {
            Ok(submission_id) => Ok(WatchdogParentCompactionResult::Submitted {
                parent_thread_id,
                submission_id,
            }),
            Err(err) => {
                let mut compacting = self.watchdog_compactions_in_progress.lock().await;
                compacting.remove(&parent_thread_id);
                Err(err)
            }
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) async fn run_watchdogs_once_for_tests(&self) {
        self.watchdogs.run_once().await;
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) async fn force_watchdog_due_for_tests(&self, target_thread_id: ThreadId) {
        self.watchdogs.force_due_for_tests(target_thread_id).await;
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) async fn set_watchdog_active_helper_for_tests(
        &self,
        target_thread_id: ThreadId,
        helper_thread_id: ThreadId,
    ) {
        self.watchdogs
            .set_active_helper_for_tests(target_thread_id, helper_thread_id)
            .await;
    }

    pub(crate) async fn list_agents(
        &self,
        owner_thread_id: ThreadId,
        recursive: bool,
    ) -> CodexResult<Vec<AgentListing>> {
        let state = self.upgrade()?;
        let threads = state.list_threads().await;

        let mut parent_by_thread: HashMap<ThreadId, Option<ThreadId>> =
            HashMap::with_capacity(threads.len());
        let mut status_by_thread: HashMap<ThreadId, AgentStatus> =
            HashMap::with_capacity(threads.len());

        for (thread_id, thread) in &threads {
            let snapshot = thread.config_snapshot().await;
            let parent_thread_id = match snapshot.session_source {
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id, ..
                }) => Some(parent_thread_id),
                _ => None,
            };
            parent_by_thread.insert(*thread_id, parent_thread_id);
            status_by_thread.insert(*thread_id, thread.agent_status().await);
        }

        let mut children_by_parent: HashMap<ThreadId, Vec<ThreadId>> = HashMap::new();
        for (thread_id, parent_thread_id) in &parent_by_thread {
            if let Some(parent_thread_id) = parent_thread_id {
                children_by_parent
                    .entry(*parent_thread_id)
                    .or_default()
                    .push(*thread_id);
            }
        }

        for children in children_by_parent.values_mut() {
            children.sort_by_key(ToString::to_string);
        }

        let mut listings = Vec::new();
        let mut queue: VecDeque<(ThreadId, usize)> = VecDeque::new();
        if let Some(children) = children_by_parent.get(&owner_thread_id) {
            for child in children {
                queue.push_back((*child, 1));
            }
        }

        while let Some((thread_id, depth)) = queue.pop_front() {
            listings.push(AgentListing {
                thread_id,
                parent_thread_id: parent_by_thread.get(&thread_id).copied().flatten(),
                status: status_by_thread
                    .get(&thread_id)
                    .cloned()
                    .unwrap_or(AgentStatus::NotFound),
                depth,
            });

            if recursive && let Some(children) = children_by_parent.get(&thread_id) {
                for child in children {
                    queue.push_back((*child, depth + 1));
                }
            }
        }

        Ok(listings)
    }

    fn upgrade(&self) -> CodexResult<Arc<ThreadManagerState>> {
        self.manager
            .upgrade()
            .ok_or_else(|| CodexErr::UnsupportedOperation("thread manager dropped".to_string()))
    }
}

fn build_collab_inbox_items(
    role: CollabInboxDeliveryRole,
    sender_thread_id: ThreadId,
    message: String,
    prepend_turn_start_user_message: bool,
) -> CodexResult<Vec<ResponseInputItem>> {
    let mut items = Vec::new();
    if prepend_turn_start_user_message {
        items.push(ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: String::new(),
            }],
        });
    }
    let role_items = match role {
        CollabInboxDeliveryRole::Tool => {
            let call_id = format!("collab_inbox_{}", Uuid::new_v4());
            let payload = CollabInboxPayload::new(sender_thread_id, message);
            let output = serde_json::to_string(&payload).map_err(|err| {
                CodexErr::UnsupportedOperation(format!(
                    "failed to serialize collab inbox payload: {err}"
                ))
            })?;

            vec![
                ResponseInputItem::FunctionCall {
                    name: COLLAB_INBOX_KIND.to_string(),
                    arguments: "{}".to_string(),
                    call_id: call_id.clone(),
                },
                ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload {
                        body: FunctionCallOutputBody::Text(output),
                        ..Default::default()
                    },
                },
            ]
        }
        CollabInboxDeliveryRole::Assistant => {
            let text = format!("{COLLAB_INBOX_MESSAGE_PREFIX}{sender_thread_id}] {message}");
            vec![ResponseInputItem::Message {
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText { text }],
            }]
        }
        CollabInboxDeliveryRole::Developer => {
            let text = format!("{COLLAB_INBOX_MESSAGE_PREFIX}{sender_thread_id}] {message}");
            vec![ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText { text }],
            }]
        }
    };
    items.extend(role_items);
    Ok(items)
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
    use codex_protocol::config_types::ModeKind;
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
            collaboration_mode_kind: ModeKind::Default,
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
    async fn send_collab_message_to_idle_thread_prepends_empty_user_message() {
        let harness = AgentControlHarness::new().await;
        let (receiver_thread_id, _thread) = harness.start_thread().await;
        let sender_thread_id = ThreadId::new();

        let submission_id = harness
            .control
            .send_collab_message(
                receiver_thread_id,
                sender_thread_id,
                "watchdog update".to_string(),
            )
            .await
            .expect("send_collab_message should succeed");
        assert!(!submission_id.is_empty());

        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|(thread_id, op)| {
                *thread_id == receiver_thread_id && matches!(op, Op::InjectResponseItems { .. })
            })
            .expect("expected injected collab inbox op");

        let Op::InjectResponseItems { items } = captured.1 else {
            unreachable!("matched above");
        };
        assert_eq!(items.len(), 3);
        match &items[0] {
            ResponseInputItem::Message { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(
                    content,
                    &vec![ContentItem::InputText {
                        text: String::new()
                    }]
                );
            }
            other => panic!("expected prepended user message, got {other:?}"),
        }
        match &items[1] {
            ResponseInputItem::FunctionCall {
                name, arguments, ..
            } => {
                assert_eq!(name, COLLAB_INBOX_KIND);
                assert_eq!(arguments, "{}");
            }
            other => panic!("expected collab function call, got {other:?}"),
        }
        match &items[2] {
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let output_text = output
                    .body
                    .to_text()
                    .expect("payload should convert to text");
                let payload: CollabInboxPayload =
                    serde_json::from_str(&output_text).expect("payload should be valid json");
                assert_eq!(payload.sender_thread_id, sender_thread_id);
                assert_eq!(payload.message, "watchdog update");
            }
            other => panic!("expected collab function call output, got {other:?}"),
        }
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

    #[test]
    fn build_collab_inbox_items_tool_role_emits_function_call_and_output() {
        let sender_thread_id = ThreadId::new();
        let message = "ping".to_string();

        let items = build_collab_inbox_items(
            CollabInboxDeliveryRole::Tool,
            sender_thread_id,
            message,
            false,
        )
        .expect("tool role should build inbox items");

        assert_eq!(items.len(), 2);

        let call_id = match &items[0] {
            ResponseInputItem::FunctionCall {
                name,
                arguments,
                call_id,
            } => {
                assert_eq!(name, COLLAB_INBOX_KIND);
                assert_eq!(arguments, "{}");
                call_id.clone()
            }
            other => panic!("expected function call item, got {other:?}"),
        };

        match &items[1] {
            ResponseInputItem::FunctionCallOutput {
                call_id: output_call_id,
                output,
            } => {
                assert_eq!(output_call_id, &call_id);
                let output_text = output
                    .body
                    .to_text()
                    .expect("payload should convert to text");
                let payload: CollabInboxPayload =
                    serde_json::from_str(&output_text).expect("payload should be valid json");
                assert!(payload.injected);
                assert_eq!(payload.kind, COLLAB_INBOX_KIND);
                assert_eq!(payload.sender_thread_id, sender_thread_id);
                assert_eq!(payload.message, "ping");
            }
            other => panic!("expected function call output item, got {other:?}"),
        }
    }

    #[test]
    fn build_collab_inbox_items_tool_role_prepends_empty_user_message_when_requested() {
        let sender_thread_id = ThreadId::new();
        let message = "ping".to_string();

        let items = build_collab_inbox_items(
            CollabInboxDeliveryRole::Tool,
            sender_thread_id,
            message,
            true,
        )
        .expect("tool role should build inbox items");

        assert_eq!(items.len(), 3);
        match &items[0] {
            ResponseInputItem::Message { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(
                    content,
                    &vec![ContentItem::InputText {
                        text: String::new()
                    }]
                );
            }
            other => panic!("expected prepended user message, got {other:?}"),
        }
        assert_matches!(&items[1], ResponseInputItem::FunctionCall { .. });
        assert_matches!(&items[2], ResponseInputItem::FunctionCallOutput { .. });
    }

    #[test]
    fn build_collab_inbox_items_assistant_role_prepends_empty_user_message_when_requested() {
        let sender_thread_id = ThreadId::new();
        let message = "hello".to_string();

        let items = build_collab_inbox_items(
            CollabInboxDeliveryRole::Assistant,
            sender_thread_id,
            message,
            true,
        )
        .expect("assistant role should build inbox items");

        assert_eq!(items.len(), 2);
        match &items[0] {
            ResponseInputItem::Message { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(
                    content,
                    &vec![ContentItem::InputText {
                        text: String::new()
                    }]
                );
            }
            other => panic!("expected prepended user message, got {other:?}"),
        }
        match &items[1] {
            ResponseInputItem::Message { role, .. } => assert_eq!(role, "assistant"),
            other => panic!("expected assistant message, got {other:?}"),
        }
    }

    #[test]
    fn build_collab_inbox_items_developer_role_prepends_empty_user_message_when_requested() {
        let sender_thread_id = ThreadId::new();
        let message = "hello".to_string();

        let items = build_collab_inbox_items(
            CollabInboxDeliveryRole::Developer,
            sender_thread_id,
            message,
            true,
        )
        .expect("developer role should build inbox items");

        assert_eq!(items.len(), 2);
        match &items[0] {
            ResponseInputItem::Message { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(
                    content,
                    &vec![ContentItem::InputText {
                        text: String::new()
                    }]
                );
            }
            other => panic!("expected prepended user message, got {other:?}"),
        }
        match &items[1] {
            ResponseInputItem::Message { role, .. } => assert_eq!(role, "developer"),
            other => panic!("expected developer message, got {other:?}"),
        }
    }
}
