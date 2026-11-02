//! Host inputs for child startup and root-only capability restrictions.

use super::admission::AgentTurnAdmission;
use super::usage::AgentUsageAccounting;
use crate::agent::api::AgentControl;
use crate::agents_md_manager::SessionInstructions;
use crate::exec_policy::ExecPolicyManager;
use codex_execpolicy::Policy;
use codex_execpolicy::RequirementsExecPolicy;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::Arc;

/// Restrict a thread's root-only capabilities as for a native subagent.
///
/// A trusted host supplies this marker in the thread's startup [`codex_extension_api::ExtensionDataInit`],
/// including when cold-resuming the thread. Core captures it once at startup.
/// Embeddings can read it during extension startup and MCP projection to apply their own
/// root-only restrictions. Session attribution, agent lineage and the thread's permission
/// profile come from their usual startup inputs. Native subagents are always restricted.
///
/// For agent forks, hosts must also supply history with native agent-fork provenance.
/// This marks copied messages as inherited and excludes the source's Guardian checkpoints.
/// Obtaining current root evidence for Guardian is a separate host responsibility.
#[derive(Clone, Copy, Debug)]
pub struct RestrictRootCapabilities;

/// The root's session ID, supplied by a trusted host when starting a fresh child.
///
/// Insert this into the thread's startup [`codex_extension_api::ExtensionDataInit`].
/// The child keeps its own distinct thread ID. Resumed threads take their session
/// ID from persisted metadata, and native children take it from their controller.
/// To restrict root capabilities, also supply [`RestrictRootCapabilities`].
#[derive(Clone, Copy, Debug)]
pub struct InheritedSessionId(pub SessionId);

/// A fresh child ID reserved in the host's tree.
/// The host commits membership, serializes startup attempts and deduplicates retries.
/// Core derives the parent, session and path from the invocation and child request.
/// The controller supplies runtime coordination by default. To use host-owned admission and
/// accounting, a startup contributor must explicitly select `AgentRuntimeOwnership::Host`.
pub struct HostThreadSpawnReservation {
    pub thread_id: ThreadId,
    pub control: Arc<dyn AgentControl>,
    /// Admission provider for a host-owned child. A lifecycle contributor may also install it.
    pub turn_admission: Option<Arc<dyn AgentTurnAdmission>>,
    /// Optional accounting for a host-owned child. Without a provider from the reservation or
    /// a contributor, host-owned children disable accounting. Controller-owned children keep
    /// their controller's accounting.
    pub usage_accounting: Option<Arc<dyn AgentUsageAccounting>>,
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
