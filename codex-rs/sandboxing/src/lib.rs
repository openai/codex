#[cfg(target_os = "linux")]
mod bwrap;
mod denial;
pub mod landlock;
mod manager;
pub mod policy_transforms;
#[cfg(target_os = "macos")]
pub mod seatbelt;
mod spawn;
mod windows;

#[cfg(target_os = "linux")]
pub use bwrap::find_system_bwrap_in_path;
#[cfg(target_os = "linux")]
pub use bwrap::system_bwrap_warning;
pub use codex_windows_sandbox::WindowsSandboxProxySettingsMode;
pub use denial::is_likely_sandbox_denied;
pub use manager::SandboxCommand;
pub use manager::SandboxDirectSpawnTransformRequest;
pub use manager::SandboxExecRequest;
pub use manager::SandboxManager;
pub use manager::SandboxTransformError;
pub use manager::SandboxTransformRequest;
pub use manager::SandboxType;
pub use manager::SandboxablePreference;
pub use manager::compatibility_sandbox_policy_for_permission_profile;
pub use manager::get_platform_sandbox;
pub use manager::with_managed_mitm_ca_readable_root;
pub use spawn::SpawnRequest;
pub use spawn::WindowsSandboxSpawnRequest;
pub use spawn::spawn_process;
pub use windows::WindowsSandboxFilesystemOverrides;
pub use windows::permission_profile_supports_windows_restricted_token_sandbox;
pub use windows::resolve_windows_elevated_filesystem_overrides;
pub use windows::resolve_windows_restricted_token_filesystem_overrides;
pub use windows::unsupported_windows_restricted_token_sandbox_reason;
pub use windows::windows_sandbox_uses_elevated_backend;

use codex_protocol::error::CodexErr;

#[cfg(not(target_os = "linux"))]
pub fn system_bwrap_warning(
    _permission_profile: &codex_protocol::models::PermissionProfile,
) -> Option<String> {
    None
}

impl From<SandboxTransformError> for CodexErr {
    fn from(err: SandboxTransformError) -> Self {
        match err {
            error @ SandboxTransformError::InvalidCommandCwd { .. }
            | error @ SandboxTransformError::InvalidSandboxPolicyCwd { .. } => {
                CodexErr::InvalidRequest(error.to_string())
            }
            SandboxTransformError::MissingLinuxSandboxExecutable => {
                CodexErr::LandlockSandboxExecutableNotProvided
            }
            SandboxTransformError::EnvironmentNetworkProxy(message) => {
                CodexErr::UnsupportedOperation(message)
            }
            #[cfg(target_os = "linux")]
            SandboxTransformError::Wsl1UnsupportedForBubblewrap => {
                CodexErr::UnsupportedOperation(crate::bwrap::WSL1_BWRAP_WARNING.to_string())
            }
            #[cfg(not(target_os = "macos"))]
            SandboxTransformError::SeatbeltUnavailable => CodexErr::UnsupportedOperation(
                "seatbelt sandbox is only available on macOS".to_string(),
            ),
            #[cfg(target_os = "windows")]
            SandboxTransformError::WindowsSandboxPreparation(message) => {
                CodexErr::UnsupportedOperation(message)
            }
        }
    }
}
