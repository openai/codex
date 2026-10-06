//! Bounded agent failure diagnostics, separate from error behavior and user-visible messages.
//! The first annotation wins so outer spawn stages preserve more precise failure origins.

use super::CodexErr;

/// A failure origin, or the spawn stage when no more precise origin is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum AgentErrorContext {
    ExecutionCapacity,
    ResidencyCapacity,
    RegistryCapacity,
    DuplicatePath,
    NicknameUnavailable,
    ManagerUnavailable,
    RuntimeShutdown,
    ForkHistory,
    ChildStartup,
    InputAdmission,
}

impl CodexErr {
    /// Attach bounded diagnostics without replacing a more precise inner annotation.
    pub fn with_agent_context(mut self, context: AgentErrorContext) -> Self {
        self.agent_context.get_or_insert(context);
        self
    }

    pub fn agent_context(&self) -> Option<AgentErrorContext> {
        self.agent_context
    }
}
