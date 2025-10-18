use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::mpsc::UnboundedReceiver;

/// Identifier assigned to a delegate run. Mirrors the orchestrator's run id.
pub type DelegateRunId = String;

/// Additional hints the primary agent can pass to a delegate tool invocation.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DelegateToolContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
}

/// Invocation strategy for the delegate tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DelegateInvocationMode {
    /// Blocks the caller until the delegate completes, returning its summary.
    Immediate,
    /// Starts the delegate in the background and returns immediately.
    Detached,
}

impl Default for DelegateInvocationMode {
    fn default() -> Self {
        Self::Immediate
    }
}

/// Single entry in a batched delegate request.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DelegateToolBatchEntry {
    pub agent_id: String,
    pub prompt: String,
    #[serde(default)]
    pub context: DelegateToolContext,
    #[serde(default)]
    pub mode: DelegateInvocationMode,
}

/// Payload sent by the primary agent when invoking the delegate tool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DelegateToolRequest {
    pub agent_id: String,
    pub prompt: String,
    #[serde(default)]
    pub context: DelegateToolContext,
    #[serde(default, skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub caller_conversation_id: Option<String>,
    #[serde(default)]
    pub mode: DelegateInvocationMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch: Vec<DelegateToolBatchEntry>,
}

/// Event emitted while a delegate run is in flight.
#[derive(Debug, Clone)]
pub enum DelegateToolEvent {
    Started {
        run_id: DelegateRunId,
        agent_id: String,
        prompt: String,
        started_at: SystemTime,
        parent_run_id: Option<DelegateRunId>,
    },
    Delta {
        run_id: DelegateRunId,
        agent_id: String,
        chunk: String,
    },
    Completed {
        run_id: DelegateRunId,
        agent_id: String,
        output: Option<String>,
        duration: Duration,
    },
    Failed {
        run_id: DelegateRunId,
        agent_id: String,
        error: String,
    },
}

/// Result returned when a delegate request is accepted.
#[derive(Debug, Clone)]
pub struct DelegateToolRun {
    pub run_id: DelegateRunId,
    pub agent_id: String,
}

#[derive(thiserror::Error, Debug)]
pub enum DelegateToolError {
    #[error("another delegate is already running")]
    DelegateInProgress,
    #[error("delegate queue is full")]
    QueueFull,
    #[error("agent `{0}` not found")]
    AgentNotFound(String),
    #[error("delegate setup failed: {0}")]
    SetupFailed(String),
}

pub type DelegateEventReceiver = UnboundedReceiver<DelegateToolEvent>;

/// Adapter abstraction that lets front-ends wire their orchestrator into the core tool handler.
#[async_trait]
pub trait DelegateToolAdapter: Send + Sync {
    async fn subscribe(&self) -> DelegateEventReceiver;

    async fn delegate(
        &self,
        request: DelegateToolRequest,
    ) -> Result<DelegateToolRun, DelegateToolError>;
}
