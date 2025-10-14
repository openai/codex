use std::sync::Arc;
use std::time::SystemTime;

use codex_common::CliConfigOverrides;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
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
                    orchestrator
                        .emit(DelegateEvent::Completed {
                            run_id: run_id_clone.clone(),
                            agent_id: output.agent_id,
                            output: output.message,
                            duration: output.duration,
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
        let conversation = conversation_bundle.conversation;

        let mut items = Vec::new();
        if !request.prompt.text.trim().is_empty() {
            items.push(InputItem::Text {
                text: request.prompt.text.clone(),
            });
        }
        if items.is_empty() {
            return Err(DelegateFailure {
                agent_id: agent_id.clone(),
                error: "delegated prompt is empty".to_string(),
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

                    let _ = conversation.submit(Op::Shutdown).await;

                    return Ok(DelegateSuccess {
                        agent_id,
                        message,
                        duration,
                    });
                }
                EventMsg::Error(err) => {
                    let _ = conversation.submit(Op::Shutdown).await;
                    return Err(DelegateFailure {
                        agent_id,
                        error: format!("delegate reported error: {}", err.message),
                    });
                }
                EventMsg::TurnAborted(reason) => {
                    let _ = conversation.submit(Op::Shutdown).await;
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
    message: Option<String>,
    duration: Duration,
}

struct DelegateFailure {
    agent_id: AgentId,
    error: String,
}
