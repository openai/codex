#[cfg(target_os = "linux")]
mod bwrap;
pub mod landlock;
pub mod macos_permissions;
mod manager;
pub mod policy_transforms;
#[cfg(target_os = "macos")]
pub mod seatbelt;
#[cfg(target_os = "macos")]
mod seatbelt_permissions;

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

#[cfg(not(target_os = "linux"))]
pub fn system_bwrap_warning() -> Option<String> {
    None
}
