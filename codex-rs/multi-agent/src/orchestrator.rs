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

/// Identifier used to correlate delegate runs.
pub type DelegateRunId = String;

/// Request payload used when delegating work to a sub-agent.
#[derive(Debug, Clone)]
pub struct DelegateRequest {
    pub agent_id: AgentId,
    pub prompt: DelegatePrompt,
    pub user_initial: Vec<InputItem>,
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
    },
    Failed {
        run_id: DelegateRunId,
        agent_id: AgentId,
        error: String,
    },
}

/// Errors that can surface when orchestrating delegates.
#[derive(thiserror::Error, Debug)]
pub enum OrchestratorError {
    #[error("another delegate is already running")]
    DelegateInProgress,
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
    active_run: Mutex<Option<DelegateRunId>>,
    sessions: Mutex<HashMap<String, StoredDelegateSession>>,
}

impl AgentOrchestrator {
    pub fn new(
        global_codex_home: impl Into<std::path::PathBuf>,
        auth_manager: Arc<AuthManager>,
        session_source: SessionSource,
        cli_overrides: CliConfigOverrides,
        config_overrides: ConfigOverrides,
    ) -> Self {
        let loader = AgentConfigLoader::new(global_codex_home.into());
        Self {
            loader,
            auth_manager,
            session_source,
            cli_overrides,
            config_overrides,
            listeners: Mutex::new(Vec::new()),
            active_run: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Register a listener that receives [`DelegateEvent`] updates.
    pub async fn subscribe(self: &Arc<Self>) -> mpsc::UnboundedReceiver<DelegateEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.listeners.lock().await.push(tx);
        rx
    }

    /// Trigger a delegate run. Returns the run id if successfully enqueued.
    pub async fn delegate(
        self: &Arc<Self>,
        request: DelegateRequest,
    ) -> std::result::Result<DelegateRunId, OrchestratorError> {
        let mut active = self.active_run.lock().await;
        if active.is_some() {
            return Err(OrchestratorError::DelegateInProgress);
        }

        let run_id = Uuid::new_v4().to_string();
        *active = Some(run_id.clone());
        drop(active);

        let prompt_text = request.prompt.text.clone();
        self.emit(DelegateEvent::Started {
            run_id: run_id.clone(),
            agent_id: request.agent_id.clone(),
            prompt: prompt_text,
            started_at: SystemTime::now(),
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
                    let agent_id = output.agent_id.clone();
                    let message = output.message.clone();
                    let duration = output.duration;
                    orchestrator
                        .emit(DelegateEvent::Completed {
                            run_id: run_id_clone.clone(),
                            agent_id,
                            output: message,
                            duration,
                        })
                        .await;
                }
                Err(err) => {
                    orchestrator
                        .emit(DelegateEvent::Failed {
                            run_id: run_id_clone.clone(),
                            agent_id: err.agent_id,
                            error: err.error,
                        })
                        .await;
                }
            }

            let mut active = orchestrator.active_run.lock().await;
            *active = None;
        });

        Ok(run_id)
    }

    async fn emit(&self, event: DelegateEvent) {
        let mut listeners = self.listeners.lock().await;
        listeners.retain(|tx| tx.send(event.clone()).is_ok());
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
        let context = loader
            .load(Some(&agent_id), &cli_overrides, config_overrides)
            .await
            .map_err(|err| DelegateFailure {
                agent_id: agent_id.clone(),
                error: format!("failed to load agent config: {err:#}"),
            })?;

        let config = context.into_config();
        let cwd = config.cwd.clone();
        let config_clone = config.clone();
        let conversation_manager = Arc::new(ConversationManager::new(
            auth_manager.clone(),
            session_source,
        ));

        let conversation_bundle = conversation_manager
            .new_conversation(config)
            .await
            .map_err(|err| DelegateFailure {
                agent_id: agent_id.clone(),
                error: format!("failed to start conversation: {err:#}"),
            })?;
        let conversation_id = conversation_bundle.conversation_id.to_string();
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
            })?;

        let mut aggregated = String::new();
        loop {
            let event = conversation
                .next_event()
                .await
                .map_err(|err| DelegateFailure {
                    agent_id: agent_id.clone(),
                    error: format!("failed to read delegate events: {err:#}"),
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
                    });
                }
                EventMsg::Error(err) => {
                    return Err(DelegateFailure {
                        agent_id,
                        error: format!("delegate reported error: {}", err.message),
                    });
                }
                EventMsg::TurnAborted(reason) => {
                    return Err(DelegateFailure {
                        agent_id,
                        error: format!("delegate aborted: {:?}", reason.reason),
                    });
                }
                EventMsg::ShutdownComplete => break,
                _ => {}
            }
        }

        Err(DelegateFailure {
            agent_id,
            error: "delegate ended unexpectedly".to_string(),
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
}

struct DelegateFailure {
    agent_id: AgentId,
    error: String,
}

struct StoredDelegateSession {
    summary: DelegateSessionSummary,
    conversation: Arc<CodexConversation>,
    session_configured: Arc<SessionConfiguredEvent>,
    config: Config,
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
            } => CoreDelegateToolEvent::Started {
                run_id,
                agent_id: agent_id.as_str().to_string(),
                prompt,
                started_at,
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
        let agent_id = AgentId::parse(request.agent_id.as_str())
            .map_err(|_| DelegateToolError::AgentNotFound(request.agent_id.clone()))?;

        let run_id = self
            .orchestrator
            .delegate(DelegateRequest {
                agent_id: agent_id.clone(),
                prompt: DelegatePrompt::new(request.prompt),
                user_initial: Vec::new(),
            })
            .await
            .map_err(Self::map_error)?;

        Ok(DelegateToolRun {
            run_id,
            agent_id: request.agent_id,
        })
    }
}
