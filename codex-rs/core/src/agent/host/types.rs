//! Prepared inputs and host reservations for native child startup.

use crate::agent::api::AgentControl;
use crate::agents_md_manager::SessionInstructions;
use crate::exec_policy::ExecPolicyManager;
use crate::session::session::AppServerClientMetadata;
use crate::thread_manager::StartThreadOptions;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use std::sync::Arc;

/// A fresh child ID reserved in the host's tree.
/// The host commits membership, serializes startup attempts and deduplicates retries.
/// Core derives parent, session and path from [`PreparedThreadSpawn`]; the controller
/// supplies runtime coordination, and its `spawn` method is not called.
pub struct HostThreadSpawnReservation {
    pub thread_id: ThreadId,
    pub control: Arc<dyn AgentControl>,
}

/// Opaque, in-process snapshot for starting a child without its source runtime.
/// Settings and authority reflect preparation time. The destination must supply compatible
/// services and register the captured environment IDs with matching working directories and
/// capability mappings. Detached preparation rejects thread attachments, live instruction providers,
/// enabled EMA-authenticated MCP servers, and unresolved environment configuration.
/// The child inherits user and thread instructions and discovers repository instructions
/// in its destination environment.
pub struct PreparedThreadSpawn {
    pub(super) caller: ThreadId,
    pub(super) session_id: SessionId,
    pub(super) agent_path: AgentPath,
    pub(super) client_metadata: AppServerClientMetadata,
    pub(super) thread_options: StartThreadOptions,
    pub(super) inheritance: PreparedChildInheritance,
}

impl PreparedThreadSpawn {
    /// Canonical path of the child in the agent tree.
    pub fn agent_path(&self) -> &AgentPath {
        &self.agent_path
    }
}

pub(crate) struct PreparedChildInheritance {
    pub(crate) instructions: SessionInstructions,
    pub(crate) exec_policy: Option<Arc<ExecPolicyManager>>,
    pub(crate) originator: String,
    pub(crate) attachment_source: Option<ThreadId>,
}
