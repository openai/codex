use crate::capabilities::SelectedCapabilityRoot;
use crate::config_types::ShellEnvironmentPolicy;
use crate::models::PermissionProfileSnapshot;

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

/// Resolved configuration for a thread/environment attachment.
#[derive(Clone, PartialEq)]
pub struct EnvironmentConfig {
    /// Whether shell tools may start login shells in this environment.
    pub allow_login_shell: bool,
    /// Resolved permissions for this thread's environment attachment.
    pub permission_profile: PermissionProfileSnapshot,
    /// Controls which environment variables shell commands may inherit.
    pub shell_environment_policy: ShellEnvironmentPolicy,
    /// Capability roots selected for this thread's environment attachment.
    pub selected_capability_roots: Vec<SelectedCapabilityRoot>,
}

impl std::fmt::Debug for EnvironmentConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnvironmentConfig")
            .field("allow_login_shell", &self.allow_login_shell)
            .field("permission_profile", &self.permission_profile)
            .field("shell_environment_policy", &"<redacted>")
            .field("selected_capability_roots", &self.selected_capability_roots)
            .finish()
    }
}
