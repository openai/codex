use crate::capabilities::SelectedCapabilityRoot;

/// Resolved configuration supplied by the owner of a thread/environment attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentConfig {
    /// Whether shell tools may start login shells in this environment.
    pub allow_login_shell: bool,
    /// Capability roots selected for this thread's environment attachment.
    pub selected_capability_roots: Vec<SelectedCapabilityRoot>,
}
