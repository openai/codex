//! Startup-owned Linux PID isolation policy, independent of request permissions.

/// Selects the PID namespace policy for Linux bubblewrap sandboxes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LinuxSandboxPidNamespace {
    /// Create a new PID namespace, retaining the legacy `/proc` mount fallback.
    #[default]
    Isolate,
    /// Reuse the caller's PID namespace and `/proc`, allowing signals to other same-UID processes.
    Inherit,
}

impl std::str::FromStr for LinuxSandboxPidNamespace {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "isolate" => Ok(Self::Isolate),
            "inherit" => Ok(Self::Inherit),
            _ => Err("expected 'isolate' or 'inherit'".to_string()),
        }
    }
}
