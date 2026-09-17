//! Identifies the platform sandbox implementation selected for execution.

use serde::Deserialize;
use serde::Serialize;

use crate::config_types::WindowsSandboxLevel;

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
