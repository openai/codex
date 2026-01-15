use crate::agent::AgentStatus;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::thread_manager::ThreadManagerState;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::watch;

/// Control-plane handle for multi-agent operations.
/// `AgentControl` is held by each session (via `SessionServices`). It provides capability to
/// spawn new agents and the inter-agent communication layer.
#[derive(Clone, Default)]
pub(crate) struct AgentControl {
    /// Weak handle back to the global thread registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ThreadManagerState -> CodexThread -> Session -> SessionServices -> ThreadManagerState`.
    manager: Weak<ThreadManagerState>,
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ThreadManagerState>) -> Self {
        Self { manager }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    pub(crate) async fn spawn_agent(
        &self,
        config: crate::config::Config,
        prompt: String,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let new_thread = state.spawn_new_thread(config, self.clone()).await?;

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
                        // Plain text conversion has no UI element ranges.
                        text_elements: Vec::new(),
                    }],
                    final_output_json_schema: None,
                },
            )
            .await;
        if matches!(result, Err(CodexErr::InternalAgentDied)) {
            let _ = state.remove_thread(&agent_id).await;
        }
        result
    }

    /// Submit a shutdown request to an existing agent thread.
    pub(crate) async fn shutdown_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        let result = state.send_op(agent_id, Op::Shutdown {}).await;
        let _ = state.remove_thread(&agent_id).await;
        result
    }

    #[allow(dead_code)] // Will be used for collab tools.
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

    fn upgrade(&self) -> CodexResult<Arc<ThreadManagerState>> {
        self.manager
            .upgrade()
            .ok_or_else(|| CodexErr::UnsupportedOperation("thread manager dropped".to_string()))
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

    async fn test_config() -> (TempDir, Config) {
        let home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await
            .expect("load default test config");
        (home, config)
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
            .spawn_agent(config, "hello".to_string())
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
            .spawn_agent(harness.config.clone(), "spawned".to_string())
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
}
