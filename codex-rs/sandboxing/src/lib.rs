#[cfg(target_os = "linux")]
mod bwrap;
pub mod landlock;
mod manager;
pub mod policy_transforms;
#[cfg(target_os = "macos")]
pub mod seatbelt;
mod violation;

#[cfg(target_os = "linux")]
pub use bwrap::find_system_bwrap_in_path;
#[cfg(target_os = "linux")]
pub use bwrap::system_bwrap_warning;
pub use manager::SandboxCommand;
pub use manager::SandboxExecRequest;
pub use manager::SandboxManager;
pub use manager::SandboxTransformError;
pub use manager::SandboxTransformRequest;
pub use manager::SandboxType;
pub use manager::SandboxablePreference;
pub use manager::get_platform_sandbox;
pub use violation::FileSystemSandboxViolation;
pub use violation::FileSystemSandboxViolationReason;
pub use violation::NetworkSandboxViolation;
pub use violation::SandboxViolationEvent;
pub use violation::classify_filesystem_sandbox_violation;
pub use violation::is_likely_sandbox_denied;
pub use violation::record_filesystem_sandbox_violation;
pub use violation::record_network_sandbox_violation;
pub use violation::record_sandbox_violation;

use codex_protocol::error::CodexErr;

#[cfg(not(target_os = "linux"))]
pub fn system_bwrap_warning(
    _sandbox_policy: &codex_protocol::protocol::SandboxPolicy,
) -> Option<String> {
    None
}

impl From<SandboxTransformError> for CodexErr {
    fn from(err: SandboxTransformError) -> Self {
        match err {
            SandboxTransformError::MissingLinuxSandboxExecutable => {
                CodexErr::LandlockSandboxExecutableNotProvided
            }
            #[cfg(target_os = "linux")]
            SandboxTransformError::Wsl1UnsupportedForBubblewrap => {
                CodexErr::UnsupportedOperation(crate::bwrap::WSL1_BWRAP_WARNING.to_string())
            }
            #[cfg(not(target_os = "macos"))]
            SandboxTransformError::SeatbeltUnavailable => CodexErr::UnsupportedOperation(
                "seatbelt sandbox is only available on macOS".to_string(),
            ),
        }
    }
}
