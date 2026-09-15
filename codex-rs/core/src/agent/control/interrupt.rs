//! Applies V2 interruption rules to a registered agent without loading its runtime.
//!
//! Root and self targets are rejected. An unloaded or already-dead runtime is a successful
//! interruption; this operation never reloads it.

use super::AgentControl;
use crate::agent::AgentStatus;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;

pub(crate) struct AgentInterruptOutcome {
    pub(crate) agent_path: AgentPath,
    pub(crate) previous_status: AgentStatus,
}

/// Keeps request validation distinct from runtime failures for the tool adapter's error mapping.
#[derive(Debug)]
pub(crate) enum AgentInterruptError {
    InvalidRequest(String),
    Agent(CodexErr),
}

impl AgentControl {
    /// Interrupts a spawned agent's current task, preserving the status observed before dispatch.
    pub(crate) async fn interrupt_spawned_agent(
        &self,
        caller: ThreadId,
        target: ThreadId,
    ) -> Result<AgentInterruptOutcome, AgentInterruptError> {
        let receiver_agent = self
            .ensure_agent_known(target)
            .map_err(AgentInterruptError::Agent)?;
        if receiver_agent
            .agent_path
            .as_ref()
            .is_some_and(AgentPath::is_root)
        {
            return Err(AgentInterruptError::InvalidRequest(
                "root is not a spawned agent".to_string(),
            ));
        }
        if target == caller {
            return Err(AgentInterruptError::InvalidRequest(
                "an agent cannot interrupt itself; return your result and let the parent interrupt you if needed"
                    .to_string(),
            ));
        }
        let agent_path = receiver_agent.agent_path.clone().ok_or_else(|| {
            AgentInterruptError::InvalidRequest("target agent is missing an agent_path".to_string())
        })?;
        let previous_status = self.get_status(target).await;
        match self.interrupt_agent(target).await {
            Ok(_) => {}
            Err(err)
                if matches!(
                    err.details(),
                    CodexErrorDetails::ThreadNotFound(_) | CodexErrorDetails::InternalAgentDied
                ) => {}
            Err(err) => return Err(AgentInterruptError::Agent(err)),
        }
        Ok(AgentInterruptOutcome {
            agent_path,
            previous_status,
        })
    }
}
