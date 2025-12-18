use tokio::sync::Mutex;

use crate::codex::SessionConfiguration;
use crate::state::ActiveTurn;
use crate::state::SessionState;
use codex_protocol::protocol::AgentId;

/// Per-agent mutable state shared across async tasks.
///
/// The struct itself is stored in an `Arc`, so fields use `Mutex` to guard
/// concurrent mutation rather than additional `Arc` layers.
pub(crate) struct AgentState {
    pub(crate) agent_id: AgentId,
    /// Session configuration + conversation history for this agent.
    pub(crate) state: Mutex<SessionState>,
    /// Active turn state is tracked per-agent (each agent can have its own task).
    pub(crate) active_turn: Mutex<Option<ActiveTurn>>,
}

impl AgentState {
    pub(crate) fn new(agent_id: AgentId, session_configuration: SessionConfiguration) -> Self {
        Self {
            agent_id,
            state: Mutex::new(SessionState::new(session_configuration)),
            active_turn: Mutex::new(None),
        }
    }
}
