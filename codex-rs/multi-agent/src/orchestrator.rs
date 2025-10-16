use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use async_trait::async_trait;
use codex_common::CliConfigOverrides;
use codex_core::AuthManager;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::delegate_tool::DelegateEventReceiver as CoreDelegateEventReceiver;
use codex_core::delegate_tool::DelegateInvocationMode;
use codex_core::delegate_tool::DelegateToolAdapter;
use codex_core::delegate_tool::DelegateToolError;
use codex_core::delegate_tool::DelegateToolEvent as CoreDelegateToolEvent;
use codex_core::delegate_tool::DelegateToolRequest;
use codex_core::delegate_tool::DelegateToolRun;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::SessionSource;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AgentConfigLoader;
use crate::AgentId;

fn prompt_preview(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    const MAX_LEN: usize = 120;
    let preview = trimmed.chars().take(MAX_LEN).collect::<String>();
    Some(preview)
}

/// Identifier used to correlate delegate runs.
pub type DelegateRunId = String;

/// Request payload used when delegating work to a sub-agent.
#[derive(Debug, Clone)]
pub struct DelegateRequest {
    pub agent_id: AgentId,
    pub prompt: DelegatePrompt,
    pub user_initial: Vec<InputItem>,
    pub parent_run_id: Option<DelegateRunId>,
    pub mode: DelegateInvocationMode,
}

/// The prompt content forwarded to the sub-agent.
#[derive(Debug, Clone)]
pub struct DelegatePrompt {
    pub text: String,
}

impl DelegatePrompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Progress and completion updates emitted by the orchestrator.
#[derive(Debug, Clone)]
pub enum DelegateEvent {
    Started {
        run_id: DelegateRunId,
        agent_id: AgentId,
        prompt: String,
        started_at: SystemTime,
        parent_run_id: Option<DelegateRunId>,
        mode: DelegateSessionMode,
    },
    Delta {
        run_id: DelegateRunId,
        agent_id: AgentId,
        chunk: String,
    },
    Completed {
        run_id: DelegateRunId,
        agent_id: AgentId,
        output: Option<String>,
        duration: Duration,
        mode: DelegateSessionMode,
    },
    Failed {
        run_id: DelegateRunId,
        agent_id: AgentId,
        error: String,
        mode: DelegateSessionMode,
    },
}

/// Errors that can surface when orchestrating delegates.
#[derive(thiserror::Error, Debug)]
pub enum OrchestratorError {
    #[error("another delegate is already running")]
    DelegateInProgress,
    #[error("delegate queue is full")]
    QueueFull,
    #[error("agent `{0}` not found")]
    AgentNotFound(String),
    #[error("failed to enqueue delegate: {0}")]
    DelegateSetupFailed(String),
    #[error("delegate session `{0}` not found")]
    SessionNotFound(String),
}

/// High-level metadata describing a delegate session available for switching.
#[derive(Debug, Clone)]
pub struct DelegateSessionSummary {
    pub conversation_id: String,
    pub agent_id: AgentId,
    pub last_interacted_at: SystemTime,
    pub cwd: PathBuf,
    pub mode: DelegateSessionMode,
}

/// Indicates whether a session originated from a detached run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegateSessionMode {
    Standard,
    Detached,
}

#[derive(Debug, Clone)]
pub struct DetachedRunSummary {
    pub run_id: String,
    pub agent_id: AgentId,
    pub started_at: SystemTime,
    pub prompt_preview: Option<String>,
    pub status: DetachedRunStatusSummary,
}

#[derive(Debug, Clone)]
pub enum DetachedRunStatusSummary {
    Pending,
    Failed {
        error: String,
        finished_at: SystemTime,
    },
}

/// Payload returned when entering an existing delegate session.
#[derive(Clone)]
pub struct ActiveDelegateSession {
    pub summary: DelegateSessionSummary,
    pub conversation: Arc<CodexConversation>,
    pub session_configured: Arc<SessionConfiguredEvent>,
    pub config: Config,
}

/// Lightweight controller that spins up sub-agent conversations on demand and
/// streams condensed updates back to the caller.
pub struct AgentOrchestrator {
    loader: AgentConfigLoader,
    auth_manager: Arc<AuthManager>,
    session_source: SessionSource,
    cli_overrides: CliConfigOverrides,
    config_overrides: ConfigOverrides,
    listeners: Mutex<Vec<mpsc::UnboundedSender<DelegateEvent>>>,
    active_runs: Mutex<Vec<DelegateRunId>>,
    sessions: Mutex<HashMap<String, StoredDelegateSession>>,
    allowed_agents: Vec<AgentId>,
    run_conversations: Mutex<HashMap<DelegateRunId, String>>,
    conversation_runs: Mutex<HashMap<String, DelegateRunId>>,
    detached_runs: Mutex<HashMap<DelegateRunId, DetachedRunRecord>>,
    max_concurrent_runs: usize,
}

impl AgentOrchestrator {
    pub fn new(
        global_codex_home: impl Into<std::path::PathBuf>,
        auth_manager: Arc<AuthManager>,
        session_source: SessionSource,
        cli_overrides: CliConfigOverrides,
        config_overrides: ConfigOverrides,
        allowed_agents: Vec<AgentId>,
        max_concurrent_runs: usize,
    ) -> Self {
        let loader = AgentConfigLoader::new(global_codex_home.into());
        Self {
            loader,
            auth_manager,
            session_source,
            cli_overrides,
            config_overrides,
            listeners: Mutex::new(Vec::new()),
            active_runs: Mutex::new(Vec::new()),
            sessions: Mutex::new(HashMap::new()),
            allowed_agents,
            run_conversations: Mutex::new(HashMap::new()),
            conversation_runs: Mutex::new(HashMap::new()),
            detached_runs: Mutex::new(HashMap::new()),
            max_concurrent_runs: max_concurrent_runs.max(1),
        }
    }

    /// Register a listener that receives [`DelegateEvent`] updates.
    pub async fn subscribe(self: &Arc<Self>) -> mpsc::UnboundedReceiver<DelegateEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.listeners.lock().await.push(tx);
        rx
    }

    async fn register_run_conversation(&self, run_id: &DelegateRunId, conversation_id: &str) {
        self.run_conversations
            .lock()
            .await
            .insert(run_id.clone(), conversation_id.to_string());
        self.conversation_runs
            .lock()
            .await
            .insert(conversation_id.to_string(), run_id.clone());
    }

    async fn clear_run_conversation(&self, run_id: &DelegateRunId) {
        if let Some(conversation_id) = self.run_conversations.lock().await.remove(run_id) {
            self.conversation_runs.lock().await.remove(&conversation_id);
        }
    }

    pub async fn parent_run_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Option<DelegateRunId> {
        self.conversation_runs
            .lock()
            .await
            .get(conversation_id)
            .cloned()
    }

    /// Trigger a delegate run. Returns the run id if successfully enqueued.
    pub async fn delegate(
        self: &Arc<Self>,
        request: DelegateRequest,
    ) -> std::result::Result<DelegateRunId, OrchestratorError> {
        let run_id = Uuid::new_v4().to_string();
        let session_mode = match request.mode {
            DelegateInvocationMode::Detached => DelegateSessionMode::Detached,
            _ => DelegateSessionMode::Standard,
        };
        {
            let mut active = self.active_runs.lock().await;
            if active.len() >= self.max_concurrent_runs {
                return Err(OrchestratorError::QueueFull);
            }
            active.push(run_id.clone());
        }
        if session_mode == DelegateSessionMode::Detached {
            let mut registry = self.detached_runs.lock().await;
            registry.insert(
                run_id.clone(),
                DetachedRunRecord {
                    agent_id: request.agent_id.clone(),
                    started_at: SystemTime::now(),
                    prompt_preview: prompt_preview(&request.prompt.text),
                    cwd: None,
                    status: DetachedRunStatus::Pending,
                },
            );
        }

        let parent_run_id = request.parent_run_id.clone();
        let prompt_text = request.prompt.text.clone();
        self.emit(DelegateEvent::Started {
            run_id: run_id.clone(),
            agent_id: request.agent_id.clone(),
            prompt: prompt_text,
            started_at: SystemTime::now(),
            parent_run_id: parent_run_id.clone(),
            mode: session_mode,
        })
        .await;

        let loader = self.loader.clone();
        let auth_manager = self.auth_manager.clone();
        let session_source = self.session_source;
        let cli_overrides = self.cli_overrides.clone();
        let config_overrides = self.config_overrides.clone();
        let orchestrator = Arc::clone(self);
        let run_id_clone = run_id.clone();
        tokio::spawn(async move {
            let orchestrator_for_task = Arc::clone(&orchestrator);
            let result = orchestrator_for_task
                .run_delegate_task(
                    loader,
                    auth_manager,
                    session_source,
                    cli_overrides,
                    config_overrides,
                    run_id_clone.clone(),
                    request,
                )
                .await;

            match result {
                Ok(output) => {
                    orchestrator.store_session(&output).await;
                    orchestrator
                        .mark_detached_ready(&run_id_clone, &output)
                        .await;
                    let agent_id = output.agent_id.clone();
                    let message = output.message.clone();
                    let duration = output.duration;
                    orchestrator
                        .emit(DelegateEvent::Completed {
                            run_id: run_id_clone.clone(),
                            agent_id,
                            output: message,
                            duration,
                            mode: output.mode,
                        })
                        .await;
                }
                Err(err) => {
                    orchestrator
                        .mark_detached_failed(&run_id_clone, &err.error)
                        .await;
                    orchestrator
                        .emit(DelegateEvent::Failed {
                            run_id: run_id_clone.clone(),
                            agent_id: err.agent_id,
                            error: err.error,
                            mode: err.mode,
                        })
                        .await;
                }
            }

            orchestrator.clear_run_conversation(&run_id_clone).await;

            let mut active = orchestrator.active_runs.lock().await;
            if let Some(pos) = active.iter().rposition(|id| id == &run_id_clone) {
                active.remove(pos);
            }
        });

        Ok(run_id)
    }

    async fn emit(&self, event: DelegateEvent) {
        let mut listeners = self.listeners.lock().await;
        listeners.retain(|tx| tx.send(event.clone()).is_ok());
    }

    /// Return the list of configured agent ids available for delegation.
    pub fn allowed_agents(&self) -> &[AgentId] {
        &self.allowed_agents
    }

    /// Return all active delegate sessions ordered by most recent interaction.
    pub async fn active_sessions(&self) -> Vec<DelegateSessionSummary> {
        let sessions = self.sessions.lock().await;
        let mut summaries: Vec<_> = sessions
            .values()
            .map(|entry| entry.summary.clone())
            .collect();
        summaries.sort_by(|a, b| b.last_interacted_at.cmp(&a.last_interacted_at));
        summaries
    }

    /// Return detached runs that are not yet ready to attach or have failed.
    pub async fn detached_runs(&self) -> Vec<DetachedRunSummary> {
        let registry = self.detached_runs.lock().await;
        let mut summaries: Vec<DetachedRunSummary> = registry
            .iter()
            .filter_map(|(run_id, record)| match &record.status {
                DetachedRunStatus::Pending => Some(DetachedRunSummary {
                    run_id: run_id.clone(),
                    agent_id: record.agent_id.clone(),
                    started_at: record.started_at,
                    prompt_preview: record.prompt_preview.clone(),
                    status: DetachedRunStatusSummary::Pending,
                }),
                DetachedRunStatus::Failed { error, finished_at } => Some(DetachedRunSummary {
                    run_id: run_id.clone(),
                    agent_id: record.agent_id.clone(),
                    started_at: record.started_at,
                    prompt_preview: record.prompt_preview.clone(),
                    status: DetachedRunStatusSummary::Failed {
                        error: error.clone(),
                        finished_at: *finished_at,
                    },
                }),
                DetachedRunStatus::Ready { .. } => None,
            })
            .collect();
        summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        summaries
    }

    /// Remove a detached run from the registry and drop any stored session if present.
    pub async fn dismiss_detached_run(&self, run_id: &str) -> Result<(), String> {
        let conversation_to_remove = {
            let mut registry = self.detached_runs.lock().await;
            let record = registry
                .get(run_id)
                .ok_or_else(|| format!("detached run `{run_id}` not found"))?;
            match &record.status {
                DetachedRunStatus::Pending => {
                    return Err("run is still in progress".to_string());
                }
                DetachedRunStatus::Ready {
                    conversation_id, ..
                } => {
                    let conversation_id = conversation_id.clone();
                    registry.remove(run_id);
                    Some(conversation_id)
                }
                DetachedRunStatus::Failed { .. } => {
                    registry.remove(run_id);
                    None
                }
            }
        };

        if let Some(conversation_id) = conversation_to_remove {
            self.remove_session(&conversation_id).await;
        }
        Ok(())
    }

    /// Enter an existing delegate session for direct interaction.
    pub async fn enter_session(
        &self,
        conversation_id: &str,
    ) -> Result<ActiveDelegateSession, OrchestratorError> {
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(conversation_id)
            .ok_or_else(|| OrchestratorError::SessionNotFound(conversation_id.to_string()))?;
        entry.summary.last_interacted_at = SystemTime::now();
        Ok(ActiveDelegateSession {
            summary: entry.summary.clone(),
            conversation: entry.conversation.clone(),
            session_configured: entry.session_configured.clone(),
            config: entry.config.clone(),
        })
    }

    /// Remove a delegate session – used when the conversation is closed or no longer usable.
    pub async fn remove_session(&self, conversation_id: &str) {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(conversation_id);
    }

    /// Refresh the session's last-interacted timestamp without opening it.
    pub async fn touch_session(&self, conversation_id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(entry) = sessions.get_mut(conversation_id) {
            entry.summary.last_interacted_at = SystemTime::now();
        }
    }

    async fn store_session(&self, success: &DelegateSuccess) {
        let mut sessions = self.sessions.lock().await;
        let summary = DelegateSessionSummary {
            conversation_id: success.conversation_id.clone(),
            agent_id: success.agent_id.clone(),
            last_interacted_at: SystemTime::now(),
            cwd: success.cwd.clone(),
            mode: success.mode,
        };
        sessions.insert(
            success.conversation_id.clone(),
            StoredDelegateSession {
                summary,
                conversation: success.conversation.clone(),
                session_configured: success.session_configured.clone(),
                config: success.config.clone(),
            },
        );
    }

    async fn mark_detached_ready(&self, run_id: &DelegateRunId, success: &DelegateSuccess) {
        let mut registry = self.detached_runs.lock().await;
        if let Some(record) = registry.get_mut(run_id) {
            record.cwd = Some(success.cwd.clone());
            record.status = DetachedRunStatus::Ready {
                conversation_id: success.conversation_id.clone(),
                _summary: success.message.clone(),
                _duration: success.duration,
                _finished_at: SystemTime::now(),
            };
        }
    }

    async fn mark_detached_failed(&self, run_id: &DelegateRunId, error: &str) {
        let mut registry = self.detached_runs.lock().await;
        if let Some(record) = registry.get_mut(run_id) {
            record.status = DetachedRunStatus::Failed {
                error: error.to_string(),
                finished_at: SystemTime::now(),
            };
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_delegate_task(
        self: Arc<Self>,
        loader: AgentConfigLoader,
        auth_manager: Arc<AuthManager>,
        session_source: SessionSource,
        cli_overrides: CliConfigOverrides,
        config_overrides: ConfigOverrides,
        run_id: DelegateRunId,
        request: DelegateRequest,
    ) -> std::result::Result<DelegateSuccess, DelegateFailure> {
        let start = SystemTime::now();
        let agent_id = request.agent_id.clone();
        let session_mode = match request.mode {
            DelegateInvocationMode::Detached => DelegateSessionMode::Detached,
            _ => DelegateSessionMode::Standard,
        };
        let context = loader
            .load(Some(&agent_id), &cli_overrides, config_overrides)
            .await
            .map_err(|err| DelegateFailure {
                agent_id: agent_id.clone(),
                error: format!("failed to load agent config: {err:#}"),
                mode: session_mode,
            })?;

        let config = context.into_config();
        let cwd = config.cwd.clone();
        let config_clone = config.clone();
        let delegate_adapter = crate::delegate_tool_adapter(Arc::clone(&self));
        let conversation_manager = Arc::new(ConversationManager::with_delegate(
            auth_manager.clone(),
            session_source,
            Some(delegate_adapter),
        ));

        let conversation_bundle = conversation_manager
            .new_conversation(config)
            .await
            .map_err(|err| DelegateFailure {
                agent_id: agent_id.clone(),
                error: format!("failed to start conversation: {err:#}"),
                mode: session_mode,
            })?;
        let conversation_id = conversation_bundle.conversation_id.to_string();
        self.register_run_conversation(&run_id, &conversation_id)
            .await;
        let session_configured = Arc::new(conversation_bundle.session_configured);
        let conversation = conversation_bundle.conversation;

        let mut items = Vec::new();
        items.extend(request.user_initial.clone());
        if !request.prompt.text.trim().is_empty() {
            items.push(InputItem::Text {
                text: request.prompt.text.clone(),
            });
        }
        conversation
            .submit(Op::UserInput { items })
            .await
            .map_err(|err| DelegateFailure {
                agent_id: agent_id.clone(),
                error: format!("failed to submit delegate prompt: {err:#}"),
                mode: session_mode,
            })?;

        let mut aggregated = String::new();
        loop {
            let event = conversation
                .next_event()
                .await
                .map_err(|err| DelegateFailure {
                    agent_id: agent_id.clone(),
                    error: format!("failed to read delegate events: {err:#}"),
                    mode: session_mode,
                })?;

            match event.msg {
                EventMsg::AgentMessage(msg) => {
                    if aggregated.is_empty() {
                        aggregated = msg.message.clone();
                        self.emit(DelegateEvent::Delta {
                            run_id: run_id.clone(),
                            agent_id: agent_id.clone(),
                            chunk: msg.message,
                        })
                        .await;
                    } else {
                        aggregated = msg.message;
                    }
                }
                EventMsg::AgentMessageDelta(delta) => {
                    aggregated.push_str(&delta.delta);
                    self.emit(DelegateEvent::Delta {
                        run_id: run_id.clone(),
                        agent_id: agent_id.clone(),
                        chunk: delta.delta,
                    })
                    .await;
                }
                EventMsg::TaskComplete(task_complete) => {
                    let duration = start.elapsed().unwrap_or(Duration::ZERO);
                    let message = task_complete
                        .last_agent_message
                        .or_else(|| (!aggregated.is_empty()).then_some(aggregated.clone()));

                    return Ok(DelegateSuccess {
                        agent_id,
                        conversation_id,
                        conversation: conversation.clone(),
                        session_configured: session_configured.clone(),
                        cwd: cwd.clone(),
                        config: config_clone.clone(),
                        message,
                        duration,
                        mode: session_mode,
                    });
                }
                EventMsg::Error(err) => {
                    return Err(DelegateFailure {
                        agent_id,
                        error: format!("delegate reported error: {}", err.message),
                        mode: session_mode,
                    });
                }
                EventMsg::TurnAborted(reason) => {
                    return Err(DelegateFailure {
                        agent_id,
                        error: format!("delegate aborted: {:?}", reason.reason),
                        mode: session_mode,
                    });
                }
                EventMsg::ShutdownComplete => break,
                _ => {}
            }
        }

        Err(DelegateFailure {
            agent_id,
            error: "delegate ended unexpectedly".to_string(),
            mode: session_mode,
        })
    }
}

struct DelegateSuccess {
    agent_id: AgentId,
    conversation_id: String,
    conversation: Arc<CodexConversation>,
    session_configured: Arc<SessionConfiguredEvent>,
    cwd: PathBuf,
    config: Config,
    message: Option<String>,
    duration: Duration,
    mode: DelegateSessionMode,
}

struct DelegateFailure {
    agent_id: AgentId,
    error: String,
    mode: DelegateSessionMode,
}

struct StoredDelegateSession {
    summary: DelegateSessionSummary,
    conversation: Arc<CodexConversation>,
    session_configured: Arc<SessionConfiguredEvent>,
    config: Config,
}

struct DetachedRunRecord {
    agent_id: AgentId,
    started_at: SystemTime,
    prompt_preview: Option<String>,
    cwd: Option<PathBuf>,
    status: DetachedRunStatus,
}

enum DetachedRunStatus {
    Pending,
    Ready {
        conversation_id: String,
        _summary: Option<String>,
        _duration: Duration,
        _finished_at: SystemTime,
    },
    Failed {
        error: String,
        finished_at: SystemTime,
    },
}

pub struct MultiAgentDelegateAdapter {
    orchestrator: Arc<AgentOrchestrator>,
}

impl MultiAgentDelegateAdapter {
    pub fn new(orchestrator: Arc<AgentOrchestrator>) -> Self {
        Self { orchestrator }
    }

    fn map_event(event: DelegateEvent) -> CoreDelegateToolEvent {
        match event {
            DelegateEvent::Started {
                run_id,
                agent_id,
                prompt,
                started_at,
                parent_run_id,
                mode: _,
            } => CoreDelegateToolEvent::Started {
                run_id,
                agent_id: agent_id.as_str().to_string(),
                prompt,
                started_at,
                parent_run_id,
            },
            DelegateEvent::Delta {
                run_id,
                agent_id,
                chunk,
            } => CoreDelegateToolEvent::Delta {
                run_id,
                agent_id: agent_id.as_str().to_string(),
                chunk,
            },
            DelegateEvent::Completed {
                run_id,
                agent_id,
                output,
                duration,
                mode: _,
            } => CoreDelegateToolEvent::Completed {
                run_id,
                agent_id: agent_id.as_str().to_string(),
                output,
                duration,
            },
            DelegateEvent::Failed {
                run_id,
                agent_id,
                error,
                mode: _,
            } => CoreDelegateToolEvent::Failed {
                run_id,
                agent_id: agent_id.as_str().to_string(),
                error,
            },
        }
    }

    fn map_error(err: OrchestratorError) -> DelegateToolError {
        match err {
            OrchestratorError::DelegateInProgress => DelegateToolError::DelegateInProgress,
            OrchestratorError::QueueFull => DelegateToolError::QueueFull,
            OrchestratorError::AgentNotFound(agent) => DelegateToolError::AgentNotFound(agent),
            OrchestratorError::DelegateSetupFailed(reason) => {
                DelegateToolError::SetupFailed(reason)
            }
            OrchestratorError::SessionNotFound(session_id) => {
                DelegateToolError::SetupFailed(format!("session not found: {session_id}"))
            }
        }
    }
}

#[async_trait]
impl DelegateToolAdapter for MultiAgentDelegateAdapter {
    async fn subscribe(&self) -> CoreDelegateEventReceiver {
        let mut source = self.orchestrator.subscribe().await;
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(event) = source.recv().await {
                if tx.send(Self::map_event(event)).is_err() {
                    break;
                }
            }
        });
        rx
    }

    async fn delegate(
        &self,
        request: DelegateToolRequest,
    ) -> Result<DelegateToolRun, DelegateToolError> {
        let DelegateToolRequest {
            agent_id: agent_id_str,
            prompt,
            context: _,
            caller_conversation_id,
            mode,
            ..
        } = request;

        let agent_id = AgentId::parse(agent_id_str.as_str())
            .map_err(|_| DelegateToolError::AgentNotFound(agent_id_str.clone()))?;

        let parent_run_id = if let Some(conversation_id) = caller_conversation_id.as_ref() {
            self.orchestrator
                .parent_run_for_conversation(conversation_id)
                .await
        } else {
            None
        };

        let run_id = self
            .orchestrator
            .delegate(DelegateRequest {
                agent_id: agent_id.clone(),
                prompt: DelegatePrompt::new(prompt),
                user_initial: Vec::new(),
                parent_run_id,
                mode,
            })
            .await
            .map_err(Self::map_error)?;

        Ok(DelegateToolRun {
            run_id,
            agent_id: agent_id_str,
        })
    }
}
