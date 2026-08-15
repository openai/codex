use crate::capabilities::SelectedCapabilityRoot;
use crate::models::PermissionProfileSnapshot;

/// Configuration supplied for a thread's selected environment.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Resolved configuration for a thread/environment attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentConfig {
    /// Whether shell tools may start login shells in this environment.
    pub allow_login_shell: bool,
    /// Resolved permissions for this thread's environment attachment.
    pub permission_profile: PermissionProfileSnapshot,
    /// Capability roots selected for this thread's environment attachment.
    pub selected_capability_roots: Vec<SelectedCapabilityRoot>,
}
