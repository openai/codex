//! Agent executor trait — user-implemented agent logic.
//!
//! Mirrors `a2a-js/src/server/agent_execution/agent_executor.ts`.

use crate::event::EventBus;
use crate::types::{AgentCard, SendMessageRequest};

/// Context for a single request.
pub struct RequestContext {
    /// The original send-message request.
    pub request: SendMessageRequest,
    /// Task ID assigned by the server (if a task was created).
    pub task_id: Option<String>,
    /// Context ID for this conversation.
    pub context_id: String,
}

/// Implement this trait to define your agent's execution logic.
///
/// The server calls [`execute`] for each incoming message. Your implementation
/// should publish events to the [`EventBus`] as it makes progress.
///
/// For simple (non-streaming) agents, publish a single
/// [`ExecutionEvent::Task`] or [`ExecutionEvent::Message`] and return.
pub trait AgentExecutor: Send + Sync + 'static {
    /// Execute agent logic and publish events to the bus.
    fn execute(
        &self,
        context: RequestContext,
        event_bus: &EventBus,
    ) -> impl std::future::Future<Output = Result<(), crate::error::A2AError>> + Send;

    /// Cancel a running task.
    fn cancel(
        &self,
        task_id: &str,
        event_bus: &EventBus,
    ) -> impl std::future::Future<Output = Result<(), crate::error::A2AError>> + Send;

    /// Return the agent card for discovery.
    fn agent_card(&self, base_url: &str) -> AgentCard;
}
