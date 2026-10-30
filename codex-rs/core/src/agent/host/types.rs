//! Prepared inputs and host reservations for native child startup.

use crate::agent::api::AgentControl;
use crate::agents_md_manager::SessionInstructions;
use crate::exec_policy::ExecPolicyManager;
use codex_execpolicy::Policy;
use codex_execpolicy::RequirementsExecPolicy;
use codex_protocol::ThreadId;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::Arc;

/// A fresh child ID reserved in the host's tree.
/// The host commits membership, serializes startup attempts and deduplicates retries.
/// Core derives the parent, session and path from the invocation and child request.
/// The controller supplies runtime coordination.
pub struct HostThreadSpawnReservation {
    pub thread_id: ThreadId,
    pub control: Arc<dyn AgentControl>,
}

/// Parent inputs captured from the tool invocation for a host starting or forking a child.
/// The host binds the destination's services and applies the requested child settings.
/// Repository instructions are discovered at the destination.
pub struct PreparedChildContext {
    pub originator: String,
    pub user_instructions: Option<String>,
    pub thread_instructions: Option<String>,
    pub cwd: AbsolutePathBuf,
    pub environments: Vec<TurnEnvironmentRequest>,
    pub selected_capability_roots: Vec<SelectedCapabilityRoot>,
    pub resolved_permission_profile: PermissionProfile,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    /// Parent exec policy at preparation time. Inherit it only for compatible policy sources.
    pub exec_policy: Arc<Policy>,
    /// Managed requirements used to load the policy, before any session amendments.
    pub exec_policy_source_requirements: Option<Arc<RequirementsExecPolicy>>,
    /// Current tool declarations a fresh child can inherit. The host supplies their execution route.
    pub dynamic_tools: Vec<DynamicToolSpec>,
    pub disabled_plugin_ids: Vec<String>,
}

pub(crate) struct PreparedChildInheritance {
    pub(crate) instructions: SessionInstructions,
    pub(crate) exec_policy: Option<Arc<ExecPolicyManager>>,
    pub(crate) originator: String,
    pub(crate) attachment_source: Option<ThreadId>,
}
