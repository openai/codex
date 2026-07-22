use std::sync::Arc;

use crate::ExtensionData;
use codex_mcp::McpResourceClient;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnEnvironmentSelection;

/// Trusted, host-resolved billing attribution for a thread.
///
/// Extensions may forward this value to first-party APIs. It is seeded by Core
/// after resolving persisted and host-provided originator state, rather than
/// from model- or tool-controlled input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadOriginator(pub String);

/// Input supplied when the host starts a runtime for a thread.
pub struct ThreadStartInput<'a, C> {
    /// Host configuration visible at thread start.
    pub config: &'a C,
    /// Source that created the session for this thread.
    pub session_source: &'a SessionSource,
    /// Whether persistent thread-scoped state is available for this thread.
    pub persistent_thread_state_available: bool,
    /// Execution environments selected for this thread.
    pub environments: &'a [TurnEnvironmentSelection],
    /// MCP resource access supplied by the host for this session.
    pub mcp_resource_client: Option<Arc<McpResourceClient>>,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host resumes an existing thread.
pub struct ThreadResumeInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host has no immediately pending thread work.
pub struct ThreadIdleInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host stops a thread runtime.
pub struct ThreadStopInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}
