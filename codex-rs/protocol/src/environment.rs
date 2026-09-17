//! Environment attachment authority, including the shared Full Access decision.

use crate::capabilities::SelectedCapabilityRoot;
use crate::config_types::ShellEnvironmentPolicy;
use crate::config_types::WindowsSandboxLevel;
use crate::mcp_policy::EnvironmentMcpPolicy;
use crate::models::PermissionProfile;
use crate::models::PermissionProfileSnapshot;
use crate::protocol::AskForApproval;
use crate::sandbox::SandboxType;
use codex_execpolicy::RequirementsExecPolicy;
use codex_network_proxy::EnvironmentNetworkPolicy;
use codex_utils_path_uri::PathUri;

/// Configuration supplied for a thread's selected environment.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum EnvironmentConfigState {
    /// Preserve the existing thread-derived environment configuration.
    FromThread,
    /// The owner will supply environment configuration later.
    Pending,
    /// The owner supplied configuration for this environment attachment.
    Ready(EnvironmentConfig),
    /// The owner could not supply configuration for this environment attachment.
    Failed(String),
}

/// Full Access requires no approvals and unrestricted permissions everywhere selected.
/// Thread-owned attachments inherit the fallback profile; unresolved owner authority
/// is never Full Access. All approval and background-review paths use this decision.
pub fn has_full_access<'a>(
    approval_policy: AskForApproval,
    thread_profile: &PermissionProfile,
    environments: impl IntoIterator<Item = &'a EnvironmentConfigState>,
) -> bool {
    let mut environments = environments.into_iter().peekable();
    approval_policy == AskForApproval::Never
        && if environments.peek().is_none() {
            matches!(thread_profile, PermissionProfile::Disabled)
        } else {
            environments.all(|environment| match environment {
                EnvironmentConfigState::FromThread => {
                    matches!(thread_profile, PermissionProfile::Disabled)
                }
                EnvironmentConfigState::Ready(config) => {
                    matches!(
                        config.permission_profile.permission_profile(),
                        PermissionProfile::Disabled
                    )
                }
                EnvironmentConfigState::Pending | EnvironmentConfigState::Failed(_) => false,
            })
        }
}

/// Resolved configuration for a thread/environment attachment.
#[derive(Clone, PartialEq)]
pub struct EnvironmentConfig {
    /// Whether shell tools may start login shells in this environment.
    pub allow_login_shell: bool,
    /// Effective workspace roots resolved for this environment attachment.
    pub workspace_roots: Vec<PathUri>,
    /// Resolved permissions for this thread's environment attachment.
    pub permission_profile: PermissionProfileSnapshot,
    /// Controls which environment variables shell commands may inherit.
    pub shell_environment_policy: ShellEnvironmentPolicy,
    /// Legacy Windows restricted-token setup level for this environment attachment.
    pub windows_sandbox_level: WindowsSandboxLevel,
    /// Concrete Windows sandbox backend selected for this environment attachment.
    pub windows_sandbox_type: SandboxType,
    /// Whether Windows sandbox processes use a private desktop.
    pub windows_sandbox_private_desktop: bool,
    /// Whether Linux sandbox processes use the legacy Landlock backend.
    pub use_legacy_landlock: bool,
    /// Additional managed command restrictions for this environment attachment.
    pub exec_policy: Option<RequirementsExecPolicy>,
    /// Additional managed MCP restrictions for this environment attachment.
    pub mcp_policy: Option<EnvironmentMcpPolicy>,
    /// Owner-provided traffic restrictions. `None` keeps the existing controller policy.
    pub network_policy: Option<EnvironmentNetworkPolicy>,
    /// Capability roots selected for this thread's environment attachment.
    pub selected_capability_roots: Vec<SelectedCapabilityRoot>,
}

impl std::fmt::Debug for EnvironmentConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnvironmentConfig")
            .field("allow_login_shell", &self.allow_login_shell)
            .field("workspace_roots", &self.workspace_roots)
            .field("permission_profile", &self.permission_profile)
            .field("shell_environment_policy", &"<redacted>")
            .field("windows_sandbox_level", &self.windows_sandbox_level)
            .field("windows_sandbox_type", &self.windows_sandbox_type)
            .field(
                "windows_sandbox_private_desktop",
                &self.windows_sandbox_private_desktop,
            )
            .field("use_legacy_landlock", &self.use_legacy_landlock)
            .field("exec_policy", &self.exec_policy)
            .field("mcp_policy", &self.mcp_policy)
            .field("network_policy", &self.network_policy)
            .field("selected_capability_roots", &self.selected_capability_roots)
            .finish()
    }
}
