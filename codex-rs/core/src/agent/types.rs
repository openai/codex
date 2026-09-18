//! Agent data and execution reservations shared by controllers and their callers.
//! Backend operations and local admission policy live in the implementations.

use crate::context::MultiAgentRoleInstructions;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::turn_input::CyberAccessProgram;

/// Registry identity shared by loaded and unloaded agents.
/// Registered agents have an `agent_id`; a reserved spawn can still be awaiting its ID.
#[derive(Clone, Debug, Default)]
pub struct AgentMetadata {
    pub agent_id: Option<ThreadId>,
    pub agent_path: Option<AgentPath>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpawnAgentForkMode {
    FullHistory,
    LastNTurns(usize),
}

#[derive(Clone, Debug, Default)]
pub struct SpawnAgentOptions {
    pub fork_parent_spawn_call_id: Option<String>,
    pub fork_mode: Option<SpawnAgentForkMode>,
    pub parent_thread_id: Option<ThreadId>,
    pub parent_turn_id: Option<String>,
    /// Attribute delegated usage to the turn that initiated it.
    pub turn_trigger: Option<String>,
    pub root_turn_id: Option<String>,
    pub environments: Option<Vec<TurnEnvironmentSelection>>,
    pub multi_agent_v2_usage_hints: Option<ResolvedMultiAgentV2UsageHints>,
    pub cyber_access_program: Option<CyberAccessProgram>,
}

/// Identity and status observed from a loaded agent, without a handle to its runtime.
#[derive(Clone, Debug)]
pub struct LiveAgent {
    pub thread_id: ThreadId,
    pub metadata: AgentMetadata,
    pub status: AgentStatus,
}

#[derive(Clone, Debug, Default)]
pub struct ResolvedMultiAgentV2UsageHints {
    pub root: Option<MultiAgentRoleInstructions>,
    pub subagent: Option<MultiAgentRoleInstructions>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MessageDeliveryMode {
    /// Deliver to the mailbox without starting an idle agent.
    QueueOnly,
    /// Deliver to the active turn or start work if the agent is idle.
    TriggerTurn,
}

/// Keeps model-provided encrypted content distinct from text that needs a context wrapper.
pub enum AgentMessage {
    Plaintext(String),
    Encrypted(String),
}

/// Holds a backend-owned reservation until the turn ends or is cancelled.
///
/// The permit's destructor releases capacity or arranges backend cleanup. Remote backends
/// must also recover reservations after worker loss, when no Rust destructor can run.
#[must_use = "hold the execution guard for the lifetime of the admitted turn"]
pub struct AgentExecutionGuard {
    _permit: Box<dyn Send + Sync>,
}

impl AgentExecutionGuard {
    pub fn new(permit: impl Send + Sync + 'static) -> Self {
        Self {
            _permit: Box::new(permit),
        }
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
