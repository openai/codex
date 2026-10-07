//! Identifies the sandbox implementation and overrides selected for a command.

use serde::Deserialize;
use serde::Serialize;

use crate::config_types::WindowsSandboxLevel;

/// Controller decision that overrides normal sandbox selection for a command.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxOverride {
    /// Use normal sandbox selection.
    #[default]
    NoOverride,
    /// Broaden approved filesystem access while retaining denied-read enforcement.
    EscalatedSandboxWithRestrictions,
    /// Bypass the sandbox on the first attempt.
    BypassSandboxFirstAttempt,
}

impl SandboxOverride {
    pub fn is_no_override(&self) -> bool {
        *self == Self::NoOverride
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxType {
    None,
    MacosSeatbelt,
    LinuxSeccomp,
    WindowsRestrictedToken,
    WindowsMxc,
}

impl SandboxType {
    pub fn as_metric_tag(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MacosSeatbelt => "seatbelt",
            Self::LinuxSeccomp => "seccomp",
            Self::WindowsRestrictedToken => "windows_sandbox",
            Self::WindowsMxc => "windows_mxc",
        }
    }
}

/// Preserves explicit MXC selection while honoring legacy runtime level updates.
pub fn effective_windows_sandbox_type(
    sandbox_type: SandboxType,
    sandbox_level: WindowsSandboxLevel,
) -> SandboxType {
    match (sandbox_type, sandbox_level) {
        (SandboxType::WindowsMxc, _) => SandboxType::WindowsMxc,
        (_, WindowsSandboxLevel::Disabled) => SandboxType::None,
        (_, WindowsSandboxLevel::RestrictedToken | WindowsSandboxLevel::Elevated) => {
            SandboxType::WindowsRestrictedToken
        }
    }
}
